//! `codemirror/mode/coffeescript/coffeescript.js` (`text/x-coffeescript`,
//! `text/coffeescript`, `application/vnd.coffeescript`).
//!
//! Ported function by function (JS names in comments). The scope stack
//! matters for tokens: a line indented under the top scope is styled
//! `indent`. GHD's highlighter calls `getMode({}, mime)`, so
//! `conf.indentUnit` is `undefined` and every pushed scope's offset is
//! `NaN`: kept as `f64` so the comparisons behave the same. The `align` /
//! `alignOffset` bookkeeping only feeds `indent()` and is left out.

use crate::re;

use super::super::{Mode, ModeState, StringStream, state};

const ERRORCLASS: &str = "error";

/// `scope.type`: `"coffee"` or the closing bracket
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScopeType {
    Coffee,
    Close(char),
}

#[derive(Clone, Copy, Debug)]
struct Scope {
    offset: f64,
    kind: ScopeType,
}

/// `state.tokenize`
#[derive(Clone, Debug, PartialEq)]
enum Tokenize {
    Base,
    LongComment,
    /// `tokenFactory(delimiter, singleline, outclass)`
    Factory {
        delimiter: String,
        singleline: bool,
        outclass: &'static str,
    },
}

#[derive(Clone, Debug)]
pub struct CoffeeState {
    tokenize: Tokenize,
    /// `state.scope` and its `prev` chain, innermost last
    scope: Vec<Scope>,
    prop: bool,
    dedent: bool,
}

pub struct CoffeeScript;

/// JS `\w`
fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `wordRegexp(words)`: `^((w1)|(w2)|…)\b`, consuming.
fn match_words(stream: &mut StringStream, words: &[&str]) -> bool {
    for w in words {
        if stream.match_str(w, false, false) {
            let n = w.len();
            let last_is_word = w.chars().last().is_some_and(is_word);
            let next_is_word = stream.char_at(stream.pos + n).is_some_and(is_word);
            if last_is_word != next_is_word {
                stream.pos += n;
                return true;
            }
        }
    }
    false
}

const WORD_OPERATORS: &[&str] = &[
    "and",
    "or",
    "not",
    "is",
    "isnt",
    "in",
    "instanceof",
    "typeof",
];
const INDENT_KEYWORDS: &[&str] = &[
    "for", "while", "loop", "if", "unless", "else", "switch", "try", "catch", "finally", "class",
];
const KEYWORDS: &[&str] = &[
    "for", "while", "loop", "if", "unless", "else", "switch", "try", "catch", "finally", "class",
    "break", "by", "continue", "debugger", "delete", "do", "in", "of", "new", "return", "then",
    "this", "@", "throw", "when", "until", "extends",
];
const CONSTANTS: &[&str] = &[
    "Infinity",
    "NaN",
    "undefined",
    "null",
    "true",
    "false",
    "on",
    "off",
    "yes",
    "no",
];

/// `indentKeywords.exec(current)`
fn starts_indent_keyword(current: &str) -> bool {
    INDENT_KEYWORDS
        .iter()
        .any(|w| current.starts_with(w) && !current[w.len()..].chars().next().is_some_and(is_word))
}

impl CoffeeScript {
    fn current_scope(s: &CoffeeState) -> Scope {
        s.scope.last().copied().unwrap_or(Scope {
            offset: 0.0,
            kind: ScopeType::Coffee,
        })
    }

    /// `tokenBase`
    fn token_base(stream: &mut StringStream, s: &mut CoffeeState) -> Option<&'static str> {
        // Handle scope changes
        if stream.sol() {
            let scope_offset = Self::current_scope(s).offset;
            if stream.eat_space() {
                let line_offset = stream.indentation() as f64;
                if line_offset > scope_offset && Self::current_scope(s).kind == ScopeType::Coffee {
                    return Some("indent");
                } else if line_offset < scope_offset {
                    return Some("dedent");
                }
                return None;
            } else if scope_offset > 0.0 {
                Self::dedent(stream, s);
            }
        }
        if stream.eat_space() {
            return None;
        }

        let ch = stream.peek();

        // Handle docco title comment (single line)
        if stream.match_str("####", true, false) {
            stream.skip_to_end();
            return Some("comment");
        }

