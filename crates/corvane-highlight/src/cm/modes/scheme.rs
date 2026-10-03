//! `codemirror/mode/scheme/scheme.js` (`text/x-scheme`), ported line by
//! line. GHD maps `.ss`, `.sls` and `.scm` to it.
//!
//! Only the bracket types of the JS `indentStack` matter for tokens (a
//! closing bracket pops only when it matches); indentation values are not
//! kept. The number matchers are the JS ones with the `i` flag spelled out
//! as ASCII classes (Rust's `(?i)` would also fold `ſ` onto `s`).
//! Quirks kept from the JS: `token` skips spaces before anything else, so a
//! string or `|symbol|` with spaces is several tokens, and an opening `"`
//! is a token on its own.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct Scheme;

const BUILTIN: &str = "builtin";
const COMMENT: &str = "comment";
const STRING: &str = "string";
const SYMBOL: &str = "symbol";
const ATOM: &str = "atom";
const NUMBER: &str = "number";
const BRACKET: &str = "bracket";

/// `state.mode`
#[derive(Clone, Copy, PartialEq)]
enum ModeKind {
    /// `false`
    Default,
    Str,
    Symbol,
    Comment,
    SExprComment,
}

#[derive(Clone)]
struct SchemeState {
    /// `indentStack` (bracket types only, innermost last)
    indent_stack: Vec<char>,
    mode: ModeKind,
    /// `sExprComment` (`false` = `None`)
    s_expr_comment: Option<i32>,
    /// `sExprQuote` (`false` = `None`)
    s_expr_quote: Option<i32>,
}

/// `keywords`
fn is_keyword(w: &str) -> bool {
    static KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    KEYWORDS
        .get_or_init(|| {
            "λ case-lambda call/cc class cond-expand define-class define-values exit-handler field import inherit init-field interface let*-values let-values let/ec mixin opt-lambda override protect provide public rename require require-for-syntax syntax syntax-case syntax-error unit/sig unless when with-syntax and begin call-with-current-continuation call-with-input-file call-with-output-file case cond define define-syntax define-macro defmacro delay do dynamic-wind else for-each if lambda let let* let-syntax letrec letrec-syntax map or syntax-rules abs acos angle append apply asin assoc assq assv atan boolean? caar cadr call-with-input-file call-with-output-file call-with-values car cdddar cddddr cdr ceiling char->integer char-alphabetic? char-ci<=? char-ci<? char-ci=? char-ci>=? char-ci>? char-downcase char-lower-case? char-numeric? char-ready? char-upcase char-upper-case? char-whitespace? char<=? char<? char=? char>=? char>? char? close-input-port close-output-port complex? cons cos current-input-port current-output-port denominator display eof-object? eq? equal? eqv? eval even? exact->inexact exact? exp expt #f floor force gcd imag-part inexact->exact inexact? input-port? integer->char integer? interaction-environment lcm length list list->string list->vector list-ref list-tail list? load log magnitude make-polar make-rectangular make-string make-vector max member memq memv min modulo negative? newline not null-environment null? number->string number? numerator odd? open-input-file open-output-file output-port? pair? peek-char port? positive? procedure? quasiquote quote quotient rational? rationalize read read-char real-part real? remainder reverse round scheme-report-environment set! set-car! set-cdr! sin sqrt string string->list string->number string->symbol string-append string-ci<=? string-ci<? string-ci=? string-ci>=? string-ci>? string-copy string-fill! string-length string-ref string-set! string<=? string<? string=? string>=? string>? string? substring symbol->string symbol? #t tan transcript-off transcript-on truncate values vector vector->list vector-fill! vector-length vector-ref vector-set! with-input-from-file with-output-to-file write write-char zero?"
                .split(' ')
                .collect()
        })
        .contains(w)
}

/// `[\w_\-!$%&*+\.\/:<=>?@\^~]`
fn is_atom_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_-!$%&*+./:<=>?@^~".contains(c)
}

#[derive(Clone, Copy)]
enum NumTest {
    Binary,
    Octal,
    Hex,
    Decimal,
}

