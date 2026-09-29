//! `codemirror/mode/handlebars/handlebars.js`
//! (`text/x-handlebars-template`): the `handlebars-tags` simple mode
//! (`codemirror/addon/mode/simple.js`), and `handlebars`, which is that mode
//! on its own or - with a `base` mode in its configuration, as vue uses
//! it - the base mode multiplexed with `handlebars-tags` between `{{` and
//! `}}` / `}}}` (`codemirror/addon/mode/multiplex.js`).
//!
//! `getMode` names the returned object after the spec: alone it is
//! `handlebars` (the tokens get `m-handlebars`); inside the multiplexer
//! the inner mode keeps its own `handlebars-tags` name.
//!
//! JS `.` matches one UTF-16 unit, Rust's a whole char, so a comment
//! character outside the BMP is one token here and two in GHD.

use std::sync::{Arc, OnceLock};

use super::super::Mode;
use super::super::simple::{SimpleMode, r};
use super::multiplex::{Delim, Multiplex, Other};

/// `CodeMirror.defineSimpleMode("handlebars-tags", …)` under `name`.
fn handlebars_tags(name: &'static str) -> SimpleMode {
    SimpleMode::new(
        name,
        vec![
            (
                "start",
                vec![
                    r(r"\{\{\{", "tag").push("handlebars_raw"),
                    r(r"\{\{!--", "comment").push("dash_comment"),
                    r(r"\{\{!", "comment").push("comment"),
                    r(r"\{\{", "tag").push("handlebars"),
                ],
            ),
            ("handlebars_raw", vec![r(r"\}\}\}", "tag").pop()]),
            (
                "handlebars",
                vec![
                    r(r"\}\}", "tag").pop(),
                    // Double and single quotes
                    r(r#""(?:[^\\"]|\\.)*"?"#, "string"),
                    r(r"'(?:[^\\']|\\.)*'?", "string"),
                    // Handlebars keywords
                    r(r">|[#/]([A-Za-z_][A-Za-z0-9_]*)", "keyword"),
                    r(r"(?:else|this)(?![A-Za-z0-9_])", "keyword"),
                    // Numeral
                    r(r"[0-9]+", "number"),
                    // Atoms like = and .
                    r(r"=|~|@|true|false", "atom"),
                    // Paths
                    r(r"(?:\.\.\/)*(?:[A-Za-z_][A-Za-z0-9_\.]*)+", "variable-2"),
                ],
            ),
            (
                "dash_comment",
                vec![
                    r(r"--\}\}", "comment").pop(),
                    // Commented code
                    r(r".", "comment"),
                ],
            ),
            (
                "comment",
                vec![r(r"\}\}", "comment").pop(), r(r".", "comment")],
            ),
        ],
    )
}

/// `CodeMirror.getMode({}, "text/x-handlebars-template")`: no `base`, so
/// the `handlebars-tags` mode itself, renamed `handlebars`.
pub fn handlebars() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| Arc::new(handlebars_tags("handlebars")))
        .clone()
}

/// `CodeMirror.getMode(config, {name: "handlebars", base})`: `base`
/// multiplexed with `handlebars-tags` between `{{` and `}}` / `}}}`.
pub fn handlebars_with_base(base: Arc<dyn Mode>) -> Arc<dyn Mode> {
    let tags: Arc<dyn Mode> = Arc::new(handlebars_tags("handlebars-tags"));
    Arc::new(Multiplex::new(
        "handlebars",
        base,
        vec![Other {
            parse_delimiters: true,
            ..Other::new(
                Delim::Str("{{"),
                Some(Delim::Re(crate::re!(r"\}\}\}?").clone())),
                tags,
            )
        }],
    ))
}