        // Handle multi line comments
        if stream.match_str("###", true, false) {
            s.tokenize = Tokenize::LongComment;
            return Self::long_comment(stream, s);
        }

        // Single line comment
        if ch == Some('#') {
            stream.skip_to_end();
            return Some("comment");
        }

        // Handle number literals
        if stream.match_re(re!(r"^-?[0-9.]"), false).is_some() {
            let mut float_literal = false;
            // Floats
            if stream.matches(re!(r"^-?[0-9]*\.[0-9]+([eE][+\-]?[0-9]+)?")) {
                float_literal = true;
            }
            if stream.matches(re!(r"^-?[0-9]+\.[0-9]*")) {
                float_literal = true;
            }
            if stream.matches(re!(r"^-?\.[0-9]+")) {
                float_literal = true;
            }

            if float_literal {
                // prevent from getting extra . on 1..
                if stream.peek() == Some('.') {
                    stream.back_up(1);
                }
                return Some("number");
            }
            // Integers
            let mut int_literal = false;
            // Hex
            if stream.matches(re!(r"^-?0[xX][0-9a-fA-F]+")) {
                int_literal = true;
            }
            // Decimal
            if stream.matches(re!(r"^-?[1-9][0-9]*(e[+\-]?[0-9]+)?")) {
                int_literal = true;
            }
            // Zero by itself with no other piece of number.
            if stream.matches(re!(r"^-?0(?![0-9xX])")) {
                int_literal = true;
            }
            if int_literal {
                return Some("number");
            }
        }

        // Handle strings
        if stream.matches(re!(r#"^('{3}|"{3}|['"])"#)) {
            s.tokenize = Tokenize::Factory {
                delimiter: stream.current().to_string(),
                singleline: false,
                outclass: "string",
            };
            return Self::tokenize(stream, s);
        }
        // Handle regex literals
        if stream.matches(re!(r"^(/{3}|/)")) {
            // prevent highlight of division
            if stream.current() != "/" || stream.match_re(re!(r"^.*/"), false).is_some() {
                s.tokenize = Tokenize::Factory {
                    delimiter: stream.current().to_string(),
                    singleline: true,
                    outclass: "string-2",
                };
                return Self::tokenize(stream, s);
            }
            stream.back_up(1);
        }

        // Handle operators and delimiters
        if stream.matches(re!(
            r"^(?:->|=>|\+[+=]?|-[\-=]?|\*[*=]?|/[/=]?|[=!]=|<[><]?=?|>>?=?|%=?|&=?|\|=?|\^=?|~|!|\?|(or|and|\|\||&&|\?)=)"
        )) || match_words(stream, WORD_OPERATORS)
        {
            return Some("operator");
        }
        if stream.matches(re!(r"^(?:[()\[\]{},:`=;]|\.\.?\.?)")) {
            return Some("punctuation");
        }

        if match_words(stream, CONSTANTS) {
            return Some("atom");
        }

        if stream.matches(re!(r"^@[_A-Za-z$][_A-Za-z$0-9]*"))
            || (s.prop && stream.matches(re!(r"^[_A-Za-z$][_A-Za-z$0-9]*")))
        {
            return Some("property");
        }

        if match_words(stream, KEYWORDS) {
            return Some("keyword");
        }

        if stream.matches(re!(r"^[_A-Za-z$][_A-Za-z$0-9]*")) {
            return Some("variable");
        }

        // Handle non-detected items
        stream.next();
        Some(ERRORCLASS)
    }

