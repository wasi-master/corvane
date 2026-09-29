//! A syntax check standing in for pug.js's
//! `Function('', 'var x ' + value)`: whether a `var x` declaration followed
//! by an attribute's value text so far parses as a JavaScript function body
//! (sloppy mode). pug uses the answer to tell where one unquoted attribute
//! value ends and the next attribute name starts.
//!
//! Only syntax is checked, like `Function` does before running anything:
//! a small lexer (strings, template literals with `${}` parts, numbers,
//! regexes, punctuators) and a recursive-descent parser for expressions
//! (operators, calls, members, optional chaining, array / object literals,
//! arrow functions, `function` / `class` expressions, `new`). Function and
//! class bodies are only checked for balanced brackets.

#[derive(Clone, Debug, PartialEq)]
enum T {
    Ident(String),
    Num,
    Str,
    Template,
    Regex,
    Punct(&'static str),
}

const PUNCTS: &[&str] = &[
    ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==", "!=",
    "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=",
    "**", "<<", ">>", "{", "}", "(", ")", "[", "]", ";", ",", "<", ">", "+", "-", "*", "/", "%",
    "&", "|", "^", "!", "~", "?", ":", "=", ".",
];

/// Words that can never be an identifier in a sloppy function body.
const RESERVED: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
];

/// Keywords after which a `/` starts a regex.
const REGEX_AFTER: &[&str] = &[
    "return",
    "typeof",
    "instanceof",
    "in",
    "of",
    "new",
    "delete",
    "void",
    "throw",
    "case",
    "do",
    "else",
    "yield",
    "await",
];

fn ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$' || (!c.is_ascii() && c.is_alphabetic())
}

fn ident_part(c: char) -> bool {
    ident_start(c) || c.is_ascii_digit() || (!c.is_ascii() && c.is_alphanumeric())
}

/// JS `WhiteSpace` / `LineTerminator`
fn is_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

struct Lexer<'a> {
    chars: &'a [char],
    i: usize,
}

