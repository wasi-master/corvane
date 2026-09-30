//! `.docs/flags.md` is rendered from the registry; the test below
//! keeps it current (`UPDATE_FLAGS_DOC=1 cargo test -p corvane-core flags_doc`
//! rewrites it).

use std::fmt::Write;

use super::{Category, Kind, Nature, Preset, REGISTRY, Value};

/// Deviations that stay fixed (engine or platform level); listed at the end
/// of the generated page.
pub const NOT_TOGGLEABLE: &[&str] = &[
    "Native Markdown layout instead of a sandboxed webview (`crates/corvane-ui/src/markdown.rs`)",
    "Image diff \"Difference\" mode blended on the CPU",
    "CodeMirror mode ports and syntect fallback for highlighting",
    "Dark-theme illustrations pre-darkened under `assets/illustrations/dark/`",
    "Alive events over one WebSocket per account",
    "Pull request cache keyed by endpoint + `owner/name`",
    "Tutorial panel fixed at 350 px; Welcome fixed at the 1.2 scale; the Welcome footer's \"no usage metrics\" sentence",
    "Push / pull toolbar button fixed at 230 px",
    "Unsafe repositories detected from the failing git call itself",
    "Main worktree path recorded on every refresh (GHD only on a worktree switch)",
    "Finder › Services › \"Open in Corvane\" (declared in Info.plist)",
    "Self-updater banner wording and single-release \"what's new\"",
    "Spellcheck through NSSpellChecker's automatic language",
    "Diff text selection colour and behaviour",
    "List rows as `AXRow` nodes; keyboard navigation gaps (see deviations.md › Keyboard)",
    "Stash marker: Corvane already uses GHD's `!!GitHub_Desktop<branch>`",
];

fn value_cell(def: &super::FlagDef, value: &Value) -> String {
    match (&def.kind, value) {
        (Kind::Bool, Value::Bool(true)) => "on".into(),
        (Kind::Bool, Value::Bool(false)) => "off".into(),
        (Kind::Number { unit, .. }, Value::Number(n)) => match unit {
            Some(unit) => format!("{n} {unit}"),
            None => n.to_string(),
        },
        (Kind::Select { .. }, v) => format!("`{}`", v.render()),
        (_, v) => format!("`{}`", v.render()),
    }
}

fn escape(cell: &str) -> String {
    cell.replace('|', "\\|").replace('\n', " ")
}

/// The whole Markdown page.
pub fn render_markdown() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Flags");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Generated from `crates/corvane-core/src/flags/registry.rs` by \
         `UPDATE_FLAGS_DOC=1 cargo test -p corvane-core flags_doc`; do not edit by hand."
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Every deviation from GitHub Desktop 3.6.6 that can be switched off, and every \
         Corvane-only extra, is a flag: a numeric id in a category block plus a slug, shown as \
         `201-commit-templates`. \"on\" always means Corvane's deviation is active. Open the \
         dialog with **Corvane › Flags…** (⌘⇧,), `x-corvane://flags?q=<search>` or \
         `CORVANE_POPUP=flags[:<search>]`."
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Flags tagged **Bug fix** fix behaviour GitHub Desktop plainly gets wrong; the dialog hides \
         them unless **Show bug fixes** is ticked (display only: presets and `CORVANE_FLAGS` still \
         apply). The rest are features: new capabilities, options or looks."
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "## Presets");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "A preset is the base layer; per-flag overrides sit on top (\"Custom\"). Picking a preset \
         clears the overrides."
    );
    let _ = writeln!(out);
    for preset in Preset::ALL {
        let _ = writeln!(
            out,
            "- **{}** (`{}`): {}",
            preset.title(),
            preset.slug(),
            preset.description()
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## `CORVANE_FLAGS`");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Flag values for one session, never persisted; the flags it names are locked in the \
         dialog. Comma-separated entries: `preset=<slug>`, `<id>-<slug>=<value>`, `<slug>=<value>`, \
         `<id>=<value>`, or a bare toggle key for `on`. Toggle values: `on|off|true|false|1|0|yes|no`. \
         Bad entries are logged and skipped. The parity harness runs Corvane with \
         `CORVANE_FLAGS=preset=github-desktop`."
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "## Flags");
    for category in Category::ALL {
        let defs: Vec<_> = REGISTRY
            .iter()
            .filter(|d| d.category() == category)
            .collect();
        if defs.is_empty() {
            continue;
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "### {} · {}", category.block(), category.title());
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "| Flag | Kind | Corvane | GitHub Desktop | Familiar | Everything | Restart | Upstream | Code |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|");
        for def in defs {
            let kind = match def.kind {
                Kind::Bool => "toggle".to_string(),
                Kind::Select { options } => format!(
                    "select: {}",
                    options
                        .iter()
                        .map(|o| format!("`{}`", o.value))
                        .collect::<Vec<_>>()
                        .join(" / ")
                ),
                Kind::Number { min, max, unit } => format!(
                    "number {min}–{max}{}",
                    unit.map(|u| format!(" {u}")).unwrap_or_default()
                ),
                Kind::Text { .. } => "text".to_string(),
            };
            let upstream = if def.upstream.is_empty() {
                "—".to_string()
            } else {
                def.upstream
                    .iter()
                    .map(|u| format!("[{}]({})", u.label(), u.url()))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let _ = writeln!(
                out,
                "| **`{}`**{} {}<br>{}<br>*GitHub Desktop: {}* | {} | {} | {} | {} | {} | {} | {} | {} |",
                def.ident(),
                match def.nature {
                    Nature::BugFix => " · **Bug fix**",
                    Nature::Feature => "",
                },
                escape(def.title),
                escape(def.summary),
                escape(def.ghd_behaviour),
                escape(&kind),
                value_cell(def, &def.corvane),
                value_cell(def, &def.ghd),
                value_cell(def, &def.familiar),
                value_cell(def, &def.everything),
                if def.restart { "yes" } else { "" },
                upstream,
                def.code
                    .iter()
                    .map(|c| format!("`{c}`"))
                    .collect::<Vec<_>>()
                    .join("<br>"),
            );
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Not toggleable");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Deviations that are engine or platform decisions and have no flag \
         (`.docs/deviations.md` has the details):"
    );
    let _ = writeln!(out);
    for item in NOT_TOGGLEABLE {
        let _ = writeln!(out, "- {item}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_doc_is_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.docs/flags.md");
        let rendered = render_markdown();
        if std::env::var_os("UPDATE_FLAGS_DOC").is_some() {
            std::fs::write(path, &rendered).unwrap();
            return;
        }
        let on_disk = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            on_disk == rendered,
            ".docs/flags.md is stale: run `UPDATE_FLAGS_DOC=1 cargo test -p corvane-core flags_doc`"
        );
    }

    #[test]
    fn every_flag_is_in_the_page() {
        let page = render_markdown();
        for def in REGISTRY {
            assert!(page.contains(&def.ident()), "{} missing", def.ident());
        }
    }
}
