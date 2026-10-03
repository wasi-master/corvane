//! `codemirror/mode/swift/swift.js` (`text/x-swift`).
//!
//! `state.tokenize` is a stack of tokenizer closures in JS; here it is a
//! stack of [`Tokenize`] values (the `depth` captured by
//! `tokenUntilClosingParen` lives in its variant). The bracket context
//! (`Context`, `pushContext`, `popContext`) only drives indentation and is
//! not ported.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct Swift;

#[derive(Clone, Copy, PartialEq)]
enum Quote {
    Double,
    Single,
    Triple,
}

impl Quote {
    fn text(self) -> &'static str {
        match self {
            Quote::Double => "\"",
            Quote::Single => "'",
            Quote::Triple => "\"\"\"",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    /// `tokenString.bind(null, openQuote)`
    Str(Quote),
    /// `tokenUntilClosingParen()` with its `depth`
    UntilParen(u32),
    /// `tokenComment`
    Comment,
}

#[derive(Clone, Copy, PartialEq)]
enum Prev {
    Define,
    Style(&'static str),
}

#[derive(Clone)]
struct SwiftState {
    prev: Option<Prev>,
    tokenize: Vec<Tokenize>,
}

// `keywords`
fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "_" | "var"
            | "let"
            | "actor"
            | "class"
            | "enum"
            | "extension"
            | "import"
            | "protocol"
            | "struct"
            | "func"
            | "typealias"
            | "associatedtype"
            | "open"
            | "public"
            | "internal"
            | "fileprivate"
            | "private"
            | "deinit"
            | "init"
            | "new"
            | "override"
            | "self"
            | "subscript"
            | "super"
            | "convenience"
            | "dynamic"
            | "final"
            | "indirect"
            | "lazy"
            | "required"
            | "static"
            | "unowned"
            | "unowned(safe)"
            | "unowned(unsafe)"
            | "weak"
            | "as"
            | "is"
            | "break"
            | "case"
            | "continue"
            | "default"
            | "else"
            | "fallthrough"
            | "for"
            | "guard"
            | "if"
            | "in"
            | "repeat"
            | "switch"
            | "where"
            | "while"
            | "defer"
            | "return"
            | "inout"
            | "mutating"
            | "nonmutating"
            | "isolated"
            | "nonisolated"
            | "catch"
            | "do"
            | "rethrows"
            | "throw"
            | "throws"
            | "async"
            | "await"
            | "try"
            | "didSet"
            | "get"
            | "set"
            | "willSet"
            | "assignment"
            | "associativity"
            | "infix"
            | "left"
            | "none"
            | "operator"
            | "postfix"
            | "precedence"
            | "precedencegroup"
            | "prefix"
            | "right"
            | "Any"
            | "AnyObject"
            | "Type"
            | "dynamicType"
            | "Self"
            | "Protocol"
            | "__COLUMN__"
            | "__FILE__"
            | "__FUNCTION__"
            | "__LINE__"
    )
}

// `definingKeywords`
fn is_defining_keyword(w: &str) -> bool {
    matches!(
        w,
        "var"
            | "let"
            | "actor"
            | "class"
            | "enum"
            | "extension"
            | "import"
            | "protocol"
            | "struct"
            | "func"
            | "typealias"
            | "associatedtype"
            | "for"
    )
}

// `atoms`
fn is_atom(w: &str) -> bool {
    matches!(w, "true" | "false" | "nil" | "self" | "super" | "_")
}

// `types`
fn is_type(w: &str) -> bool {
    matches!(
        w,
        "Array"
            | "Bool"
            | "Character"
            | "Dictionary"
            | "Double"
            | "Float"
            | "Int"
            | "Int8"
            | "Int16"
            | "Int32"
            | "Int64"
            | "Never"
            | "Optional"
            | "Set"
            | "String"
            | "UInt8"
            | "UInt16"
            | "UInt32"
            | "UInt64"
            | "Void"
    )
}

/// `operators = "+-/*%=|&<>~^?!"`
fn is_operator(c: char) -> bool {
    "+-/*%=|&<>~^?!".contains(c)
}

/// `punc = ":;,.(){}[]"`
fn is_punc(c: char) -> bool {
    ":;,.(){}[]".contains(c)
}