impl Lexer<'_> {
    fn peek(&self, k: usize) -> Option<char> {
        self.chars.get(self.i + k).copied()
    }

    /// Skip whitespace and comments; `Err` on an unterminated comment.
    fn skip_trivia(&mut self) -> Result<(), ()> {
        loop {
            match (self.peek(0), self.peek(1)) {
                (Some(c), _) if is_space(c) => self.i += 1,
                (Some('/'), Some('/')) => {
                    while self.peek(0).is_some_and(|c| c != '\n') {
                        self.i += 1;
                    }
                }
                (Some('/'), Some('*')) => {
                    self.i += 2;
                    loop {
                        match (self.peek(0), self.peek(1)) {
                            (Some('*'), Some('/')) => {
                                self.i += 2;
                                break;
                            }
                            (Some(_), _) => self.i += 1,
                            (None, _) => return Err(()),
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn string(&mut self, quote: char) -> Result<T, ()> {
        self.i += 1;
        loop {
            match self.peek(0) {
                None | Some('\n' | '\r') => return Err(()),
                Some('\\') => self.i += 2,
                Some(c) => {
                    self.i += 1;
                    if c == quote {
                        return Ok(T::Str);
                    }
                }
            }
        }
    }

    /// A template literal; every `${…}` part must be an expression.
    fn template(&mut self) -> Result<T, ()> {
        self.i += 1;
        loop {
            match self.peek(0) {
                None => return Err(()),
                Some('\\') => self.i += 2,
                Some('`') => {
                    self.i += 1;
                    return Ok(T::Template);
                }
                Some('$') if self.peek(1) == Some('{') => {
                    self.i += 2;
                    let toks = self.tokens_until_brace()?;
                    let mut p = Parser { toks: &toks, i: 0 };
                    p.expression()?;
                    if p.i != toks.len() {
                        return Err(());
                    }
                }
                Some(_) => self.i += 1,
            }
        }
    }

    /// Tokens up to the `}` closing a template's `${`.
    fn tokens_until_brace(&mut self) -> Result<Vec<T>, ()> {
        let mut out = Vec::new();
        let mut depth = 0usize;
        loop {
            self.skip_trivia()?;
            if self.peek(0).is_none() {
                return Err(());
            }
            if self.peek(0) == Some('}') && depth == 0 {
                self.i += 1;
                return Ok(out);
            }
            let t = self.token(out.last())?;
            match t {
                T::Punct("{") => depth += 1,
                T::Punct("}") => depth -= 1,
                _ => {}
            }
            out.push(t);
        }
    }

    fn regex(&mut self) -> Result<T, ()> {
        self.i += 1;
        let mut class = false;
        loop {
            match self.peek(0) {
                None | Some('\n' | '\r') => return Err(()),
                Some('\\') => self.i += 2,
                Some('[') => {
                    class = true;
                    self.i += 1;
                }
                Some(']') => {
                    class = false;
                    self.i += 1;
                }
                Some('/') if !class => {
                    self.i += 1;
                    while self.peek(0).is_some_and(ident_part) {
                        self.i += 1;
                    }
                    return Ok(T::Regex);
                }
                Some(_) => self.i += 1,
            }
        }
    }

    fn number(&mut self) -> Result<T, ()> {
        let c = self.peek(0);
        if c == Some('0') && self.peek(1).is_some_and(|c| "xXoObB".contains(c)) {
            self.i += 2;
            while self
                .peek(0)
                .is_some_and(|c| c.is_ascii_hexdigit() || c == '_')
            {
                self.i += 1;
            }
        } else {
            while self.peek(0).is_some_and(|c| c.is_ascii_digit() || c == '_') {
                self.i += 1;
            }
            if self.peek(0) == Some('.') {
                self.i += 1;
                while self.peek(0).is_some_and(|c| c.is_ascii_digit() || c == '_') {
                    self.i += 1;
                }
            }
            if self.peek(0).is_some_and(|c| c == 'e' || c == 'E') {
                self.i += 1;
                if self.peek(0).is_some_and(|c| c == '+' || c == '-') {
                    self.i += 1;
                }
                if !self.peek(0).is_some_and(|c| c.is_ascii_digit()) {
                    return Err(());
                }
                while self.peek(0).is_some_and(|c| c.is_ascii_digit() || c == '_') {
                    self.i += 1;
                }
            }
        }
        if self.peek(0) == Some('n') {
            self.i += 1;
        }
        // `3in` / `1x` are errors: no identifier right after a number
        if self.peek(0).is_some_and(ident_part) {
            return Err(());
        }
        Ok(T::Num)
    }

    fn token(&mut self, prev: Option<&T>) -> Result<T, ()> {
        let Some(c) = self.peek(0) else {
            return Err(());
        };
        if c == '"' || c == '\'' {
            return self.string(c);
        }
        if c == '`' {
            return self.template();
        }
        if c.is_ascii_digit() || (c == '.' && self.peek(1).is_some_and(|c| c.is_ascii_digit())) {
            return self.number();
        }
        if ident_start(c) || c == '\\' {
            let start = self.i;
            while self.peek(0).is_some_and(|c| ident_part(c) || c == '\\') {
                self.i += 1;
            }
            return Ok(T::Ident(self.chars[start..self.i].iter().collect()));
        }
        if c == '/' {
            let regex_ok = match prev {
                None => true,
                Some(T::Punct(p)) => !matches!(*p, ")" | "]" | "}"),
                Some(T::Ident(w)) => REGEX_AFTER.contains(&w.as_str()),
                _ => false,
            };
            if regex_ok {
                return self.regex();
            }
        }
        for p in PUNCTS {
            let n = p.chars().count();
            if self.chars.len() >= self.i + n
                && p.chars().zip(&self.chars[self.i..]).all(|(a, b)| a == *b)
            {
                // `?.` followed by a digit is `?` then a number
                if *p == "?." && self.peek(2).is_some_and(|c| c.is_ascii_digit()) {
                    continue;
                }
                self.i += n;
                return Ok(T::Punct(p));
            }
        }
        Err(())
    }
}

fn lex(src: &str) -> Result<Vec<T>, ()> {
    let chars: Vec<char> = src.chars().collect();
    let mut lx = Lexer {
        chars: &chars,
        i: 0,
    };
    let mut out = Vec::new();
    loop {
        lx.skip_trivia()?;
        if lx.peek(0).is_none() {
            return Ok(out);
        }
        let t = lx.token(out.last())?;
        out.push(t);
    }
}

type R = Result<(), ()>;

struct Parser<'a> {
    toks: &'a [T],
    i: usize,
}

fn is_punct(t: Option<&T>, p: &str) -> bool {
    matches!(t, Some(T::Punct(q)) if *q == p)
}

fn is_word(t: Option<&T>, w: &str) -> bool {
    matches!(t, Some(T::Ident(q)) if q == w)
}

impl Parser<'_> {
    fn peek(&self) -> Option<&T> {
        self.toks.get(self.i)
    }
    fn peek_at(&self, k: usize) -> Option<&T> {
        self.toks.get(self.i + k)
    }
    fn eat(&mut self, p: &str) -> bool {
        if is_punct(self.peek(), p) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, p: &str) -> R {
        if self.eat(p) { Ok(()) } else { Err(()) }
    }

    /// Index just past the bracket matching the one at `self.i`.
    fn balanced_end(&self, from: usize) -> Result<usize, ()> {
        let mut depth = 0usize;
        let mut i = from;
        while let Some(t) = self.toks.get(i) {
            match t {
                T::Punct("(" | "[" | "{") => depth += 1,
                T::Punct(")" | "]" | "}") => {
                    depth = depth.checked_sub(1).ok_or(())?;
                    if depth == 0 {
                        return Ok(i + 1);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        Err(())
    }

    fn skip_balanced(&mut self, open: &str) -> R {
        if !is_punct(self.peek(), open) {
            return Err(());
        }
        self.i = self.balanced_end(self.i)?;
        Ok(())
    }

    /// `Expression`: assignments separated by commas.
    fn expression(&mut self) -> R {
        self.assign()?;
        while self.eat(",") {
            self.assign()?;
        }
        Ok(())
    }

    /// Arrow function body after `=>`.
    fn arrow_body(&mut self) -> R {
        if is_punct(self.peek(), "{") {
            self.skip_balanced("{")
        } else {
            self.assign()
        }
    }

    fn binding_ident(t: Option<&T>) -> bool {
        matches!(t, Some(T::Ident(w)) if !RESERVED.contains(&w.as_str()))
    }

    /// `AssignmentExpression`
    fn assign(&mut self) -> R {
        // arrow functions: `a =>`, `async a =>`, `(…) =>`, `async (…) =>`
        let mut k = 0;
        if is_word(self.peek(), "async")
            && (Self::binding_ident(self.peek_at(1)) || is_punct(self.peek_at(1), "("))
        {
            k = 1;
        }
        if Self::binding_ident(self.peek_at(k)) && is_punct(self.peek_at(k + 1), "=>") {
            self.i += k + 2;
            return self.arrow_body();
        }
        if is_punct(self.peek_at(k), "(") {
            let end = self.balanced_end(self.i + k)?;
            if is_punct(self.toks.get(end), "=>") {
                self.i = end + 1;
                return self.arrow_body();
            }
        }
        let simple = self.conditional()?;
        if let Some(T::Punct(p)) = self.peek()
            && matches!(
                *p,
                "=" | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "%="
                    | "**="
                    | "<<="
                    | ">>="
                    | ">>>="
                    | "&="
                    | "|="
                    | "^="
                    | "&&="
                    | "||="
                    | "??="
            )
        {
            if !simple {
                return Err(());
            }
            self.i += 1;
            return self.assign();
        }
        Ok(())
    }

    /// `ConditionalExpression`; `Ok(true)` for a simple assignment target.
    fn conditional(&mut self) -> Result<bool, ()> {
        let simple = self.binary()?;
        if self.eat("?") {
            self.assign()?;
            self.expect(":")?;
            self.assign()?;
            return Ok(false);
        }
        Ok(simple)
    }

    fn binary(&mut self) -> Result<bool, ()> {
        let mut simple = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(T::Punct(p)) => matches!(
                    *p,
                    "||" | "&&"
                        | "??"
                        | "|"
                        | "^"
                        | "&"
                        | "=="
                        | "!="
                        | "==="
                        | "!=="
                        | "<"
                        | ">"
                        | "<="
                        | ">="
                        | "<<"
                        | ">>"
                        | ">>>"
                        | "+"
                        | "-"
                        | "*"
                        | "/"
                        | "%"
                        | "**"
                ),
                Some(T::Ident(w)) => w == "instanceof" || w == "in",
                _ => false,
            };
            if !op {
                return Ok(simple);
            }
            self.i += 1;
            self.unary()?;
            simple = false;
        }
    }

    fn unary(&mut self) -> Result<bool, ()> {
        match self.peek() {
            Some(T::Punct("!" | "~" | "+" | "-" | "++" | "--")) => {
                self.i += 1;
                self.unary()?;
                Ok(false)
            }
            Some(T::Ident(w)) if matches!(w.as_str(), "typeof" | "void" | "delete") => {
                self.i += 1;
                self.unary()?;
                Ok(false)
            }
            _ => {
                let simple = self.call_member()?;
                if self.eat("++") || self.eat("--") {
                    return if simple { Ok(false) } else { Err(()) };
                }
                Ok(simple)
            }
        }
    }

    fn arguments(&mut self) -> R {
        self.expect("(")?;
        while !self.eat(")") {
            self.eat("...");
            self.assign()?;
            if !self.eat(",") {
                return self.expect(")");
            }
        }
        Ok(())
    }

    fn property_name(&mut self) -> R {
        match self.peek() {
            Some(T::Ident(_) | T::Str | T::Num) => {
                self.i += 1;
                Ok(())
            }
            Some(T::Punct("[")) => {
                self.i += 1;
                self.assign()?;
                self.expect("]")
            }
            _ => Err(()),
        }
    }

    /// `MemberExpression` / `CallExpression`; `Ok(true)` when the result
    /// can be assigned to.
    fn call_member(&mut self) -> Result<bool, ()> {
        let mut simple = if is_word(self.peek(), "new") {
            self.i += 1;
            if self.eat(".") {
                // new.target is only valid inside functions, as here
                if !is_word(self.peek(), "target") {
                    return Err(());
                }
                self.i += 1;
            } else {
                self.new_target()?;
                if is_punct(self.peek(), "(") {
                    self.arguments()?;
                }
            }
            false
        } else {
            self.primary()?
        };
        loop {
            match self.peek() {
                Some(T::Punct(".")) => {
                    self.i += 1;
                    if !matches!(self.peek(), Some(T::Ident(_))) {
                        return Err(());
                    }
                    self.i += 1;
                    simple = true;
                }
                Some(T::Punct("?.")) => {
                    self.i += 1;
                    match self.peek() {
                        Some(T::Ident(_)) => self.i += 1,
                        Some(T::Punct("[")) => {
                            self.i += 1;
                            self.expression()?;
                            self.expect("]")?;
                        }
                        Some(T::Punct("(")) => self.arguments()?,
                        _ => return Err(()),
                    }
                    simple = false;
                }
                Some(T::Punct("[")) => {
                    self.i += 1;
                    self.expression()?;
                    self.expect("]")?;
                    simple = true;
                }
                Some(T::Punct("(")) => {
                    self.arguments()?;
                    simple = false;
                }
                Some(T::Template) => {
                    self.i += 1;
                    simple = false;
                }
                _ => return Ok(simple),
            }
        }
    }

    /// The callee of `new`: a member expression without calls.
    fn new_target(&mut self) -> R {
        if is_word(self.peek(), "new") {
            self.i += 1;
            self.new_target()?;
            if is_punct(self.peek(), "(") {
                self.arguments()?;
            }
        } else {
            self.primary()?;
        }
        loop {
            if self.eat(".") {
                if !matches!(self.peek(), Some(T::Ident(_))) {
                    return Err(());
                }
                self.i += 1;
            } else if self.eat("[") {
                self.expression()?;
                self.expect("]")?;
            } else if matches!(self.peek(), Some(T::Template)) {
                self.i += 1;
            } else {
                return Ok(());
            }
        }
    }

    /// `PrimaryExpression`; `Ok(true)` for an identifier.
    fn primary(&mut self) -> Result<bool, ()> {
        let Some(t) = self.peek().cloned() else {
            return Err(());
        };
        self.i += 1;
        match t {
            T::Num | T::Str | T::Template | T::Regex => Ok(false),
            T::Ident(w) => match w.as_str() {
                "this" | "null" | "true" | "false" => Ok(false),
                "function" => {
                    self.eat("*");
                    if Self::binding_ident(self.peek()) {
                        self.i += 1;
                    }
                    self.skip_balanced("(")?;
                    self.skip_balanced("{")?;
                    Ok(false)
                }
                "async" if is_word(self.peek(), "function") => {
                    self.primary()?;
                    Ok(false)
                }
                "class" => {
                    if Self::binding_ident(self.peek()) {
                        self.i += 1;
                    }
                    if is_word(self.peek(), "extends") {
                        self.i += 1;
                        self.call_member()?;
                    }
                    self.skip_balanced("{")?;
                    Ok(false)
                }
                w if RESERVED.contains(&w) => Err(()),
                _ => Ok(true),
            },
            T::Punct("(") => {
                self.expression()?;
                self.expect(")")?;
                Ok(false)
            }
            T::Punct("[") => loop {
                if self.eat("]") {
                    return Ok(false);
                }
                if self.eat(",") {
                    continue;
                }
                self.eat("...");
                self.assign()?;
                if !self.eat(",") {
                    self.expect("]")?;
                    return Ok(false);
                }
            },
            T::Punct("{") => loop {
                if self.eat("}") {
                    return Ok(false);
                }
                self.object_property()?;
                if !self.eat(",") {
                    self.expect("}")?;
                    return Ok(false);
                }
            },
            _ => Err(()),
        }
    }

    fn object_property(&mut self) -> R {
        if self.eat("...") {
            return self.assign();
        }
        // `get` / `set` / `async` / `*` prefixes of methods
        let prefixed = matches!(self.peek(), Some(T::Ident(w)) if matches!(w.as_str(), "get" | "set" | "async"))
            && !matches!(self.peek_at(1), Some(T::Punct("," | "}" | ":" | "(")));
        if prefixed {
            self.i += 1;
        }
        let star = self.eat("*");
        let shorthand = Self::binding_ident(self.peek());
        self.property_name()?;
        if is_punct(self.peek(), "(") {
            self.skip_balanced("(")?;
            return self.skip_balanced("{");
        }
        if prefixed || star {
            return Err(());
        }
        if self.eat(":") {
            return self.assign();
        }
        if shorthand && matches!(self.peek(), Some(T::Punct("," | "}"))) {
            return Ok(());
        }
        Err(())
    }
}

/// `Function('', 'var x ' + rest)` would not throw a `SyntaxError`.
pub fn var_x_parses(rest: &str) -> bool {
    let Ok(toks) = lex(rest) else {
        return false;
    };
    let mut p = Parser { toks: &toks, i: 0 };
    let ok = (|| -> R {
        // var x [= init] (, name [= init])* [;]
        loop {
            if p.eat("=") {
                p.assign()?;
            }
            if !p.eat(",") {
                break;
            }
            if !Parser::binding_ident(p.peek()) {
                return Err(());
            }
            p.i += 1;
        }
        p.eat(";");
        if p.i == toks.len() { Ok(()) } else { Err(()) }
    })();
    ok.is_ok()
}

#[cfg(test)]
mod tests {
    use super::var_x_parses;

    #[test]
    fn declarations() {
        for ok in [
            "",
            "=\"a\"",
            "='a' ",
            "=a.b(c, d)[0]",
            "={a: 1, b, [c]: `x${y}z`, m() {}}",
            "=(a) => a + 1",
            "=a ? b : c",
            "=[1, , ...rest]",
            "=new Date().getTime()",
            "=/re/g.test(s)",
            "=a, b = 2",
            "=typeof a === 'string' && !b",
            "=x;",
        ] {
            assert!(var_x_parses(ok), "{ok}");
        }
        for bad in [
            "=", "=\"a", "=a b", "={a: 1", "=`x${`", "=class", "=1 = 2", "=a +", "\"a\"", "=f(",
        ] {
            assert!(!var_x_parses(bad), "{bad}");
        }
    }
}
