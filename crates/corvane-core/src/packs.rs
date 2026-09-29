//! On-demand packs in the app (`corvane_packs`): which packs
//! are installed, the manifest for Settings › Advanced, download / remove
//! with progress, and handing the `syntax-extended` dump to
//! `corvane_highlight`. No GHD equivalent (Electron ships everything).
//!
//! Testing hooks: `CORVANE_PACKS_MANIFEST=<url|path>` (the manifest),
//! `CORVANE_INSTALL_PACK=<name>` installs that pack at launch.

use std::collections::HashMap;

use corvane_packs::{InstalledPack, PackError, PackKind, PackManifest};
use gpui_kit::{App, AsyncApp};
use tracing::{error, info, warn};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;

/// A download / install in flight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackProgress {
    pub received: u64,
    pub total: Option<u64>,
}

/// `AppState::packs`
#[derive(Clone, Debug, Default)]
pub struct PacksState {
    /// The signed manifest, once fetched for Settings › Advanced.
    pub manifest: Option<PackManifest>,
    pub manifest_error: Option<String>,
    pub manifest_loading: bool,
    pub installed: HashMap<PackKind, InstalledPack>,
    pub progress: HashMap<PackKind, PackProgress>,
    /// The last install / remove error per pack, shown next to its row.
    pub errors: HashMap<PackKind, String>,
}

impl PacksState {
    /// Whether the pack's contents are usable: compiled in (full build) or
    /// installed.
    pub fn available(&self, kind: PackKind) -> bool {
        self.bundled(kind) || self.installed.contains_key(&kind)
    }

    /// Whether this build compiled the pack in.
    pub fn bundled(&self, kind: PackKind) -> bool {
        match kind {
            PackKind::SyntaxExtended => corvane_highlight::syntaxes::extended_bundled(),
            PackKind::GitPortable | PackKind::GitLfs => false,
        }
    }
}

/// Every pack Settings › Advanced lists (the git packs are not published
/// yet, so only the grammar pack for now).
pub const OFFERED_PACKS: &[PackKind] = &[PackKind::SyntaxExtended];

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Point the consumer at an installed pack's data.
fn activate(pack: &InstalledPack) {
    if pack.kind == PackKind::SyntaxExtended {
        match corvane_highlight::syntaxes::use_extended_dump(&pack.entry_path()) {
            Ok(count) => info!(count, version = %pack.version, "extended grammars loaded"),
            Err(err) => warn!(%err, "could not load the extended grammars"),
        }
    }
}

fn deactivate(kind: PackKind) {
    if kind == PackKind::SyntaxExtended {
        corvane_highlight::syntaxes::clear_extended_dump();
    }
}

