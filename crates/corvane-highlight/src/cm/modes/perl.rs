//! `codemirror/mode/perl/perl.js` (`text/x-perl`).
//!
//! Ported line by line; the JS function names are kept on the Rust ones.
//! `state.tokenize` holds closures in JS (`tokenChain`'s and
//! `tokenSOMETHING`'s); here it is the [`Tokenize`] enum carrying what they
//! capture. perl.js adds its own stream helpers (`look`, `prefix`, `suffix`,
//! `eatSuffix`), ported as local functions.
//!
//! The `PERL` table is folded into [`perl_style`]: for every key the style
//! `tokenPerl` derives from its value (`[x, y]` → `x` when `y` is truthy,
//! 1‥5 → keyword / def / atom / operator / variable-2). Keys whose value is
//! `null` (`m`, `q`, `s`, `tr`, …) are falsy and so absent. Keys inherited
//! from `Object.prototype` are truthy in JS but only ever reach the
//! `"meta"` fallbacks, which absent keys hit too.

use super::super::{Mode, ModeState, StringStream, state};
use crate::re;

/// `RXstyle`
const RX_STYLE: &str = "string-2";

/// `PERL[word]`, resolved to the style `tokenPerl` gives it; `None` when
/// the entry is missing or falsy.
fn perl_style(word: &str) -> Option<&'static str> {
    Some(match word {
        "->" | "++" | "--" | "**" | "=~" | "!~" | "*" | "/" | "%" | "x" | "+" | "-" | "."
        | "<<" | ">>" | "<" | ">" | "<=" | ">=" | "lt" | "gt" | "le" | "ge" | "==" | "!="
        | "<=>" | "eq" | "ne" | "cmp" | "~~" | "&" | "|" | "^" | "&&" | "||" | "//" | ".."
        | "..." | "?" | ":" | "=" | "+=" | "-=" | "*=" | "," | "=>" | "::" | "not" | "and"
        | "or" | "xor" => "operator",
        "BEGIN"
        | "END"
        | "PRINT"
        | "PRINTF"
        | "GETC"
        | "READ"
        | "READLINE"
        | "DESTROY"
        | "TIE"
        | "TIEHANDLE"
        | "UNTIE"
        | "STDIN"
        | "STDIN_TOP"
        | "STDOUT"
        | "STDOUT_TOP"
        | "STDERR"
        | "STDERR_TOP"
        | "$ARG"
        | "$_"
        | "@ARG"
        | "@_"
        | "$LIST_SEPARATOR"
        | "$\""
        | "$PROCESS_ID"
        | "$PID"
        | "$$"
        | "$REAL_GROUP_ID"
        | "$GID"
        | "$("
        | "$EFFECTIVE_GROUP_ID"
        | "$EGID"
        | "$)"
        | "$PROGRAM_NAME"
        | "$0"
        | "$SUBSCRIPT_SEPARATOR"
        | "$SUBSEP"
        | "$;"
        | "$REAL_USER_ID"
        | "$UID"
        | "$<"
        | "$EFFECTIVE_USER_ID"
        | "$EUID"
        | "$>"
        | "$a"
        | "$b"
        | "$COMPILING"
        | "$^C"
        | "$DEBUGGING"
        | "$^D"
        | "${^ENCODING}"
        | "$ENV"
        | "%ENV"
        | "$SYSTEM_FD_MAX"
        | "$^F"
        | "@F"
        | "${^GLOBAL_PHASE}"
        | "$^H"
        | "%^H"
        | "@INC"
        | "%INC"
        | "$INPLACE_EDIT"
        | "$^I"
        | "$^M"
        | "$OSNAME"
        | "$^O"
        | "${^OPEN}"
        | "$PERLDB"
        | "$^P"
        | "$SIG"
        | "%SIG"
        | "$BASETIME"
        | "$^T"
        | "${^TAINT}"
        | "${^UNICODE}"
        | "${^UTF8CACHE}"
        | "${^UTF8LOCALE}"
        | "$PERL_VERSION"
        | "$^V"
        | "${^WIN32_SLOPPY_STAT}"
        | "$EXECUTABLE_NAME"
        | "$^X"
        | "$1"
        | "$MATCH"
        | "$&"
        | "${^MATCH}"
        | "$PREMATCH"
        | "$`"
        | "${^PREMATCH}"
        | "$POSTMATCH"
        | "$'"
        | "${^POSTMATCH}"
        | "$LAST_PAREN_MATCH"
        | "$+"
        | "$LAST_SUBMATCH_RESULT"
        | "$^N"
        | "@LAST_MATCH_END"
        | "@+"
        | "%LAST_PAREN_MATCH"
        | "%+"
        | "@LAST_MATCH_START"
        | "@-"
        | "%LAST_MATCH_START"
        | "%-"
        | "$LAST_REGEXP_CODE_RESULT"
        | "$^R"
        | "${^RE_DEBUG_FLAGS}"
        | "${^RE_TRIE_MAXBUF}"
        | "$ARGV"
        | "@ARGV"
        | "ARGV"
        | "ARGVOUT"
        | "$OUTPUT_FIELD_SEPARATOR"
        | "$OFS"
        | "$,"
        | "$INPUT_LINE_NUMBER"
        | "$NR"
        | "$."
        | "$INPUT_RECORD_SEPARATOR"
        | "$RS"
        | "$/"
        | "$OUTPUT_RECORD_SEPARATOR"
        | "$ORS"
        | "$\\"
        | "$OUTPUT_AUTOFLUSH"
        | "$|"
        | "$ACCUMULATOR"
        | "$^A"
        | "$FORMAT_FORMFEED"
        | "$^L"
        | "$FORMAT_PAGE_NUMBER"
        | "$%"
        | "$FORMAT_LINES_LEFT"
        | "$-"
        | "$FORMAT_LINE_BREAK_CHARACTERS"
        | "$:"
        | "$FORMAT_LINES_PER_PAGE"
        | "$="
        | "$FORMAT_TOP_NAME"
        | "$^"
        | "$FORMAT_NAME"
        | "$~"
        | "${^CHILD_ERROR_NATIVE}"
        | "$EXTENDED_OS_ERROR"
        | "$^E"
        | "$EXCEPTIONS_BEING_CAUGHT"
        | "$^S"
        | "$WARNING"
        | "$^W"
        | "${^WARNING_BITS}"
        | "$OS_ERROR"
        | "$ERRNO"
        | "$!"
        | "%OS_ERROR"
        | "%ERRNO"
        | "%!"
        | "$CHILD_ERROR"
        | "$?"
        | "$EVAL_ERROR"
        | "$@"
        | "$OFMT"
        | "$#"
        | "$*"
        | "$ARRAY_BASE"
        | "$["
        | "$OLD_PERL_VERSION"
        | "$]" => "variable-2",
        "if" | "elsif" | "else" | "while" | "unless" | "for" | "foreach" | "abs" | "accept"
        | "alarm" | "atan2" | "bind" | "binmode" | "bless" | "bootstrap" | "break" | "caller"
        | "chdir" | "chmod" | "chomp" | "chop" | "chown" | "chr" | "chroot" | "close"
        | "closedir" | "connect" | "continue" | "cos" | "crypt" | "dbmclose" | "dbmopen"
        | "default" | "defined" | "delete" | "die" | "do" | "dump" | "each" | "endgrent"
        | "endhostent" | "endnetent" | "endprotoent" | "endpwent" | "endservent" | "eof"
        | "eval" | "exec" | "exists" | "exit" | "exp" | "fcntl" | "fileno" | "flock" | "fork"
        | "format" | "formline" | "getc" | "getgrent" | "getgrgid" | "getgrnam"
        | "gethostbyaddr" | "gethostbyname" | "gethostent" | "getlogin" | "getnetbyaddr"
        | "getnetbyname" | "getnetent" | "getpeername" | "getpgrp" | "getppid" | "getpriority"
        | "getprotobyname" | "getprotobynumber" | "getprotoent" | "getpwent" | "getpwnam"
        | "getpwuid" | "getservbyname" | "getservbyport" | "getservent" | "getsockname"
        | "getsockopt" | "given" | "glob" | "gmtime" | "goto" | "grep" | "hex" | "import"
        | "index" | "int" | "ioctl" | "join" | "keys" | "kill" | "last" | "lc" | "lcfirst"
        | "length" | "link" | "listen" | "localtime" | "lock" | "log" | "lstat" | "map"
        | "mkdir" | "msgctl" | "msgget" | "msgrcv" | "msgsnd" | "new" | "next" | "no" | "oct"
        | "open" | "opendir" | "ord" | "pack" | "package" | "pipe" | "pop" | "pos" | "print"
        | "printf" | "prototype" | "push" | "rand" | "read" | "readdir" | "readline"
        | "readlink" | "readpipe" | "recv" | "redo" | "ref" | "rename" | "require" | "reset"
        | "return" | "reverse" | "rewinddir" | "rindex" | "rmdir" | "say" | "scalar" | "seek"
        | "seekdir" | "select" | "semctl" | "semget" | "semop" | "send" | "setgrent"
        | "sethostent" | "setnetent" | "setpgrp" | "setpriority" | "setprotoent" | "setpwent"
        | "setservent" | "setsockopt" | "shift" | "shmctl" | "shmget" | "shmread" | "shmwrite"
        | "shutdown" | "sin" | "sleep" | "socket" | "socketpair" | "sort" | "splice" | "split"
        | "sprintf" | "sqrt" | "srand" | "stat" | "state" | "study" | "sub" | "substr"
        | "symlink" | "syscall" | "sysopen" | "sysread" | "sysseek" | "system" | "syswrite"
        | "tell" | "telldir" | "tie" | "tied" | "time" | "times" | "truncate" | "uc"
        | "ucfirst" | "umask" | "undef" | "unlink" | "unpack" | "unshift" | "untie" | "use"
        | "utime" | "values" | "vec" | "wait" | "waitpid" | "wantarray" | "warn" | "when"
        | "write" => "keyword",
        "local" | "my" | "our" => "def",
        _ => return None,
    })
}

