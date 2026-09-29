//! `codemirror/mode/q/q.js` (`text/x-q`), ported line by line. GHD maps
//! `.q` to it.
//!
//! The JS mode also keeps a bracket context for indentation, driven by
//! `curPunc`, which `tokenBase` only ever resets to `null`; it never
//! changes a style, so only the tokenizers are ported. There is no
//! `eatSpace` in `token`: every whitespace char is its own `whitespace`
//! token, as in GHD.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct Q;

#[derive(Clone, Copy)]
enum Tokenize {
    /// `tokenBase`
    Base,
    /// `tokenLineComment`
    LineComment,
    /// `tokenBlockComment`
    BlockComment,
    /// `tokenCommentToEOF`
    CommentToEof,
    /// `tokenString`
    Str,
}

#[derive(Clone)]
struct QState {
    tokenize: Tokenize,
}

/// JS `\s`
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// `keywords = buildRE([...])` (whole word, case-sensitive)
fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "abs"
            | "acos"
            | "aj"
            | "aj0"
            | "all"
            | "and"
            | "any"
            | "asc"
            | "asin"
            | "asof"
            | "atan"
            | "attr"
            | "avg"
            | "avgs"
            | "bin"
            | "by"
            | "ceiling"
            | "cols"
            | "cor"
            | "cos"
            | "count"
            | "cov"
            | "cross"
            | "csv"
            | "cut"
            | "delete"
            | "deltas"
            | "desc"
            | "dev"
            | "differ"
            | "distinct"
            | "div"
            | "do"
            | "each"
            | "ej"
            | "enlist"
            | "eval"
            | "except"
            | "exec"
            | "exit"
            | "exp"
            | "fby"
            | "fills"
            | "first"
            | "fkeys"
            | "flip"
            | "floor"
            | "from"
            | "get"
            | "getenv"
            | "group"
            | "gtime"
            | "hclose"
            | "hcount"
            | "hdel"
            | "hopen"
            | "hsym"
            | "iasc"
            | "idesc"
            | "if"
            | "ij"
            | "in"
            | "insert"
            | "inter"
            | "inv"
            | "key"
            | "keys"
            | "last"
            | "like"
            | "list"
            | "lj"
            | "load"
            | "log"
            | "lower"
            | "lsq"
            | "ltime"
            | "ltrim"
            | "mavg"
            | "max"
            | "maxs"
            | "mcount"
            | "md5"
            | "mdev"
            | "med"
            | "meta"
            | "min"
            | "mins"
            | "mmax"
            | "mmin"
            | "mmu"
            | "mod"
            | "msum"
            | "neg"
            | "next"
            | "not"
            | "null"
            | "or"
            | "over"
            | "parse"
            | "peach"
            | "pj"
            | "plist"
            | "prd"
            | "prds"
            | "prev"
            | "prior"
            | "rand"
            | "rank"
            | "ratios"
            | "raze"
            | "read0"
            | "read1"
            | "reciprocal"
            | "reverse"
            | "rload"
            | "rotate"
            | "rsave"
            | "rtrim"
            | "save"
            | "scan"
            | "select"
            | "set"
            | "setenv"
            | "show"
            | "signum"
            | "sin"
            | "sqrt"
            | "ss"
            | "ssr"
            | "string"
            | "sublist"
            | "sum"
            | "sums"
            | "sv"
            | "system"
            | "tables"
            | "tan"
            | "til"
            | "trim"
            | "txf"
            | "type"
            | "uj"
            | "ungroup"
            | "union"
            | "update"
            | "upper"
            | "upsert"
            | "value"
            | "var"
            | "view"
            | "views"
            | "vs"
            | "wavg"
            | "where"
            | "while"
            | "within"
            | "wj"
            | "wj1"
            | "wsum"
            | "xasc"
            | "xbar"
            | "xcol"
            | "xcols"
            | "xdesc"
            | "xexp"
            | "xgroup"
            | "xkey"
            | "xlog"
            | "xprev"
            | "xrank"
    )
}

/// `E = /[|/&^!+:\\\-*%$=~#;@><,?_\'\"\[\(\]\)\s{}]/`
fn is_e(c: char) -> bool {
    is_js_space(c)
        || matches!(
            c,
            '|' | '/'
                | '&'
                | '^'
                | '!'
                | '+'
                | ':'
                | '\\'
                | '-'
                | '*'
                | '%'
                | '$'
                | '='
                | '~'
                | '#'
                | ';'
                | '@'
                | '>'
                | '<'
                | ','
                | '?'
                | '_'
                | '\''
                | '"'
                | '['
                | '('
                | ']'
                | ')'
                | '{'
                | '}'
        )
}

/// `/^\\\s*$/.test(s)`
fn is_backslash_line(s: &str) -> bool {
    s.strip_prefix('\\')
        .is_some_and(|rest| rest.chars().all(is_js_space))
}

