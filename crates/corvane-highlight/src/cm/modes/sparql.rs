//! `codemirror/mode/sparql/sparql.js` (`application/sparql-query`), ported
//! line by line. GHD maps `.rq` to it.
//!
//! The JS mode also keeps a bracket/pattern context (`pushContext`,
//! `popContext`, `curPunc`) for indentation only; it never changes a
//! token's style, so only the tokenizers are ported.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct Sparql;

#[derive(Clone, Copy)]
enum Tokenize {
    /// `tokenBase`
    Base,
    /// `tokenLiteral(quote)`
    Literal(char),
}

#[derive(Clone)]
struct SparqlState {
    tokenize: Tokenize,
}

/// `ops` (`wordRegexp`: whole word, case-insensitive; JS `i` without `u`
/// folds ASCII only here since the word is ASCII past its first char)
const OPS: &[&str] = &[
    "str",
    "lang",
    "langmatches",
    "datatype",
    "bound",
    "sameterm",
    "isiri",
    "isuri",
    "iri",
    "uri",
    "bnode",
    "count",
    "sum",
    "min",
    "max",
    "avg",
    "sample",
    "group_concat",
    "rand",
    "abs",
    "ceil",
    "floor",
    "round",
    "concat",
    "substr",
    "strlen",
    "replace",
    "ucase",
    "lcase",
    "encode_for_uri",
    "contains",
    "strstarts",
    "strends",
    "strbefore",
    "strafter",
    "year",
    "month",
    "day",
    "hours",
    "minutes",
    "seconds",
    "timezone",
    "tz",
    "now",
    "uuid",
    "struuid",
    "md5",
    "sha1",
    "sha256",
    "sha384",
    "sha512",
    "coalesce",
    "if",
    "strlang",
    "strdt",
    "isnumeric",
    "regex",
    "exists",
    "isblank",
    "isliteral",
    "a",
    "bind",
];

/// `keywords`
const KEYWORDS: &[&str] = &[
    "base",
    "prefix",
    "select",
    "distinct",
    "reduced",
    "construct",
    "describe",
    "ask",
    "from",
    "named",
    "where",
    "order",
    "limit",
    "offset",
    "filter",
    "optional",
    "graph",
    "by",
    "asc",
    "desc",
    "as",
    "having",
    "undef",
    "values",
    "group",
    "minus",
    "in",
    "not",
    "service",
    "silent",
    "using",
    "insert",
    "delete",
    "union",
    "true",
    "false",
    "with",
    "data",
    "copy",
    "to",
    "move",
    "add",
    "create",
    "drop",
    "clear",
    "load",
    "into",
];

fn in_words(words: &[&str], w: &str) -> bool {
    words.iter().any(|k| k.eq_ignore_ascii_case(w))
}

/// `operatorChars = /[*+\-<>=&|\^\/!\?]/`
fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '*' | '+' | '-' | '<' | '>' | '=' | '&' | '|' | '^' | '/' | '!' | '?'
    )
}

// eatPnLocal
fn eat_pn_local(stream: &mut StringStream) {
    stream.matches(re!(
        r"(\.(?=[A-Za-z0-9_\-\\%])|[:A-Za-z0-9_-]|\\[-\\_~.!$&'()*+,;=/?#@%]|%[a-fA-F0-9][a-fA-F0-9])+"
    ));
}

// tokenBase
fn token_base(stream: &mut StringStream, s: &mut SparqlState) -> &'static str {
    let Some(ch) = stream.next() else {
        return "variable";
    };
    if ch == '$' || ch == '?' {
        if ch == '?' && stream.match_re(re!(r"\s"), false).is_some() {
            return "operator";
        }
        stream.matches(re!(
            r"^[A-Za-z0-9_\x{C0}-\x{D6}\x{D8}-\x{F6}\x{F8}-\x{2FF}\x{370}-\x{37D}\x{37F}-\x{1FFF}\x{200C}-\x{200D}\x{2070}-\x{218F}\x{2C00}-\x{2FEF}\x{3001}-\x{D7FF}\x{F900}-\x{FDCF}\x{FDF0}-\x{FFFD}][A-Za-z0-9_\x{B7}\x{C0}-\x{D6}\x{D8}-\x{F6}\x{F8}-\x{37D}\x{37F}-\x{1FFF}\x{200C}-\x{200D}\x{203F}-\x{2040}\x{2070}-\x{218F}\x{2C00}-\x{2FEF}\x{3001}-\x{D7FF}\x{F900}-\x{FDCF}\x{FDF0}-\x{FFFD}]*"
        ));
        return "variable-2";
    } else if ch == '<' && stream.match_re(re!(r"^[\s\x{a0}=]"), false).is_none() {
        stream.matches(re!(r"^[^\s\x{a0}>]*>?"));
        return "atom";
    } else if ch == '"' || ch == '\'' {
        s.tokenize = Tokenize::Literal(ch);
        return token_literal(stream, s, ch);
    } else if matches!(ch, '{' | '}' | '(' | ')' | ',' | '.' | ';' | '[' | ']') {
        return "bracket";
    } else if ch == '#' {
        stream.skip_to_end();
        return "comment";
    } else if is_operator_char(ch) {
        return "operator";
    } else if ch == ':' {
        eat_pn_local(stream);
        return "atom";
    } else if ch == '@' {
        stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '-');
        return "meta";
    } else if ch.is_ascii_alphabetic()
        && stream.matches(re!(r"(([A-Za-z_\-0-9]|\.)*([A-Za-z_\-0-9]))?:"))
    {
        eat_pn_local(stream);
        return "atom";
    }
    stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_');
    let word = stream.current();
    if in_words(OPS, word) {
        "builtin"
    } else if in_words(KEYWORDS, word) {
        "keyword"
    } else {
        "variable"
    }
}

// tokenLiteral(quote)
fn token_literal(stream: &mut StringStream, s: &mut SparqlState, quote: char) -> &'static str {
    let mut escaped = false;
    while let Some(ch) = stream.next() {
        if ch == quote && !escaped {
            s.tokenize = Tokenize::Base;
            break;
        }
        escaped = !escaped && ch == '\\';
    }
    "string"
}

impl Mode for Sparql {
    fn name(&self) -> &'static str {
        "sparql"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(SparqlState {
            tokenize: Tokenize::Base,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<SparqlState>(st);
        if stream.eat_space() {
            return None;
        }
        let style = match s.tokenize {
            Tokenize::Base => token_base(stream, s),
            Tokenize::Literal(q) => token_literal(stream, s, q),
        };
        Some(style.into())
    }
}
