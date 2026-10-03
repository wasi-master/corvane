//! `codemirror/mode/cypher/cypher.js` (`application/x-cypher-query`),
//! ported line by line. GHD maps `.cql` to it.
//!
//! The JS mode keeps a bracket/pattern context (`pushContext`, `popContext`,
//! `curPunc`) for indentation only and a stateless `tokenBase`, so the port
//! has no state at all.

use crate::cm::{Mode, ModeState, StringStream};
use crate::re;

pub struct Cypher;

/// `funcs` (`wordRegexp`: whole word, case-insensitive)
const FUNCS: &[&str] = &[
    "abs",
    "acos",
    "allShortestPaths",
    "asin",
    "atan",
    "atan2",
    "avg",
    "ceil",
    "coalesce",
    "collect",
    "cos",
    "cot",
    "count",
    "degrees",
    "e",
    "endnode",
    "exp",
    "extract",
    "filter",
    "floor",
    "haversin",
    "head",
    "id",
    "keys",
    "labels",
    "last",
    "left",
    "length",
    "log",
    "log10",
    "lower",
    "ltrim",
    "max",
    "min",
    "node",
    "nodes",
    "percentileCont",
    "percentileDisc",
    "pi",
    "radians",
    "rand",
    "range",
    "reduce",
    "rel",
    "relationship",
    "relationships",
    "replace",
    "reverse",
    "right",
    "round",
    "rtrim",
    "shortestPath",
    "sign",
    "sin",
    "size",
    "split",
    "sqrt",
    "startnode",
    "stdev",
    "stdevp",
    "str",
    "substring",
    "sum",
    "tail",
    "tan",
    "timestamp",
    "toFloat",
    "toInt",
    "toString",
    "trim",
    "type",
    "upper",
];

/// `preds`
const PREDS: &[&str] = &[
    "all", "and", "any", "contains", "exists", "has", "in", "none", "not", "or", "single", "xor",
];

/// `keywords`
const KEYWORDS: &[&str] = &[
    "as",
    "asc",
    "ascending",
    "assert",
    "by",
    "case",
    "commit",
    "constraint",
    "create",
    "csv",
    "cypher",
    "delete",
    "desc",
    "descending",
    "detach",
    "distinct",
    "drop",
    "else",
    "end",
    "ends",
    "explain",
    "false",
    "fieldterminator",
    "foreach",
    "from",
    "headers",
    "in",
    "index",
    "is",
    "join",
    "limit",
    "load",
    "match",
    "merge",
    "null",
    "on",
    "optional",
    "order",
    "periodic",
    "profile",
    "remove",
    "return",
    "scan",
    "set",
    "skip",
    "start",
    "starts",
    "then",
    "true",
    "union",
    "unique",
    "unwind",
    "using",
    "when",
    "where",
    "with",
    "call",
    "yield",
];

/// `systemKeywords`
const SYSTEM_KEYWORDS: &[&str] = &[
    "access",
    "active",
    "assign",
    "all",
    "alter",
    "as",
    "catalog",
    "change",
    "copy",
    "create",
    "constraint",
    "constraints",
    "current",
    "database",
    "databases",
    "dbms",
    "default",
    "deny",
    "drop",
    "element",
    "elements",
    "exists",
    "from",
    "grant",
    "graph",
    "graphs",
    "if",
    "index",
    "indexes",
    "label",
    "labels",
    "management",
    "match",
    "name",
    "names",
    "new",
    "node",
    "nodes",
    "not",
    "of",
    "on",
    "or",
    "password",
    "populated",
    "privileges",
    "property",
    "read",
    "relationship",
    "relationships",
    "remove",
    "replace",
    "required",
    "revoke",
    "role",
    "roles",
    "set",
    "show",
    "start",
    "status",
    "stop",
    "suspended",
    "to",
    "traverse",
    "type",
    "types",
    "user",
    "users",
    "with",
    "write",
];

/// `wordRegexp(words).test(w)`: JS `i` without `u` only folds ASCII onto
/// ASCII, and every listed word is ASCII.
fn in_words(words: &[&str], w: &str) -> bool {
    words.iter().any(|k| k.eq_ignore_ascii_case(w))
}

/// `operatorChars = /[*+\-<>=&|~%^]/`
fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '*' | '+' | '-' | '<' | '>' | '=' | '&' | '|' | '~' | '%' | '^'
    )
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

// tokenBase
fn token_base(stream: &mut StringStream) -> Option<&'static str> {
    let ch = stream.next()?;
    if ch == '"' {
        stream.matches(re!(r#"^[^"]*""#));
        return Some("string");
    }
    if ch == '\'' {
        stream.matches(re!(r"^[^']*'"));
        return Some("string");
    }
    if matches!(ch, '{' | '}' | '(' | ')' | ',' | '.' | ';' | '[' | ']') {
        Some("node")
    } else if ch == '/' && stream.eat('/').is_some() {
        stream.skip_to_end();
        Some("comment")
    } else if is_operator_char(ch) {
        stream.eat_while_if(is_operator_char);
        None
    } else {
        stream.eat_while_if(is_word);
        if stream.eat(':').is_some() {
            stream.eat_while_if(|c| is_word(c) || c == '-');
            return Some("atom");
        }
        let word = stream.current();
        if in_words(FUNCS, word) {
            return Some("builtin");
        }
        if in_words(PREDS, word) {
            return Some("def");
        }
        if in_words(KEYWORDS, word) || in_words(SYSTEM_KEYWORDS, word) {
            return Some("keyword");
        }
        Some("variable")
    }
}

impl Mode for Cypher {
    fn name(&self) -> &'static str {
        "cypher"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(())
    }

    // token
    fn token(&self, stream: &mut StringStream, _state: &mut dyn ModeState) -> Option<String> {
        if stream.eat_space() {
            return None;
        }
        token_base(stream).map(Into::into)
    }
}
