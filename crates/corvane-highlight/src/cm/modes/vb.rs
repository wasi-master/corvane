//! `codemirror/mode/vb/vb.js` (`text/x-vb`).
//!
//! Ported line by line. The JS state only feeds `indent` (`currentIndent`,
//! `doInCurrentLine`, `dedent` never returns true, `indentInfo` is always
//! `null`), and the only tokenizer besides `tokenBase` is the string one,
//! which always ends on the line it starts on (`singleline`, no
//! `singleLineStringErrors`), so the state is empty here.
//!
//! The JS `wordRegexp(words)` is `/^((w1)|(w2)…)\b/i`: every word is made of
//! `\w` chars (plus a leading `#`), so it matches exactly when the maximal
//! ASCII word run at `pos` equals one of the words, compared ASCII
//! case-insensitively like JS's non-unicode `i` flag.

use crate::cm::{Mode, ModeState, StringStream};
use crate::re;

const ERRORCLASS: &str = "error";

/// `operatorKeywords` (`wordOperators`)
fn is_operator_keyword(w: &str) -> bool {
    matches!(
        w,
        "and" | "andalso" | "or" | "orelse" | "xor" | "in" | "not" | "is" | "isnot" | "like"
    )
}

/// `doOpening`, `openingKeywords`, `middleKeywords`, `end`, `endKeywords`,
/// `commontypes` and `commonKeywords`: all styled `keyword`.
fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        // doOpening
        "do"
        // openingKeywords
        | "class" | "module" | "sub" | "enum" | "select" | "while" | "if" | "function" | "get"
        | "set" | "property" | "try" | "structure" | "synclock" | "using" | "with"
        // middleKeywords
        | "else" | "elseif" | "case" | "catch" | "finally"
        // doubleClosing, endKeywords
        | "end" | "next" | "loop"
        // commontypes
        | "object" | "boolean" | "char" | "string" | "byte" | "sbyte" | "short" | "ushort"
        | "int16" | "uint16" | "integer" | "uinteger" | "int32" | "uint32" | "long" | "ulong"
        | "int64" | "uint64" | "decimal" | "single" | "double" | "float" | "date" | "datetime"
        | "intptr" | "uintptr"
        // commonKeywords
        | "#const" | "#else" | "#elseif" | "#end" | "#if" | "#region" | "addhandler"
        | "addressof" | "alias" | "as" | "byref" | "byval" | "cbool" | "cbyte" | "cchar"
        | "cdate" | "cdbl" | "cdec" | "cint" | "clng" | "cobj" | "compare" | "const"
        | "continue" | "csbyte" | "cshort" | "csng" | "cstr" | "cuint" | "culng" | "cushort"
        | "declare" | "default" | "delegate" | "dim" | "directcast" | "each" | "erase"
        | "error" | "event" | "exit" | "explicit" | "false" | "for" | "friend" | "gettype"
        | "goto" | "handles" | "implements" | "imports" | "infer" | "inherits" | "interface"
        | "isfalse" | "istrue" | "lib" | "me" | "mod" | "mustinherit" | "mustoverride" | "my"
        | "mybase" | "myclass" | "namespace" | "narrowing" | "new" | "nothing"
        | "notinheritable" | "notoverridable" | "of" | "off" | "on" | "operator" | "option"
        | "optional" | "out" | "overloads" | "overridable" | "overrides" | "paramarray"
        | "partial" | "private" | "protected" | "public" | "raiseevent" | "readonly" | "redim"
        | "removehandler" | "resume" | "return" | "shadows" | "shared" | "static" | "step"
        | "stop" | "strict" | "then" | "throw" | "to" | "true" | "trycast" | "typeof" | "until"
        | "when" | "widening" | "withevents" | "writeonly"
    )
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The ASCII-lowercased word (optional `#` + `\w*` run) at `pos` and its
/// length, if short enough to be one of the listed words.
fn word_at(stream: &StringStream) -> Option<([u8; 16], usize)> {
    let mut buf = [0u8; 16];
    let mut n = 0;
    let mut i = stream.pos;
    if stream.char_at(i) == Some('#') {
        buf[0] = b'#';
        n = 1;
        i += 1;
    }
    while let Some(c) = stream.char_at(i).filter(|c| is_word(*c)) {
        if n == buf.len() {
            return None;
        }
        buf[n] = c.to_ascii_lowercase() as u8;
        n += 1;
        i += 1;
    }
    Some((buf, n))
}

