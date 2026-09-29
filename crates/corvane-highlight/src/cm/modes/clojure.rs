//! `codemirror/mode/clojure/clojure.js` (`text/x-clojure`,
//! `text/x-clojurescript`, `application/edn`).
//!
//! Ported line by line; the JS function names are kept on the Rust ones.
//! The `ctx` stack only computes indentation, which the highlighter never
//! asks for, so it is left out; of `state.lastToken` only "was it `(`"
//! decides a style, so that is all the state keeps.
//!
//! JS regexes without the `u` flag see UTF-16 units: `.` in
//! `characterLiteral` is spelled as "any BMP char but a line terminator",
//! so `\😀` fails there like it does in GHD.

use std::collections::HashSet;

use super::super::{Mode, ModeState, StringStream, state};
use crate::re;

const ATOMS: &[&str] = &["false", "nil", "true"];
const SPECIAL_FORMS: &[&str] = &[
    ".",
    "catch",
    "def",
    "do",
    "if",
    "monitor-enter",
    "monitor-exit",
    "new",
    "quote",
    "recur",
    "set!",
    "throw",
    "try",
    "var",
];
const CORE_SYMBOLS: &[&str] = &[
    "*",
    "*'",
    "*1",
    "*2",
    "*3",
    "*agent*",
    "*allow-unresolved-vars*",
    "*assert*",
    "*clojure-version*",
    "*command-line-args*",
    "*compile-files*",
    "*compile-path*",
    "*compiler-options*",
    "*data-readers*",
    "*default-data-reader-fn*",
    "*e",
    "*err*",
    "*file*",
    "*flush-on-newline*",
    "*fn-loader*",
    "*in*",
    "*math-context*",
    "*ns*",
    "*out*",
    "*print-dup*",
    "*print-length*",
    "*print-level*",
    "*print-meta*",
    "*print-namespace-maps*",
    "*print-readably*",
    "*read-eval*",
    "*reader-resolver*",
    "*source-path*",
    "*suppress-read*",
    "*unchecked-math*",
    "*use-context-classloader*",
    "*verbose-defrecords*",
    "*warn-on-reflection*",
    "+",
    "+'",
    "-",
    "-'",
    "->",
    "->>",
    "->ArrayChunk",
    "->Eduction",
    "->Vec",
    "->VecNode",
    "->VecSeq",
    "-cache-protocol-fn",
    "-reset-methods",
    "..",
    "/",
    "<",
    "<=",
    "=",
    "==",
    ">",
    ">=",
    "EMPTY-NODE",
    "Inst",
    "StackTraceElement->vec",
    "Throwable->map",
    "accessor",
    "aclone",
    "add-classpath",
    "add-watch",
    "agent",
    "agent-error",
    "agent-errors",
    "aget",
    "alength",
    "alias",
    "all-ns",
    "alter",
    "alter-meta!",
    "alter-var-root",
    "amap",
    "ancestors",
    "and",
    "any?",
    "apply",
    "areduce",
    "array-map",
    "as->",
    "aset",
    "aset-boolean",
    "aset-byte",
    "aset-char",
    "aset-double",
    "aset-float",
    "aset-int",
    "aset-long",
    "aset-short",
    "assert",
    "assoc",
    "assoc!",
    "assoc-in",
    "associative?",
    "atom",
    "await",
    "await-for",
    "await1",
    "bases",
    "bean",
    "bigdec",
    "bigint",
    "biginteger",
    "binding",
    "bit-and",
    "bit-and-not",
    "bit-clear",
    "bit-flip",
    "bit-not",
    "bit-or",
    "bit-set",
    "bit-shift-left",
    "bit-shift-right",
    "bit-test",
    "bit-xor",
    "boolean",
    "boolean-array",
    "boolean?",
    "booleans",
    "bound-fn",
    "bound-fn*",
    "bound?",
    "bounded-count",
    "butlast",
    "byte",
    "byte-array",
    "bytes",
    "bytes?",
    "case",
    "cast",
    "cat",
    "char",
    "char-array",
    "char-escape-string",
    "char-name-string",
    "char?",
    "chars",
    "chunk",
    "chunk-append",
    "chunk-buffer",
    "chunk-cons",
    "chunk-first",
    "chunk-next",
    "chunk-rest",
    "chunked-seq?",
    "class",
    "class?",
    "clear-agent-errors",
    "clojure-version",
    "coll?",
    "comment",
    "commute",
    "comp",
    "comparator",
    "compare",
    "compare-and-set!",
    "compile",
    "complement",
    "completing",
    "concat",
    "cond",
    "cond->",
    "cond->>",
    "condp",
    "conj",
    "conj!",
    "cons",
    "constantly",
    "construct-proxy",
    "contains?",
    "count",
    "counted?",
    "create-ns",
    "create-struct",
    "cycle",
    "dec",
    "dec'",
    "decimal?",
    "declare",
    "dedupe",
    "default-data-readers",
    "definline",
    "definterface",
    "defmacro",
    "defmethod",
    "defmulti",
    "defn",
    "defn-",
    "defonce",
    "defprotocol",
    "defrecord",
    "defstruct",
    "deftype",
    "delay",
    "delay?",
    "deliver",
    "denominator",
    "deref",
    "derive",
    "descendants",
    "destructure",
    "disj",
    "disj!",
    "dissoc",
    "dissoc!",
    "distinct",
    "distinct?",
    "doall",
    "dorun",
    "doseq",
    "dosync",
    "dotimes",
    "doto",
    "double",
    "double-array",
    "double?",
    "doubles",
    "drop",
    "drop-last",
    "drop-while",
    "eduction",
    "empty",
    "empty?",
    "ensure",
    "ensure-reduced",
    "enumeration-seq",
    "error-handler",
    "error-mode",
    "eval",
    "even?",
    "every-pred",
    "every?",
    "ex-data",
    "ex-info",
    "extend",
    "extend-protocol",
    "extend-type",
    "extenders",
    "extends?",
    "false?",
    "ffirst",
    "file-seq",
    "filter",
    "filterv",
    "find",
    "find-keyword",
    "find-ns",
    "find-protocol-impl",
    "find-protocol-method",
    "find-var",
    "first",
    "flatten",
    "float",
    "float-array",
    "float?",
    "floats",
    "flush",
    "fn",
    "fn?",
    "fnext",
    "fnil",
    "for",
    "force",
    "format",
    "frequencies",
    "future",
    "future-call",
    "future-cancel",
    "future-cancelled?",
    "future-done?",
    "future?",
    "gen-class",
    "gen-interface",
    "gensym",
    "get",
    "get-in",
    "get-method",
    "get-proxy-class",
    "get-thread-bindings",
    "get-validator",
    "group-by",
    "halt-when",
    "hash",
    "hash-combine",
    "hash-map",
    "hash-ordered-coll",
    "hash-set",
    "hash-unordered-coll",
    "ident?",
    "identical?",
    "identity",
    "if-let",
    "if-not",
    "if-some",
    "ifn?",
    "import",
    "in-ns",
    "inc",
    "inc'",
    "indexed?",
    "init-proxy",
    "inst-ms",
    "inst-ms*",
    "inst?",
    "instance?",
    "int",
    "int-array",
    "int?",
    "integer?",
    "interleave",
    "intern",
    "interpose",
    "into",
    "into-array",
    "ints",
    "io!",
    "isa?",
    "iterate",
    "iterator-seq",
    "juxt",
    "keep",
    "keep-indexed",
    "key",
    "keys",
    "keyword",
    "keyword?",
    "last",
    "lazy-cat",
    "lazy-seq",
    "let",
    "letfn",
    "line-seq",
    "list",
    "list*",
    "list?",
    "load",
    "load-file",
    "load-reader",
    "load-string",
    "loaded-libs",
    "locking",
    "long",
    "long-array",
    "longs",
    "loop",
    "macroexpand",
    "macroexpand-1",
    "make-array",
    "make-hierarchy",
    "map",
    "map-entry?",
    "map-indexed",
    "map?",
    "mapcat",
    "mapv",
    "max",
    "max-key",
    "memfn",
    "memoize",
    "merge",
    "merge-with",
    "meta",
    "method-sig",
    "methods",
    "min",
    "min-key",
    "mix-collection-hash",
    "mod",
    "munge",
    "name",
    "namespace",
    "namespace-munge",
    "nat-int?",
    "neg-int?",
    "neg?",
    "newline",
    "next",
    "nfirst",
    "nil?",
    "nnext",
    "not",
    "not-any?",
    "not-empty",
    "not-every?",
    "not=",
    "ns",
    "ns-aliases",
    "ns-imports",
    "ns-interns",
    "ns-map",
    "ns-name",
    "ns-publics",
    "ns-refers",
    "ns-resolve",
    "ns-unalias",
    "ns-unmap",
    "nth",
    "nthnext",
    "nthrest",
    "num",
    "number?",
    "numerator",
    "object-array",
    "odd?",
    "or",
    "parents",
    "partial",
    "partition",
    "partition-all",
    "partition-by",
    "pcalls",
    "peek",
    "persistent!",
    "pmap",
    "pop",
    "pop!",
    "pop-thread-bindings",
    "pos-int?",
    "pos?",
    "pr",
    "pr-str",
    "prefer-method",
    "prefers",
    "primitives-classnames",
    "print",
    "print-ctor",
    "print-dup",
    "print-method",
    "print-simple",
    "print-str",
    "printf",
    "println",
    "println-str",
    "prn",
    "prn-str",
    "promise",
    "proxy",
    "proxy-call-with-super",
    "proxy-mappings",
    "proxy-name",
    "proxy-super",
    "push-thread-bindings",
    "pvalues",
    "qualified-ident?",
    "qualified-keyword?",
    "qualified-symbol?",
    "quot",
    "rand",
    "rand-int",
    "rand-nth",
    "random-sample",
    "range",
    "ratio?",
    "rational?",
    "rationalize",
    "re-find",
    "re-groups",
    "re-matcher",
    "re-matches",
    "re-pattern",
    "re-seq",
    "read",
    "read-line",
    "read-string",
    "reader-conditional",
    "reader-conditional?",
    "realized?",
    "record?",
    "reduce",
    "reduce-kv",
    "reduced",
    "reduced?",
    "reductions",
    "ref",
    "ref-history-count",
    "ref-max-history",
    "ref-min-history",
    "ref-set",
    "refer",
    "refer-clojure",
    "reify",
    "release-pending-sends",
    "rem",
    "remove",
    "remove-all-methods",
    "remove-method",
    "remove-ns",
    "remove-watch",
    "repeat",
    "repeatedly",
    "replace",
    "replicate",
    "require",
    "reset!",
    "reset-meta!",
    "reset-vals!",
    "resolve",
    "rest",
    "restart-agent",
    "resultset-seq",
    "reverse",
    "reversible?",
    "rseq",
    "rsubseq",
    "run!",
    "satisfies?",
    "second",
    "select-keys",
    "send",
    "send-off",
    "send-via",
    "seq",
    "seq?",
    "seqable?",
    "seque",
    "sequence",
    "sequential?",
    "set",
    "set-agent-send-executor!",
    "set-agent-send-off-executor!",
    "set-error-handler!",
    "set-error-mode!",
    "set-validator!",
    "set?",
    "short",
    "short-array",
    "shorts",
    "shuffle",
    "shutdown-agents",
    "simple-ident?",
    "simple-keyword?",
    "simple-symbol?",
    "slurp",
    "some",
    "some->",
    "some->>",
    "some-fn",
    "some?",
    "sort",
    "sort-by",
    "sorted-map",
    "sorted-map-by",
    "sorted-set",
    "sorted-set-by",
    "sorted?",
    "special-symbol?",
    "spit",
    "split-at",
    "split-with",
    "str",
    "string?",
    "struct",
    "struct-map",
    "subs",
    "subseq",
    "subvec",
    "supers",
    "swap!",
    "swap-vals!",
    "symbol",
    "symbol?",
    "sync",
    "tagged-literal",
    "tagged-literal?",
    "take",
    "take-last",
    "take-nth",
    "take-while",
    "test",
    "the-ns",
    "thread-bound?",
    "time",
    "to-array",
    "to-array-2d",
    "trampoline",
    "transduce",
    "transient",
    "tree-seq",
    "true?",
    "type",
    "unchecked-add",
    "unchecked-add-int",
    "unchecked-byte",
    "unchecked-char",
    "unchecked-dec",
    "unchecked-dec-int",
    "unchecked-divide-int",
    "unchecked-double",
    "unchecked-float",
    "unchecked-inc",
    "unchecked-inc-int",
    "unchecked-int",
    "unchecked-long",
    "unchecked-multiply",
    "unchecked-multiply-int",
    "unchecked-negate",
    "unchecked-negate-int",
    "unchecked-remainder-int",
    "unchecked-short",
    "unchecked-subtract",
    "unchecked-subtract-int",
    "underive",
    "unquote",
    "unquote-splicing",
    "unreduced",
    "unsigned-bit-shift-right",
    "update",
    "update-in",
    "update-proxy",
    "uri?",
    "use",
    "uuid?",
    "val",
    "vals",
    "var-get",
    "var-set",
    "var?",
    "vary-meta",
    "vec",
    "vector",
    "vector-of",
    "vector?",
    "volatile!",
    "volatile?",
    "vreset!",
    "vswap!",
    "when",
    "when-first",
    "when-let",
    "when-not",
    "when-some",
    "while",
    "with-bindings",
    "with-bindings*",
    "with-in-str",
    "with-loading-context",
    "with-local-vars",
    "with-meta",
    "with-open",
    "with-out-str",
    "with-precision",
    "with-redefs",
    "with-redefs-fn",
    "xml-seq",
    "zero?",
    "zipmap",
];

