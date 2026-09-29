//! `codemirror/mode/protobuf/protobuf.js` (`text/x-protobuf`), ported line
//! by line. GHD maps `.proto` to it. The mode is stateless (no
//! `startState`) and has no block comments.

use crate::cm::{Mode, ModeState, StringStream};
use crate::re;

pub struct Protobuf;

// `keywordArray`
fn is_keyword(w: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "package",
        "message",
        "import",
        "syntax",
        "required",
        "optional",
        "repeated",
        "reserved",
        "default",
        "extensions",
        "packed",
        "bool",
        "bytes",
        "double",
        "enum",
        "float",
        "string",
        "int32",
        "int64",
        "uint32",
        "uint64",
        "sint32",
        "sint64",
        "fixed32",
        "fixed64",
        "sfixed32",
        "sfixed64",
        "option",
        "service",
        "rpc",
        "returns",
    ];
    KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(w))
}

/// `[_A-Za-z\xa1-￿]` (astral chars count: JS sees their surrogates)
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c >= '\u{a1}'
}

fn is_ident(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

/// `tokenBase`
fn token_base(stream: &mut StringStream) -> Option<&'static str> {
    // whitespaces
    if stream.eat_space() {
        return None;
    }

    // Handle one line Comments
    if stream.match_str("//", true, false) {
        stream.skip_to_end();
        return Some("comment");
    }

    // Handle Number Literals
    if matches!(stream.peek(), Some('0'..='9' | '.' | '+' | '-')) {
        if stream.matches(re!(r"^[+-]?0x[0-9a-fA-F]+")) {
            return Some("number");
        }
        if stream.matches(re!(r"^[+-]?[0-9]*\.[0-9]+([EeDd][+-]?[0-9]+)?")) {
            return Some("number");
        }
        if stream.matches(re!(r"^[+-]?[0-9]+([EeDd][+-]?[0-9]+)?")) {
            return Some("number");
        }
    }

    // Handle Strings
    if stream.matches(re!(r#"^"([^"]|(""))*""#)) {
        return Some("string");
    }
    if stream.matches(re!(r"^'([^']|(''))*'")) {
        return Some("string");
    }

    // Handle words: `^((package)|…)\b` (case-insensitive, ASCII `\b`):
    // the ASCII word run at `pos` must be a keyword
    let start = stream.pos;
    let mut end = start;
    while stream
        .char_at(end)
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        end += 1;
    }
    if end > start && is_keyword(stream.slice(start, end)) {
        stream.pos = end;
        return Some("keyword");
    }
    if stream.peek().is_some_and(is_ident_start) {
        stream.next();
        stream.eat_while_if(is_ident);
        return Some("variable");
    }

    // Handle non-detected items
    stream.next();
    None
}

impl Mode for Protobuf {
    fn name(&self) -> &'static str {
        "protobuf"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(())
    }

    fn token(&self, stream: &mut StringStream, _st: &mut dyn ModeState) -> Option<String> {
        token_base(stream).map(str::to_string)
    }
}
