//! `codemirror/mode/crystal/crystal.js` (`text/x-crystal`).
//!
//! Ported line by line. `state.tokenize` is a stack of closures in JS; here
//! it is a stack of [`Tok`] values carrying the closures' captured
//! variables. `currentIndent`, `blocks` and `lastStyle` only feed `indent`
//! and are dropped; `lastToken` is only ever compared with `"\\"` and `"."`,
//! so [`CrystalState`] keeps just that.
//!
//! `wordRegExp(words)` is `/^(?:w1|w2…)\b/` tested on the whole matched
//! identifier: with ASCII `\b` it holds exactly when the identifier's
//! leading `[A-Za-z0-9_]` run is one of the words (`end?`, `ifé` count).
//! The identifier and type classes (`[a-z_\u009F-￿]…`) are scanned
//! per UTF-16 unit, like the JS regexes, and quote delimiters are compared
//! as code units, so astral chars split exactly where JS splits them.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

/// `keywords`
fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "abstract"
            | "alias"
            | "as"
            | "asm"
            | "begin"
            | "break"
            | "case"
            | "class"
            | "def"
            | "do"
            | "else"
            | "elsif"
            | "end"
            | "ensure"
            | "enum"
            | "extend"
            | "for"
            | "fun"
            | "if"
            | "include"
            | "instance_sizeof"
            | "lib"
            | "macro"
            | "module"
            | "next"
            | "of"
            | "out"
            | "pointerof"
            | "private"
            | "protected"
            | "rescue"
            | "return"
            | "require"
            | "select"
            | "sizeof"
            | "struct"
            | "super"
            | "then"
            | "type"
            | "typeof"
            | "uninitialized"
            | "union"
            | "unless"
            | "until"
            | "when"
            | "while"
            | "with"
            | "yield"
            | "__DIR__"
            | "__END_LINE__"
            | "__FILE__"
            | "__LINE__"
    )
}

/// `atomWords`
fn is_atom_word(w: &str) -> bool {
    matches!(w, "true" | "false" | "nil" | "self")
}

/// The captured variables of a `state.tokenize` closure.
#[derive(Clone, PartialEq)]
enum Tok {
    Base,
    /// `tokenNest(begin, end, style, started)`
    Nest {
        begin: &'static str,
        end: &'static str,
        style: Option<&'static str>,
        started: bool,
    },
    /// `tokenMacro(begin, end, started)`: `%`/`%` or `{`/`}`
    Macro {
        curly: bool,
        started: bool,
    },
    /// `tokenMacroDef`
    MacroDef,
    /// `tokenFollowIdent`
    FollowIdent,
    /// `tokenFollowType`
    FollowType,
    /// `tokenQuote(end, style, embed)`; `end` is a UTF-16 unit, `None` for
    /// `undefined` (a `%r` at the end of a line)
    Quote {
        end: Option<u16>,
        style: &'static str,
        embed: bool,
    },
    /// `tokenHereDoc(phrase, embed)`
    HereDoc {
        phrase: String,
        embed: bool,
    },
}

/// `state.lastToken` as far as the mode looks at it.
#[derive(Clone, Copy, PartialEq)]
enum LastToken {
    Other,
    Backslash,
    Dot,
}

#[derive(Clone)]
struct CrystalState {
    tokenize: Vec<Tok>,
    last_token: LastToken,
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_lowercase() || c == '_' || c >= '\u{9f}'
}
fn is_type_start(c: char) -> bool {
    c.is_ascii_uppercase() || c == '_' || c >= '\u{9f}'
}
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c >= '\u{9f}'
}
fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// JS `\s`.
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

/// `stream.match(idents)`
fn match_idents(stream: &mut StringStream) -> bool {
    if stream.eat_if(is_ident_start).is_some() {
        stream.eat_while_if(is_ident_char);
        true
    } else {
        false
    }
}

/// `stream.match(types)`
fn match_types(stream: &mut StringStream) -> bool {
    if stream.eat_if(is_type_start).is_some() {
        stream.eat_while_if(is_ident_char);
        true
    } else {
        false
    }
}

/// `stream.match(operators)`
fn match_operators(stream: &mut StringStream) -> bool {
    stream.matches(re!(r"^(?:[-+/%|&^]|\*\*?|[<>]{2})"))
}

/// `stream.match(conditionalOperators)`
fn match_conditional(stream: &mut StringStream) -> bool {
    stream.matches(re!(r"^(?:[=!]~|===|<=>|[<>=!]=?|[|&]{2}|~)"))
}