// tokenBase
fn token_base(stream: &mut StringStream, s: &mut QState) -> &'static str {
    let sol = stream.sol();
    let Some(c) = stream.next() else {
        return "error";
    };
    if sol {
        if c == '/' {
            s.tokenize = Tokenize::LineComment;
            return token_line_comment(stream, s);
        } else if c == '\\' {
            if stream.eol() || stream.peek().is_some_and(is_js_space) {
                stream.skip_to_end();
                if is_backslash_line(stream.current()) {
                    s.tokenize = Tokenize::CommentToEof;
                    token_comment_to_eof(stream);
                } else {
                    s.tokenize = Tokenize::Base;
                }
                return "comment";
            }
            s.tokenize = Tokenize::Base;
            return "builtin";
        }
    }
    if is_js_space(c) {
        return if stream.peek() == Some('/') {
            stream.skip_to_end();
            "comment"
        } else {
            "whitespace"
        };
    }
    if c == '"' {
        s.tokenize = Tokenize::Str;
        return token_string(stream, s);
    }
    if c == '`' {
        stream.eat_while_if(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '/' | '.'));
        return "symbol";
    }
    if (c == '.' && stream.peek().is_some_and(|p| p.is_ascii_digit())) || c.is_ascii_digit() {
        let mut t = None;
        stream.back_up(1);
        if stream.matches(re!(
            r"^[0-9]{4}\.[0-9]{2}(m|\.[0-9]{2}([DT]([0-9]{2}(:[0-9]{2}(:[0-9]{2}(\.[0-9]{1,9})?)?)?)?)?)"
        )) || stream.matches(re!(
            r"^[0-9]+D([0-9]{2}(:[0-9]{2}(:[0-9]{2}(\.[0-9]{1,9})?)?)?)"
        )) || stream.matches(re!(r"^[0-9]{2}:[0-9]{2}(:[0-9]{2}(\.[0-9]{1,9})?)?"))
            || stream.matches(re!(r"^[0-9]+[ptuv]{1}"))
        {
            t = Some("temporal");
        } else if stream.matches(re!(r"^0[NwW]{1}"))
            || stream.matches(re!(r"^0x[0-9a-fA-F]*"))
            || stream.matches(re!(r"^[01]+[b]{1}"))
            || stream.matches(re!(r"^[0-9]+[chijn]{1}"))
            || stream.matches(re!(r"^(?:-?[0-9]*(\.[0-9]*)?(e[+\-]?[0-9]+)?(e|f)?)"))
        {
            t = Some("number");
        }
        if let Some(t) = t
            && stream.peek().is_none_or(is_e)
        {
            return t;
        }
        stream.next();
        return "error";
    }
    if c.is_ascii_alphabetic() || c == '.' {
        stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
        return if is_keyword(stream.current()) {
            "keyword"
        } else {
            "variable"
        };
    }
    // operators and brackets are unstyled (`return null`)
    if matches!(
        c,
        '|' | '/'
            | '&'
            | '^'
            | '!'
            | '+'
            | ':'
            | '\\'
            | '-'
            | '*'
            | '%'
            | '$'
            | '='
            | '~'
            | '#'
            | ';'
            | '@'
            | '>'
            | '<'
            | '.'
            | ','
            | '?'
            | '_'
            | '\''
            | '{'
            | '}'
            | '('
            | '['
            | ']'
            | ')'
    ) {
        return "";
    }
    "error"
}

// tokenLineComment
fn token_line_comment(stream: &mut StringStream, s: &mut QState) -> &'static str {
    stream.skip_to_end();
    if stream
        .current()
        .trim_end_matches(is_js_space)
        .ends_with('/')
    {
        s.tokenize = Tokenize::BlockComment;
        token_block_comment(stream, s);
    } else {
        s.tokenize = Tokenize::Base;
    }
    "comment"
}

// tokenBlockComment
fn token_block_comment(stream: &mut StringStream, s: &mut QState) -> &'static str {
    let f = stream.sol() && stream.peek() == Some('\\');
    stream.skip_to_end();
    if f && is_backslash_line(stream.current()) {
        s.tokenize = Tokenize::Base;
    }
    "comment"
}

// tokenCommentToEOF
fn token_comment_to_eof(stream: &mut StringStream) -> &'static str {
    stream.skip_to_end();
    "comment"
}

// tokenString
fn token_string(stream: &mut StringStream, s: &mut QState) -> &'static str {
    let mut escaped = false;
    let mut end = false;
    while let Some(next) = stream.next() {
        if next == '"' && !escaped {
            end = true;
            break;
        }
        escaped = !escaped && next == '\\';
    }
    if end {
        s.tokenize = Tokenize::Base;
    }
    "string"
}

impl Mode for Q {
    fn name(&self) -> &'static str {
        "q"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(QState {
            tokenize: Tokenize::Base,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<QState>(st);
        let style = match s.tokenize {
            Tokenize::Base => token_base(stream, s),
            Tokenize::LineComment => token_line_comment(stream, s),
            Tokenize::BlockComment => token_block_comment(stream, s),
            Tokenize::CommentToEof => token_comment_to_eof(stream),
            Tokenize::Str => token_string(stream, s),
        };
        (!style.is_empty()).then(|| style.into())
    }
}