fn token_base(
    stream: &mut StringStream,
    s: &mut SwiftState,
    prev: Option<Prev>,
) -> Option<&'static str> {
    if stream.eat_space() {
        return None;
    }

    let ch = stream.peek()?;
    if ch == '/' {
        if stream.match_str("//", true, false) {
            stream.skip_to_end();
            return Some("comment");
        }
        if stream.match_str("/*", true, false) {
            s.tokenize.push(Tokenize::Comment);
            return token_comment(stream, s);
        }
    }
    // instruction
    if stream.matches(re!(r"^#[A-Za-z]+")) {
        return Some("builtin");
    }
    // attribute
    if stream.matches(re!(r"^@(?:\$[0-9]+|(`?)[_A-Za-z][_A-Za-z$0-9]*\1)")) {
        return Some("attribute");
    }
    // binary
    if stream.matches(re!(r"^-?0b[01][01_]*")) {
        return Some("number");
    }
    // octal
    if stream.matches(re!(r"^-?0o[0-7][0-7_]*")) {
        return Some("number");
    }
    // hexadecimal
    if stream.matches(re!(
        r"^-?0x[0-9A-Fa-f][0-9A-Fa-f_]*(?:(?:\.[0-9A-Fa-f][0-9A-Fa-f_]*)?[Pp]-?[0-9][0-9_]*)?"
    )) {
        return Some("number");
    }
    // decimal
    if stream.matches(re!(
        r"^-?[0-9][0-9_]*(?:\.[0-9][0-9_]*)?(?:[Ee]-?[0-9][0-9_]*)?"
    )) {
        return Some("number");
    }
    // property
    if stream.matches(re!(r"^\.(?:\$[0-9]+|(`?)[_A-Za-z][_A-Za-z$0-9]*\1)")) {
        return Some("property");
    }
    if is_operator(ch) {
        stream.next();
        return Some("operator");
    }
    if is_punc(ch) {
        stream.next();
        stream.match_str("..", true, false);
        return Some("punctuation");
    }
    // /("""|"|')/
    let quote = if stream.match_str("\"\"\"", true, false) {
        Some(Quote::Triple)
    } else if stream.match_str("\"", true, false) {
        Some(Quote::Double)
    } else if stream.match_str("'", true, false) {
        Some(Quote::Single)
    } else {
        None
    };
    if let Some(quote) = quote {
        s.tokenize.push(Tokenize::Str(quote));
        return token_string(quote, stream, s);
    }

    // identifier: /^\$\d+|(`?)[_A-Za-z][_A-Za-z$0-9]*\1/ (the second
    // alternative is unanchored in JS, but a later match is rejected anyway)
    if stream.matches(re!(r"^(?:\$[0-9]+|(`?)[_A-Za-z][_A-Za-z$0-9]*\1)")) {
        let ident = stream.current();
        if is_type(ident) {
            return Some("variable-2");
        }
        if is_atom(ident) {
            return Some("atom");
        }
        if is_keyword(ident) {
            if is_defining_keyword(ident) {
                s.prev = Some(Prev::Define);
            }
            return Some("keyword");
        }
        if prev == Some(Prev::Define) {
            return Some("def");
        }
        return Some("variable");
    }

    stream.next();
    None
}

/// `tokenUntilClosingParen()`: the tokenizer at `index` in the stack.
fn token_until_closing_paren(
    index: usize,
    stream: &mut StringStream,
    s: &mut SwiftState,
    prev: Option<Prev>,
) -> Option<&'static str> {
    let inner = token_base(stream, s, prev);
    if inner == Some("punctuation") {
        let Some(Tokenize::UntilParen(depth)) = s.tokenize.get(index).copied() else {
            return inner;
        };
        let current = stream.current();
        if current == "(" {
            s.tokenize[index] = Tokenize::UntilParen(depth + 1);
        } else if current == ")" {
            if depth == 0 {
                stream.back_up(1);
                s.tokenize.pop();
                return dispatch(stream, s, None);
            } else {
                s.tokenize[index] = Tokenize::UntilParen(depth - 1);
            }
        }
    }
    inner
}

/// `tokenString(openQuote, stream, state)`
fn token_string(
    quote: Quote,
    stream: &mut StringStream,
    s: &mut SwiftState,
) -> Option<&'static str> {
    let single_line = quote != Quote::Triple;
    let mut escaped = false;
    while let Some(ch) = stream.peek() {
        if escaped {
            stream.next();
            if ch == '(' {
                s.tokenize.push(Tokenize::UntilParen(0));
                return Some("string");
            }
            escaped = false;
        } else if stream.match_str(quote.text(), true, false) {
            s.tokenize.pop();
            return Some("string");
        } else {
            stream.next();
            escaped = ch == '\\';
        }
    }
    if single_line {
        s.tokenize.pop();
    }
    Some("string")
}

/// `tokenComment`
fn token_comment(stream: &mut StringStream, s: &mut SwiftState) -> Option<&'static str> {
    while let Some(ch) = stream.next() {
        if ch == '/' && stream.eat('*').is_some() {
            s.tokenize.push(Tokenize::Comment);
        } else if ch == '*' && stream.eat('/').is_some() {
            s.tokenize.pop();
            break;
        }
    }
    Some("comment")
}

/// `(state.tokenize[state.tokenize.length - 1] || tokenBase)(stream, state, prev)`
fn dispatch(
    stream: &mut StringStream,
    s: &mut SwiftState,
    prev: Option<Prev>,
) -> Option<&'static str> {
    match s.tokenize.last().copied() {
        None => token_base(stream, s, prev),
        Some(Tokenize::Str(q)) => token_string(q, stream, s),
        Some(Tokenize::UntilParen(_)) => {
            token_until_closing_paren(s.tokenize.len() - 1, stream, s, prev)
        }
        Some(Tokenize::Comment) => token_comment(stream, s),
    }
}

impl Mode for Swift {
    fn name(&self) -> &'static str {
        "swift"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(SwiftState {
            prev: None,
            tokenize: Vec::new(),
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<SwiftState>(st);
        let prev = s.prev.take();
        let style = dispatch(stream, s, prev);
        match style {
            None | Some("comment") => s.prev = prev,
            Some(style) if s.prev.is_none() => s.prev = Some(Prev::Style(style)),
            _ => {}
        }
        style.map(str::to_string)
    }
}