fn num_test(stream: &mut StringStream, t: NumTest) -> bool {
    match t {
        // isBinaryNumber
        NumTest::Binary => stream.matches(re!(
            r#"^(?:[-+][iI]|[-+][01]+#*(?:\/[01]+#*)?[iI]|[-+]?[01]+#*(?:\/[01]+#*)?@[-+]?[01]+#*(?:\/[01]+#*)?|[-+]?[01]+#*(?:\/[01]+#*)?[-+](?:[01]+#*(?:\/[01]+#*)?)?[iI]|[-+]?[01]+#*(?:\/[01]+#*)?)(?=[()\s\x{feff};"]|$)"#
        )),
        // isOctalNumber
        NumTest::Octal => stream.matches(re!(
            r#"^(?:[-+][iI]|[-+][0-7]+#*(?:\/[0-7]+#*)?[iI]|[-+]?[0-7]+#*(?:\/[0-7]+#*)?@[-+]?[0-7]+#*(?:\/[0-7]+#*)?|[-+]?[0-7]+#*(?:\/[0-7]+#*)?[-+](?:[0-7]+#*(?:\/[0-7]+#*)?)?[iI]|[-+]?[0-7]+#*(?:\/[0-7]+#*)?)(?=[()\s\x{feff};"]|$)"#
        )),
        // isHexNumber
        NumTest::Hex => stream.matches(re!(
            r#"^(?:[-+][iI]|[-+][0-9a-fA-F]+#*(?:\/[0-9a-fA-F]+#*)?[iI]|[-+]?[0-9a-fA-F]+#*(?:\/[0-9a-fA-F]+#*)?@[-+]?[0-9a-fA-F]+#*(?:\/[0-9a-fA-F]+#*)?|[-+]?[0-9a-fA-F]+#*(?:\/[0-9a-fA-F]+#*)?[-+](?:[0-9a-fA-F]+#*(?:\/[0-9a-fA-F]+#*)?)?[iI]|[-+]?[0-9a-fA-F]+#*(?:\/[0-9a-fA-F]+#*)?)(?=[()\s\x{feff};"]|$)"#
        )),
        NumTest::Decimal => is_decimal_number(stream),
    }
}

/// `isDecimalNumber(stream)` (without the backup)
fn is_decimal_number(stream: &mut StringStream) -> bool {
    stream.matches(re!(
        r#"^(?:[-+][iI]|[-+](?:(?:(?:[0-9]+#+\.?#*|[0-9]+\.[0-9]*#*|\.[0-9]+#*|[0-9]+)(?:[eEsSfFdDlL][-+]?[0-9]+)?)|[0-9]+#*\/[0-9]+#*)[iI]|[-+]?(?:(?:(?:[0-9]+#+\.?#*|[0-9]+\.[0-9]*#*|\.[0-9]+#*|[0-9]+)(?:[eEsSfFdDlL][-+]?[0-9]+)?)|[0-9]+#*\/[0-9]+#*)@[-+]?(?:(?:(?:[0-9]+#+\.?#*|[0-9]+\.[0-9]*#*|\.[0-9]+#*|[0-9]+)(?:[eEsSfFdDlL][-+]?[0-9]+)?)|[0-9]+#*\/[0-9]+#*)|[-+]?(?:(?:(?:[0-9]+#+\.?#*|[0-9]+\.[0-9]*#*|\.[0-9]+#*|[0-9]+)(?:[eEsSfFdDlL][-+]?[0-9]+)?)|[0-9]+#*\/[0-9]+#*)[-+](?:(?:(?:[0-9]+#+\.?#*|[0-9]+\.[0-9]*#*|\.[0-9]+#*|[0-9]+)(?:[eEsSfFdDlL][-+]?[0-9]+)?)|[0-9]+#*\/[0-9]+#*)?[iI]|(?:(?:(?:[0-9]+#+\.?#*|[0-9]+\.[0-9]*#*|\.[0-9]+#*|[0-9]+)(?:[eEsSfFdDlL][-+]?[0-9]+)?)|[0-9]+#*\/[0-9]+#*))(?=[()\s\x{feff};"]|$)"#
    ))
}

/// `processEscapedSequence`
fn process_escaped_sequence(stream: &mut StringStream, token: char, s: &mut SchemeState) {
    let mut escaped = false;
    while let Some(next) = stream.next() {
        if next == token && !escaped {
            s.mode = ModeKind::Default;
            break;
        }
        escaped = !escaped && next == '\\';
    }
}

