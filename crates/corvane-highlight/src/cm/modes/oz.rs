//! `codemirror/mode/oz/oz.js` (`text/x-oz`), ported line by line. GHD maps
//! `.oz` to it.
//!
//! `state.currentIndent` / `doInCurrentLine` only feed `indent`, so they
//! are left out. `wordRegexp`'s trailing JS `\b` (ASCII) becomes a
//! `(?![A-Za-z0-9_])` lookahead: every listed word ends in a word char.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct Oz;

#[derive(Clone, Copy)]
enum Tokenize {
    /// `tokenBase`
    Base,
    /// `tokenClass`
    Class,
    /// `tokenMeth`
    Meth,
    /// `tokenFunProc`
    FunProc,
    /// `tokenComment`
    Comment,
    /// `tokenString(quote)`
    Str(char),
}

#[derive(Clone)]
struct OzState {
    tokenize: Tokenize,
    has_passed_first_stage: bool,
}

/// `singleOperators = /[\^@!\|<>#~\.\*\-\+\\/,=]/`
fn is_single_operator(c: char) -> bool {
    matches!(
        c,
        '^' | '@'
            | '!'
            | '|'
            | '<'
            | '>'
            | '#'
            | '~'
            | '.'
            | '*'
            | '-'
            | '+'
            | '\\'
            | '/'
            | ','
            | '='
    )
}

// tokenBase
fn token_base(stream: &mut StringStream, s: &mut OzState) -> Option<&'static str> {
    if stream.eat_space() {
        return None;
    }
    // Brackets
    if stream.matches(re!(r"^(?:[{}])")) {
        return Some("bracket");
    }
    // Special [] keyword
    if stream.match_str("[]", true, false) {
        return Some("keyword");
    }
    // Operators (tripleOperators, doubleOperators)
    if stream.matches(re!(r"^(?:(:::)|(\.\.\.)|(=<:)|(>=:))"))
        || stream.matches(re!(
            r"^(?:(<-)|(:=)|(=<)|(>=)|(<=)|(<:)|(>:)|(=:)|(\\=)|(\\=:)|(!!)|(==)|(::))"
        ))
    {
        return Some("operator");
    }
    // Atoms
    if stream.matches(re!(r"^((true)|(false)|(nil)|(unit))(?![A-Za-z0-9_])")) {
        return Some("atom");
    }
    // Opening keywords
    if let Some(matched) = stream.match_re(
        re!(
            r"^((local)|(proc)|(fun)|(case)|(class)|(if)|(cond)|(or)|(dis)|(choice)|(not)|(thread)|(try)|(raise)|(lock)|(for)|(suchthat)|(meth)|(functor))(?![A-Za-z0-9_])"
        ),
        true,
    ) {
        // Special matching for signatures
        match matched.text.as_str() {
            "proc" | "fun" => s.tokenize = Tokenize::FunProc,
            "class" => s.tokenize = Tokenize::Class,
            "meth" => s.tokenize = Tokenize::Meth,
            _ => {}
        }
        return Some("keyword");
    }
    // Middle and other keywords
    if stream.matches(re!(
        r"^((in)|(then)|(else)|(of)|(elseof)|(elsecase)|(elseif)|(catch)|(finally)|(with)|(require)|(prepare)|(import)|(export)|(define)|(do))(?![A-Za-z0-9_])"
    )) || stream.matches(re!(
        r"^((andthen)|(at)|(attr)|(declare)|(feat)|(from)|(lex)|(mod)|(div)|(mode)|(orelse)|(parser)|(prod)|(prop)|(scanner)|(self)|(syn)|(token))(?![A-Za-z0-9_])"
    )) {
        return Some("keyword");
    }
    // End keywords
    if stream.matches(re!(r"^((end))(?![A-Za-z0-9_])")) {
        return Some("keyword");
    }
    // Eat the next char for next comparisons
    let ch = stream.next()?;
    // Strings
    if ch == '"' || ch == '\'' {
        s.tokenize = Tokenize::Str(ch);
        return Some(token_string(stream, s, ch));
    }
    // Numbers
    if ch == '~' || ch.is_ascii_digit() {
        if ch == '~' {
            if !stream.peek().is_some_and(|c| c.is_ascii_digit()) {
                return None;
            } else if (stream.next() == Some('0') && stream.matches(re!(r"^[xX][0-9a-fA-F]+")))
                || stream.matches(re!(r"^[0-9]*(\.[0-9]+)?([eE][~+]?[0-9]+)?"))
            {
                return Some("number");
            }
        }
        if (ch == '0' && stream.matches(re!(r"^[xX][0-9a-fA-F]+")))
            || stream.matches(re!(r"^[0-9]*(\.[0-9]+)?([eE][~+]?[0-9]+)?"))
        {
            return Some("number");
        }
        return None;
    }
    // Comments
    if ch == '%' {
        stream.skip_to_end();
        return Some("comment");
    } else if ch == '/' && stream.eat('*').is_some() {
        s.tokenize = Tokenize::Comment;
        return token_comment(stream, s);
    }
    // Single operators
    if is_single_operator(ch) {
        return Some("operator");
    }
    // If nothing match, we skip the entire alphanumeric block
    stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_');
    Some("variable")
}