/// `delimiter` tested on one char: `/^(?:[\\\[\]\s"(),;@^`{}~]|$)/`
fn is_delimiter(c: char) -> bool {
    matches!(
        c,
        '\\' | '[' | ']' | '"' | '(' | ')' | ',' | ';' | '@' | '^' | '`' | '{' | '}' | '~'
    ) || c.is_whitespace()
}

/// The first element of the JS `[type, style]` pair.
#[derive(Clone, Copy, PartialEq)]
enum Type {
    Space,
    Open,
    Close,
    Symbol,
    Null,
}

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    Base,
    InString,
    InComment,
}

#[derive(Clone)]
struct ClojureState {
    /// `state.lastToken === "("`
    last_open_paren: bool,
    tokenize: Tokenize,
}

pub struct Clojure {
    atom: HashSet<&'static str>,
    /// `specialForm` and `coreSymbol`, which give the same style
    keyword: HashSet<&'static str>,
}

impl Clojure {
    pub fn new() -> Self {
        Self {
            atom: ATOMS.iter().copied().collect(),
            keyword: SPECIAL_FORMS.iter().chain(CORE_SYMBOLS).copied().collect(),
        }
    }

    /// `base`
    fn base(
        &self,
        stream: &mut StringStream,
        state: &mut ClojureState,
    ) -> (Type, Option<&'static str>) {
        if stream.eat_space() || stream.eat(',').is_some() {
            return (Type::Space, None);
        }
        // numberLiteral
        if stream.matches(re!(
            r##"^(?:[+\-]?[0-9]+(?:(?:N|(?:[eE][+\-]?[0-9]+))|(?:\.?[0-9]*(?:M|(?:[eE][+\-]?[0-9]+))?)|/[0-9]+|[xX][0-9a-fA-F]+|r[0-9a-zA-Z]+)?(?=[\\\[\]\s"#'(),;@^`{}~]|$))"##
        )) {
            return (Type::Null, Some("number"));
        }
        // characterLiteral
        if stream.matches(re!(
            r##"^(?:\\(?:backspace|formfeed|newline|return|space|tab|o[0-7]{3}|u[0-9A-Fa-f]{4}|x[0-9A-Fa-f]{4}|[^\n\r\x{2028}\x{2029}\x{10000}-\x{10ffff}])?(?=[\\\[\]\s"(),;@^`{}~]|$))"##
        )) {
            return (Type::Null, Some("string-2"));
        }
        if stream.eat('"').is_some() {
            state.tokenize = Tokenize::InString;
            return in_string(stream, state);
        }
        if stream.eat_if(|c| matches!(c, '(' | '[' | '{')).is_some() {
            return (Type::Open, Some("bracket"));
        }
        if stream.eat_if(|c| matches!(c, ')' | ']' | '}')).is_some() {
            return (Type::Close, Some("bracket"));
        }
        if stream.eat(';').is_some() {
            stream.skip_to_end();
            return (Type::Space, Some("comment"));
        }
        if stream
            .eat_if(|c| matches!(c, '#' | '\'' | '@' | '^' | '`' | '~'))
            .is_some()
        {
            return (Type::Null, Some("meta"));
        }

        // qualifiedSymbol
        let matches = stream.match_re(
            re!(
                r##"^(?:(?:[^\\/\[\]0-9\s"#'(),;@^`{}~.][^\\\[\]\s"(),;@^`{}~./]*(?:\.[^\\/\[\]0-9\s"#'(),;@^`{}~.][^\\\[\]\s"(),;@^`{}~./]*)*/)?(?:/|[^\\/\[\]0-9\s"#'(),;@^`{}~][^\\\[\]\s"(),;@^`{}~]*)*(?=[\\\[\]\s"(),;@^`{}~]|$))"##
            ),
            true,
        );
        let symbol = match &matches {
            Some(m) if !m.text.is_empty() => m.text.as_str(),
            _ => {
                // advance stream by at least one character so we don't get stuck.
                stream.next();
                stream.eat_while_if(|c| !is_delimiter(c));
                return (Type::Null, Some("error"));
            }
        };

        if symbol == "comment" && state.last_open_paren {
            state.tokenize = Tokenize::InComment;
            return in_comment(stream, state);
        }
        if self.atom.contains(symbol) || symbol.starts_with(':') {
            return (Type::Symbol, Some("atom"));
        }
        if self.keyword.contains(symbol) {
            return (Type::Symbol, Some("keyword"));
        }
        if state.last_open_paren {
            return (Type::Symbol, Some("builtin")); // other operator
        }
        (Type::Symbol, Some("variable"))
    }
}