/// The `default:` branch of `token`.
fn token_default(stream: &mut StringStream, s: &mut SchemeState) -> Option<&'static str> {
    let ch = stream.next()?;
    if ch == '"' {
        s.mode = ModeKind::Str;
        Some(STRING)
    } else if ch == '\'' {
        if matches!(stream.peek(), Some('(' | '[')) {
            if s.s_expr_quote.is_none() {
                s.s_expr_quote = Some(0);
            } // else already in a quoted expression
        } else {
            stream.eat_while_if(is_atom_char);
        }
        Some(ATOM)
    } else if ch == '|' {
        s.mode = ModeKind::Symbol;
        Some(SYMBOL)
    } else if ch == '#' {
        if stream.eat('|').is_some() {
            // Multi-line comment
            s.mode = ModeKind::Comment;
            Some(COMMENT)
        } else if stream
            .eat_if(|c| matches!(c, 't' | 'f' | 'T' | 'F'))
            .is_some()
        {
            // #t/#f (atom)
            Some(ATOM)
        } else if stream.eat(';').is_some() {
            // S-Expr comment
            s.mode = ModeKind::SExprComment;
            Some(COMMENT)
        } else {
            let mut test = None;
            let mut has_exactness = false;
            let mut has_radix = true;
            if stream
                .eat_if(|c| matches!(c, 'e' | 'i' | 'E' | 'I'))
                .is_some()
            {
                has_exactness = true;
            } else {
                stream.back_up(1); // must be radix specifier
            }
            if stream.matches(re!(r"^#[bB]")) {
                test = Some(NumTest::Binary);
            } else if stream.matches(re!(r"^#[oO]")) {
                test = Some(NumTest::Octal);
            } else if stream.matches(re!(r"^#[xX]")) {
                test = Some(NumTest::Hex);
            } else if stream.matches(re!(r"^#[dD]")) {
                test = Some(NumTest::Decimal);
            } else if matches!(stream.peek(), Some('-' | '+' | '0'..='9' | '.')) {
                has_radix = false;
                test = Some(NumTest::Decimal);
            // re-consume the initial # if all matches failed
            } else if !has_exactness {
                stream.eat('#');
            }
            let mut ret = None;
            if let Some(t) = test {
                if has_radix && !has_exactness {
                    // consume optional exactness after radix
                    stream.matches(re!(r"^#[eiEI]"));
                }
                if num_test(stream, t) {
                    ret = Some(NUMBER);
                }
            }
            ret
        }
    } else if matches!(ch, '-' | '+' | '0'..='9' | '.') && {
        // isDecimalNumber(stream, true): the backup stays when it fails
        stream.back_up(1);
        is_decimal_number(stream)
    } {
        // match non-prefixed number, must be decimal
        Some(NUMBER)
    } else if ch == ';' {
        // comment
        stream.skip_to_end(); // rest of the line is a comment
        Some(COMMENT)
    } else if ch == '(' || ch == '[' {
        // the JS eats the next word to pick an indentation, then backs up
        // to just after the bracket
        s.indent_stack.push(ch);
        stream.pos = stream.start + 1;

        if let Some(n) = s.s_expr_comment.as_mut() {
            *n += 1;
        }
        if let Some(n) = s.s_expr_quote.as_mut() {
            *n += 1;
        }

        Some(BRACKET)
    } else if ch == ')' || ch == ']' {
        let mut ret = BRACKET;
        let open = if ch == ')' { '(' } else { '[' };
        if s.indent_stack.last() == Some(&open) {
            s.indent_stack.pop();

            if let Some(n) = s.s_expr_comment.as_mut() {
                *n -= 1;
                if *n == 0 {
                    ret = COMMENT; // final closing bracket
                    s.s_expr_comment = None; // turn off s-expr commenting mode
                }
            }
            if let Some(n) = s.s_expr_quote.as_mut() {
                *n -= 1;
                if *n == 0 {
                    ret = ATOM; // final closing bracket
                    s.s_expr_quote = None; // turn off s-expr quote mode
                }
            }
        }
        Some(ret)
    } else {
        stream.eat_while_if(is_atom_char);
        if is_keyword(stream.current()) {
            Some(BUILTIN)
        } else {
            Some("variable")
        }
    }
}

impl Mode for Scheme {
    fn name(&self) -> &'static str {
        "scheme"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(SchemeState {
            indent_stack: Vec::new(),
            mode: ModeKind::Default,
            s_expr_comment: None,
            s_expr_quote: None,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<SchemeState>(st);
        // skip spaces
        if stream.eat_space() {
            return None;
        }
        let return_type = match s.mode {
            // multi-line string parsing mode
            ModeKind::Str => {
                process_escaped_sequence(stream, '"', s);
                Some(STRING)
            }
            // escape symbol
            ModeKind::Symbol => {
                process_escaped_sequence(stream, '|', s);
                Some(SYMBOL)
            }
            // comment parsing mode
            ModeKind::Comment => {
                let mut maybe_end = false;
                while let Some(next) = stream.next() {
                    if next == '#' && maybe_end {
                        s.mode = ModeKind::Default;
                        break;
                    }
                    maybe_end = next == '|';
                }
                Some(COMMENT)
            }
            // s-expr commenting mode
            ModeKind::SExprComment => {
                s.mode = ModeKind::Default;
                if matches!(stream.peek(), Some('(' | '[')) {
                    // actually start scheme s-expr commenting mode
                    s.s_expr_comment = Some(0);
                    token_default(stream, s)
                } else {
                    // if not we just comment the entire of the next token
                    stream.eat_while_if(|c| {
                        !(c.is_whitespace() || c == '\u{feff}' || "()[]".contains(c))
                    }); // eat symbol atom
                    Some(COMMENT)
                }
            }
            ModeKind::Default => token_default(stream, s),
        };
        let style = if s.s_expr_comment.is_some() {
            Some(COMMENT)
        } else if s.s_expr_quote.is_some() {
            Some(ATOM)
        } else {
            return_type
        };
        style.map(str::to_string)
    }
}
