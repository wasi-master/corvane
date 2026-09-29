//! `codemirror/mode/sass/sass.js` (`text/x-sass`), ported line by line;
//! the JS function names are kept on the Rust ones. GHD maps no extension
//! to it; nesting modes (vue) use it.
//!
//! GHD calls `getMode({}, …)`, so `config.indentUnit` is `undefined`: every
//! styled token computes `withCurrentIndent` as `NaN`, which empties
//! `state.scopes`, and the next `indent(state)` on a later line reads
//! `state.scopes[0].offset` of an empty array and throws a `TypeError` - in
//! practice at the second selector of a file. GHD's worker then posts no
//! tokens for the document at all, which a [`Mode`] cannot express; the
//! port stops styling at the throw instead (that token and every later one
//! are unstyled). Scope offsets only feed `indent`, so only the number of
//! scopes is kept.

use super::super::{Mode, ModeState, StringStream, state};
use super::css::{COLOR_KEYWORDS, FONT_PROPERTIES, PROPERTY_KEYWORDS, VALUE_KEYWORDS, has};
use crate::re;

/// `state.tokenizer`
#[derive(Clone, PartialEq, Eq)]
enum Tokenizer {
    Base,
    /// `comment(indentation, multiLine)`
    Comment {
        indentation: usize,
        multi_line: bool,
    },
    /// `buildStringTokenizer(quote, greedy)`
    Str {
        quote: char,
        greedy: bool,
    },
    /// `buildInterpolationTokenizer(currentTokenizer)`
    Interpolation(Box<Tokenizer>),
    /// `urlTokens`
    Url,
}

#[derive(Clone)]
struct SassState {
    tokenizer: Tokenizer,
    /// `state.scopes.length`
    scopes: usize,
    indent_count: usize,
    cursor_half: bool,
    /// `undefined` at first
    prev_prop: Option<String>,
    /// `indent` threw: GHD's worker stopped here
    crashed: bool,
}

/// `indent(state)` threw (`state.scopes[0]` is `undefined`).
struct Crash;

type Tok = Result<Option<&'static str>, Crash>;

/// `indent(state)`
fn indent(s: &mut SassState) -> Result<(), Crash> {
    if s.indent_count == 0 {
        s.indent_count += 1;
        if s.scopes == 0 {
            return Err(Crash);
        }
        s.scopes += 1;
    }
    Ok(())
}

/// `dedent(state)`
fn dedent(s: &mut SassState) {
    if s.scopes == 1 {
        return;
    }
    s.scopes = s.scopes.saturating_sub(1);
}

/// `isEndLine(stream)`
fn is_end_line(stream: &mut StringStream) -> bool {
    stream.peek().is_none() || stream.match_re(re!(r"\s+$"), false).is_some()
}

