//! Compare tree-sitter query sources for every grammar (tools/ts-queries):
//! whether each candidate compiles against the pinned grammar (after
//! dropping the patterns that use node types or fields the pinned version
//! does not have), how much of the samples it colours and how evenly (one
//! class covering nearly all of it usually means a catch-all pattern
//! winning, like a trailing `(variable) @type`).
//!
//! ```text
//! python3 tools/ts-queries/sync.py --candidates
//! cargo run -p corvane-highlight --example ts-queries -- target/ts-queries/candidates
//! ```
//!
//! Prints one row per grammar and source, then per grammar the `source` and
//! `drop` list for languages.toml: the first of nvim, upstream, helix that
//! compiles with at most `MAX_DROPPED` patterns dropped, else the one that
//! drops fewest (README.md).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use corvane_highlight::treesitter::{self, Grammar};
use tree_sitter::{Query, QueryErrorKind};

const SOURCES: [&str; 3] = ["nvim", "upstream", "helix"];
const MAX_DROPPED: usize = 5;

struct Outcome {
    status: String,
    drops: Vec<String>,
    dropped: usize,
    ok: bool,
}

fn main() {
    let candidates = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/ts-queries/candidates".into()),
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let table = corvane_grammars::corvane_grammars_v1().cast::<treesitter::ffi::Table>();
    // SAFETY: the compiled-in table lives for the whole process.
    if let Err(err) = unsafe { treesitter::register_table("bundled", table) } {
        eprintln!("{err}");
        std::process::exit(1);
    }
    let samples = samples(&[root.join("tests/cm/samples"), root.join("tests/ts/samples")]);
    let mut picks = Vec::new();
    for grammar in treesitter::library::grammars() {
        let files: Vec<PathBuf> = samples
            .iter()
            .filter(|p| {
                let name = p.to_string_lossy().to_string();
                treesitter::detect::for_path(std::slice::from_ref(&grammar), &name, "").is_some()
            })
            .cloned()
            .collect();
        let mut outcomes: Vec<(&str, Outcome)> = Vec::new();
        for source in SOURCES {
            let dir = candidates.join(source).join(&grammar.name);
            let Ok(highlights) = std::fs::read_to_string(dir.join("highlights.scm")) else {
                continue;
            };
            let read = |kind: &str| std::fs::read_to_string(dir.join(kind)).unwrap_or_default();
            let candidate = Grammar {
                name: grammar.name.clone(),
                language: grammar.language.clone(),
                highlights,
                injections: read("injections.scm"),
                locals: read("locals.scm"),
                extensions: grammar.extensions.clone(),
                filenames: grammar.filenames.clone(),
                first_line: grammar.first_line.clone(),
                aliases: grammar.aliases.clone(),
                injects: grammar.injects.clone(),
            };
            // query analysis explodes on some patterns: give each source a
            // deadline (the thread is left behind; the tool exits at the end)
            let (tx, rx) = mpsc::channel();
            let files = files.clone();
            std::thread::spawn(move || {
                let _ = tx.send(evaluate(candidate, &files));
            });
            let outcome = rx.recv_timeout(Duration::from_secs(20)).unwrap_or(Outcome {
                status: "TIMEOUT".into(),
                drops: Vec::new(),
                dropped: 0,
                ok: false,
            });
            println!(
                "{}\t{source}\t{}\t{}",
                grammar.name,
                outcome.status,
                if outcome.drops.is_empty() {
                    String::new()
                } else {
                    format!(
                        "drop {} ({} patterns)",
                        outcome.drops.join(","),
                        outcome.dropped
                    )
                }
            );
            outcomes.push((source, outcome));
        }
        let pick = outcomes
            .iter()
            .find(|(_, o)| o.ok && o.dropped <= MAX_DROPPED)
            .or_else(|| {
                outcomes
                    .iter()
                    .filter(|(_, o)| o.ok)
                    .min_by_key(|(_, o)| o.dropped)
            });
        picks.push(match pick {
            Some((source, o)) => format!(
                "{} source={source} drop=[{}]",
                grammar.name,
                o.drops
                    .iter()
                    .map(|d| format!("{d:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            None => format!("{} source=none", grammar.name),
        });
    }
    println!();
    for pick in picks {
        println!("{pick}");
    }
}

/// Compile the candidate, dropping patterns that use unknown node types or
/// fields until it compiles, then measure it on the samples.
fn evaluate(mut grammar: Grammar, files: &[PathBuf]) -> Outcome {
    let mut drops: Vec<String> = Vec::new();
    let mut dropped = 0;
    loop {
        let mut failure = None;
        for text in [&grammar.highlights, &grammar.injections, &grammar.locals] {
            if text.trim().is_empty() {
                continue;
            }
            if let Err(err) = Query::new(&grammar.language, text) {
                failure = Some(err);
                break;
            }
        }
        let Some(err) = failure else {
            break;
        };
        let droppable = matches!(err.kind, QueryErrorKind::NodeType | QueryErrorKind::Field);
        let name = err.message.trim_matches('"').to_string();
        if !droppable || drops.len() >= 40 || drops.contains(&name) {
            return Outcome {
                status: format!(
                    "ERROR {} at {}:{}",
                    err.message,
                    err.row + 1,
                    err.column + 1
                )
                .replace('\n', " "),
                drops,
                dropped,
                ok: false,
            };
        }
        drops.push(name);
        for text in [
            &mut grammar.highlights,
            &mut grammar.injections,
            &mut grammar.locals,
        ] {
            let (kept, n) = drop_patterns(text, &drops);
            *text = kept;
            dropped += n;
        }
    }
    Outcome {
        status: coverage(&grammar, files),
        drops,
        dropped,
        ok: true,
    }
}

/// tools/ts-queries/sync.py `drop_patterns`: remove the top-level patterns
/// that use one of `names` as a node type, an anonymous node or a field.
fn drop_patterns(text: &str, names: &[String]) -> (String, usize) {
    let mut kept = String::new();
    let mut dropped = 0;
    for pattern in split_patterns(text) {
        let code: String = pattern
            .lines()
            .map(strip_comment)
            .collect::<Vec<_>>()
            .join("\n");
        if names.iter().any(|n| uses(&code, n)) {
            dropped += 1;
        } else {
            kept.push_str(&pattern);
        }
    }
    (kept, dropped)
}

fn uses(code: &str, name: &str) -> bool {
    if code.contains(&format!("\"{name}\"")) {
        return true;
    }
    let bytes = code.as_bytes();
    let mut start = 0;
    while let Some(at) = code[start..].find(name) {
        let at = start + at;
        let end = at + name.len();
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(end).copied();
        let node =
            before == Some(b'(') && after.is_some_and(|c| c.is_ascii_whitespace() || c == b')');
        let field = after == Some(b':')
            && before.is_none_or(|c| {
                !(c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'-')
            });
        if node || field {
            return true;
        }
        start = end;
    }
    false
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ';' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Top-level patterns in order, each with the comments above it.
fn split_patterns(text: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    let mut buf = String::new();
    let mut depth = 0i32;
    let mut has_body = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let flush = |buf: &mut String, patterns: &mut Vec<String>, has_body: &mut bool| {
        if !buf.is_empty() {
            if !buf.ends_with('\n') {
                buf.push('\n');
            }
            patterns.push(std::mem::take(buf));
        }
        *has_body = false;
    };
    while i < chars.len() {
        let c = chars[i];
        if c == ';' {
            if depth == 0 && has_body {
                flush(&mut buf, &mut patterns, &mut has_body);
            }
            while i < chars.len() && chars[i] != '\n' {
                buf.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                buf.push('\n');
                i += 1;
            }
            continue;
        }
        if c == '"' {
            if depth == 0 && has_body {
                flush(&mut buf, &mut patterns, &mut has_body);
            }
            buf.push(c);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    buf.push(chars[i]);
                    i += 1;
                }
                buf.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                buf.push('"');
                i += 1;
            }
            if depth == 0 {
                has_body = true;
            }
            continue;
        }
        if c == '(' || c == '[' {
            if depth == 0 && has_body {
                flush(&mut buf, &mut patterns, &mut has_body);
            }
            depth += 1;
        } else if c == ')' || c == ']' {
            depth -= 1;
            if depth == 0 {
                has_body = true;
            }
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &mut patterns, &mut has_body);
    patterns
}

fn samples(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            out.extend(entries.filter_map(|e| e.ok()).map(|e| e.path()));
        }
    }
    out.sort();
    out
}

/// "ok  <coloured %>  <largest class %> <class>" over the samples.
fn coverage(grammar: &Grammar, files: &[PathBuf]) -> String {
    if files.is_empty() {
        return "ok\t(no samples)".into();
    }
    let mut total = 0usize;
    let mut classes: HashMap<String, usize> = HashMap::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        total += text.chars().filter(|c| !c.is_whitespace()).count();
        match treesitter::highlight_with_grammar(grammar, &lines) {
            Ok(spans) => {
                for (line, spans) in lines.iter().zip(spans) {
                    for span in spans {
                        let n = line[span.range.clone()]
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .count();
                        *classes.entry(format!("{:?}", span.class)).or_default() += n;
                    }
                }
            }
            Err(err) => return format!("ERROR highlighting: {err}"),
        }
    }
    let coloured: usize = classes.values().sum();
    let (top, top_n) = classes
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(c, n)| (c.clone(), *n))
        .unwrap_or_default();
    format!(
        "ok\t{:.0}% coloured\t{:.0}% {top}",
        coloured as f64 * 100.0 / total.max(1) as f64,
        top_n as f64 * 100.0 / coloured.max(1) as f64
    )
}