/// `state.tokenize`
#[derive(Clone)]
enum Tokenize {
    /// `tokenPerl`
    Perl,
    /// the closure of `tokenChain(stream, state, chain, style, tail)`;
    /// `tail` is always `RXmodifiers` when set
    Chain {
        chain: [Option<char>; 2],
        style: &'static str,
        tail: bool,
    },
    /// the closure of `tokenSOMETHING(stream, state, string)`
    Something(String),
}

#[derive(Clone)]
struct PerlState {
    tokenize: Tokenize,
    chain: Option<char>,
    style: &'static str,
    tail: bool,
}

/// JS `/\w/` (ASCII)
fn is_w(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `/[\^'"!~\/]/`
fn is_quote_delim(c: char) -> bool {
    matches!(c, '^' | '\'' | '"' | '!' | '~' | '/')
}

/// `(` `[` `{` `<` → the closing bracket
fn closing(c: char) -> Option<char> {
    Some(match c {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '<' => '>',
        _ => return None,
    })
}

/// `/[(\[{<\^'"!~\/]/`
fn is_delim(c: char) -> bool {
    closing(c).is_some() || is_quote_delim(c)
}

/// `RXmodifiers`: `/[goseximacplud]/`
fn is_rx_modifier(c: char) -> bool {
    matches!(
        c,
        'g' | 'o' | 's' | 'e' | 'x' | 'i' | 'm' | 'a' | 'c' | 'p' | 'l' | 'u' | 'd'
    )
}

/// `/[=|\\\-#?@;:&`~\^!\[\]*'"$+.,\/<>()]/` (punctuation variables)
fn is_special_var(c: char) -> bool {
    matches!(
        c,
        '=' | '|'
            | '\\'
            | '-'
            | '#'
            | '?'
            | '@'
            | ';'
            | ':'
            | '&'
            | '`'
            | '~'
            | '^'
            | '!'
            | '['
            | ']'
            | '*'
            | '\''
            | '"'
            | '$'
            | '+'
            | '.'
            | ','
            | '/'
            | '<'
            | '>'
            | '('
            | ')'
    )
}

/// `/[:+\-\^*$&%@=<>!?|\/~\.]/`
fn is_op(c: char) -> bool {
    matches!(
        c,
        ':' | '+'
            | '-'
            | '^'
            | '*'
            | '$'
            | '&'
            | '%'
            | '@'
            | '='
            | '<'
            | '>'
            | '!'
            | '?'
            | '|'
            | '/'
            | '~'
            | '.'
    )
}

/// `look(stream, c)`: `stream.string.charAt(stream.pos + c)`
fn look(stream: &StringStream, c: isize) -> Option<char> {
    let i = stream.pos as isize + c;
    if i < 0 {
        return None;
    }
    stream.char_at(i as usize)
}

/// `prefix(stream, c)` with `c` given: `substr(max(pos - c, 0), c)`
fn prefix_n<'s>(stream: &'s StringStream, c: usize) -> &'s str {
    let x = stream.pos.saturating_sub(c);
    stream.slice(x, x + c)
}