impl Default for Clojure {
    fn default() -> Self {
        Self::new()
    }
}

/// `inString`
fn in_string(stream: &mut StringStream, state: &mut ClojureState) -> (Type, Option<&'static str>) {
    let mut escaped = false;
    while let Some(next) = stream.next() {
        if next == '"' && !escaped {
            state.tokenize = Tokenize::Base;
            break;
        }
        escaped = !escaped && next == '\\';
    }
    (Type::Null, Some("string"))
}

/// `inComment`: the parenthesis count starts over on every line
fn in_comment(stream: &mut StringStream, state: &mut ClojureState) -> (Type, Option<&'static str>) {
    let mut parenthesis_count = 1;
    while let Some(next) = stream.next() {
        if next == ')' {
            parenthesis_count -= 1;
        }
        if next == '(' {
            parenthesis_count += 1;
        }
        if parenthesis_count == 0 {
            stream.back_up(1);
            state.tokenize = Tokenize::Base;
            break;
        }
    }
    (Type::Space, Some("comment"))
}

impl Mode for Clojure {
    fn name(&self) -> &'static str {
        "clojure"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(ClojureState {
            last_open_paren: false,
            tokenize: Tokenize::Base,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = state::<ClojureState>(st);
        let (ty, style) = match state.tokenize {
            Tokenize::Base => self.base(stream, state),
            Tokenize::InString => in_string(stream, state),
            Tokenize::InComment => in_comment(stream, state),
        };
        if ty != Type::Space {
            state.last_open_paren = stream.current() == "(";
        }
        style.map(str::to_string)
    }
}