/// `stream.eatWhile(/[\w-]/)`
fn eat_word(stream: &mut StringStream) -> bool {
    stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `keywordsRegexp`: `/^true|false|null|auto/`, a prefix match
fn keyword(stream: &mut StringStream) -> bool {
    stream.matches(re!(r"true|false|null|auto"))
}

/// `opRegexp`: only its first alternative is anchored, but a match must
/// start at the stream position anyway
fn operator(stream: &mut StringStream) -> bool {
    stream.matches(re!(
        r"\(|\)|=|>|<|==|>=|<=|\+|-|!=|/|\*|%|and|or|not|;|\{|\}|:"
    ))
}

pub struct Sass;

impl Sass {
    fn call(&self, stream: &mut StringStream, s: &mut SassState) -> Tok {
        match s.tokenizer.clone() {
            Tokenizer::Base => self.token_base(stream, s),
            Tokenizer::Comment {
                indentation,
                multi_line,
            } => self.comment(indentation, multi_line, stream, s),
            Tokenizer::Str { quote, greedy } => Ok(Self::string(quote, greedy, stream, s)),
            Tokenizer::Interpolation(current) => {
                if stream.peek() == Some('}') {
                    stream.next();
                    s.tokenizer = *current;
                    return Ok(Some("operator"));
                }
                self.token_base(stream, s)
            }
            Tokenizer::Url => Ok(Self::url_tokens(stream, s)),
        }
    }

    /// `urlTokens`
    fn url_tokens(stream: &mut StringStream, s: &mut SassState) -> Option<&'static str> {
        match stream.peek() {
            Some(')') => {
                stream.next();
                s.tokenizer = Tokenizer::Base;
                Some("operator")
            }
            Some('(') => {
                stream.next();
                stream.eat_space();
                Some("operator")
            }
            Some(q @ ('\'' | '"')) => {
                stream.next();
                s.tokenizer = Tokenizer::Str {
                    quote: q,
                    greedy: true,
                };
                Some("string")
            }
            _ => {
                s.tokenizer = Tokenizer::Str {
                    quote: ')',
                    greedy: false,
                };
                Some("string")
            }
        }
    }

    /// `comment(indentation, multiLine)`
    fn comment(
        &self,
        indentation: usize,
        multi_line: bool,
        stream: &mut StringStream,
        s: &mut SassState,
    ) -> Tok {
        if stream.sol() && stream.indentation() <= indentation {
            s.tokenizer = Tokenizer::Base;
            return self.token_base(stream, s);
        }
        if multi_line && stream.skip_to_str("*/") {
            stream.next();
            stream.next();
            s.tokenizer = Tokenizer::Base;
        } else {
            stream.skip_to_end();
        }
        Ok(Some("comment"))
    }

    /// `buildStringTokenizer(quote, greedy)`'s `stringTokenizer`
    fn string(
        quote: char,
        greedy: bool,
        stream: &mut StringStream,
        s: &mut SassState,
    ) -> Option<&'static str> {
        let next_char = stream.next();
        let peek_char = stream.peek();
        let previous_char = stream.pos.checked_sub(2).and_then(|i| stream.char_at(i));

        let ending = (next_char != Some('\\') && peek_char == Some(quote))
            || (next_char == Some(quote) && previous_char != Some('\\'));

        if ending {
            if next_char != Some(quote) && greedy {
                stream.next();
            }
            if is_end_line(stream) {
                s.cursor_half = false;
            }
            s.tokenizer = Tokenizer::Base;
            Some("string")
        } else if next_char == Some('#') && peek_char == Some('{') {
            s.tokenizer = Tokenizer::Interpolation(Box::new(Tokenizer::Str { quote, greedy }));
            stream.next();
            Some("operator")
        } else {
            Some("string")
        }
    }

    /// `tokenBase`
    fn token_base(&self, stream: &mut StringStream, s: &mut SassState) -> Tok {
        let ch = stream.peek();

        // Comment
        if stream.match_str("/*", true, false) {
            s.tokenizer = Tokenizer::Comment {
                indentation: stream.indentation(),
                multi_line: true,
            };
            return self.call(stream, s);
        }
        if stream.match_str("//", true, false) {
            s.tokenizer = Tokenizer::Comment {
                indentation: stream.indentation(),
                multi_line: false,
            };
            return self.call(stream, s);
        }

        // Interpolation
        if stream.match_str("#{", true, false) {
            s.tokenizer = Tokenizer::Interpolation(Box::new(Tokenizer::Base));
            return Ok(Some("operator"));
        }

        // Strings
        if let Some(q @ ('"' | '\'')) = ch {
            stream.next();
            s.tokenizer = Tokenizer::Str {
                quote: q,
                greedy: true,
            };
            return Ok(Some("string"));
        }

        if !s.cursor_half {
            // first half i.e. before : for key-value pairs
            // including selectors

            if ch == Some('-') && stream.matches(re!(r"^-[A-Za-z0-9_]+-")) {
                return Ok(Some("meta"));
            }

            if ch == Some('.') {
                stream.next();
                if stream.matches(re!(r"^[A-Za-z0-9_-]+")) {
                    indent(s)?;
                    return Ok(Some("qualifier"));
                } else if stream.peek() == Some('#') {
                    indent(s)?;
                    return Ok(Some("tag"));
                }
            }

            if ch == Some('#') {
                stream.next();
                // ID selectors
                if stream.matches(re!(r"^[A-Za-z0-9_-]+")) {
                    indent(s)?;
                    return Ok(Some("builtin"));
                }
                if stream.peek() == Some('#') {
                    indent(s)?;
                    return Ok(Some("tag"));
                }
            }

            // Variables
            if ch == Some('$') {
                stream.next();
                eat_word(stream);
                return Ok(Some("variable-2"));
            }

            // Numbers
            if stream.matches(re!(r"^-?[0-9\.]+")) {
                return Ok(Some("number"));
            }

            // Units
            if stream.matches(re!(r"^(px|em|in)(?![A-Za-z0-9_])")) {
                return Ok(Some("unit"));
            }

            if keyword(stream) {
                return Ok(Some("keyword"));
            }

            if stream.matches(re!(r"^url")) && stream.peek() == Some('(') {
                s.tokenizer = Tokenizer::Url;
                return Ok(Some("atom"));
            }

            // Match shortcut mixin definition
            if ch == Some('=') && stream.matches(re!(r"^=[A-Za-z0-9_-]+")) {
                indent(s)?;
                return Ok(Some("meta"));
            }

            // Match shortcut mixin definition
            if ch == Some('+') && stream.matches(re!(r"^\+[A-Za-z0-9_-]+")) {
                return Ok(Some("variable-3"));
            }

            if ch == Some('@')
                && stream.match_str("@extend", true, false)
                && !stream.matches(re!(r"\s*[A-Za-z0-9_]"))
            {
                dedent(s);
            }

            // Indent Directives
            if stream.matches(re!(
                r"^@(else if|if|media|else|for|each|while|mixin|function)"
            )) {
                indent(s)?;
                return Ok(Some("def"));
            }

            // Other Directives
            if ch == Some('@') {
                stream.next();
                eat_word(stream);
                return Ok(Some("def"));
            }

            if eat_word(stream) {
                if stream
                    .match_re(re!(r#" *: *[A-Za-z0-9_\-+$#!("']"#), false)
                    .is_some()
                {
                    let word = stream.current().to_lowercase();
                    let prop = format!("{}-{word}", s.prev_prop.as_deref().unwrap_or("undefined"));
                    if has(PROPERTY_KEYWORDS, &prop) {
                        return Ok(Some("property"));
                    } else if has(PROPERTY_KEYWORDS, &word) {
                        s.prev_prop = Some(word);
                        return Ok(Some("property"));
                    } else if has(FONT_PROPERTIES, &word) {
                        return Ok(Some("property"));
                    }
                    return Ok(Some("tag"));
                } else if stream.match_re(re!(r" *:"), false).is_some() {
                    indent(s)?;
                    s.cursor_half = true;
                    s.prev_prop = Some(stream.current().to_lowercase());
                    return Ok(Some("property"));
                } else if stream.match_re(re!(r" *,"), false).is_some() {
                    return Ok(Some("tag"));
                } else {
                    indent(s)?;
                    return Ok(Some("tag"));
                }
            }

            if ch == Some(':') {
                // could be a pseudo-element
                if stream.matches(re!(r"^::?[a-zA-Z_][A-Za-z0-9_\-]*")) {
                    return Ok(Some("variable-3"));
                }
                stream.next();
                s.cursor_half = true;
                return Ok(Some("operator"));
            }
        } else {
            if ch == Some('#') {
                stream.next();
                // Hex numbers
                if stream.matches(re!(r"[0-9a-fA-F]{6}|[0-9a-fA-F]{3}")) {
                    if is_end_line(stream) {
                        s.cursor_half = false;
                    }
                    return Ok(Some("number"));
                }
            }

            // Numbers
            if stream.matches(re!(r"^-?[0-9\.]+")) {
                if is_end_line(stream) {
                    s.cursor_half = false;
                }
                return Ok(Some("number"));
            }

            // Units
            if stream.matches(re!(r"^(px|em|in)(?![A-Za-z0-9_])")) {
                if is_end_line(stream) {
                    s.cursor_half = false;
                }
                return Ok(Some("unit"));
            }

            if keyword(stream) {
                if is_end_line(stream) {
                    s.cursor_half = false;
                }
                return Ok(Some("keyword"));
            }

            if stream.matches(re!(r"^url")) && stream.peek() == Some('(') {
                s.tokenizer = Tokenizer::Url;
                if is_end_line(stream) {
                    s.cursor_half = false;
                }
                return Ok(Some("atom"));
            }

            // Variables
            if ch == Some('$') {
                stream.next();
                eat_word(stream);
                if is_end_line(stream) {
                    s.cursor_half = false;
                }
                return Ok(Some("variable-2"));
            }

            // bang character for !important, !default, etc.
            if ch == Some('!') {
                stream.next();
                s.cursor_half = false;
                return Ok(Some(if stream.matches(re!(r"^[A-Za-z0-9_]+")) {
                    "keyword"
                } else {
                    "operator"
                }));
            }

            if operator(stream) {
                if is_end_line(stream) {
                    s.cursor_half = false;
                }
                return Ok(Some("operator"));
            }

            // attributes
            if eat_word(stream) {
                if is_end_line(stream) {
                    s.cursor_half = false;
                }
                let word = stream.current().to_lowercase();
                if has(VALUE_KEYWORDS, &word) {
                    return Ok(Some("atom"));
                } else if has(COLOR_KEYWORDS, &word) {
                    return Ok(Some("keyword"));
                } else if has(PROPERTY_KEYWORDS, &word) {
                    s.prev_prop = Some(word);
                    return Ok(Some("property"));
                } else {
                    return Ok(Some("tag"));
                }
            }

            if is_end_line(stream) {
                s.cursor_half = false;
                return Ok(None);
            }
        }

        if operator(stream) {
            return Ok(Some("operator"));
        }

        // If we haven't returned by now, we move 1 character
        // and return an error
        stream.next();
        Ok(None)
    }

    /// `tokenLexer`
    fn token_lexer(&self, stream: &mut StringStream, s: &mut SassState) -> Tok {
        if stream.sol() {
            s.indent_count = 0;
        }
        let style = self.call(stream, s)?;
        let current = stream.current();

        if current == "@return" || current == "}" {
            dedent(s);
        }

        if style.is_some() {
            // `startOfToken + config.indentUnit * state.indentCount` is NaN:
            // no scope's offset is `<=` it
            s.scopes = 0;
        }

        Ok(style)
    }
}

impl Mode for Sass {
    fn name(&self) -> &'static str {
        "sass"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(SassState {
            tokenizer: Tokenizer::Base,
            scopes: 1,
            indent_count: 0,
            cursor_half: false,
            prev_prop: None,
            crashed: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<SassState>(st);
        if !s.crashed
            && let Ok(style) = self.token_lexer(stream, s)
        {
            return style.map(str::to_string);
        }
        s.crashed = true;
        stream.skip_to_end();
        None
    }
}