/// `prefix(stream)`: `substr(0, pos - 1)`
fn prefix<'s>(stream: &'s StringStream) -> &'s str {
    stream.slice(0, stream.pos.saturating_sub(1))
}

/// `suffix(stream, c)`
fn suffix<'s>(stream: &'s StringStream, c: usize) -> &'s str {
    let y = stream.len();
    let x = (y + 1).saturating_sub(stream.pos);
    let n = if c < y { c } else { x };
    stream.slice(stream.pos, stream.pos + n)
}

/// `eatSuffix(stream, c)`
fn eat_suffix(stream: &mut StringStream, c: usize) {
    let x = stream.pos + c;
    let y = stream.len().saturating_sub(1);
    stream.pos = x.min(y);
}

// tokenChain(stream, state, chain, style, tail)
fn token_chain(
    stream: &mut StringStream,
    state: &mut PerlState,
    chain: [Option<char>; 2],
    style: &'static str,
    tail: bool,
) -> &'static str {
    state.chain = None;
    state.style = "";
    state.tail = false;
    state.tokenize = Tokenize::Chain { chain, style, tail };
    chain_closure(stream, state, chain, style, tail)
}

// the function tokenChain installs as state.tokenize
fn chain_closure(
    stream: &mut StringStream,
    state: &mut PerlState,
    chain: [Option<char>; 2],
    style: &'static str,
    tail: bool,
) -> &'static str {
    let mut e = false;
    while let Some(c) = stream.next() {
        if Some(c) == chain[0] && !e {
            if let Some(next) = chain[1] {
                state.chain = Some(next);
                state.style = style;
                state.tail = tail;
            } else if tail {
                stream.eat_while_if(is_rx_modifier);
            }
            state.tokenize = Tokenize::Perl;
            return style;
        }
        e = !e && c == '\\';
    }
    style
}

