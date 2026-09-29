//! `codemirror/mode/pig/pig.js` (`text/x-pig`), ported line by line. GHD
//! maps `.pig` to it.
//!
//! As in the JS, strings are styled `error` and the `text/x-pig` MIME sets
//! no `multiLineStrings`.

use std::borrow::Cow;

use crate::cm::{Mode, ModeState, StringStream, state};

pub struct Pig;

#[derive(Clone, Copy)]
enum Tokenize {
    /// `tokenBase`
    Base,
    /// `tokenComment`
    Comment,
    /// `tokenString(quote)`
    Str(char),
}

#[derive(Clone)]
struct PigState {
    tokenize: Tokenize,
}

/// `pBuiltins` (the trailing and doubled spaces add a `""` key, which no
/// word can hit since a word is never empty)
const BUILTINS: &[&str] = &[
    "ABS",
    "ACOS",
    "ARITY",
    "ASIN",
    "ATAN",
    "AVG",
    "BAGSIZE",
    "BINSTORAGE",
    "BLOOM",
    "BUILDBLOOM",
    "CBRT",
    "CEIL",
    "CONCAT",
    "COR",
    "COS",
    "COSH",
    "COUNT",
    "COUNT_STAR",
    "COV",
    "CONSTANTSIZE",
    "CUBEDIMENSIONS",
    "DIFF",
    "DISTINCT",
    "DOUBLEABS",
    "DOUBLEAVG",
    "DOUBLEBASE",
    "DOUBLEMAX",
    "DOUBLEMIN",
    "DOUBLEROUND",
    "DOUBLESUM",
    "EXP",
    "FLOOR",
    "FLOATABS",
    "FLOATAVG",
    "FLOATMAX",
    "FLOATMIN",
    "FLOATROUND",
    "FLOATSUM",
    "GENERICINVOKER",
    "INDEXOF",
    "INTABS",
    "INTAVG",
    "INTMAX",
    "INTMIN",
    "INTSUM",
    "INVOKEFORDOUBLE",
    "INVOKEFORFLOAT",
    "INVOKEFORINT",
    "INVOKEFORLONG",
    "INVOKEFORSTRING",
    "INVOKER",
    "ISEMPTY",
    "JSONLOADER",
    "JSONMETADATA",
    "JSONSTORAGE",
    "LAST_INDEX_OF",
    "LCFIRST",
    "LOG",
    "LOG10",
    "LOWER",
    "LONGABS",
    "LONGAVG",
    "LONGMAX",
    "LONGMIN",
    "LONGSUM",
    "MAX",
    "MIN",
    "MAPSIZE",
    "MONITOREDUDF",
    "NONDETERMINISTIC",
    "OUTPUTSCHEMA",
    "PIGSTORAGE",
    "PIGSTREAMING",
    "RANDOM",
    "REGEX_EXTRACT",
    "REGEX_EXTRACT_ALL",
    "REPLACE",
    "ROUND",
    "SIN",
    "SINH",
    "SIZE",
    "SQRT",
    "STRSPLIT",
    "SUBSTRING",
    "SUM",
    "STRINGCONCAT",
    "STRINGMAX",
    "STRINGMIN",
    "STRINGSIZE",
    "TAN",
    "TANH",
    "TOBAG",
    "TOKENIZE",
    "TOMAP",
    "TOP",
    "TOTUPLE",
    "TRIM",
    "TEXTLOADER",
    "TUPLESIZE",
    "UCFIRST",
    "UPPER",
    "UTF8STORAGECONVERTER",
];

/// `pKeywords`
const KEYWORDS: &[&str] = &[
    "VOID",
    "IMPORT",
    "RETURNS",
    "DEFINE",
    "LOAD",
    "FILTER",
    "FOREACH",
    "ORDER",
    "CUBE",
    "DISTINCT",
    "COGROUP",
    "JOIN",
    "CROSS",
    "UNION",
    "SPLIT",
    "INTO",
    "IF",
    "OTHERWISE",
    "ALL",
    "AS",
    "BY",
    "USING",
    "INNER",
    "OUTER",
    "ONSCHEMA",
    "PARALLEL",
    "PARTITION",
    "GROUP",
    "AND",
    "OR",
    "NOT",
    "GENERATE",
    "FLATTEN",
    "ASC",
    "DESC",
    "IS",
    "STREAM",
    "THROUGH",
    "STORE",
    "MAPREDUCE",
    "SHIP",
    "CACHE",
    "INPUT",
    "OUTPUT",
    "STDERROR",
    "STDIN",
    "STDOUT",
    "LIMIT",
    "SAMPLE",
    "LEFT",
    "RIGHT",
    "FULL",
    "EQ",
    "GT",
    "LT",
    "GTE",
    "LTE",
    "NEQ",
    "MATCHES",
    "TRUE",
    "FALSE",
    "DUMP",
];