impl Dispatcher {
    /// At launch: find installed packs on disk and activate them.
    pub fn load_installed_packs(cx: &mut App) {
        spawn_bg(
            cx,
            || {
                let mut found = Vec::new();
                for kind in OFFERED_PACKS {
                    if let Some(pack) = corvane_packs::installed(*kind) {
                        activate(&pack);
                        found.push(pack);
                    }
                }
                found
            },
            |found, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    for pack in found {
                        s.packs.installed.insert(pack.kind, pack);
                    }
                    cx.notify();
                });
                if let Ok(name) = std::env::var("CORVANE_INSTALL_PACK")
                    && let Some(kind) = OFFERED_PACKS.iter().find(|k| k.name() == name)
                {
                    Self::install_pack(*kind, cx);
                }
            },
        );
    }

    /// Fetch the signed manifest (Settings › Advanced opening, Retry).
    pub fn refresh_packs_manifest(cx: &mut App) {
        let state = Self::state(cx);
        if state.read(cx).packs.manifest_loading {
            return;
        }
        state.update(cx, |s, cx| {
            s.packs.manifest_loading = true;
            s.packs.manifest_error = None;
            cx.notify();
        });
        spawn_bg(cx, corvane_packs::fetch_manifest, |result, cx| {
            Self::state(cx).update(cx, |s, cx| {
                s.packs.manifest_loading = false;
                match result {
                    Ok(manifest) => s.packs.manifest = Some(manifest),
                    Err(err) => {
                        warn!(%err, "could not fetch the packs manifest");
                        s.packs.manifest_error = Some(err.to_string());
                    }
                }
                cx.notify();
            });
        });
    }

    /// Download, verify and install `kind` from the manifest (fetching the
    /// manifest first when needed), then activate it.
    pub fn install_pack(kind: PackKind, cx: &mut App) {
        let state = Self::state(cx);
        let entry = {
            let s = state.read(cx);
            if s.packs.progress.contains_key(&kind) {
                return;
            }
            s.packs
                .manifest
                .as_ref()
                .and_then(|m| m.entry_for(kind, app_version()).cloned())
        };
        let Some(entry) = entry else {
            // no manifest yet: fetch it, then try again
            let has_manifest = state.read(cx).packs.manifest.is_some();
            if has_manifest {
                Self::state(cx).update(cx, |s, cx| {
                    s.packs.errors.insert(
                        kind,
                        format!(
                            "{} is not available for Corvane {}",
                            kind.title(),
                            app_version()
                        ),
                    );
                    cx.notify();
                });
                return;
            }
            state.update(cx, |s, cx| {
                s.packs.manifest_loading = true;
                s.packs.manifest_error = None;
                s.packs.progress.insert(
                    kind,
                    PackProgress {
                        received: 0,
                        total: None,
                    },
                );
                cx.notify();
            });
            spawn_bg(cx, corvane_packs::fetch_manifest, move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.packs.manifest_loading = false;
                    s.packs.progress.remove(&kind);
                    match &result {
                        Ok(manifest) => s.packs.manifest = Some(manifest.clone()),
                        Err(err) => {
                            s.packs.manifest_error = Some(err.to_string());
                            s.packs.errors.insert(kind, err.to_string());
                        }
                    }
                    cx.notify();
                });
                if result.is_ok() {
                    Self::install_pack(kind, cx);
                }
            });
            return;
        };
        state.update(cx, |s, cx| {
            s.packs.errors.remove(&kind);
            s.packs.progress.insert(
                kind,
                PackProgress {
                    received: 0,
                    total: Some(entry.size).filter(|s| *s > 0),
                },
            );
            cx.notify();
        });
        let (tx, rx) = async_channel::unbounded::<(u64, Option<u64>)>();
        cx.spawn({
            let state = state.clone();
            async move |cx: &mut AsyncApp| {
                while let Ok((received, total)) = rx.recv().await {
                    state.update(cx, |s, cx| {
                        if let Some(p) = s.packs.progress.get_mut(&kind) {
                            p.received = received;
                            if total.is_some() {
                                p.total = total;
                            }
                            cx.notify();
                        }
                    });
                }
            }
        })
        .detach();
        spawn_bg(
            cx,
            move || {
                let mut last = 0u64;
                let mut progress = |received: u64, total: Option<u64>| {
                    if received == 0 || received - last >= 256 * 1024 {
                        last = received;
                        let _ = tx.try_send((received, total));
                    }
                };
                let pack = corvane_packs::install(&entry, app_version(), &mut progress)?;
                activate(&pack);
                Ok::<InstalledPack, PackError>(pack)
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.packs.progress.remove(&kind);
                    match result {
                        Ok(pack) => {
                            s.packs.installed.insert(kind, pack);
                        }
                        Err(err) => {
                            error!(%err, pack = kind.name(), "could not install the pack");
                            s.packs.errors.insert(kind, err.to_string());
                        }
                    }
                    cx.notify();
                });
            },
        );
    }

    /// Remove `kind` from disk and fall back to the compiled-in data.
    pub fn uninstall_pack(kind: PackKind, cx: &mut App) {
        spawn_bg(
            cx,
            move || {
                deactivate(kind);
                corvane_packs::uninstall(kind)
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    match result {
                        Ok(()) => {
                            s.packs.installed.remove(&kind);
                            s.packs.errors.remove(&kind);
                        }
                        Err(err) => {
                            s.packs.errors.insert(kind, err.to_string());
                        }
                    }
                    cx.notify();
                });
            },
        );
    }
}
