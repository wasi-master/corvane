//! `codemirror/mode/javascript/javascript.js`: the JavaScript / TypeScript /
//! JSON / JSON-LD tokenizer and its combinator parser (the parser decides
//! `def`, `property`, `variable-2`, `type` and contextual keywords).
//!
//! Ported function by function; the JS names are kept in comments. The JS
//! parser's continuation stack holds closures; here they are the [`C`]
//! enum. Editor-only parts (`indent`, lexical columns and alignment) are
//! left out since they never change a token.
//!
//! JS quirks kept on purpose, because GHD shows them:
//! - `type` / `content` are scratch variables shared by every call of one
//!   mode instance, so a char `tokenBase` does not recognise (`\`, a lone
//!   `#`) reuses the previous token's type ([`Scratch`]).
//! - `findFatArrow` clears `fatArrowAt` only when it is truthy, so a `0`
//!   survives to later lines, and its TypeScript return-type skip uses the
//!   match index relative to the token start.
//! - `maybeoperatorNoComma` called from the stack gets `noComma ==
//!   undefined`, so `=>` there takes the comma-allowing `arrowBody`.

use std::borrow::Cow;

use crate::re;

use super::super::{Mode, ModeState, StringStream, state};

/// `parserConfig` options that change tokens.
#[derive(Clone, Copy, Debug, Default)]
pub struct JsConfig {
    pub json: bool,
    pub jsonld: bool,
    pub typescript: bool,
    /// `trackScope` (default true): locals get `variable-2`.
    pub no_track_scope: bool,
}

pub struct JsMode {
    json: bool,
    jsonld: bool,
    ts: bool,
    track_scope: bool,
}

/// `state.tokenize`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tokenize {
    Base,
    /// `tokenString(quote)`
    String(char),
    Comment,
    Quasi,
}

/// The parser's continuations (`cc` entries): the named combinators, plus
/// the closures `expect(wanted)`, `pushlex(type, info)`, `commasep(what,
/// end, sep)` (and its `proceed` and inner function) and `maybeTarget`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum C {
    Statement,
    MaybeCatchBinding,
    Expression,
    ExpressionNoComma,
    ParenExpr,
    MaybeExpression,
    MaybeOperatorComma,
    MaybeOperatorNoComma,
    Quasi,
    ContinueQuasi,
    ArrowBody,
    ArrowBodyNoComma,
    MaybeTarget(bool),
    Target,
    TargetNoComma,
    MaybeLabel,
    Property,
    ObjProp,
    GetterSetter,
    AfterProp,
    Commasep {
        what: &'static C,
        end: &'static str,
        sep: Option<&'static str>,
    },
    Proceed {
        what: &'static C,
        end: &'static str,
        sep: Option<&'static str>,
    },
    /// the anonymous function `proceed` continues with after a separator
    CommasepNext {
        what: &'static C,
        end: &'static str,
    },
    Block,
    MaybeType,
    MaybeTypeOrIn,
    MaybeRetType,
    IsKw,
    TypeExpr,
    MaybeReturnType,
    TypeProps,
    TypeProp,
    QuasiType,
    ContinueQuasiType,
    TypeArg,
    AfterType,
    MaybeTypeArgs,
    TypeParam,
    MaybeTypeDefault,
    VarDef,
    Pattern,
    PropPattern,
    EltPattern,
    MaybeAssign,
    VarDefCont,
    MaybeElse,
    ForSpec,
    ForSpec1,
    ForSpec2,
    FunctionDef,
    FunctionDecl,
    TypeName,
    FunArg,
    ClassExpression,
    ClassName,
    ClassNameAfter,
    ClassBody,
    ClassField,
    AfterExport,
    ExportField,
    AfterImport,
    ImportSpec,
    MaybeMoreImports,
    MaybeAs,
    MaybeFrom,
    ArrayLiteral,
    EnumDef,
    EnumMember,
    Expect(&'static str),
    // the `.lex` combinators
    PushLex(&'static str, Option<&'static str>),
    PopLex,
    PushContext,
    PushBlockContext,
    PopContext,
}

impl C {
    fn is_lex(self) -> bool {
        matches!(
            self,
            C::PushLex(..) | C::PopLex | C::PushContext | C::PushBlockContext | C::PopContext
        )
    }
}

fn commasep(what: &'static C, end: &'static str) -> C {
    C::Commasep {
        what,
        end,
        sep: None,
    }
}

/// `JSLexical` (only the fields that affect tokens).
#[derive(Clone, Debug)]
struct Lexical {
    kind: &'static str,
    info: Option<&'static str>,
}

/// `Context`: a scope's variables (`Var` list) and whether it is a block.
#[derive(Clone, Debug)]
struct Scope {
    vars: Vec<Cow<'static, str>>,
    block: bool,
}

/// The JS module-level scratch variables `type` and `content`.
#[derive(Clone, Debug, Default)]
pub struct Scratch {
    kind: &'static str,
    /// `content`; empty stands for `undefined`
    content: String,
}

#[derive(Clone, Debug)]
pub struct JsState {
    tokenize: Tokenize,
    last_type: &'static str,
    cc: Vec<C>,
    /// `state.lexical` and its `prev` chain, innermost last
    lexical: Vec<Lexical>,
    local_vars: Vec<Cow<'static, str>>,
    /// `state.context` and its `prev` chain, innermost last
    context: Vec<Scope>,
    fat_arrow_at: Option<isize>,
    pub(crate) scratch: Scratch,
}

fn default_vars() -> Vec<Cow<'static, str>> {
    vec![Cow::Borrowed("this"), Cow::Borrowed("arguments")]
}

/// `wordRE`: `/[\w$\xa1-\uffff]/` (astral chars are surrogates in JS, which
/// fall in the range too).
fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c >= '\u{a1}'
}

/// `isOperatorChar`
fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '*' | '&' | '%' | '=' | '<' | '>' | '!' | '?' | '|' | '~' | '^' | '@'
    )
}

/// JS `\s`
fn is_js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// `keywords[word]` → (type, style)
fn keyword(word: &str) -> Option<(&'static str, &'static str)> {
    Some(match word {
        "if" => ("if", "keyword"),
        "while" | "with" => ("keyword a", "keyword"),
        "else" | "do" | "try" | "finally" => ("keyword b", "keyword"),
        "return" | "break" | "continue" => ("keyword d", "keyword"),
        "new" => ("new", "keyword"),
        "delete" | "void" | "throw" | "yield" | "extends" | "await" => ("keyword c", "keyword"),
        "debugger" => ("debugger", "keyword"),
        "var" | "const" | "let" => ("var", "keyword"),
        "function" => ("function", "keyword"),
        "catch" => ("catch", "keyword"),
        "for" => ("for", "keyword"),
        "switch" => ("switch", "keyword"),
        "case" => ("case", "keyword"),
        "default" => ("default", "keyword"),
        "in" | "typeof" | "instanceof" => ("operator", "keyword"),
        "true" | "false" | "null" | "undefined" | "NaN" | "Infinity" => ("atom", "atom"),
        "this" => ("this", "keyword"),
        "class" => ("class", "keyword"),
        "super" => ("atom", "keyword"),
        "export" => ("export", "keyword"),
        "import" => ("import", "keyword"),
        _ => return None,
    })
}

