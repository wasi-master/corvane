//! `codemirror/mode/htmlembedded/htmlembedded.js`: htmlmixed multiplexed
//! ([`super::multiplex`]) with `<%-- … --%>` comments and `<% … %>` blocks
//! in the MIME's `scriptingModeSpec` (`application/x-ejs` → javascript,
//! `application/x-aspx` → `text/x-csharp`, `application/x-jsp` →
//! `text/x-java`, `application/x-erb` → ruby).
//!
//! GHD's worker loads htmlembedded.js with htmlmixed.js (xml, javascript,
//! css) and multiplex.js only, so of those specs only javascript resolves:
//! `getMode` turns the others into the `null` mode, and GHD leaves the code
//! inside `<% … %>` of .aspx, .cshtml and .jsp files unstyled. The port
//! does the same.
//!
//! The comment "mode" is an anonymous object (`{token}` without a name), so
//! the tokens inside a `<%-- --%>` comment carry no `m-` class.

use std::sync::{Arc, OnceLock};

use super::htmlmixed::NullMode;
use super::javascript::JsState;
use super::multiplex::{Delim, Multiplex, Other};
use crate::cm::{Mode, ModeState, StringStream};

/// The inner `{token}` mode of `<%-- … --%>`.
struct Comment {
    close: &'static str,
}

impl Mode for Comment {
    fn name(&self) -> &'static str {
        ""
    }
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(())
    }
    fn token(&self, stream: &mut StringStream, _state: &mut dyn ModeState) -> Option<String> {
        if !stream.skip_to_str(self.close) {
            stream.skip_to_end();
        }
        Some("comment".to_string())
    }
}

/// The javascript mode instance's `type` / `content` scratch outlives each
/// `<% … %>` block (one `getMode` for the whole document).
fn carry_js(prev: &dyn ModeState, next: &mut dyn ModeState) {
    if let (Some(prev), Some(next)) = (
        prev.as_any().downcast_ref::<JsState>(),
        next.as_any_mut().downcast_mut::<JsState>(),
    ) {
        next.scratch = prev.scratch.clone();
    }
}

/// `CodeMirror.defineMode("htmlembedded", …)` with `scriptingModeSpec`.
pub fn htmlembedded(scripting_mode_spec: &str) -> Arc<dyn Mode> {
    let close_comment = "--%>";
    let comment = Other {
        delim_style: Some("comment"),
        ..Other::new(
            Delim::Str("<%--"),
            Some(Delim::Str(close_comment)),
            Arc::new(Comment {
                close: close_comment,
            }),
        )
    };
    let mode = super::html_modes(scripting_mode_spec).unwrap_or_else(|| Arc::new(NullMode));
    let script = Other {
        carry: Some(carry_js),
        ..Other::new(Delim::Str("<%"), Some(Delim::Str("%>")), mode)
    };
    Arc::new(Multiplex::new(
        "htmlembedded",
        super::htmlmixed(),
        vec![comment, script],
    ))
}

/// The four `defineMIME`s of htmlembedded.js, each built once.
pub fn for_mime(mime: &str) -> Option<Arc<dyn Mode>> {
    static EJS: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    static ASPX: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    static JSP: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    static ERB: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    let (cell, spec) = match mime {
        "application/x-ejs" => (&EJS, "javascript"),
        "application/x-aspx" => (&ASPX, "text/x-csharp"),
        "application/x-jsp" => (&JSP, "text/x-java"),
        "application/x-erb" => (&ERB, "ruby"),
        _ => return None,
    };
    Some(cell.get_or_init(|| htmlembedded(spec)).clone())
}
