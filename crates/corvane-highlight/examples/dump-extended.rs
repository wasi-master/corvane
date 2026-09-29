//! Writes the `syntax-extended` pack's `syntaxes.packdump`
//! (`packaging/packs.sh`): the two-face grammar collection as a syntect dump.
//!
//! `cargo run -p corvane-highlight --features pack-builder --example dump-extended -- <out>`

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "syntaxes.packdump".to_string());
    match corvane_highlight::syntaxes::write_extended_dump(std::path::Path::new(&out)) {
        Ok(count) => println!("wrote {count} syntaxes to {out}"),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