    fn tokenize(stream: &mut StringStream, s: &mut CoffeeState) -> Option<&'static str> {
        match &s.tokenize {
            Tokenize::Base => Self::token_base(stream, s),
            Tokenize::LongComment => Self::long_comment(stream, s),
            Tokenize::Factory {
                delimiter,
                singleline,
                outclass,
            } => {
                let (delimiter, singleline, outclass) = (delimiter.clone(), *singleline, *outclass);
                Self::token_factory(&delimiter, singleline, outclass, stream, s)
            }
        }
    }

    /// `tokenFactory(delimiter, singleline, outclass)`
    fn token_factory(
        delimiter: &str,
        singleline: bool,
        outclass: &'static str,
        stream: &mut StringStream,
        s: &mut CoffeeState,
    ) -> Option<&'static str> {
        while !stream.eol() {
            stream.eat_while_if(|c| !matches!(c, '\'' | '"' | '/' | '\\'));
            if stream.eat('\\').is_some() {
                stream.next();
                if singleline && stream.eol() {
                    return Some(outclass);
                }
            } else if stream.match_str(delimiter, true, false) {
                s.tokenize = Tokenize::Base;
                return Some(outclass);
            } else {
                stream.eat_if(|c| matches!(c, '\'' | '"' | '/'));
            }
        }
        if singleline {
            // parserConf.singleLineStringErrors is unset
            s.tokenize = Tokenize::Base;
        }
        Some(outclass)
    }

    /// `longComment`
    fn long_comment(stream: &mut StringStream, s: &mut CoffeeState) -> Option<&'static str> {
        while !stream.eol() {
            stream.eat_while_if(|c| c != '#');
            if stream.match_str("###", true, false) {
                s.tokenize = Tokenize::Base;
                break;
            }
            stream.eat_while('#');
        }
        Some("comment")
    }

    /// `indent(stream, state, type)`
    fn indent(s: &mut CoffeeState, kind: ScopeType) {
        let mut offset = 0.0;
        for scope in s.scope.iter().rev() {
            if matches!(scope.kind, ScopeType::Coffee | ScopeType::Close('}')) {
                // `scope.offset + conf.indentUnit`, undefined in GHD
                offset = scope.offset + f64::NAN;
                break;
            }
        }
        s.scope.push(Scope { offset, kind });
    }

    /// `dedent(stream, state)`: `true` when no scope matches the line's
    /// indentation.
    fn dedent(stream: &StringStream, s: &mut CoffeeState) -> bool {
        if s.scope.len() < 2 {
            return false;
        }
        if Self::current_scope(s).kind == ScopeType::Coffee {
            let indent = stream.indentation() as f64;
            if !s.scope.iter().any(|scope| scope.offset == indent) {
                return true;
            }
            while s.scope.len() > 1 && Self::current_scope(s).offset != indent {
                s.scope.pop();
            }
            false
        } else {
            s.scope.pop();
            false
        }
    }

    /// `tokenLexer`
    fn token_lexer(stream: &mut StringStream, s: &mut CoffeeState) -> Option<&'static str> {
        let style = Self::tokenize(stream, s);
        let current = stream.current();

        // Handle scope changes.
        if current == "return" {
            s.dedent = true;
        }
        if ((current == "->" || current == "=>") && stream.eol()) || style == Some("indent") {
            Self::indent(s, ScopeType::Coffee);
        }
        // `"[({".indexOf(current)`
        match current {
            "[" => Self::indent(s, ScopeType::Close(']')),
            "(" => Self::indent(s, ScopeType::Close(')')),
            "{" => Self::indent(s, ScopeType::Close('}')),
            "" => Self::indent(s, ScopeType::Close(']')),
            _ => {}
        }
        if starts_indent_keyword(current) {
            Self::indent(s, ScopeType::Coffee);
        }
        if current == "then" {
            Self::dedent(stream, s);
        }

        if style == Some("dedent") && Self::dedent(stream, s) {
            return Some(ERRORCLASS);
        }
        if let Some(close) = match current {
            "]" => Some(']'),
            ")" => Some(')'),
            "}" => Some('}'),
            _ => None,
        } {
            while Self::current_scope(s).kind == ScopeType::Coffee && s.scope.len() > 1 {
                s.scope.pop();
            }
            if Self::current_scope(s).kind == ScopeType::Close(close) && s.scope.len() > 1 {
                s.scope.pop();
            }
        }
        if s.dedent && stream.eol() {
            if Self::current_scope(s).kind == ScopeType::Coffee && s.scope.len() > 1 {
                s.scope.pop();
            }
            s.dedent = false;
        }

        style
    }
}

impl Mode for CoffeeScript {
    fn name(&self) -> &'static str {
        "coffeescript"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(CoffeeState {
            tokenize: Tokenize::Base,
            scope: vec![Scope {
                offset: 0.0,
                kind: ScopeType::Coffee,
            }],
            prop: false,
            dedent: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<CoffeeState>(st);
        let style = Self::token_lexer(stream, s);
        if let Some(style) = style
            && style != "comment"
        {
            s.prop = style == "punctuation" && stream.current() == ".";
        }
        style.map(str::to_string)
    }
}