/// A char as a one-char punctuation type (`ret(ch)`).
fn punct(c: char) -> Option<&'static str> {
    Some(match c {
        '[' => "[",
        ']' => "]",
        '{' => "{",
        '}' => "}",
        '(' => "(",
        ')' => ")",
        ',' => ",",
        ';' => ";",
        ':' => ":",
        '.' => ".",
        _ => return None,
    })
}

/// `isModifier`
fn is_modifier(name: &str) -> bool {
    matches!(
        name,
        "public" | "private" | "protected" | "abstract" | "readonly"
    )
}

/// `atomicTypes`
fn is_atomic(t: &str) -> bool {
    matches!(
        t,
        "atom" | "number" | "variable" | "string" | "regexp" | "this" | "import" | "jsonld-keyword"
    )
}

impl JsMode {
    pub fn new(config: JsConfig) -> Self {
        Self {
            json: config.json || config.jsonld,
            jsonld: config.jsonld,
            ts: config.typescript,
            track_scope: !config.no_track_scope,
        }
    }

    pub fn start_js(&self) -> JsState {
        JsState {
            tokenize: Tokenize::Base,
            last_type: "sof",
            cc: Vec::new(),
            lexical: vec![Lexical {
                kind: "block",
                info: None,
            }],
            local_vars: Vec::new(),
            context: Vec::new(),
            fat_arrow_at: None,
            scratch: Scratch::default(),
        }
    }