// tokenClass
fn token_class(stream: &mut StringStream, s: &mut OzState) -> Option<&'static str> {
    if stream.eat_space() {
        return None;
    }
    stream.matches(re!(
        r"^(?:([A-Z][A-Za-z0-9_]*)|(`[^\n\r\x{2028}\x{2029}]+`))"
    ));
    s.tokenize = Tokenize::Base;
    Some("variable-3")
}

// tokenMeth
fn token_meth(stream: &mut StringStream, s: &mut OzState) -> Option<&'static str> {
    if stream.eat_space() {
        return None;
    }
    stream.matches(re!(
        r"^(?:([a-zA-Z][A-Za-z0-9_]*)|(`[^\n\r\x{2028}\x{2029}]+`))"
    ));
    s.tokenize = Tokenize::Base;
    Some("def")
}

// tokenFunProc
fn token_fun_proc(stream: &mut StringStream, s: &mut OzState) -> Option<&'static str> {
    if stream.eat_space() {
        return None;
    }
    if !s.has_passed_first_stage && stream.eat('{').is_some() {
        s.has_passed_first_stage = true;
        Some("bracket")
    } else if s.has_passed_first_stage {
        stream.matches(re!(
            r"^(?:([A-Z][A-Za-z0-9_]*)|(`[^\n\r\x{2028}\x{2029}]+`)|\$)"
        ));
        s.has_passed_first_stage = false;
        s.tokenize = Tokenize::Base;
        Some("def")
    } else {
        s.tokenize = Tokenize::Base;
        None
    }
}

// tokenComment
fn token_comment(stream: &mut StringStream, s: &mut OzState) -> Option<&'static str> {
    let mut maybe_end = false;
    while let Some(ch) = stream.next() {
        if ch == '/' && maybe_end {
            s.tokenize = Tokenize::Base;
            break;
        }
        maybe_end = ch == '*';
    }
    Some("comment")
}

// tokenString(quote)
fn token_string(stream: &mut StringStream, s: &mut OzState, quote: char) -> &'static str {
    let mut escaped = false;
    let mut end = false;
    while let Some(next) = stream.next() {
        if next == quote && !escaped {
            end = true;
            break;
        }
        escaped = !escaped && next == '\\';
    }
    if end || !escaped {
        s.tokenize = Tokenize::Base;
    }
    "string"
}

impl Mode for Oz {
    fn name(&self) -> &'static str {
        "oz"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(OzState {
            tokenize: Tokenize::Base,
            has_passed_first_stage: false,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<OzState>(st);
        let style = match s.tokenize {
            Tokenize::Base => token_base(stream, s),
            Tokenize::Class => token_class(stream, s),
            Tokenize::Meth => token_meth(stream, s),
            Tokenize::FunProc => token_fun_proc(stream, s),
            Tokenize::Comment => token_comment(stream, s),
            Tokenize::Str(q) => Some(token_string(stream, s, q)),
        };
        style.map(Into::into)
    }
}
