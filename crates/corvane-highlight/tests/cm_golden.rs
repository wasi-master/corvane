//! Every ported CodeMirror mode must tokenize the samples in `tests/cm`
//! exactly like GitHub Desktop's own JavaScript does (`expected/*.tokens`,
//! regenerated with `tools/cm-oracle/gen.py`): same token boundaries, same
//! styles, including the `m-<mode>` class. Samples whose mode is not ported
//! yet are skipped.

use std::fs;
use std::path::Path;

use corvane_highlight::cm;

#[test]
fn ported_modes_match_github_desktop() {
    // CM_GOLDEN_DIR points the test at another samples/ + expected/ pair
    // (e.g. generated fuzz variants)
    let dir = std::env::var_os("CM_GOLDEN_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cm"));
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut entries: Vec<_> = fs::read_dir(dir.join("expected"))
        .unwrap()
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(sample) = name.strip_suffix(".tokens") else {
            continue;
        };
        let Some(mode) = cm::mode_for_path(sample) else {
            continue;
        };
        let text = fs::read_to_string(dir.join("samples").join(sample)).unwrap();
        // GHD splits on /\r?\n|\r/
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = text.split('\n').collect();
        let got: Vec<String> = cm::run(&*mode, &lines, 4)
            .iter()
            .enumerate()
            .flat_map(|(ix, toks)| {
                toks.iter()
                    .map(move |(s, l, t)| format!("{ix} {s} {l} {t}"))
            })
            .collect();
        let want: Vec<String> = fs::read_to_string(entry.path())
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect();
        checked += 1;
        if got != want {
            let first = got
                .iter()
                .zip(&want)
                .position(|(a, b)| a != b)
                .unwrap_or(got.len().min(want.len()));
            let show = |v: &[String]| {
                v.iter()
                    .skip(first.saturating_sub(2))
                    .take(6)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n    ")
            };
            failures.push(format!(
                "{sample}: {} tokens vs GHD {}; first difference at token {first}\n  got:\n    {}\n  GHD:\n    {}",
                got.len(),
                want.len(),
                show(&got),
                show(&want)
            ));
        }
    }
    assert!(checked > 0, "no ported mode has a sample");
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}