    /// `ret(tp, style, cont)`
    fn ret(
        s: &mut JsState,
        tp: &'static str,
        style: Option<&'static str>,
        content: Option<&str>,
    ) -> Option<&'static str> {
        s.scratch.kind = tp;
        s.scratch.content.clear();
        if let Some(c) = content {
            s.scratch.content.push_str(c);
        }
        style
    }

    /// `readRegexp`
    fn read_regexp(stream: &mut StringStream) {
        let mut escaped = false;
        let mut in_set = false;
        while let Some(next) = stream.next() {
            if !escaped {
                if next == '/' && !in_set {
                    return;
                }
                if next == '[' {
                    in_set = true;
                } else if in_set && next == ']' {
                    in_set = false;
                }
            }
            escaped = !escaped && next == '\\';
        }
    }

    /// `stream.match(/^\b(([gimyus])(?![gimyus]*\2))+\b/)`: a run of
    /// distinct flag letters not followed by a word char.
    fn match_regexp_flags(stream: &mut StringStream) {
        let is_flag = |c: char| matches!(c, 'g' | 'i' | 'm' | 'y' | 'u' | 's');
        let start = stream.pos;
        let mut end = start;
        while stream.char_at(end).is_some_and(is_flag) {
            end += 1;
        }
        if end == start {
            return;
        }
        let run = stream.slice(start, end);
        let distinct = run
            .char_indices()
            .all(|(i, c)| !run[i + c.len_utf8()..].contains(c));
        let boundary = !stream
            .char_at(end)
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        if distinct && boundary {
            stream.pos = end;
        }
    }

    /// `tokenBase`
    fn token_base(&self, stream: &mut StringStream, s: &mut JsState) -> Option<&'static str> {
        // at the end of the line (a nesting mode calling in after skipping
        // the rest) JS tests `undefined` as the string "undefined": only
        // the word test matches, and the word is the current token text
        let ch = stream.next().unwrap_or('u');
        if ch == '"' || ch == '\'' {
            s.tokenize = Tokenize::String(ch);
            return self.token_string(ch, stream, s);
        } else if ch == '.'
            && stream
                .match_re(re!(r"^[0-9][0-9_]*(?:[eE][+\-]?[0-9_]+)?"), true)
                .is_some()
        {
            return Self::ret(s, "number", Some("number"), None);
        } else if ch == '.' && stream.match_str("..", true, false) {
            return Self::ret(s, "spread", Some("meta"), None);
        } else if let Some(p) = punct(ch) {
            return Self::ret(s, p, None, None);
        } else if ch == '=' && stream.eat('>').is_some() {
            return Self::ret(s, "=>", Some("operator"), None);
        } else if ch == '0'
            && stream
                .match_re(re!(r"^(?:x[0-9A-Fa-f_]+|o[0-7_]+|b[01_]+)n?"), true)
                .is_some()
        {
            return Self::ret(s, "number", Some("number"), None);
        } else if ch.is_ascii_digit() {
            stream.match_re(
                re!(r"^[0-9_]*(?:n|(?:\.[0-9_]*)?(?:[eE][+\-]?[0-9_]+)?)?"),
                true,
            );
            return Self::ret(s, "number", Some("number"), None);
        } else if ch == '/' {
            if stream.eat('*').is_some() {
                s.tokenize = Tokenize::Comment;
                return Self::token_comment(stream, s);
            } else if stream.eat('/').is_some() {
                stream.skip_to_end();
                return Self::ret(s, "comment", Some("comment"), None);
            } else if Self::expression_allowed_at(stream, s, 1) {
                Self::read_regexp(stream);
                Self::match_regexp_flags(stream);
                return Self::ret(s, "regexp", Some("string-2"), None);
            } else {
                stream.eat('=');
                let cur = stream.current();
                s.scratch.kind = "operator";
                s.scratch.content.clear();
                s.scratch.content.push_str(cur);
                return Some("operator");
            }
        } else if ch == '`' {
            s.tokenize = Tokenize::Quasi;
            return Self::token_quasi(stream, s);
        } else if ch == '#' && stream.peek() == Some('!') {
            stream.skip_to_end();
            return Self::ret(s, "meta", Some("meta"), None);
        } else if ch == '#' && stream.eat_while_if(is_word) {
            return Self::ret(s, "variable", Some("property"), None);
        } else if (ch == '<' && stream.match_str("!--", true, false))
            || (ch == '-'
                && stream.match_str("->", true, false)
                && stream.slice(0, stream.start).chars().all(is_js_space))
        {
            stream.skip_to_end();
            return Self::ret(s, "comment", Some("comment"), None);
        } else if is_operator_char(ch) {
            if ch != '>' || s.lexical.last().is_none_or(|l| l.kind != ">") {
                if stream.eat('=').is_some() {
                    if ch == '!' || ch == '=' {
                        stream.eat('=');
                    }
                } else if matches!(ch, '<' | '>' | '*' | '+' | '-' | '|' | '&' | '?') {
                    stream.eat(ch);
                    if ch == '>' {
                        stream.eat(ch);
                    }
                }
            }
            if ch == '?' && stream.eat('.').is_some() {
                return Self::ret(s, ".", None, None);
            }
            let cur = stream.current();
            s.scratch.kind = "operator";
            s.scratch.content.clear();
            s.scratch.content.push_str(cur);
            return Some("operator");
        } else if is_word(ch) {
            stream.eat_while_if(is_word);
            let word = stream.current();
            if s.last_type != "." {
                if let Some((tp, style)) = keyword(word) {
                    s.scratch.kind = tp;
                    s.scratch.content.clear();
                    s.scratch.content.push_str(word);
                    return Some(style);
                }
                if word == "async"
                    && stream
                        .match_re(
                            re!(r"^(\s|\/\*([^*]|\*(?!\/))*?\*\/)*[\[\(A-Za-z0-9_]"),
                            false,
                        )
                        .is_some()
                {
                    s.scratch.kind = "async";
                    s.scratch.content.clear();
                    s.scratch.content.push_str("async");
                    return Some("keyword");
                }
            }
            let word = stream.current();
            s.scratch.kind = "variable";
            s.scratch.content.clear();
            s.scratch.content.push_str(word);
            return Some("variable");
        }
        // falls off the end: `type` and `content` keep their old values
        None
    }

    /// `tokenString(quote)`
    fn token_string(
        &self,
        quote: char,
        stream: &mut StringStream,
        s: &mut JsState,
    ) -> Option<&'static str> {
        let mut escaped = false;
        if self.jsonld
            && stream.peek() == Some('@')
            && stream
                .match_re(
                    re!(
                        r#"^@(context|id|value|language|type|container|list|set|reverse|index|base|vocab|graph)""#
                    ),
                    true,
                )
                .is_some()
        {
            s.tokenize = Tokenize::Base;
            return Self::ret(s, "jsonld-keyword", Some("meta"), None);
        }
        while let Some(next) = stream.next() {
            if next == quote && !escaped {
                break;
            }
            escaped = !escaped && next == '\\';
        }
        if !escaped {
            s.tokenize = Tokenize::Base;
        }
        Self::ret(s, "string", Some("string"), None)
    }

    /// `tokenComment`
    fn token_comment(stream: &mut StringStream, s: &mut JsState) -> Option<&'static str> {
        let mut maybe_end = false;
        while let Some(ch) = stream.next() {
            if ch == '/' && maybe_end {
                s.tokenize = Tokenize::Base;
                break;
            }
            maybe_end = ch == '*';
        }
        Self::ret(s, "comment", Some("comment"), None)
    }

    /// `tokenQuasi`
    fn token_quasi(stream: &mut StringStream, s: &mut JsState) -> Option<&'static str> {
        let mut escaped = false;
        while let Some(next) = stream.next() {
            if !escaped && (next == '`' || next == '$' && stream.eat('{').is_some()) {
                s.tokenize = Tokenize::Base;
                break;
            }
            escaped = !escaped && next == '\\';
        }
        let cur = stream.current();
        s.scratch.kind = "quasi";
        s.scratch.content.clear();
        s.scratch.content.push_str(cur);
        Some("string-2")
    }

    /// `findFatArrow`
    fn find_fat_arrow(&self, stream: &StringStream, s: &mut JsState) {
        if matches!(s.fat_arrow_at, Some(x) if x != 0) {
            s.fat_arrow_at = None;
        }
        let rest = stream.slice(stream.start, stream.len());
        let Some(b) = rest.find("=>") else {
            return;
        };
        let mut arrow = (stream.start + rest[..b].chars().count()) as isize;

        if self.ts {
            // Try to skip TypeScript return type declarations after the
            // arguments (the JS uses the index within the slice as is)
            let head = &rest[..b];
            if let Ok(Some(m)) =
                re!(r"(?::\s*(?:[A-Za-z0-9_]+(?:<[^>]*>|\[\])?|\{[^}]*\})\s*$)").find(head)
            {
                arrow = head[..m.start()].chars().count() as isize;
            }
        }

        let at = |i: isize| -> Option<char> {
            if i < 0 {
                None
            } else {
                stream.char_at(i as usize)
            }
        };
        const BRACKETS: &str = "([{}])";
        let mut depth = 0;
        let mut saw_something = false;
        let mut pos = arrow - 1;
        while pos >= 0 {
            let Some(ch) = at(pos) else {
                pos -= 1;
                continue;
            };
            let bracket = BRACKETS.find(ch);
            match bracket {
                Some(b) if b < 3 => {
                    if depth == 0 {
                        pos += 1;
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        if ch == '(' {
                            saw_something = true;
                        }
                        break;
                    }
                }
                Some(_) => depth += 1,
                None if is_word(ch) => saw_something = true,
                None if matches!(ch, '"' | '\'' | '/' | '`') => loop {
                    if pos == 0 {
                        return;
                    }
                    let next = at(pos - 1);
                    if next == Some(ch) && at(pos - 2) != Some('\\') {
                        pos -= 1;
                        break;
                    }
                    pos -= 1;
                },
                None if saw_something && depth == 0 => {
                    pos += 1;
                    break;
                }
                None => {}
            }
            pos -= 1;
        }
        if saw_something && depth == 0 {
            s.fat_arrow_at = Some(pos);
        }
    }

    /// `expressionAllowed(stream, state, backUp)`
    fn expression_allowed_at(stream: &StringStream, s: &JsState, back_up: usize) -> bool {
        (s.tokenize == Tokenize::Base
            && matches!(
                s.last_type,
                "operator"
                    | "sof"
                    | "keyword b"
                    | "keyword c"
                    | "keyword d"
                    | "case"
                    | "new"
                    | "export"
                    | "default"
                    | "spread"
                    | "["
                    | "{"
                    | "}"
                    | "("
                    | ","
                    | ";"
                    | ":"
                    | "=>"
            ))
            || (s.last_type == "quasi"
                && stream
                    .slice(0, stream.pos.saturating_sub(back_up))
                    .trim_end_matches(is_js_space)
                    .ends_with('{'))
    }

    /// `mode.expressionAllowed(stream, state)` (for jsx)
    pub fn expression_allowed(&self, stream: &StringStream, s: &JsState) -> bool {
        Self::expression_allowed_at(stream, s, 0)
    }

    /// `mode.skipExpression(state)`
    pub fn skip_expression(&self, s: &mut JsState) {
        let lines: &[&str] = &[];
        let mut stream = StringStream::new("", 2, lines, 0);
        self.parse_js(s, Some("atom"), "atom", "true", &mut stream);
    }

    /// `mode.token`
    pub fn token_js(&self, stream: &mut StringStream, s: &mut JsState) -> Option<&'static str> {
        if stream.sol() {
            self.find_fat_arrow(stream, s);
        }
        if s.tokenize != Tokenize::Comment && stream.eat_space() {
            return None;
        }
        let style = match s.tokenize {
            Tokenize::Base => self.token_base(stream, s),
            Tokenize::String(q) => self.token_string(q, stream, s),
            Tokenize::Comment => Self::token_comment(stream, s),
            Tokenize::Quasi => Self::token_quasi(stream, s),
        };
        let kind = s.scratch.kind;
        if kind == "comment" {
            return style;
        }
        let content = std::mem::take(&mut s.scratch.content);
        s.last_type = if kind == "operator" && (content == "++" || content == "--") {
            "incdec"
        } else {
            kind
        };
        let style = self.parse_js(s, style, kind, &content, stream);
        s.scratch.content = content;
        style
    }

    /// `parseJS`
    fn parse_js(
        &self,
        s: &mut JsState,
        style: Option<&'static str>,
        kind: &'static str,
        content: &str,
        stream: &mut StringStream,
    ) -> Option<&'static str> {
        let mut cx = Cx {
            mode: self,
            st: s,
            stream,
            marked: None,
            style,
        };
        loop {
            let combinator = cx.st.cc.pop().unwrap_or(if self.json {
                C::Expression
            } else {
                C::Statement
            });
            if cx.call(combinator, kind, content) {
                while let Some(&top) = cx.st.cc.last() {
                    if !top.is_lex() {
                        break;
                    }
                    cx.st.cc.pop();
                    cx.call(top, kind, content);
                }
                if let Some(m) = cx.marked {
                    return Some(m);
                }
                if kind == "variable" && cx.in_scope(content) {
                    return Some("variable-2");
                }
                return style;
            }
        }
    }
}

