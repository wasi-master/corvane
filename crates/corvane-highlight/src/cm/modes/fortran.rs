//! `codemirror/mode/fortran/fortran.js` (`text/x-fortran`).
//!
//! Ported line by line. Words are looked up lower-cased with JS
//! `toLowerCase` semantics (Rust's `to_lowercase` agrees on the chars that
//! can reach the tables, e.g. the Kelvin sign → `k`).

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

#[derive(Clone)]
struct FortranState {
    /// `state.tokenize`: `None` = `tokenBase`, `Some(quote)` = `tokenString(quote)`
    tokenize: Option<char>,
}

/// `isOperatorChar = /[+\-*&=<>\/\:]/`
fn is_operator_char(c: char) -> bool {
    matches!(c, '+' | '-' | '*' | '&' | '=' | '<' | '>' | '/' | ':')
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The style of a word (`stream.current().toLowerCase()` looked up).
fn word_style(word: &str) -> &'static str {
    let lookup = |w: &str| {
        if is_keyword(w) {
            "keyword"
        } else if is_builtin(w) || is_data_type(w) {
            "builtin"
        } else {
            "variable"
        }
    };
    if word.is_ascii() {
        let mut buf = [0u8; 32];
        if word.len() > buf.len() {
            return "variable";
        }
        let buf = &mut buf[..word.len()];
        buf.copy_from_slice(word.as_bytes());
        buf.make_ascii_lowercase();
        lookup(std::str::from_utf8(buf).unwrap_or(""))
    } else {
        lookup(&word.to_lowercase())
    }
}

/// `tokenBase`
fn token_base(stream: &mut StringStream, s: &mut FortranState) -> Option<&'static str> {
    if stream.matches(re!(r"(?i)^\.(and|or|eq|lt|le|gt|ge|ne|not|eqv|neqv)\.")) {
        return Some("operator");
    }

    let ch = stream.next()?;
    if ch == '!' {
        stream.skip_to_end();
        return Some("comment");
    }
    if ch == '"' || ch == '\'' {
        s.tokenize = Some(ch);
        return token_string(ch, stream, s);
    }
    if matches!(ch, '[' | ']' | '(' | ')' | ',') {
        return None;
    }
    if ch.is_ascii_digit() {
        stream.eat_while_if(|c| is_word(c) || c == '.');
        return Some("number");
    }
    if is_operator_char(ch) {
        stream.eat_while_if(is_operator_char);
        return Some("operator");
    }
    stream.eat_while_if(|c| is_word(c) || c == '$');
    Some(word_style(stream.current()))
}

/// `tokenString(quote)`
fn token_string(
    quote: char,
    stream: &mut StringStream,
    s: &mut FortranState,
) -> Option<&'static str> {
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
        s.tokenize = None;
    }
    Some("string")
}

pub struct Fortran;

impl Mode for Fortran {
    fn name(&self) -> &'static str {
        "fortran"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(FortranState { tokenize: None })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<FortranState>(st);
        if stream.eat_space() {
            return None;
        }
        let style = match s.tokenize {
            None => token_base(stream, s),
            Some(q) => token_string(q, stream, s),
        };
        style.map(str::to_string)
    }
}

/// `keywords`
fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "abstract"
            | "accept"
            | "allocatable"
            | "allocate"
            | "array"
            | "assign"
            | "asynchronous"
            | "backspace"
            | "bind"
            | "block"
            | "byte"
            | "call"
            | "case"
            | "class"
            | "close"
            | "common"
            | "contains"
            | "continue"
            | "cycle"
            | "data"
            | "deallocate"
            | "decode"
            | "deferred"
            | "dimension"
            | "do"
            | "elemental"
            | "else"
            | "encode"
            | "end"
            | "endif"
            | "entry"
            | "enumerator"
            | "equivalence"
            | "exit"
            | "external"
            | "extrinsic"
            | "final"
            | "forall"
            | "format"
            | "function"
            | "generic"
            | "go"
            | "goto"
            | "if"
            | "implicit"
            | "import"
            | "include"
            | "inquire"
            | "intent"
            | "interface"
            | "intrinsic"
            | "module"
            | "namelist"
            | "non_intrinsic"
            | "non_overridable"
            | "none"
            | "nopass"
            | "nullify"
            | "open"
            | "optional"
            | "options"
            | "parameter"
            | "pass"
            | "pause"
            | "pointer"
            | "print"
            | "private"
            | "program"
            | "protected"
            | "public"
            | "pure"
            | "read"
            | "recursive"
            | "result"
            | "return"
            | "rewind"
            | "save"
            | "select"
            | "sequence"
            | "stop"
            | "subroutine"
            | "target"
            | "then"
            | "to"
            | "type"
            | "use"
            | "value"
            | "volatile"
            | "where"
            | "while"
            | "write"
    )
}

