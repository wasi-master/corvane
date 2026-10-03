//! The self-updater's minisign public key is compiled in from
//! `CORVANE_UPDATE_PUBLIC_KEY` (see `packaging/release.md`). Without it the
//! binary carries a placeholder key that rejects every real release, so say
//! so loudly on every build.

fn main() {
    println!("cargo:rerun-if-env-changed=CORVANE_UPDATE_PUBLIC_KEY");
    let key = std::env::var("CORVANE_UPDATE_PUBLIC_KEY").unwrap_or_default();
    if key.trim().is_empty() {
        println!(
            "cargo:warning=CORVANE_UPDATE_PUBLIC_KEY is not set: the self-updater ships with a \
             placeholder minisign key and will refuse every real update (packaging/release.md)"
        );
    }
}