/// The JS `cx` object: the parser's view of the current token.
struct Cx<'a, 's> {
    mode: &'a JsMode,
    st: &'a mut JsState,
    stream: &'a mut StringStream<'s>,
    marked: Option<&'static str>,
    style: Option<&'static str>,
}

impl Cx<'_, '_> {
    /// `pass(...)`: push in reverse so the first runs first
    fn pass(&mut self, cs: &[C]) -> bool {
        self.st.cc.extend(cs.iter().rev());
        false
    }
    /// `cont(...)`
    fn cont(&mut self, cs: &[C]) -> bool {
        self.pass(cs);
        true
    }
    /// `contCommasep(what, end, info, ...rest)`
    fn cont_commasep(
        &mut self,
        what: &'static C,
        end: &'static str,
        info: Option<&'static str>,
        rest: &[C],
    ) -> bool {
        self.st.cc.extend(rest.iter());
        self.cont(&[C::PushLex(end, info), commasep(what, end), C::PopLex])
    }
    fn look(&mut self, re: &fancy_regex::Regex) -> bool {
        self.stream.match_re(re, false).is_some()
    }
    fn ts(&self) -> bool {
        self.mode.ts
    }

    /// `inScope`
    fn in_scope(&self, varname: &str) -> bool {
        if !self.mode.track_scope {
            return false;
        }
        self.st.local_vars.iter().any(|v| v == varname)
            || self
                .st
                .context
                .iter()
                .any(|cx| cx.vars.iter().any(|v| v == varname))
    }

    /// `register`
    fn register(&mut self, varname: &str) {
        self.marked = Some("def");
        if !self.mode.track_scope {
            return;
        }
        let st = &mut *self.st;
        let Some(top) = st.context.last() else {
            // no context: global, and no `globalVars` are configured
            return;
        };
        let info = st.lexical.last().and_then(|l| l.info);
        if info == Some("var") && top.block {
            // registerVarScoped: the innermost non-block scope (none: the
            // JS falls through to the global case, a no-op here)
            if let Some(scope) = st.context.iter_mut().rev().find(|c| !c.block)
                && !scope.vars.iter().any(|v| v == varname)
            {
                scope.vars.push(Cow::Owned(varname.to_string()));
            }
        } else if !st.local_vars.iter().any(|v| v == varname) {
            st.local_vars.push(Cow::Owned(varname.to_string()));
        }
    }

