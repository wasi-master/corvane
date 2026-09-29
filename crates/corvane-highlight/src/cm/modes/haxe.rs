//! `codemirror/mode/haxe/haxe.js` (`text/x-haxe` → the `haxe` mode,
//! `text/x-hxml` → the `hxml` mode).
//!
//! Ported line by line, parser included, since its combinators decide
//! `def`, `property`, `variable-2` and `variable-3`. The closures
//! `expect(wanted)`, `commasep(what, end)` and its `proceed` become [`C`]
//! variants carrying their captured values. Editor-only parts (`indent`,
//! the lexical columns and alignment, `state.indented`) are dropped, but
//! `pushlex` / `poplex` stay on the combinator stack as no-ops so the stack
//! behaves exactly as in the JS.
//!
//! `state.importedtypes` starts as the array of default types; the first
//! `registerimport` turns it into a `{name, next}` list whose `length` is
//! `undefined`, after which `imported` is always false. [`HaxeState`]
//! keeps just that flag.

use crate::cm::{Mode, ModeState, StringStream, state};

/// The JS scratch variables `type` and `content` set by `ret`.
struct Tok {
    kind: &'static str,
    style: Option<&'static str>,
    /// `content`; empty stands for `undefined`
    content: String,
}

fn ret(kind: &'static str, style: Option<&'static str>) -> Tok {
    Tok {
        kind,
        style,
        content: String::new(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    Base,
    /// `haxeTokenString(quote)`
    Str(char),
    /// `haxeTokenComment`
    Comment,
}

/// The `what` of `commasep(what, end)`.
#[derive(Clone, Copy, PartialEq)]
enum What {
    MaybeExpression,
    Expression,
    ObjProp,
    MetaArgs,
    FunArg,
    TypeProp,
}

impl What {
    fn comb(self) -> C {
        match self {
            What::MaybeExpression => C::MaybeExpression,
            What::Expression => C::Expression,
            What::ObjProp => C::ObjProp,
            What::MetaArgs => C::MetaArgs,
            What::FunArg => C::FunArg,
            What::TypeProp => C::TypeProp,
        }
    }
}

/// A combinator on `state.cc`.
#[derive(Clone, Copy, PartialEq)]
enum C {
    Statement,
    Expression,
    MaybeExpression,
    MaybeOperator,
    MaybeAttribute,
    MetaDef,
    MetaArgs,
    ImportDef,
    TypeDef,
    MaybeLabel,
    Property,
    ObjProp,
    Block,
    VarDef1,
    VarDef2,
    ForSpec1,
    ForIn,
    FunctionDef,
    TypeUse,
    TypeString,
    TypeProp,
    FunArg,
    /// `expect(wanted)`
    Expect(&'static str),
    /// `commasep(what, end)`
    CommaSep(What, &'static str),
    /// its inner `proceed`
    Proceed(What, &'static str),
    PushContext,
    /// `.lex`
    PopContext,
    /// `pushlex(...)` (`.lex`, no effect on tokens)
    PushLex,
    /// `.lex`, no effect on tokens
    PopLex,
}

impl C {
    fn is_lex(self) -> bool {
        matches!(self, C::PopContext | C::PushLex | C::PopLex)
    }
}

#[derive(Clone)]
struct HaxeState {
    tokenize: Tokenize,
    re_allowed: bool,
    kw_allowed: bool,
    cc: Vec<C>,
    /// `state.localVars` (`None` = `undefined`), head last
    local_vars: Option<Vec<String>>,
    /// `state.context` chain, innermost last: each saves `vars`
    context: Vec<Option<Vec<String>>>,
    /// `state.importedtypes` is no longer the default-types array
    imports_registered: bool,
}

/// `isOperatorChar = /[+\-*&%=<>!?|]/`
fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '*' | '&' | '%' | '=' | '<' | '>' | '!' | '?' | '|'
    )
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `keywords[word]` as `(type, style)`.
fn keyword(w: &str) -> Option<(&'static str, &'static str)> {
    Some(match w {
        "if" | "while" => ("keyword a", "keyword"),
        "else" | "do" | "try" => ("keyword b", "keyword"),
        "return" | "break" | "continue" | "new" | "throw" => ("keyword c", "keyword"),
        "var" => ("var", "keyword"),
        "inline" | "static" | "public" | "private" => ("attribute", "attribute"),
        "using" | "import" => ("import", "keyword"),
        "cast" => ("cast", "keyword"),
        "macro" => ("macro", "keyword"),
        "function" => ("function", "keyword"),
        "catch" => ("catch", "keyword"),
        "untyped" => ("untyped", "keyword"),
        "callback" => ("cb", "keyword"),
        "for" => ("for", "keyword"),
        "switch" => ("switch", "keyword"),
        "case" => ("case", "keyword"),
        "default" => ("default", "keyword"),
        "in" => ("operator", "keyword"),
        "never" => ("property_access", "keyword"),
        "trace" => ("trace", "keyword"),
        "class" | "abstract" | "enum" | "interface" | "typedef" | "extends" | "implements"
        | "dynamic" => ("typedef", "keyword"),
        "true" | "false" | "null" => ("atom", "atom"),
        _ => return None,
    })
}

/// `toUnescaped(stream, end)`
fn to_unescaped(stream: &mut StringStream, end: char) -> bool {
    let mut escaped = false;
    while let Some(next) = stream.next() {
        if next == end && !escaped {
            return true;
        }
        escaped = !escaped && next == '\\';
    }
    false
}

fn punc(ch: char) -> &'static str {
    match ch {
        '[' => "[",
        ']' => "]",
        '{' => "{",
        '}' => "}",
        '(' => "(",
        ')' => ")",
        ',' => ",",
        ';' => ";",
        ':' => ":",
        _ => ".",
    }
}

/// `haxeTokenBase`
fn token_base(stream: &mut StringStream, s: &mut HaxeState) -> Tok {
    let Some(ch) = stream.next() else {
        return ret("variable", Some("variable"));
    };
    if ch == '"' || ch == '\'' {
        s.tokenize = Tokenize::Str(ch);
        return token_string(ch, stream, s);
    }
    if matches!(
        ch,
        '[' | ']' | '{' | '}' | '(' | ')' | ',' | ';' | ':' | '.'
    ) {
        return ret(punc(ch), None);
    }
    if ch == '0' && stream.eat_if(|c| c == 'x' || c == 'X').is_some() {
        stream.eat_while_if(|c| c.is_ascii_hexdigit());
        return ret("number", Some("number"));
    }
    if ch.is_ascii_digit() || (ch == '-' && stream.eat_if(|c| c.is_ascii_digit()).is_some()) {
        stream.matches(crate::re!(
            r"^[0-9]*(?:\.[0-9]*(?!\.))?(?:[eE][+\-]?[0-9]+)?"
        ));
        return ret("number", Some("number"));
    }
    if s.re_allowed && ch == '~' && stream.eat('/').is_some() {
        to_unescaped(stream, '/');
        stream.eat_while_if(|c| matches!(c, 'g' | 'i' | 'm' | 's' | 'u'));
        return ret("regexp", Some("string-2"));
    }
    if ch == '/' {
        if stream.eat('*').is_some() {
            s.tokenize = Tokenize::Comment;
            return token_comment(stream, s);
        }
        if stream.eat('/').is_some() {
            stream.skip_to_end();
            return ret("comment", Some("comment"));
        }
        stream.eat_while_if(is_operator_char);
        return Tok {
            kind: "operator",
            style: None,
            content: stream.current().to_string(),
        };
    }
    if ch == '#' {
        stream.skip_to_end();
        return ret("conditional", Some("meta"));
    }
    if ch == '@' {
        stream.eat(':');
        stream.eat_while_if(is_word);
        return ret("metadata", Some("meta"));
    }
    if is_operator_char(ch) {
        stream.eat_while_if(is_operator_char);
        return Tok {
            kind: "operator",
            style: None,
            content: stream.current().to_string(),
        };
    }
    if ch.is_ascii_uppercase() {
        stream.eat_while_if(|c| is_word(c) || c == '<' || c == '>');
        return Tok {
            kind: "type",
            style: Some("variable-3"),
            content: stream.current().to_string(),
        };
    }
    stream.eat_while_if(is_word);
    let word = stream.current().to_string();
    match keyword(&word) {
        Some((kind, style)) if s.kw_allowed => Tok {
            kind,
            style: Some(style),
            content: word,
        },
        _ => Tok {
            kind: "variable",
            style: Some("variable"),
            content: word,
        },
    }
}

/// `haxeTokenString(quote)`
fn token_string(quote: char, stream: &mut StringStream, s: &mut HaxeState) -> Tok {
    if to_unescaped(stream, quote) {
        s.tokenize = Tokenize::Base;
    }
    ret("string", Some("string"))
}

/// `haxeTokenComment`
fn token_comment(stream: &mut StringStream, s: &mut HaxeState) -> Tok {
    let mut maybe_end = false;
    while let Some(ch) = stream.next() {
        if ch == '/' && maybe_end {
            s.tokenize = Tokenize::Base;
            break;
        }
        maybe_end = ch == '*';
    }
    ret("comment", Some("comment"))
}

/// `atomicTypes`
fn is_atomic(kind: &str) -> bool {
    matches!(kind, "atom" | "number" | "variable" | "string" | "regexp")
}

fn starts_upper(value: &str) -> bool {
    value.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

/// The JS `cx` object.
struct Cx<'a> {
    st: &'a mut HaxeState,
    marked: Option<&'static str>,
}

impl Cx<'_> {
    /// `pass(...)`
    fn pass(&mut self, cs: &[C]) -> bool {
        self.st.cc.extend(cs.iter().rev());
        false
    }
    /// `cont(...)`
    fn cont(&mut self, cs: &[C]) -> bool {
        self.pass(cs);
        true
    }

    /// `register(varname)` (no `globalVars` in GHD)
    fn register(&mut self, varname: &str) {
        if !self.st.context.is_empty() {
            self.marked = Some("def");
            let vars = self.st.local_vars.get_or_insert_with(Vec::new);
            if vars.iter().any(|v| v == varname) {
                return;
            }
            vars.push(varname.to_string());
        }
    }

    /// `registerimport`
    fn register_import(&mut self) {
        self.st.imports_registered = true;
    }

    fn call(&mut self, c: C, kind: &'static str, value: &str) -> bool {
        use C::*;
        match c {
            // statement
            Statement => match kind {
                "@" => self.cont(&[MetaDef]),
                "var" => self.cont(&[PushLex, VarDef1, Expect(";"), PopLex]),
                "keyword a" => self.cont(&[PushLex, Expression, Statement, PopLex]),
                "keyword b" => self.cont(&[PushLex, Statement, PopLex]),
                "{" => self.cont(&[PushLex, PushContext, Block, PopLex, PopContext]),
                ";" => self.cont(&[]),
                "attribute" => self.cont(&[MaybeAttribute]),
                "function" => self.cont(&[FunctionDef]),
                "for" => self.cont(&[
                    PushLex,
                    Expect("("),
                    PushLex,
                    ForSpec1,
                    Expect(")"),
                    PopLex,
                    Statement,
                    PopLex,
                ]),
                "variable" => self.cont(&[PushLex, MaybeLabel]),
                "switch" => self.cont(&[
                    PushLex,
                    Expression,
                    PushLex,
                    Expect("{"),
                    Block,
                    PopLex,
                    PopLex,
                ]),
                "case" => self.cont(&[Expression, Expect(":")]),
                "default" => self.cont(&[Expect(":")]),
                "catch" => self.cont(&[
                    PushLex,
                    PushContext,
                    Expect("("),
                    FunArg,
                    Expect(")"),
                    Statement,
                    PopLex,
                    PopContext,
                ]),
                "import" => self.cont(&[ImportDef, Expect(";")]),
                "typedef" => self.cont(&[TypeDef]),
                _ => self.pass(&[PushLex, Expression, Expect(";"), PopLex]),
            },
            // expression
            Expression => {
                if is_atomic(kind) || kind == "type" {
                    return self.cont(&[MaybeOperator]);
                }
                match kind {
                    "function" => self.cont(&[FunctionDef]),
                    "keyword c" => self.cont(&[MaybeExpression]),
                    "(" => {
                        self.cont(&[PushLex, MaybeExpression, Expect(")"), PopLex, MaybeOperator])
                    }
                    "operator" => self.cont(&[Expression]),
                    "[" => self.cont(&[
                        PushLex,
                        CommaSep(What::MaybeExpression, "]"),
                        PopLex,
                        MaybeOperator,
                    ]),
                    "{" => {
                        self.cont(&[PushLex, CommaSep(What::ObjProp, "}"), PopLex, MaybeOperator])
                    }
                    _ => self.cont(&[]),
                }
            }
            // maybeexpression: `type.match(/[;\}\)\],]/)`
            MaybeExpression => {
                if kind.contains([';', '}', ')', ']', ',']) {
                    self.pass(&[])
                } else {
                    self.pass(&[Expression])
                }
            }
            MaybeOperator => {
                if kind == "operator" && (value.contains("++") || value.contains("--")) {
                    return self.cont(&[MaybeOperator]);
                }
                match kind {
                    "operator" | ":" => self.cont(&[Expression]),
                    "(" => self.cont(&[
                        PushLex,
                        CommaSep(What::Expression, ")"),
                        PopLex,
                        MaybeOperator,
                    ]),
                    "." => self.cont(&[Property, MaybeOperator]),
                    "[" => self.cont(&[PushLex, Expression, Expect("]"), PopLex, MaybeOperator]),
                    _ => false,
                }
            }
            MaybeAttribute => match kind {
                "attribute" => self.cont(&[MaybeAttribute]),
                "function" => self.cont(&[FunctionDef]),
                "var" => self.cont(&[VarDef1]),
                _ => false,
            },
            MetaDef => match kind {
                ":" | "variable" => self.cont(&[MetaDef]),
                "(" => self.cont(&[PushLex, CommaSep(What::MetaArgs, ")"), PopLex, Statement]),
                _ => false,
            },
            MetaArgs => kind == "variable" && self.cont(&[]),
            ImportDef => {
                if kind == "variable" && starts_upper(value) {
                    self.register_import();
                    self.cont(&[])
                } else if kind == "variable" || kind == "property" || kind == "." || value == "*" {
                    self.cont(&[ImportDef])
                } else {
                    false
                }
            }
            TypeDef => {
                if kind == "variable" && starts_upper(value) {
                    self.register_import();
                    self.cont(&[])
                } else {
                    kind == "type" && starts_upper(value) && self.cont(&[])
                }
            }
            MaybeLabel => {
                if kind == ":" {
                    self.cont(&[PopLex, Statement])
                } else {
                    self.pass(&[MaybeOperator, Expect(";"), PopLex])
                }
            }
            Property => {
                if kind == "variable" {
                    self.marked = Some("property");
                    self.cont(&[])
                } else {
                    false
                }
            }
            ObjProp => {
                if kind == "variable" {
                    self.marked = Some("property");
                }
                is_atomic(kind) && self.cont(&[Expect(":"), Expression])
            }
            CommaSep(what, end) => {
                if kind == end {
                    self.cont(&[])
                } else {
                    self.pass(&[what.comb(), Proceed(what, end)])
                }
            }
            Proceed(what, end) => {
                if kind == "," {
                    self.cont(&[what.comb(), Proceed(what, end)])
                } else if kind == end {
                    self.cont(&[])
                } else {
                    self.cont(&[Expect(end)])
                }
            }
            Block => {
                if kind == "}" {
                    self.cont(&[])
                } else {
                    self.pass(&[Statement, Block])
                }
            }
            VarDef1 => {
                if kind == "variable" {
                    self.register(value);
                    self.cont(&[TypeUse, VarDef2])
                } else {
                    self.cont(&[])
                }
            }
            VarDef2 => {
                if value == "=" {
                    self.cont(&[Expression, VarDef2])
                } else {
                    kind == "," && self.cont(&[VarDef1])
                }
            }
            ForSpec1 => {
                if kind == "variable" {
                    self.register(value);
                    self.cont(&[ForIn, Expression])
                } else {
                    self.pass(&[])
                }
            }
            ForIn => value == "in" && self.cont(&[]),
            FunctionDef => {
                if kind == "variable" || kind == "type" {
                    self.register(value);
                    self.cont(&[FunctionDef])
                } else if value == "new" {
                    self.cont(&[FunctionDef])
                } else if kind == "(" {
                    self.cont(&[
                        PushLex,
                        PushContext,
                        CommaSep(What::FunArg, ")"),
                        PopLex,
                        TypeUse,
                        Statement,
                        PopContext,
                    ])
                } else {
                    false
                }
            }
            TypeUse => kind == ":" && self.cont(&[TypeString]),
            TypeString => match kind {
                "type" | "variable" => self.cont(&[]),
                "{" => self.cont(&[PushLex, CommaSep(What::TypeProp, "}"), PopLex]),
                _ => false,
            },
            TypeProp => kind == "variable" && self.cont(&[TypeUse]),
            FunArg => {
                if kind == "variable" {
                    self.register(value);
                    self.cont(&[TypeUse])
                } else {
                    false
                }
            }
            // expect(wanted)
            Expect(wanted) => {
                if kind == wanted {
                    self.cont(&[])
                } else if wanted == ";" {
                    self.pass(&[])
                } else {
                    self.cont(&[Expect(wanted)])
                }
            }
            // pushcontext
            PushContext => {
                if self.st.context.is_empty() {
                    self.st.local_vars = Some(vec!["this".to_string()]);
                }
                let vars = self.st.local_vars.clone();
                self.st.context.push(vars);
                false
            }
            // popcontext
            PopContext => {
                if let Some(vars) = self.st.context.pop() {
                    self.st.local_vars = vars;
                }
                false
            }
            PushLex | PopLex => false,
        }
    }

    /// `inScope`
    fn in_scope(&self, varname: &str) -> bool {
        self.st
            .local_vars
            .as_ref()
            .is_some_and(|v| v.iter().any(|n| n == varname))
    }

    /// `imported`
    fn imported(&self, typename: &str) -> bool {
        if typename.starts_with(|c: char| c.is_ascii_lowercase()) || self.st.imports_registered {
            return false;
        }
        matches!(
            typename,
            "Int" | "Float" | "String" | "Void" | "Std" | "Bool" | "Dynamic" | "Array"
        )
    }
}

/// `parseHaxe`
fn parse_haxe(s: &mut HaxeState, tok: &Tok) -> Option<&'static str> {
    let mut cx = Cx {
        st: s,
        marked: None,
    };
    loop {
        let combinator = cx.st.cc.pop().unwrap_or(C::Statement);
        if cx.call(combinator, tok.kind, &tok.content) {
            while let Some(&top) = cx.st.cc.last() {
                if !top.is_lex() {
                    break;
                }
                cx.st.cc.pop();
                cx.call(top, tok.kind, &tok.content);
            }
            if let Some(m) = cx.marked {
                return Some(m);
            }
            if tok.kind == "variable" && cx.in_scope(&tok.content) {
                return Some("variable-2");
            }
            if tok.kind == "variable" && cx.imported(&tok.content) {
                return Some("variable-3");
            }
            return tok.style;
        }
    }
}

pub struct Haxe;

impl Mode for Haxe {
    fn name(&self) -> &'static str {
        "haxe"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(HaxeState {
            tokenize: Tokenize::Base,
            re_allowed: true,
            kw_allowed: true,
            cc: Vec::new(),
            local_vars: None,
            context: Vec::new(),
            imports_registered: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<HaxeState>(st);
        if stream.eat_space() {
            return None;
        }
        let tok = match s.tokenize {
            Tokenize::Base => token_base(stream, s),
            Tokenize::Str(q) => token_string(q, stream, s),
            Tokenize::Comment => token_comment(stream, s),
        };
        if tok.kind == "comment" {
            return tok.style.map(str::to_string);
        }
        s.re_allowed = tok.kind == "operator"
            || tok.kind == "keyword c"
            || matches!(tok.kind, "[" | "{" | "}" | "(" | "," | ";" | ":");
        s.kw_allowed = tok.kind != ".";
        parse_haxe(s, &tok).map(str::to_string)
    }
}

#[derive(Clone)]
struct HxmlState {
    define: bool,
    in_string: bool,
}

/// The `hxml` mode.
pub struct Hxml;

impl Mode for Hxml {
    fn name(&self) -> &'static str {
        "hxml"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(HxmlState {
            define: false,
            in_string: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<HxmlState>(st);
        let ch = stream.peek();
        let sol = stream.sol();

        if ch == Some('#') {
            stream.skip_to_end();
            return Some("comment".into());
        }
        if sol && ch == Some('-') {
            let mut style = "variable-2";
            stream.eat('-');
            if stream.peek() == Some('-') {
                stream.eat('-');
                style = "keyword a";
            }
            if stream.peek() == Some('D') {
                stream.eat('D');
                style = "keyword c";
                s.define = true;
            }
            stream.eat_while_if(|c| c.is_ascii_alphabetic());
            return Some(style.into());
        }

        if !s.in_string && ch == Some('\'') {
            s.in_string = true;
            stream.next();
        }
        if s.in_string {
            if !stream.skip_to('\'') {
                stream.skip_to_end();
            }
            if stream.peek() == Some('\'') {
                stream.next();
                s.in_string = false;
            }
            return Some("string".into());
        }

        stream.next();
        None
    }
}