// tokenSOMETHING(stream, state, string)
fn token_something(
    stream: &mut StringStream,
    state: &mut PerlState,
    string: String,
) -> &'static str {
    state.tokenize = Tokenize::Something(string);
    something_closure(stream, state)
}

// the function tokenSOMETHING installs as state.tokenize
fn something_closure(stream: &mut StringStream, state: &mut PerlState) -> &'static str {
    if matches!(&state.tokenize, Tokenize::Something(s) if stream.string() == s) {
        state.tokenize = Tokenize::Perl;
    }
    stream.skip_to_end();
    "string"
}

/// A bareword's style: `PERL[word]` unless the char before it (`l`) is `:`.
fn word_style(word: &str, l: Option<char>) -> &'static str {
    match perl_style(word) {
        Some(style) if l != Some(':') => style,
        _ => "meta",
    }
}

/// The `qx` / `qq` / `qw` / `qr` branches: the delimiter after the second
/// letter (`look(stream, 1)`), `None` when there is none.
fn quote_like(
    stream: &mut StringStream,
    state: &mut PerlState,
    style: &'static str,
    tail: bool,
) -> Option<&'static str> {
    let c = look(stream, 1)?;
    if let Some(close) = closing(c) {
        eat_suffix(stream, 2);
        return Some(token_chain(stream, state, [Some(close), None], style, tail));
    }
    if is_quote_delim(c) {
        eat_suffix(stream, 1);
        let open = stream.eat(c);
        return Some(token_chain(stream, state, [open, None], style, tail));
    }
    None
}