    fn call(&mut self, c: C, t: &'static str, v: &str) -> bool {
        match c {
            C::Statement => self.statement(t, v),
            C::MaybeCatchBinding => t == "(" && self.cont(&[C::FunArg, C::Expect(")")]),
            C::Expression => self.expression_inner(t, v, false),
            C::ExpressionNoComma => self.expression_inner(t, v, true),
            C::ParenExpr => {
                if t != "(" {
                    return self.pass(&[]);
                }
                self.cont(&[
                    C::PushLex(")", None),
                    C::MaybeExpression,
                    C::Expect(")"),
                    C::PopLex,
                ])
            }
            C::MaybeExpression => {
                if t.contains([';', '}', ')', ']', ',']) {
                    return self.pass(&[]);
                }
                self.pass(&[C::Expression])
            }
            C::MaybeOperatorComma => {
                if t == "," {
                    return self.cont(&[C::MaybeExpression]);
                }
                self.maybe_operator_no_comma(t, v, Some(false))
            }
            C::MaybeOperatorNoComma => self.maybe_operator_no_comma(t, v, None),
            C::Quasi => {
                if t != "quasi" {
                    return self.pass(&[]);
                }
                if !v.ends_with("${") {
                    return self.cont(&[C::Quasi]);
                }
                self.cont(&[C::MaybeExpression, C::ContinueQuasi])
            }
            C::ContinueQuasi => {
                if t == "}" {
                    self.marked = Some("string-2");
                    self.st.tokenize = Tokenize::Quasi;
                    return self.cont(&[C::Quasi]);
                }
                false
            }
            C::ArrowBody | C::ArrowBodyNoComma => {
                self.mode.find_fat_arrow(self.stream, self.st);
                let next = if t == "{" {
                    C::Statement
                } else if c == C::ArrowBody {
                    C::Expression
                } else {
                    C::ExpressionNoComma
                };
                self.pass(&[next])
            }
            C::MaybeTarget(no_comma) => {
                if t == "." {
                    self.cont(&[if no_comma {
                        C::TargetNoComma
                    } else {
                        C::Target
                    }])
                } else if t == "variable" && self.ts() {
                    self.cont(&[
                        C::MaybeTypeArgs,
                        if no_comma {
                            C::MaybeOperatorNoComma
                        } else {
                            C::MaybeOperatorComma
                        },
                    ])
                } else {
                    self.pass(&[if no_comma {
                        C::ExpressionNoComma
                    } else {
                        C::Expression
                    }])
                }
            }
            C::Target | C::TargetNoComma => {
                if v == "target" {
                    self.marked = Some("keyword");
                    return self.cont(&[if c == C::Target {
                        C::MaybeOperatorComma
                    } else {
                        C::MaybeOperatorNoComma
                    }]);
                }
                false
            }
            C::MaybeLabel => {
                if t == ":" {
                    return self.cont(&[C::PopLex, C::Statement]);
                }
                self.pass(&[C::MaybeOperatorComma, C::Expect(";"), C::PopLex])
            }
            C::Property => {
                if t == "variable" {
                    self.marked = Some("property");
                    return self.cont(&[]);
                }
                false
            }
            C::ObjProp => self.objprop(t, v),
            C::GetterSetter => {
                if t != "variable" {
                    return self.pass(&[C::AfterProp]);
                }
                self.marked = Some("property");
                self.cont(&[C::FunctionDef])
            }
            C::AfterProp => {
                if t == ":" {
                    return self.cont(&[C::ExpressionNoComma]);
                }
                if t == "(" {
                    return self.pass(&[C::FunctionDef]);
                }
                false
            }
            C::Commasep { what, end, sep } => {
                if t == end || v == end {
                    return self.cont(&[]);
                }
                self.pass(&[*what, C::Proceed { what, end, sep }])
            }
            C::Proceed { what, end, sep } => {
                // `sep.indexOf(type) > -1` or `type == ","`
                if sep.map_or(t == ",", |sep| !t.is_empty() && sep.contains(t)) {
                    return self
                        .cont(&[C::CommasepNext { what, end }, C::Proceed { what, end, sep }]);
                }
                if t == end || v == end {
                    return self.cont(&[]);
                }
                if sep.is_some_and(|sep| sep.contains(';')) {
                    return self.pass(&[*what]);
                }
                self.cont(&[C::Expect(end)])
            }
            C::CommasepNext { what, end } => {
                if t == end || v == end {
                    return self.pass(&[]);
                }
                self.pass(&[*what])
            }
            C::Block => {
                if t == "}" {
                    return self.cont(&[]);
                }
                self.pass(&[C::Statement, C::Block])
            }
            C::MaybeType => {
                if self.ts() {
                    if t == ":" {
                        return self.cont(&[C::TypeExpr]);
                    }
                    if v == "?" {
                        return self.cont(&[C::MaybeType]);
                    }
                }
                false
            }
            C::MaybeTypeOrIn => {
                if self.ts() && (t == ":" || v == "in") {
                    return self.cont(&[C::TypeExpr]);
                }
                false
            }
            C::MaybeRetType => {
                if self.ts() && t == ":" {
                    if self.look(re!(r"^\s*[A-Za-z0-9_]+\s+is(?![A-Za-z0-9_])")) {
                        return self.cont(&[C::Expression, C::IsKw, C::TypeExpr]);
                    }
                    return self.cont(&[C::TypeExpr]);
                }
                false
            }
            C::IsKw => {
                if v == "is" {
                    self.marked = Some("keyword");
                    return self.cont(&[]);
                }
                false
            }
            C::TypeExpr => self.typeexpr(t, v),
            C::MaybeReturnType => t == "=>" && self.cont(&[C::TypeExpr]),
            C::TypeProps => {
                if t.contains(['}', ')', ']']) {
                    return self.cont(&[]);
                }
                if t == "," || t == ";" {
                    return self.cont(&[C::TypeProps]);
                }
                self.pass(&[C::TypeProp, C::TypeProps])
            }
            C::TypeProp => self.typeprop(t, v),
            C::QuasiType => {
                if t != "quasi" {
                    return self.pass(&[]);
                }
                if !v.ends_with("${") {
                    return self.cont(&[C::QuasiType]);
                }
                self.cont(&[C::TypeExpr, C::ContinueQuasiType])
            }
            C::ContinueQuasiType => {
                if t == "}" {
                    self.marked = Some("string-2");
                    self.st.tokenize = Tokenize::Quasi;
                    return self.cont(&[C::QuasiType]);
                }
                false
            }
            C::TypeArg => {
                if (t == "variable" && self.look(re!(r"^\s*[?:]"))) || v == "?" {
                    return self.cont(&[C::TypeArg]);
                }
                if t == ":" {
                    return self.cont(&[C::TypeExpr]);
                }
                if t == "spread" {
                    return self.cont(&[C::TypeArg]);
                }
                self.pass(&[C::TypeExpr])
            }
            C::AfterType => self.after_type(t, v),
            C::MaybeTypeArgs => {
                if v == "<" {
                    return self.cont(&[
                        C::PushLex(">", None),
                        commasep(&C::TypeExpr, ">"),
                        C::PopLex,
                        C::AfterType,
                    ]);
                }
                false
            }
            C::TypeParam => self.pass(&[C::TypeExpr, C::MaybeTypeDefault]),
            C::MaybeTypeDefault => v == "=" && self.cont(&[C::TypeExpr]),
            C::VarDef => {
                if v == "enum" {
                    self.marked = Some("keyword");
                    return self.cont(&[C::EnumDef]);
                }
                self.pass(&[C::Pattern, C::MaybeType, C::MaybeAssign, C::VarDefCont])
            }
            C::Pattern => self.pattern(t, v),
            C::PropPattern => self.proppattern(t, v),
            C::EltPattern => self.pass(&[C::Pattern, C::MaybeAssign]),
            C::MaybeAssign => v == "=" && self.cont(&[C::ExpressionNoComma]),
            C::VarDefCont => t == "," && self.cont(&[C::VarDef]),
            C::MaybeElse => {
                if t == "keyword b" && v == "else" {
                    return self.cont(&[C::PushLex("form", Some("else")), C::Statement, C::PopLex]);
                }
                false
            }
            C::ForSpec => {
                if v == "await" {
                    return self.cont(&[C::ForSpec]);
                }
                if t == "(" {
                    return self.cont(&[C::PushLex(")", None), C::ForSpec1, C::PopLex]);
                }
                false
            }
            C::ForSpec1 => {
                if t == "var" {
                    return self.cont(&[C::VarDef, C::ForSpec2]);
                }
                if t == "variable" {
                    return self.cont(&[C::ForSpec2]);
                }
                self.pass(&[C::ForSpec2])
            }
            C::ForSpec2 => {
                if t == ")" {
                    return self.cont(&[]);
                }
                if t == ";" {
                    return self.cont(&[C::ForSpec2]);
                }
                if v == "in" || v == "of" {
                    self.marked = Some("keyword");
                    return self.cont(&[C::Expression, C::ForSpec2]);
                }
                self.pass(&[C::Expression, C::ForSpec2])
            }
            C::FunctionDef | C::FunctionDecl => {
                if v == "*" {
                    self.marked = Some("keyword");
                    return self.cont(&[c]);
                }
                if t == "variable" {
                    self.register(v);
                    return self.cont(&[c]);
                }
                if t == "(" {
                    if c == C::FunctionDef {
                        return self.cont(&[
                            C::PushContext,
                            C::PushLex(")", None),
                            commasep(&C::FunArg, ")"),
                            C::PopLex,
                            C::MaybeRetType,
                            C::Statement,
                            C::PopContext,
                        ]);
                    }
                    return self.cont(&[
                        C::PushContext,
                        C::PushLex(")", None),
                        commasep(&C::FunArg, ")"),
                        C::PopLex,
                        C::MaybeRetType,
                        C::PopContext,
                    ]);
                }
                if self.ts() && v == "<" {
                    return self.cont(&[
                        C::PushLex(">", None),
                        commasep(&C::TypeParam, ">"),
                        C::PopLex,
                        c,
                    ]);
                }
                false
            }
            C::TypeName => {
                if t == "keyword" || t == "variable" {
                    self.marked = Some("type");
                    return self.cont(&[C::TypeName]);
                }
                if v == "<" {
                    return self.cont(&[
                        C::PushLex(">", None),
                        commasep(&C::TypeParam, ">"),
                        C::PopLex,
                    ]);
                }
                false
            }
            C::FunArg => {
                if v == "@" {
                    // `cont(expression, funarg)` without returning
                    self.cont(&[C::Expression, C::FunArg]);
                }
                if t == "spread" {
                    return self.cont(&[C::FunArg]);
                }
                if self.ts() && is_modifier(v) {
                    self.marked = Some("keyword");
                    return self.cont(&[C::FunArg]);
                }
                if self.ts() && t == "this" {
                    return self.cont(&[C::MaybeType, C::MaybeAssign]);
                }
                self.pass(&[C::Pattern, C::MaybeType, C::MaybeAssign])
            }
            C::ClassExpression => {
                if t == "variable" {
                    return self.call(C::ClassName, t, v);
                }
                self.call(C::ClassNameAfter, t, v)
            }
            C::ClassName => {
                if t == "variable" {
                    self.register(v);
                    return self.cont(&[C::ClassNameAfter]);
                }
                false
            }
            C::ClassNameAfter => {
                if v == "<" {
                    return self.cont(&[
                        C::PushLex(">", None),
                        commasep(&C::TypeParam, ">"),
                        C::PopLex,
                        C::ClassNameAfter,
                    ]);
                }
                if v == "extends" || v == "implements" || (self.ts() && t == ",") {
                    if v == "implements" {
                        self.marked = Some("keyword");
                    }
                    let next = if self.ts() {
                        C::TypeExpr
                    } else {
                        C::Expression
                    };
                    return self.cont(&[next, C::ClassNameAfter]);
                }
                if t == "{" {
                    return self.cont(&[C::PushLex("}", None), C::ClassBody, C::PopLex]);
                }
                false
            }
            C::ClassBody => self.class_body(t, v),
            C::ClassField => {
                if v == "!" || v == "?" {
                    return self.cont(&[C::ClassField]);
                }
                if t == ":" {
                    return self.cont(&[C::TypeExpr, C::MaybeAssign]);
                }
                if v == "=" {
                    return self.cont(&[C::ExpressionNoComma]);
                }
                let n = self.st.lexical.len();
                let is_interface = n >= 2 && self.st.lexical[n - 2].info == Some("interface");
                self.pass(&[if is_interface {
                    C::FunctionDecl
                } else {
                    C::FunctionDef
                }])
            }
            C::AfterExport => {
                if v == "*" {
                    self.marked = Some("keyword");
                    return self.cont(&[C::MaybeFrom, C::Expect(";")]);
                }
                if v == "default" {
                    self.marked = Some("keyword");
                    return self.cont(&[C::Expression, C::Expect(";")]);
                }
                if t == "{" {
                    return self.cont(&[
                        commasep(&C::ExportField, "}"),
                        C::MaybeFrom,
                        C::Expect(";"),
                    ]);
                }
                self.pass(&[C::Statement])
            }
            C::ExportField => {
                if v == "as" {
                    self.marked = Some("keyword");
                    return self.cont(&[C::Expect("variable")]);
                }
                if t == "variable" {
                    return self.pass(&[C::ExpressionNoComma, C::ExportField]);
                }
                false
            }
            C::AfterImport => {
                if t == "string" {
                    return self.cont(&[]);
                }
                if t == "(" {
                    return self.pass(&[C::Expression]);
                }
                if t == "." {
                    return self.pass(&[C::MaybeOperatorComma]);
                }
                self.pass(&[C::ImportSpec, C::MaybeMoreImports, C::MaybeFrom])
            }
            C::ImportSpec => {
                if t == "{" {
                    return self.cont_commasep(&C::ImportSpec, "}", None, &[]);
                }
                if t == "variable" {
                    self.register(v);
                }
                if v == "*" {
                    self.marked = Some("keyword");
                }
                self.cont(&[C::MaybeAs])
            }
            C::MaybeMoreImports => t == "," && self.cont(&[C::ImportSpec, C::MaybeMoreImports]),
            C::MaybeAs => {
                if v == "as" {
                    self.marked = Some("keyword");
                    return self.cont(&[C::ImportSpec]);
                }
                false
            }
            C::MaybeFrom => {
                if v == "from" {
                    self.marked = Some("keyword");
                    return self.cont(&[C::Expression]);
                }
                false
            }
            C::ArrayLiteral => {
                if t == "]" {
                    return self.cont(&[]);
                }
                self.pass(&[commasep(&C::ExpressionNoComma, "]")])
            }
            C::EnumDef => self.pass(&[
                C::PushLex("form", None),
                C::Pattern,
                C::Expect("{"),
                C::PushLex("}", None),
                commasep(&C::EnumMember, "}"),
                C::PopLex,
                C::PopLex,
            ]),
            C::EnumMember => self.pass(&[C::Pattern, C::MaybeAssign]),
            C::Expect(wanted) => {
                if t == wanted {
                    return self.cont(&[]);
                }
                if wanted == ";" || t == "}" || t == ")" || t == "]" {
                    return self.pass(&[]);
                }
                self.cont(&[C::Expect(wanted)])
            }
            // the `.lex` combinators return undefined
            C::PushLex(kind, info) => {
                self.st.lexical.push(Lexical { kind, info });
                false
            }
            C::PopLex => {
                if self.st.lexical.len() > 1 {
                    self.st.lexical.pop();
                }
                false
            }
            C::PushContext => {
                let vars = std::mem::replace(&mut self.st.local_vars, default_vars());
                self.st.context.push(Scope { vars, block: false });
                false
            }
            C::PushBlockContext => {
                let vars = std::mem::take(&mut self.st.local_vars);
                self.st.context.push(Scope { vars, block: true });
                false
            }
            C::PopContext => {
                if let Some(scope) = self.st.context.pop() {
                    self.st.local_vars = scope.vars;
                }
                false
            }
        }
    }