/// `tokenBase`
fn token_base(stream: &mut StringStream) -> Option<&'static str> {
    if stream.eat_space() {
        return None;
    }
    let ch = stream.peek();

    // Handle Comments
    if ch == Some('\'') {
        stream.skip_to_end();
        return Some("comment");
    }

    // Handle Number Literals
    if stream
        .match_re(re!(r"(?i)^((&H)|(&O))?[0-9\.a-f]"), false)
        .is_some()
    {
        // Floats
        let float_literal = stream.matches(re!(r"(?i)^[0-9]*\.[0-9]+F?"))
            || stream.matches(re!(r"^[0-9]+\.[0-9]*F?"))
            || stream.matches(re!(r"^\.[0-9]+F?"));
        if float_literal {
            // Float literals may be "imaginary"
            stream.eat_if(|c| c == 'j' || c == 'J');
            return Some("number");
        }
        // Integers
        let mut int_literal = false;
        if stream.matches(re!(r"(?i)^&H[0-9a-f]+")) || stream.matches(re!(r"(?i)^&O[0-7]+")) {
            int_literal = true;
        } else if stream.matches(re!(r"^[1-9][0-9]*F?")) {
            stream.eat_if(|c| c == 'j' || c == 'J');
            int_literal = true;
        } else if stream.matches(re!(r"(?i)^0(?![0-9x])")) {
            int_literal = true;
        }
        if int_literal {
            // Integer literals may be "long"
            stream.eat_if(|c| c == 'l' || c == 'L');
            return Some("number");
        }
    }

    // Handle Strings
    if stream.eat('"').is_some() {
        return token_string(stream);
    }

    // Handle operators and Delimiters
    if stream.matches(re!(r"^((//=)|(>>=)|(<<=)|(\*\*=))"))
        || stream.matches(re!(r"^((\+=)|(\-=)|(\*=)|(%=)|(/=)|(&=)|(\|=)|(\^=))"))
    {
        return None;
    }
    if stream.matches(re!(r"^((==)|(<>)|(<=)|(>=)|(<>)|(<<)|(>>)|(//)|(\*\*))"))
        || stream
            .eat_if(|c| {
                matches!(
                    c,
                    '+' | '-' | '*' | '/' | '%' | '&' | '\\' | '|' | '^' | '~' | '<' | '>' | '!'
                )
            })
            .is_some()
    {
        return Some("operator");
    }
    let word = word_at(stream);
    if let Some((buf, n)) = word
        && n > 0
        && buf[0] != b'#'
        && is_operator_keyword(std::str::from_utf8(&buf[..n]).unwrap_or(""))
    {
        stream.pos += n;
        return Some("operator");
    }
    if stream
        .eat_if(|c| {
            matches!(
                c,
                '(' | ')' | '[' | ']' | '{' | '}' | '@' | ',' | ':' | '`' | '=' | ';' | '.'
            )
        })
        .is_some()
    {
        return None;
    }
    if let Some((buf, n)) = word
        && is_keyword(std::str::from_utf8(&buf[..n]).unwrap_or(""))
    {
        stream.pos += n;
        return Some("keyword");
    }

    // identifiers
    if stream.matches(re!(r"^[_A-Za-z][_A-Za-z0-9]*")) {
        return Some("variable");
    }

    // Handle non-detected items
    stream.next();
    Some(ERRORCLASS)
}

/// `tokenStringFactory('"')`
fn token_string(stream: &mut StringStream) -> Option<&'static str> {
    while !stream.eol() {
        stream.eat_while_if(|c| c != '\'' && c != '"');
        if stream.eat('"').is_some() {
            return Some("string");
        }
        stream.eat_if(|c| c == '\'' || c == '"');
    }
    // singleline: back to tokenBase
    Some("string")
}

pub struct Vb;

#[derive(Clone)]
struct VbState;

impl Mode for Vb {
    fn name(&self) -> &'static str {
        "vb"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(VbState)
    }

    /// `token` → `tokenLexer`
    fn token(&self, stream: &mut StringStream, _st: &mut dyn ModeState) -> Option<String> {
        let mut style = token_base(stream);
        // Handle '.' connected identifiers
        if stream.current() == "." {
            style = token_base(stream);
            style = Some(if style == Some("variable") {
                "variable"
            } else {
                ERRORCLASS
            });
        }
        style.map(str::to_string)
    }
}