/// `builtins`
fn is_builtin(w: &str) -> bool {
    matches!(
        w,
        "abort"
            | "abs"
            | "access"
            | "achar"
            | "acos"
            | "adjustl"
            | "adjustr"
            | "aimag"
            | "aint"
            | "alarm"
            | "all"
            | "allocated"
            | "alog"
            | "amax"
            | "amin"
            | "amod"
            | "and"
            | "anint"
            | "any"
            | "asin"
            | "associated"
            | "atan"
            | "besj"
            | "besjn"
            | "besy"
            | "besyn"
            | "bit_size"
            | "btest"
            | "cabs"
            | "ccos"
            | "ceiling"
            | "cexp"
            | "char"
            | "chdir"
            | "chmod"
            | "clog"
            | "cmplx"
            | "command_argument_count"
            | "complex"
            | "conjg"
            | "cos"
            | "cosh"
            | "count"
            | "cpu_time"
            | "cshift"
            | "csin"
            | "csqrt"
            | "ctime"
            | "c_funloc"
            | "c_loc"
            | "c_associated"
            | "c_null_ptr"
            | "c_null_funptr"
            | "c_f_pointer"
            | "c_null_char"
            | "c_alert"
            | "c_backspace"
            | "c_form_feed"
            | "c_new_line"
            | "c_carriage_return"
            | "c_horizontal_tab"
            | "c_vertical_tab"
            | "dabs"
            | "dacos"
            | "dasin"
            | "datan"
            | "date_and_time"
            | "dbesj"
            | "dbesjn"
            | "dbesy"
            | "dbesyn"
            | "dble"
            | "dcos"
            | "dcosh"
            | "ddim"
            | "derf"
            | "derfc"
            | "dexp"
            | "digits"
            | "dim"
            | "dint"
            | "dlog"
            | "dmax"
            | "dmin"
            | "dmod"
            | "dnint"
            | "dot_product"
            | "dprod"
            | "dsign"
            | "dsinh"
            | "dsin"
            | "dsqrt"
            | "dtanh"
            | "dtan"
            | "dtime"
            | "eoshift"
            | "epsilon"
            | "erf"
            | "erfc"
            | "etime"
            | "exit"
            | "exp"
            | "exponent"
            | "extends_type_of"
            | "fdate"
            | "fget"
            | "fgetc"
            | "float"
            | "floor"
            | "flush"
            | "fnum"
            | "fputc"
            | "fput"
            | "fraction"
            | "fseek"
            | "fstat"
            | "ftell"
            | "gerror"
            | "getarg"
            | "get_command"
            | "get_command_argument"
            | "get_environment_variable"
            | "getcwd"
            | "getenv"
            | "getgid"
            | "getlog"
            | "getpid"
            | "getuid"
            | "gmtime"
            | "hostnm"
            | "huge"
            | "iabs"
            | "iachar"
            | "iand"
            | "iargc"
            | "ibclr"
            | "ibits"
            | "ibset"
            | "ichar"
            | "idate"
            | "idim"
            | "idint"
            | "idnint"
            | "ieor"
            | "ierrno"
            | "ifix"
            | "imag"
            | "imagpart"
            | "index"
            | "int"
            | "ior"
            | "irand"
            | "isatty"
            | "ishft"
            | "ishftc"
            | "isign"
            | "iso_c_binding"
            | "is_iostat_end"
            | "is_iostat_eor"
            | "itime"
            | "kill"
            | "kind"
            | "lbound"
            | "len"
            | "len_trim"
            | "lge"
            | "lgt"
            | "link"
            | "lle"
            | "llt"
            | "lnblnk"
            | "loc"
            | "log"
            | "logical"
            | "long"
            | "lshift"
            | "lstat"
            | "ltime"
            | "matmul"
            | "max"
            | "maxexponent"
            | "maxloc"
            | "maxval"
            | "mclock"
            | "merge"
            | "move_alloc"
            | "min"
            | "minexponent"
            | "minloc"
            | "minval"
            | "mod"
            | "modulo"
            | "mvbits"
            | "nearest"
            | "new_line"
            | "nint"
            | "not"
            | "or"
            | "pack"
            | "perror"
            | "precision"
            | "present"
            | "product"
            | "radix"
            | "rand"
            | "random_number"
            | "random_seed"
            | "range"
            | "real"
            | "realpart"
            | "rename"
            | "repeat"
            | "reshape"
            | "rrspacing"
            | "rshift"
            | "same_type_as"
            | "scale"
            | "scan"
            | "second"
            | "selected_int_kind"
            | "selected_real_kind"
            | "set_exponent"
            | "shape"
            | "short"
            | "sign"
            | "signal"
            | "sinh"
            | "sin"
            | "sleep"
            | "sngl"
            | "spacing"
            | "spread"
            | "sqrt"
            | "srand"
            | "stat"
            | "sum"
            | "symlnk"
            | "system"
            | "system_clock"
            | "tan"
            | "tanh"
            | "time"
            | "tiny"
            | "transfer"
            | "transpose"
            | "trim"
            | "ttynam"
            | "ubound"
            | "umask"
            | "unlink"
            | "unpack"
            | "verify"
            | "xor"
            | "zabs"
            | "zcos"
            | "zexp"
            | "zlog"
            | "zsin"
            | "zsqrt"
    )
}

/// `dataTypes`
fn is_data_type(w: &str) -> bool {
    matches!(
        w,
        "c_bool"
            | "c_char"
            | "c_double"
            | "c_double_complex"
            | "c_float"
            | "c_float_complex"
            | "c_funptr"
            | "c_int"
            | "c_int16_t"
            | "c_int32_t"
            | "c_int64_t"
            | "c_int8_t"
            | "c_int_fast16_t"
            | "c_int_fast32_t"
            | "c_int_fast64_t"
            | "c_int_fast8_t"
            | "c_int_least16_t"
            | "c_int_least32_t"
            | "c_int_least64_t"
            | "c_int_least8_t"
            | "c_intmax_t"
            | "c_intptr_t"
            | "c_long"
            | "c_long_double"
            | "c_long_double_complex"
            | "c_long_long"
            | "c_ptr"
            | "c_short"
            | "c_signed_char"
            | "c_size_t"
            | "character"
            | "complex"
            | "double"
            | "integer"
            | "logical"
            | "real"
    )
}