    /// `statement`
    fn statement(&mut self, t: &'static str, v: &str) -> bool {
        match t {
            "var" => {
                let info = match v {
                    "var" => "var",
                    "let" => "let",
                    _ => "const",
                };
                return self.cont(&[
                    C::PushLex("vardef", Some(info)),
                    C::VarDef,
                    C::Expect(";"),
                    C::PopLex,
                ]);
            }
            "keyword a" => {
                return self.cont(&[
                    C::PushLex("form", None),
                    C::ParenExpr,
                    C::Statement,
                    C::PopLex,
                ]);
            }
            "keyword b" => {
                return self.cont(&[C::PushLex("form", None), C::Statement, C::PopLex]);
            }
            "keyword d" => {
                if self.look(re!(r"^\s*$")) {
                    return self.cont(&[]);
                }
                return self.cont(&[
                    C::PushLex("stat", None),
                    C::MaybeExpression,
                    C::Expect(";"),
                    C::PopLex,
                ]);
            }
            "debugger" => return self.cont(&[C::Expect(";")]),
            "{" => {
                return self.cont(&[
                    C::PushLex("}", None),
                    C::PushBlockContext,
                    C::Block,
                    C::PopLex,
                    C::PopContext,
                ]);
            }
            ";" => return self.cont(&[]),
            "if" => {
                if self.st.lexical.last().and_then(|l| l.info) == Some("else")
                    && self.st.cc.last() == Some(&C::PopLex)
                {
                    self.st.cc.pop();
                    self.call(C::PopLex, t, v);
                }
                return self.cont(&[
                    C::PushLex("form", None),
                    C::ParenExpr,
                    C::Statement,
                    C::PopLex,
                    C::MaybeElse,
                ]);
            }
            "function" => return self.cont(&[C::FunctionDef]),
            "for" => {
                return self.cont(&[
                    C::PushLex("form", None),
                    C::PushBlockContext,
                    C::ForSpec,
                    C::Statement,
                    C::PopContext,
                    C::PopLex,
                ]);
            }
            _ => {}
        }
        if t == "class" || (self.ts() && v == "interface") {
            self.marked = Some("keyword");
            let info = if t == "class" { "class" } else { "interface" };
            return self.cont(&[C::PushLex("form", Some(info)), C::ClassName, C::PopLex]);
        }
        if t == "variable" {
            if self.ts() && v == "declare" {
                self.marked = Some("keyword");
                return self.cont(&[C::Statement]);
            } else if self.ts()
                && (v == "module" || v == "enum" || v == "type")
                && self.look(re!(r"^\s*[A-Za-z0-9_]"))
            {
                self.marked = Some("keyword");
                if v == "enum" {
                    return self.cont(&[C::EnumDef]);
                } else if v == "type" {
                    return self.cont(&[
                        C::TypeName,
                        C::Expect("operator"),
                        C::TypeExpr,
                        C::Expect(";"),
                    ]);
                } else {
                    return self.cont(&[
                        C::PushLex("form", None),
                        C::Pattern,
                        C::Expect("{"),
                        C::PushLex("}", None),
                        C::Block,
                        C::PopLex,
                        C::PopLex,
                    ]);
                }
            } else if self.ts() && v == "namespace" {
                self.marked = Some("keyword");
                return self.cont(&[
                    C::PushLex("form", None),
                    C::Expression,
                    C::Statement,
                    C::PopLex,
                ]);
            } else if self.ts() && v == "abstract" {
                self.marked = Some("keyword");
                return self.cont(&[C::Statement]);
            } else {
                return self.cont(&[C::PushLex("stat", None), C::MaybeLabel]);
            }
        }
        match t {
            "switch" => {
                return self.cont(&[
                    C::PushLex("form", None),
                    C::ParenExpr,
                    C::Expect("{"),
                    C::PushLex("}", Some("switch")),
                    C::PushBlockContext,
                    C::Block,
                    C::PopLex,
                    C::PopLex,
                    C::PopContext,
                ]);
            }
            "case" => return self.cont(&[C::Expression, C::Expect(":")]),
            "default" => return self.cont(&[C::Expect(":")]),
            "catch" => {
                return self.cont(&[
                    C::PushLex("form", None),
                    C::PushContext,
                    C::MaybeCatchBinding,
                    C::Statement,
                    C::PopLex,
                    C::PopContext,
                ]);
            }
            "export" => return self.cont(&[C::PushLex("stat", None), C::AfterExport, C::PopLex]),
            "import" => return self.cont(&[C::PushLex("stat", None), C::AfterImport, C::PopLex]),
            "async" => return self.cont(&[C::Statement]),
            _ => {}
        }
        if v == "@" {
            return self.cont(&[C::Expression, C::Statement]);
        }
        self.pass(&[
            C::PushLex("stat", None),
            C::Expression,
            C::Expect(";"),
            C::PopLex,
        ])
    }