/// The `s` / `y` / `tr` branches: the delimiter, then `tokenChain` over
/// both parts.
fn substitution(stream: &mut StringStream, state: &mut PerlState) -> Option<&'static str> {
    let c = stream.eat_if(is_delim)?;
    let close = closing(c).unwrap_or(c);
    Some(token_chain(
        stream,
        state,
        [Some(close), Some(close)],
        RX_STYLE,
        true,
    ))
}

/// `/[\/>\]})\w]/.test(look(stream, -2))`
fn after_term(stream: &StringStream) -> bool {
    look(stream, -2).is_some_and(|c| matches!(c, '/' | '>' | ']' | '}' | ')') || is_w(c))
}

// tokenPerl
fn token_perl(stream: &mut StringStream, state: &mut PerlState) -> Option<&'static str> {
    if stream.eat_space() {
        return None;
    }
    if let Some(chain) = state.chain {
        let (style, tail) = (state.style, state.tail);
        return Some(token_chain(stream, state, [Some(chain), None], style, tail));
    }
    if stream.matches(re!(
        r"^(-?(([0-9][0-9_]*)?\.[0-9]+(e[+-]?[0-9]+)?|[0-9]+\.[0-9]*)|0x[0-9a-fA-F_]+|0b[01_]+|[0-9][0-9_]*(e[+-]?[0-9]+)?)"
    )) {
        return Some("number");
    }
    if stream.matches(re!(r"^<<(?=[_a-zA-Z])")) {
        // NOTE: <<SOMETHING\n...\nSOMETHING\n
        stream.eat_while_if(is_w);
        let string = stream.slice(stream.start + 2, stream.pos).to_string();
        return Some(token_something(stream, state, string));
    }
    if stream.sol() && stream.matches(re!(r"^=item(?![A-Za-z0-9_])")) {
        // NOTE: \n=item...\n=cut\n
        return Some(token_something(stream, state, "=cut".to_string()));
    }
    let ch = stream.next()?;
    if ch == '"' || ch == '\'' {
        // NOTE: ' or " or <<'SOMETHING'\n...\nSOMETHING\n or <<"SOMETHING"\n...\nSOMETHING\n
        let p3 = prefix_n(stream, 3);
        if p3.len() == 3 && p3.starts_with("<<") && p3.ends_with(ch) {
            let p = stream.pos;
            stream.eat_while_if(is_w);
            let n = stream.slice(stream.start + 1, stream.pos).to_string();
            if !n.is_empty() && stream.eat(ch).is_some() {
                return Some(token_something(stream, state, n));
            }
            stream.pos = p;
        }
        return Some(token_chain(
            stream,
            state,
            [Some(ch), None],
            "string",
            false,
        ));
    }
    if ch == 'q' && !look(stream, -2).is_some_and(is_w) {
        let result = match look(stream, 0) {
            Some('x') => quote_like(stream, state, RX_STYLE, true),
            Some('q') => quote_like(stream, state, "string", false),
            Some('w') => quote_like(stream, state, "bracket", false),
            Some('r') => quote_like(stream, state, RX_STYLE, true),
            Some(c) if closing(c).is_some() => {
                eat_suffix(stream, 1);
                let close = closing(c);
                Some(token_chain(stream, state, [close, None], "string", false))
            }
            Some(c) if is_quote_delim(c) => {
                let open = stream.eat(c);
                Some(token_chain(stream, state, [open, None], "string", false))
            }
            _ => None,
        };
        if result.is_some() {
            return result;
        }
    }
    if ch == 'm'
        && !look(stream, -2).is_some_and(is_w)
        && let Some(c) = stream.eat_if(is_delim)
    {
        let close = closing(c).unwrap_or(c);
        return Some(token_chain(
            stream,
            state,
            [Some(close), None],
            RX_STYLE,
            true,
        ));
    }
    if (ch == 's' || ch == 'y') && !after_term(stream) {
        let result = substitution(stream, state);
        if result.is_some() {
            return result;
        }
    }
    if ch == 't' && !after_term(stream) && stream.eat('r').is_some() {
        let result = substitution(stream, state);
        if result.is_some() {
            return result;
        }
    }
    if ch == '`' {
        return Some(token_chain(
            stream,
            state,
            [Some(ch), None],
            "variable-2",
            false,
        ));
    }
    if ch == '/' {
        if !re!(r"~\s*$").is_match(prefix(stream)).unwrap_or(false) {
            return Some("operator");
        }
        return Some(token_chain(stream, state, [Some(ch), None], RX_STYLE, true));
    }
    if ch == '$' {
        let p = stream.pos;
        if stream.eat_while_if(|c| c.is_ascii_digit())
            || (stream.eat('{').is_some()
                && stream.eat_while_if(|c| c.is_ascii_digit())
                && stream.eat('}').is_some())
        {
            return Some("variable-2");
        }
        stream.pos = p;
    }
    if matches!(ch, '$' | '@' | '%') {
        let p = stream.pos;
        if ((stream.eat('^').is_some() && stream.eat_if(|c| c.is_ascii_uppercase()).is_some())
            || (!look(stream, -2).is_some_and(|c| matches!(c, '@' | '$' | '%' | '&'))
                && stream.eat_if(is_special_var).is_some()))
            && perl_style(stream.current()).is_some()
        {
            return Some("variable-2");
        }
        stream.pos = p;
    }
    if matches!(ch, '$' | '@' | '%' | '&') {
        let var_char = |c: char| is_w(c) || c == '$';
        if stream.eat_while_if(var_char)
            || (stream.eat('{').is_some()
                && stream.eat_while_if(var_char)
                && stream.eat('}').is_some())
        {
            return Some(if perl_style(stream.current()).is_some() {
                "variable-2"
            } else {
                "variable"
            });
        }
    }
    if ch == '#' && look(stream, -2) != Some('$') {
        stream.skip_to_end();
        return Some("comment");
    }
    if is_op(ch) {
        let p = stream.pos;
        stream.eat_while_if(is_op);
        if perl_style(stream.current()).is_some() {
            return Some("operator");
        }
        stream.pos = p;
    }
    if ch == '_' && stream.pos == 1 {
        let style = if suffix(stream, 6) == "_END__" {
            Some("comment")
        } else if suffix(stream, 7) == "_DATA__" {
            Some("variable-2")
        } else if suffix(stream, 7) == "_C__" {
            Some("string")
        } else {
            None
        };
        if let Some(style) = style {
            return Some(token_chain(stream, state, [Some('\0'), None], style, false));
        }
    }
    if is_w(ch) {
        let p = stream.pos;
        if look(stream, -2) == Some('{')
            && (look(stream, 0) == Some('}')
                || (stream.eat_while_if(is_w) && look(stream, 0) == Some('}')))
        {
            return Some("string");
        }
        stream.pos = p;
    }
    if ch.is_ascii_uppercase() {
        let l = look(stream, -2);
        let p = stream.pos;
        stream.eat_while_if(|c| c.is_ascii_uppercase() || c == '_');
        if look(stream, 0).is_some_and(|c| c.is_ascii_digit() || c.is_ascii_lowercase()) {
            stream.pos = p;
        } else {
            return Some(word_style(stream.current(), l));
        }
    }
    if ch.is_ascii_alphabetic() || ch == '_' {
        let l = look(stream, -2);
        stream.eat_while_if(is_w);
        return Some(word_style(stream.current(), l));
    }
    None
}

pub struct Perl;

impl Mode for Perl {
    fn name(&self) -> &'static str {
        "perl"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(PerlState {
            tokenize: Tokenize::Perl,
            chain: None,
            style: "",
            tail: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = state::<PerlState>(st);
        let style = match &state.tokenize {
            Tokenize::Perl => token_perl(stream, state),
            &Tokenize::Chain { chain, style, tail } => {
                Some(chain_closure(stream, state, chain, style, tail))
            }
            Tokenize::Something(_) => Some(something_closure(stream, state)),
        };
        style.map(str::to_string)
    }
}
