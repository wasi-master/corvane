//! Compile the grammars without a usable crate (`sources.txt`, written by
//! tools/ts-queries/gen.py) from the sources tools/ts-queries/fetch.py
//! unpacks into `target/grammar-src/<name>/` (`CORVANE_GRAMMAR_SOURCES`
//! overrides the folder). A grammar whose feature is on and whose sources
//! are present gets `cfg(corvane_src = "<name>")`; a missing one is skipped
//! with a warning, so a checkout without the sources still builds.

use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=sources.txt");
    println!("cargo::rerun-if-env-changed=CORVANE_GRAMMAR_SOURCES");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let root = std::env::var_os("CORVANE_GRAMMAR_SOURCES")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../target/grammar-src"));
    let list = std::fs::read_to_string(manifest.join("sources.txt")).unwrap_or_default();
    let mut names = Vec::new();
    for line in list
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let mut parts = line.split_whitespace();
        let (Some(name), scanner) = (parts.next(), parts.next().unwrap_or("-")) else {
            continue;
        };
        names.push(format!("\"{name}\""));
        let feature = format!(
            "CARGO_FEATURE_LANG_{}",
            name.to_uppercase().replace('-', "_")
        );
        if std::env::var_os(&feature).is_none() {
            continue;
        }
        let src = root.join(name).join("src");
        let parser = src.join("parser.c");
        println!("cargo::rerun-if-changed={}", parser.display());
        if !parser.exists() {
            println!(
                "cargo::warning=no sources for the {name} grammar in {}: run tools/ts-queries/fetch.py",
                root.display()
            );
            continue;
        }
        let mut c = cc::Build::new();
        c.include(&src).file(&parser).warnings(false).std("c11");
        if scanner == "scanner.c" {
            c.file(src.join("scanner.c"));
        }
        c.compile(&format!("ts-src-{name}"));
        if scanner == "scanner.cc" {
            cc::Build::new()
                .cpp(true)
                .include(&src)
                .file(src.join("scanner.cc"))
                .warnings(false)
                .compile(&format!("ts-src-{name}-scanner"));
        }
        println!("cargo::rustc-cfg=corvane_src=\"{name}\"");
    }
    println!(
        "cargo::rustc-check-cfg=cfg(corvane_src, values({}))",
        names.join(", ")
    );
}