    /// `expressionInner`
    fn expression_inner(&mut self, t: &'static str, v: &str, no_comma: bool) -> bool {
        if self.st.fat_arrow_at == Some(self.stream.start as isize) {
            let body = if no_comma {
                C::ArrowBodyNoComma
            } else {
                C::ArrowBody
            };
            if t == "(" {
                return self.cont(&[
                    C::PushContext,
                    C::PushLex(")", None),
                    commasep(&C::FunArg, ")"),
                    C::PopLex,
                    C::Expect("=>"),
                    body,
                    C::PopContext,
                ]);
            } else if t == "variable" {
                return self.pass(&[
                    C::PushContext,
                    C::Pattern,
                    C::Expect("=>"),
                    body,
                    C::PopContext,
                ]);
            }
        }

        let maybeop = if no_comma {
            C::MaybeOperatorNoComma
        } else {
            C::MaybeOperatorComma
        };
        let expr = if no_comma {
            C::ExpressionNoComma
        } else {
            C::Expression
        };
        if is_atomic(t) {
            return self.cont(&[maybeop]);
        }
        if t == "function" {
            return self.cont(&[C::FunctionDef, maybeop]);
        }
        if t == "class" || (self.ts() && v == "interface") {
            self.marked = Some("keyword");
            return self.cont(&[C::PushLex("form", None), C::ClassExpression, C::PopLex]);
        }
        if t == "keyword c" || t == "async" {
            return self.cont(&[expr]);
        }
        if t == "(" {
            return self.cont(&[
                C::PushLex(")", None),
                C::MaybeExpression,
                C::Expect(")"),
                C::PopLex,
                maybeop,
            ]);
        }
        if t == "operator" || t == "spread" {
            return self.cont(&[expr]);
        }
        if t == "[" {
            return self.cont(&[C::PushLex("]", None), C::ArrayLiteral, C::PopLex, maybeop]);
        }
        if t == "{" {
            return self.cont_commasep(&C::ObjProp, "}", None, &[maybeop]);
        }
        if t == "quasi" {
            return self.pass(&[C::Quasi, maybeop]);
        }
        if t == "new" {
            return self.cont(&[C::MaybeTarget(no_comma)]);
        }
        self.cont(&[])
    }

    /// `maybeoperatorNoComma(type, value, noComma)`; `noComma` is
    /// `Some(false)` from `maybeoperatorComma`, `None` from the stack.
    fn maybe_operator_no_comma(
        &mut self,
        t: &'static str,
        v: &str,
        no_comma: Option<bool>,
    ) -> bool {
        let comma = no_comma == Some(false);
        let me = if comma {
            C::MaybeOperatorComma
        } else {
            C::MaybeOperatorNoComma
        };
        let expr = if comma {
            C::Expression
        } else {
            C::ExpressionNoComma
        };
        if t == "=>" {
            let body = if no_comma == Some(true) {
                C::ArrowBodyNoComma
            } else {
                C::ArrowBody
            };
            return self.cont(&[C::PushContext, body, C::PopContext]);
        }
        if t == "operator" {
            if v.contains("++") || v.contains("--") || (self.ts() && v == "!") {
                return self.cont(&[me]);
            }
            if self.ts() && v == "<" && self.look(re!(r"^([^<>]|<[^<>]*>)*>\s*\(")) {
                return self.cont(&[
                    C::PushLex(">", None),
                    commasep(&C::TypeExpr, ">"),
                    C::PopLex,
                    me,
                ]);
            }
            if v == "?" {
                return self.cont(&[C::Expression, C::Expect(":"), expr]);
            }
            return self.cont(&[expr]);
        }
        if t == "quasi" {
            return self.pass(&[C::Quasi, me]);
        }
        if t == ";" {
            return false;
        }
        if t == "(" {
            return self.cont_commasep(&C::ExpressionNoComma, ")", Some("call"), &[me]);
        }
        if t == "." {
            return self.cont(&[C::Property, me]);
        }
        if t == "[" {
            return self.cont(&[
                C::PushLex("]", None),
                C::MaybeExpression,
                C::Expect("]"),
                C::PopLex,
                me,
            ]);
        }
        if self.ts() && v == "as" {
            self.marked = Some("keyword");
            return self.cont(&[C::TypeExpr, me]);
        }
        if t == "regexp" {
            self.st.last_type = "operator";
            self.marked = Some("operator");
            let back = self.stream.pos - self.stream.start - 1;
            self.stream.back_up(back);
            return self.cont(&[expr]);
        }
        false
    }

