//! `codemirror/mode/dockerfile/dockerfile.js` (`text/x-dockerfile`), a
//! `defineSimpleMode` table.
//!
//! The JS builds its instruction regexes with the `i` flag; here only the
//! instruction words are case-insensitive (`(?i:…)`; the flag changes nothing
//! else in those regexes). `fancy_regex` has no ASCII-only case folding, so
//! `ſ` and `K` (U+017F, U+212A) also match `s` / `k` here, unlike JS.
//! `fromRegex`'s trailing `\b` (ASCII in JS) is written as a lookahead.

use std::sync::{Arc, OnceLock};

use crate::cm::Mode;
use crate::cm::simple::{SimpleMode, r, r0, rg};

/// `instructionRegex`: `[from, expose].concat(shells).concat(others)`
macro_rules! instructions {
    () => {
        "from|expose|run|cmd|entrypoint|shell|arg|from|maintainer|label|env|add|copy|volume|user|workdir|onbuild|stopsignal|healthcheck|shell"
    };
}

pub fn dockerfile() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| {
        Arc::new(SimpleMode::new(
            "dockerfile",
            vec![
                (
                    "start",
                    vec![
                        // Block comment: This is a line starting with a comment
                        r(r"^\s*#.*$", "comment").sol(),
                        // fromRegex
                        rg(
                            r"^(\s*)((?i:from))(?![A-Za-z0-9_])",
                            &[None, Some("keyword")],
                        )
                        .sol()
                        .next("from"),
                        // instructionOnlyLine: an instruction without any
                        // arguments (for convenience)
                        rg(
                            concat!(r"^(\s*)((?i:", instructions!(), r"))(\s*)(#.*)?$"),
                            &[None, Some("keyword"), None, Some("error")],
                        )
                        .sol(),
                        // shellsAsArrayRegex
                        rg(
                            r"^(\s*)((?i:run|cmd|entrypoint|shell))(\s+\[)",
                            &[None, Some("keyword"), None],
                        )
                        .sol()
                        .next("array"),
                        // exposeRegex
                        rg(r"^(\s*)((?i:expose))(\s+)", &[None, Some("keyword"), None])
                            .sol()
                            .next("expose"),
                        // instructionWithArguments: an instruction followed
                        // by arguments
                        rg(
                            concat!(r"^(\s*)((?i:", instructions!(), r"))(\s+)"),
                            &[None, Some("keyword"), None],
                        )
                        .sol()
                        .next("arguments"),
                        r0(r"."),
                    ],
                ),
                (
                    "from",
                    vec![
                        r0(r"\s*$").next("start"),
                        // Line comment without instruction arguments is an error
                        rg(r"(\s*)(#.*)$", &[None, Some("error")]).next("start"),
                        rg(r"(\s*\S+\s+)((?i:as))", &[None, Some("keyword")]).next("start"),
                        // Fail safe return to start
                        r0("").next("start"),
                    ],
                ),
                (
                    "single",
                    vec![r(r"(?:[^\\']|\\.)", "string"), r(r"'", "string").pop()],
                ),
                (
                    "double",
                    vec![r(r#"(?:[^\\"]|\\.)"#, "string"), r(r#"""#, "string").pop()],
                ),
                (
                    "array",
                    vec![
                        r0(r"\]").next("start"),
                        r(r#""(?:[^\\"]|\\.)*"?"#, "string"),
                    ],
                ),
                (
                    "expose",
                    vec![
                        r(r"[0-9]+$", "number").next("start"),
                        r0(r"[^0-9]+$").next("start"),
                        r(r"[0-9]+", "number"),
                        r0(r"[^0-9]+"),
                        // Fail safe return to start
                        r0("").next("start"),
                    ],
                ),
                (
                    "arguments",
                    vec![
                        r(r"^\s*#.*$", "comment").sol(),
                        r(r#""(?:[^\\"]|\\.)*"?$"#, "string").next("start"),
                        r(r#"""#, "string").push("double"),
                        r(r"'(?:[^\\']|\\.)*'?$", "string").next("start"),
                        r(r"'", "string").push("single"),
                        r0(r#"[^#"']+[\\`]$"#),
                        r0(r#"[^#"']+$"#).next("start"),
                        r0(r#"[^#"']+"#),
                        // Fail safe return to start
                        r0("").next("start"),
                    ],
                ),
            ],
        ))
    })
    .clone()
}