/// `stream.match(indexingOperators)`
fn match_indexing(stream: &mut StringStream) -> bool {
    stream.matches(re!(r"^(?:\[\][?=]?)"))
}

/// `stream.next()` as a UTF-16 unit.
fn next_unit(stream: &mut StringStream) -> Option<u16> {
    let u = stream.char_code_at(stream.pos)?;
    stream.pos += 1;
    Some(u)
}

/// `matching[delim]`
fn matching(u: u16) -> u16 {
    match u {
        0x5b => 0x5d, // [ ]
        0x7b => 0x7d, // { }
        0x28 => 0x29, // ( )
        0x3c => 0x3e, // < >
        _ => u,
    }
}

/// The leading `[A-Za-z0-9_]` run of the current token.
fn lead<'a>(stream: &'a StringStream) -> &'a str {
    let cur = stream.current();
    let n = cur.find(|c: char| !is_word(c)).unwrap_or(cur.len());
    &cur[..n]
}

impl CrystalState {
    /// `chain(tokenize, stream, state)`
    fn chain(&mut self, t: Tok, stream: &mut StringStream) -> Option<&'static str> {
        self.tokenize.push(t);
        self.call_top(stream)
    }

    fn call_top(&mut self, stream: &mut StringStream) -> Option<&'static str> {
        match self.tokenize.last().cloned().unwrap_or(Tok::Base) {
            Tok::Base => self.token_base(stream),
            Tok::Nest {
                begin,
                end,
                style,
                started,
            } => self.token_nest(stream, begin, end, style, started),
            Tok::Macro { curly, started } => self.token_macro(stream, curly, started),
            Tok::MacroDef => self.token_macro_def(stream),
            Tok::FollowIdent => self.token_follow_ident(stream),
            Tok::FollowType => self.token_follow_type(stream),
            Tok::Quote { end, style, embed } => self.token_quote(stream, end, style, embed),
            Tok::HereDoc { phrase, embed } => self.token_here_doc(stream, &phrase, embed),
        }
    }

    fn set_top(&mut self, t: Tok) {
        if let Some(top) = self.tokenize.last_mut() {
            *top = t;
        }
    }

    /// `tokenBase`
    fn token_base(&mut self, stream: &mut StringStream) -> Option<&'static str> {
        if stream.eat_space() {
            return None;
        }

        // Macros
        if self.last_token != LastToken::Backslash && stream.match_str("{%", false, false) {
            return self.chain(
                Tok::Macro {
                    curly: false,
                    started: false,
                },
                stream,
            );
        }
        if self.last_token != LastToken::Backslash && stream.match_str("{{", false, false) {
            return self.chain(
                Tok::Macro {
                    curly: true,
                    started: false,
                },
                stream,
            );
        }

        // Comments
        if stream.peek() == Some('#') {
            stream.skip_to_end();
            return Some("comment");
        }

        // Variables and keywords
        if match_idents(stream) {
            stream.eat_if(|c| c == '?' || c == '!');
            if stream.eat(':').is_some() {
                return Some("atom");
            } else if self.last_token == LastToken::Dot {
                return Some("property");
            } else if is_keyword(lead(stream)) {
                // nextTokenizer
                let next = match stream.current() {
                    "def" | "fun" => Some(Tok::FollowIdent),
                    "macro" => Some(Tok::MacroDef),
                    "class" | "module" | "struct" | "lib" | "enum" | "union" => {
                        Some(Tok::FollowType)
                    }
                    _ => None,
                };
                if let Some(next) = next {
                    self.tokenize.push(next);
                }
                return Some("keyword");
            } else if is_atom_word(lead(stream)) {
                return Some("atom");
            }
            return Some("variable");
        }

        // Class variables and instance variables
        // or attributes
        if stream.eat('@').is_some() {
            if stream.peek() == Some('[') {
                return self.chain(
                    Tok::Nest {
                        begin: "[",
                        end: "]",
                        style: Some("meta"),
                        started: false,
                    },
                    stream,
                );
            }
            stream.eat('@');
            let _ = match_idents(stream) || match_types(stream);
            return Some("variable-2");
        }

        // Constants and types
        if match_types(stream) {
            return Some("tag");
        }

        // Symbols or ':' operator
        if stream.eat(':').is_some() {
            if stream.eat('"').is_some() {
                return self.chain(
                    Tok::Quote {
                        end: Some(u16::from(b'"')),
                        style: "atom",
                        embed: false,
                    },
                    stream,
                );
            } else if match_idents(stream)
                || match_types(stream)
                || match_operators(stream)
                || match_conditional(stream)
                || match_indexing(stream)
            {
                return Some("atom");
            }
            stream.eat(':');
            return Some("operator");
        }

        // Strings
        if stream.eat('"').is_some() {
            return self.chain(
                Tok::Quote {
                    end: Some(u16::from(b'"')),
                    style: "string",
                    embed: true,
                },
                stream,
            );
        }

        // Strings or regexps or macro variables or '%' operator
        if stream.peek() == Some('%') {
            let mut style = "string";
            let mut embed = true;
            let delim;
            if stream.match_str("%r", true, false) {
                // Regexps
                style = "string-2";
                delim = next_unit(stream);
            } else if stream.match_str("%w", true, false) || stream.match_str("%q", true, false) {
                embed = false;
                delim = next_unit(stream);
            } else if stream
                .char_at(stream.pos + 1)
                .is_some_and(|c| !is_word(c) && !is_js_space(c) && c != '=')
            {
                // /^%([^\w\s=])/ takes one UTF-16 unit
                stream.pos += 1;
                delim = next_unit(stream);
            } else if stream.matches(re!(
                r"^%[a-zA-Z_\u{9F}-\u{10FFFF}][A-Za-z0-9_\u{9F}-\u{10FFFF}]*"
            )) {
                // Macro variables
                return Some("meta");
            } else if stream.eat('%').is_some() {
                // '%' operator
                return Some("operator");
            } else {
                delim = None;
            }
            return self.chain(
                Tok::Quote {
                    end: delim.map(matching),
                    style,
                    embed,
                },
                stream,
            );
        }

        // Here Docs
        if let Some(m) = stream.match_re(re!(r"^<<-('?)([A-Z][A-Za-z0-9_]*)\1"), true) {
            let embed = m.group(1).is_none_or(str::is_empty);
            let phrase = m.group(2).unwrap_or("").to_string();
            return self.chain(Tok::HereDoc { phrase, embed }, stream);
        }

        // Characters
        if stream.eat('\'').is_some() {
            // /^(?:[^']|\\(?:…))/: the first branch takes any one unit but `'`
            if !stream.eol() && stream.peek() != Some('\'') {
                stream.pos += 1;
            }
            stream.eat('\'');
            return Some("atom");
        }

        // Numbers
        if stream.eat('0').is_some() {
            if stream.eat('x').is_some() {
                stream.matches(re!(r"^[0-9a-fA-F_]+"));
            } else if stream.eat('o').is_some() {
                stream.matches(re!(r"^[0-7_]+"));
            } else if stream.eat('b').is_some() {
                stream.matches(re!(r"^[01_]+"));
            }
            return Some("number");
        }

        if stream.eat_if(|c| c.is_ascii_digit()).is_some() {
            stream.matches(re!(r"^[0-9_]*(?:\.[0-9_]+)?(?:[eE][+-]?[0-9]+)?"));
            return Some("number");
        }

        // Operators
        if match_operators(stream) {
            stream.eat('='); // Operators can follow assign symbol.
            return Some("operator");
        }

        if match_conditional(stream) || stream.matches(re!(r"^(?:\.(?:\.{2})?|->|[?:])")) {
            return Some("operator");
        }

        // Parens and braces
        if let Some(c) = stream.peek()
            && matches!(c, '(' | '{' | '[')
        {
            let (begin, end) = match c {
                '(' => ("(", ")"),
                '{' => ("{", "}"),
                _ => ("[", "]"),
            };
            return self.chain(
                Tok::Nest {
                    begin,
                    end,
                    style: None,
                    started: false,
                },
                stream,
            );
        }

        // Escapes
        if stream.eat('\\').is_some() {
            stream.next();
            return Some("meta");
        }

        stream.next();
        None
    }

    /// `tokenNest(begin, end, style, started)`
    fn token_nest(
        &mut self,
        stream: &mut StringStream,
        begin: &'static str,
        end: &'static str,
        style: Option<&'static str>,
        started: bool,
    ) -> Option<&'static str> {
        if !started && stream.match_str(begin, true, false) {
            self.set_top(Tok::Nest {
                begin,
                end,
                style,
                started: true,
            });
            return style;
        }

        let mut next_style = self.token_base(stream);
        if stream.current() == end {
            self.tokenize.pop();
            next_style = style;
        }
        next_style
    }

    /// `tokenMacro(begin, end, started)`
    fn token_macro(
        &mut self,
        stream: &mut StringStream,
        curly: bool,
        started: bool,
    ) -> Option<&'static str> {
        let (open, close) = if curly { ("{{", "}}") } else { ("{%", "%}") };
        if !started && stream.match_str(open, true, false) {
            self.set_top(Tok::Macro {
                curly,
                started: true,
            });
            return Some("meta");
        }

        if stream.match_str(close, true, false) {
            self.tokenize.pop();
            return Some("meta");
        }

        self.token_base(stream)
    }

    /// `tokenMacroDef`
    fn token_macro_def(&mut self, stream: &mut StringStream) -> Option<&'static str> {
        if stream.eat_space() {
            return None;
        }

        if match_idents(stream) {
            if stream.current() == "def" {
                return Some("keyword");
            }
            stream.eat_if(|c| c == '?' || c == '!');
        }

        self.tokenize.pop();
        Some("def")
    }

    /// `tokenFollowIdent`
    fn token_follow_ident(&mut self, stream: &mut StringStream) -> Option<&'static str> {
        if stream.eat_space() {
            return None;
        }

        if match_idents(stream) {
            stream.eat_if(|c| c == '!' || c == '?');
        } else {
            let _ = match_operators(stream) || match_conditional(stream) || match_indexing(stream);
        }
        self.tokenize.pop();
        Some("def")
    }

    /// `tokenFollowType`
    fn token_follow_type(&mut self, stream: &mut StringStream) -> Option<&'static str> {
        if stream.eat_space() {
            return None;
        }

        match_types(stream);
        self.tokenize.pop();
        Some("def")
    }

    /// `tokenQuote(end, style, embed)`
    fn token_quote(
        &mut self,
        stream: &mut StringStream,
        end: Option<u16>,
        style: &'static str,
        embed: bool,
    ) -> Option<&'static str> {
        let mut escaped = false;

        while !stream.eol() {
            if !escaped {
                if stream.match_str("{%", false, false) {
                    self.tokenize.push(Tok::Macro {
                        curly: false,
                        started: false,
                    });
                    return Some(style);
                }

                if stream.match_str("{{", false, false) {
                    self.tokenize.push(Tok::Macro {
                        curly: true,
                        started: false,
                    });
                    return Some(style);
                }

                if embed && stream.match_str("#{", false, false) {
                    self.tokenize.push(Tok::Nest {
                        begin: "#{",
                        end: "}",
                        style: Some("meta"),
                        started: false,
                    });
                    return Some(style);
                }

                let ch = next_unit(stream);

                if ch.is_some() && ch == end {
                    self.tokenize.pop();
                    return Some(style);
                }

                escaped = embed && ch == Some(u16::from(b'\\'));
            } else {
                stream.next();
                escaped = false;
            }
        }

        Some(style)
    }

    /// `tokenHereDoc(phrase, embed)`
    fn token_here_doc(
        &mut self,
        stream: &mut StringStream,
        phrase: &str,
        embed: bool,
    ) -> Option<&'static str> {
        if stream.sol() {
            stream.eat_space();
            if stream.match_str(phrase, true, false) {
                self.tokenize.pop();
                return Some("string");
            }
        }

        let mut escaped = false;
        while !stream.eol() {
            if !escaped {
                if stream.match_str("{%", false, false) {
                    self.tokenize.push(Tok::Macro {
                        curly: false,
                        started: false,
                    });
                    return Some("string");
                }

                if stream.match_str("{{", false, false) {
                    self.tokenize.push(Tok::Macro {
                        curly: true,
                        started: false,
                    });
                    return Some("string");
                }

                if embed && stream.match_str("#{", false, false) {
                    self.tokenize.push(Tok::Nest {
                        begin: "#{",
                        end: "}",
                        style: Some("meta"),
                        started: false,
                    });
                    return Some("string");
                }

                escaped = stream.next() == Some('\\') && embed;
            } else {
                stream.next();
                escaped = false;
            }
        }

        Some("string")
    }
}

pub struct Crystal;

impl Mode for Crystal {
    fn name(&self) -> &'static str {
        "crystal"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(CrystalState {
            tokenize: vec![Tok::Base],
            last_token: LastToken::Other,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<CrystalState>(st);
        let style = s.call_top(stream);
        if let Some(style) = style
            && style != "comment"
        {
            s.last_token = match stream.current() {
                "\\" => LastToken::Backslash,
                "." => LastToken::Dot,
                _ => LastToken::Other,
            };
        }
        style.map(str::to_string)
    }
}