    /// `objprop`
    fn objprop(&mut self, t: &'static str, v: &str) -> bool {
        if t == "async" {
            self.marked = Some("property");
            return self.cont(&[C::ObjProp]);
        } else if t == "variable" || self.style == Some("keyword") {
            self.marked = Some("property");
            if v == "get" || v == "set" {
                return self.cont(&[C::GetterSetter]);
            }
            // Work around fat-arrow-detection complication for detecting
            // typescript typed arrow params
            if self.ts()
                && self.st.fat_arrow_at == Some(self.stream.start as isize)
                && let Some(m) = self.stream.match_re(re!(r"^\s*:\s*"), false)
            {
                self.st.fat_arrow_at = Some((self.stream.pos + m.text.chars().count()) as isize);
            }
            return self.cont(&[C::AfterProp]);
        } else if t == "number" || t == "string" {
            self.marked = Some(if self.mode.jsonld {
                "property"
            } else {
                match self.style {
                    Some("number") => "number property",
                    Some("string") => "string property",
                    _ => "undefined property",
                }
            });
            return self.cont(&[C::AfterProp]);
        } else if t == "jsonld-keyword" {
            return self.cont(&[C::AfterProp]);
        } else if self.ts() && is_modifier(v) {
            self.marked = Some("keyword");
            return self.cont(&[C::ObjProp]);
        } else if t == "[" {
            return self.cont(&[C::Expression, C::MaybeType, C::Expect("]"), C::AfterProp]);
        } else if t == "spread" {
            return self.cont(&[C::ExpressionNoComma, C::AfterProp]);
        } else if v == "*" {
            self.marked = Some("keyword");
            return self.cont(&[C::ObjProp]);
        } else if t == ":" {
            return self.pass(&[C::AfterProp]);
        }
        false
    }

    /// `typeexpr`
    fn typeexpr(&mut self, t: &'static str, v: &str) -> bool {
        if v == "keyof" || v == "typeof" || v == "infer" || v == "readonly" {
            self.marked = Some("keyword");
            return self.cont(&[if v == "typeof" {
                C::ExpressionNoComma
            } else {
                C::TypeExpr
            }]);
        }
        if t == "variable" || v == "void" {
            self.marked = Some("type");
            return self.cont(&[C::AfterType]);
        }
        if v == "|" || v == "&" {
            return self.cont(&[C::TypeExpr]);
        }
        if t == "string" || t == "number" || t == "atom" {
            return self.cont(&[C::AfterType]);
        }
        if t == "[" {
            return self.cont(&[
                C::PushLex("]", None),
                C::Commasep {
                    what: &C::TypeExpr,
                    end: "]",
                    sep: Some(","),
                },
                C::PopLex,
                C::AfterType,
            ]);
        }
        if t == "{" {
            return self.cont(&[C::PushLex("}", None), C::TypeProps, C::PopLex, C::AfterType]);
        }
        if t == "(" {
            return self.cont(&[commasep(&C::TypeArg, ")"), C::MaybeReturnType, C::AfterType]);
        }
        if t == "<" {
            return self.cont(&[commasep(&C::TypeExpr, ">"), C::TypeExpr]);
        }
        if t == "quasi" {
            return self.pass(&[C::QuasiType, C::AfterType]);
        }
        false
    }

    /// `typeprop`
    fn typeprop(&mut self, t: &'static str, v: &str) -> bool {
        if t == "variable" || self.style == Some("keyword") {
            self.marked = Some("property");
            return self.cont(&[C::TypeProp]);
        } else if v == "?" || t == "number" || t == "string" {
            return self.cont(&[C::TypeProp]);
        } else if t == ":" {
            return self.cont(&[C::TypeExpr]);
        } else if t == "[" {
            return self.cont(&[
                C::Expect("variable"),
                C::MaybeTypeOrIn,
                C::Expect("]"),
                C::TypeProp,
            ]);
        } else if t == "(" {
            return self.pass(&[C::FunctionDecl, C::TypeProp]);
        } else if !t.contains([';', '}', ')', ']', ',']) {
            return self.cont(&[]);
        }
        false
    }

    /// `afterType`
    fn after_type(&mut self, t: &'static str, v: &str) -> bool {
        if v == "<" {
            return self.cont(&[
                C::PushLex(">", None),
                commasep(&C::TypeExpr, ">"),
                C::PopLex,
                C::AfterType,
            ]);
        }
        if v == "|" || t == "." || v == "&" {
            return self.cont(&[C::TypeExpr]);
        }
        if t == "[" {
            return self.cont(&[C::TypeExpr, C::Expect("]"), C::AfterType]);
        }
        if v == "extends" || v == "implements" {
            self.marked = Some("keyword");
            return self.cont(&[C::TypeExpr]);
        }
        if v == "?" {
            return self.cont(&[C::TypeExpr, C::Expect(":"), C::TypeExpr]);
        }
        false
    }

    /// `pattern`
    fn pattern(&mut self, t: &'static str, v: &str) -> bool {
        if self.ts() && is_modifier(v) {
            self.marked = Some("keyword");
            return self.cont(&[C::Pattern]);
        }
        if t == "variable" {
            self.register(v);
            return self.cont(&[]);
        }
        if t == "spread" {
            return self.cont(&[C::Pattern]);
        }
        if t == "[" {
            return self.cont_commasep(&C::EltPattern, "]", None, &[]);
        }
        if t == "{" {
            return self.cont_commasep(&C::PropPattern, "}", None, &[]);
        }
        false
    }

    /// `proppattern`
    fn proppattern(&mut self, t: &'static str, v: &str) -> bool {
        if t == "variable" && !self.look(re!(r"^\s*:")) {
            self.register(v);
            return self.cont(&[C::MaybeAssign]);
        }
        if t == "variable" {
            self.marked = Some("property");
        }
        if t == "spread" {
            return self.cont(&[C::Pattern]);
        }
        if t == "}" {
            return self.pass(&[]);
        }
        if t == "[" {
            return self.cont(&[
                C::Expression,
                C::Expect("]"),
                C::Expect(":"),
                C::PropPattern,
            ]);
        }
        self.cont(&[C::Expect(":"), C::Pattern, C::MaybeAssign])
    }

    /// `classBody`
    fn class_body(&mut self, t: &'static str, v: &str) -> bool {
        if t == "async"
            || (t == "variable"
                && (v == "static" || v == "get" || v == "set" || (self.ts() && is_modifier(v)))
                && self.look(re!(r"^\s+#?[A-Za-z0-9_$\x{a1}-\x{10ffff}]")))
        {
            self.marked = Some("keyword");
            return self.cont(&[C::ClassBody]);
        }
        if t == "variable" || self.style == Some("keyword") {
            self.marked = Some("property");
            return self.cont(&[C::ClassField, C::ClassBody]);
        }
        if t == "number" || t == "string" {
            return self.cont(&[C::ClassField, C::ClassBody]);
        }
        if t == "[" {
            return self.cont(&[
                C::Expression,
                C::MaybeType,
                C::Expect("]"),
                C::ClassField,
                C::ClassBody,
            ]);
        }
        if v == "*" {
            self.marked = Some("keyword");
            return self.cont(&[C::ClassBody]);
        }
        if self.ts() && t == "(" {
            return self.pass(&[C::FunctionDecl, C::ClassBody]);
        }
        if t == ";" || t == "," {
            return self.cont(&[C::ClassBody]);
        }
        if t == "}" {
            return self.cont(&[]);
        }
        if v == "@" {
            return self.cont(&[C::Expression, C::ClassBody]);
        }
        false
    }
}

impl Mode for JsMode {
    fn name(&self) -> &'static str {
        "javascript"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(self.start_js())
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        self.token_js(stream, state::<JsState>(st))
            .map(str::to_string)
    }
}
