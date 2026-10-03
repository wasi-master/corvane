//! Build the base build's extra TextMate grammars (tools/tm-grammars): load
//! every converted `.sublime-syntax`, reject the ones that fail to parse
//! their language's samples (regexes Oniguruma accepts and fancy-regex does
//! not, broken includes), and write syntect's default set plus the rest as
//! `assets/syntaxes.packdump`.
//!
//! ```text
//! cargo run -p corvane-highlight --features pack-builder --example tm-build -- \
//!     <syntaxes dir> <samples.json> <out.packdump> <rejected.txt>
//! ```
//!
//! `samples.json` maps a syntax's scope to sample files to parse.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use syntect::parsing::{ParseState, SyntaxDefinition, SyntaxSet, SyntaxSetBuilder};

fn main() {
    // rejected grammars panic inside syntect; their reason is recorded instead
    std::panic::set_hook(Box::new(|_| {}));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, samples, out, rejected] = &args[..] else {
        eprintln!("usage: tm-build <syntaxes dir> <samples.json> <out.packdump> <rejected.txt>");
        std::process::exit(2);
    };
    let samples: HashMap<String, Vec<PathBuf>> =
        serde_json::from_str(&std::fs::read_to_string(samples).expect("samples"))
            .expect("samples json");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "sublime-syntax"))
        .collect();
    files.sort();
    let mut defs = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for file in &files {
        match SyntaxDefinition::load_from_str(
            &std::fs::read_to_string(file).expect("read"),
            true,
            None,
        ) {
            Ok(def) => match bad_regex(&def) {
                None => defs.push(def),
                Some(err) => bad.push(format!("{}: {err}", def.scope)),
            },
            Err(err) => bad.push(format!("{}: {err}", file.display())),
        }
    }
    // reject until every remaining syntax parses its samples (checked in
    // parallel; a regex that backtracks forever loses after 20 s)
    loop {
        let set = std::sync::Arc::new(build(&defs));
        let (tx, rx) = mpsc::channel();
        let mut pending = 0;
        for def in &defs {
            let scope = def.scope.to_string();
            let paths = samples.get(&scope).cloned().unwrap_or_default();
            let (set, tx) = (set.clone(), tx.clone());
            pending += 1;
            std::thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let syntax = set
                        .find_syntax_by_scope(syntect::parsing::Scope::new(&scope).expect("scope"))
                        .ok_or_else(|| "not in the set".to_string())?;
                    check(&set, syntax, &paths)
                }))
                .unwrap_or_else(|_| Err("a regex does not compile".into()));
                let _ = tx.send((scope, result));
            });
        }
        drop(tx);
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let mut failed = Vec::new();
        let mut done = std::collections::HashSet::new();
        while pending > 0 {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match rx.recv_timeout(left) {
                Ok((scope, result)) => {
                    pending -= 1;
                    done.insert(scope.clone());
                    if let Err(err) = result {
                        failed.push((scope, err));
                    }
                }
                Err(_) => break,
            }
        }
        for def in &defs {
            let scope = def.scope.to_string();
            if !done.contains(&scope) {
                failed.push((scope, "timed out".into()));
            }
        }
        if failed.is_empty() {
            break;
        }
        for (scope, err) in failed {
            bad.push(format!("{scope}: {err}"));
            defs.retain(|d| d.scope.to_string() != scope);
        }
    }
    let set = build(&defs);
    syntect::dumps::dump_to_file(&set, out).expect("dump");
    std::fs::write(rejected, bad.join("\n") + "\n").expect("rejected");
    println!(
        "{} syntaxes ({} added), {} rejected",
        set.syntaxes().len(),
        defs.len(),
        bad.len()
    );
}

/// The first regex fancy-regex cannot compile (patterns that use the push
/// match's captures compile only once those are known, so they are skipped).
fn bad_regex(def: &SyntaxDefinition) -> Option<String> {
    for context in def.contexts.values() {
        for pattern in &context.patterns {
            if let syntect::parsing::syntax_definition::Pattern::Match(m) = pattern
                && !m.has_captures
                && let Some(err) = syntect::parsing::Regex::try_compile(m.regex.regex_str())
            {
                return Some(format!("regex {:?}: {err}", m.regex.regex_str()));
            }
        }
    }
    None
}

fn build(defs: &[SyntaxDefinition]) -> SyntaxSet {
    let mut builder: SyntaxSetBuilder = SyntaxSet::load_defaults_newlines().into_builder();
    for def in defs {
        builder.add(def.clone());
    }
    builder.build()
}

fn check(
    set: &SyntaxSet,
    syntax: &syntect::parsing::SyntaxReference,
    paths: &[PathBuf],
) -> Result<(), String> {
    let mut text = String::from("x\n");
    for p in paths {
        text.push_str(&std::fs::read_to_string(p).unwrap_or_default());
    }
    let mut state = ParseState::new(syntax);
    for line in text.lines() {
        state
            .parse_line(&format!("{line}\n"), set)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}