/// `pTypes`
const TYPES: &[&str] = &[
    "BOOLEAN",
    "INT",
    "LONG",
    "FLOAT",
    "DOUBLE",
    "CHARARRAY",
    "BYTEARRAY",
    "BAG",
    "TUPLE",
    "MAP",
];

/// `obj.propertyIsEnumerable(word.toUpperCase())`; JS `toUpperCase` is the
/// full Unicode mapping (`ı` → `I`), like Rust's.
fn has(words: &[&str], upper: &str) -> bool {
    words.contains(&upper)
}

fn upper(word: &str) -> Cow<'_, str> {
    if word.bytes().any(|b| b.is_ascii_lowercase() || b >= 0x80) {
        Cow::Owned(word.to_uppercase())
    } else {
        Cow::Borrowed(word)
    }
}

/// `isOperatorChar = /[*+\-%<>=&?:\/!|]/`
fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '*' | '+' | '-' | '%' | '<' | '>' | '=' | '&' | '?' | ':' | '/' | '!' | '|'
    )
}

// tokenComment
fn token_comment(stream: &mut StringStream, s: &mut PigState) -> &'static str {
    let mut is_end = false;
    while let Some(ch) = stream.next() {
        if ch == '/' && is_end {
            s.tokenize = Tokenize::Base;
            break;
        }
        is_end = ch == '*';
    }
    "comment"
}

// tokenString(quote)
fn token_string(stream: &mut StringStream, s: &mut PigState, quote: char) -> &'static str {
    let mut escaped = false;
    let mut end = false;
    while let Some(next) = stream.next() {
        if next == quote && !escaped {
            end = true;
            break;
        }
        escaped = !escaped && next == '\\';
    }
    // `multiLineStrings` is undefined
    if end || !escaped {
        s.tokenize = Tokenize::Base;
    }
    "error"
}

// tokenBase
fn token_base(stream: &mut StringStream, s: &mut PigState) -> Option<&'static str> {
    let ch = stream.next()?;
    if ch == '"' || ch == '\'' {
        s.tokenize = Tokenize::Str(ch);
        return Some(token_string(stream, s, ch));
    } else if matches!(ch, '[' | ']' | '{' | '}' | '(' | ')' | ',' | ';' | '.') {
        return None;
    } else if ch.is_ascii_digit() {
        stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
        return Some("number");
    } else if ch == '/' {
        if stream.eat('*').is_some() {
            s.tokenize = Tokenize::Comment;
            return Some(token_comment(stream, s));
        }
        stream.eat_while_if(is_operator_char);
        return Some("operator");
    } else if ch == '-' {
        if stream.eat('-').is_some() {
            stream.skip_to_end();
            return Some("comment");
        }
        stream.eat_while_if(is_operator_char);
        return Some("operator");
    } else if is_operator_char(ch) {
        stream.eat_while_if(is_operator_char);
        return Some("operator");
    }
    // get the while word
    stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if has(KEYWORDS, &upper(stream.current())) {
        // keywords can be used as variables like flatten(group), group.$0 etc..
        if stream.eat(')').is_none() && stream.eat('.').is_none() {
            return Some("keyword");
        }
    }
    let word = upper(stream.current());
    if has(BUILTINS, &word) {
        return Some("variable-2");
    }
    if has(TYPES, &word) {
        return Some("variable-3");
    }
    Some("variable")
}

impl Mode for Pig {
    fn name(&self) -> &'static str {
        "pig"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(PigState {
            tokenize: Tokenize::Base,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<PigState>(st);
        if stream.eat_space() {
            return None;
        }
        let style = match s.tokenize {
            Tokenize::Base => token_base(stream, s),
            Tokenize::Comment => Some(token_comment(stream, s)),
            Tokenize::Str(q) => Some(token_string(stream, s, q)),
        };
        style.map(Into::into)
    }
}
