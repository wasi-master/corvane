//! `codemirror/mode/powershell/powershell.js` (`application/x-powershell`).
//!
//! Ported line by line; the JS function names are kept on the Rust ones.
//! `state.tokenize` is a function in JS; here it is the [`Tokenize`] enum.
//! `state.returnStack` entries carry a `shouldReturnFrom` closure that
//! either always returns true or compares `state.bracketNesting` to a saved
//! value: [`Return::nesting`] is that value (`None` for "always").
//!
//! The `buildRegexp` results are spelled out below from their JS `source`.
//! JS `\d` / `\b` are ASCII there, written as `[0-9]` and ASCII lookarounds;
//! the `notCharacterOrDash` lookahead and `\b` run case-sensitively like
//! JS's (`(?-i:…)`). Rust's `(?i)` still folds `ſ` / `K` (Kelvin) onto
//! `s` / `k` inside the word lists, which JS does not.

use super::super::{Mode, ModeState, StringStream, state};
use crate::re;

/// JS `\b` (ASCII) after a word
macro_rules! wb {
    () => {
        r"(?-i:(?<=[A-Za-z0-9_])(?![A-Za-z0-9_])|(?<![A-Za-z0-9_])(?=[A-Za-z0-9_]))"
    };
}

/// `notCharacterOrDash`
macro_rules! not_character_or_dash {
    () => {
        r"(?-i:(?=[^A-Za-z0-9\-_]|$))"
    };
}

/// `state.tokenize`
#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    Base,
    DoubleQuoteString,
    StringInterpolation,
    MultiStringReturn,
    HereStringInterpolation,
    Comment,
    Variable,
    VariableWithBraces,
    MultiString,
}

/// A `state.returnStack` entry.
#[derive(Clone)]
struct Return {
    /// `shouldReturnFrom`: `state.bracketNesting === savedBracketNesting`,
    /// or always when `None`
    nesting: Option<i32>,
    tokenize: Tokenize,
}

#[derive(Clone)]
struct PsState {
    return_stack: Vec<Return>,
    bracket_nesting: i32,
    tokenize: Tokenize,
    start_quote: char,
}

/// `varNames`: `/[\w\-:]/`
fn is_var_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':')
}

/// `grammar`, tried in key order; the key is the style.
fn match_grammar(stream: &mut StringStream) -> Option<&'static str> {
    // keyword
    if stream.matches(re!(concat!(
        r"(?i)^(begin|break|catch|continue|data|default|do|dynamicparam|else|elseif|end|exit|filter|finally|for|foreach|from|function|if|in|param|process|return|switch|throw|trap|try|until|where|while)",
        not_character_or_dash!()
    ))) {
        return Some("keyword");
    }
    // number
    if stream.matches(re!(
        r"(?i)^((0x[0-9a-f]+)|(([0-9]+\.[0-9]+|[0-9]\.|\.[0-9]+|[0-9]+)(e[+\-]?[0-9]+)?))[ld]?([kmgtp]b)?"
    )) {
        return Some("number");
    }
    // operator
    if stream.matches(re!(concat!(
        r"(?i)^(-(f|b?not|[ic]?split|join|is(not)?|as|[ic]?(eq|ne|[gl][te])|[ic]?(not)?(like|match|contains)|[ic]?replace|b?(and|or|xor))",
        wb!(),
        r"|[+\-*/%]=|\+\+|--|\.\.|[+\-*&^%:=!|/]|<(?!#)|(?!#)>)"
    ))) {
        return Some("operator");
    }
    // builtin
    if stream.matches(re!(concat!(
        r"(?i)^([A-Z]:|%|\?|(",
        r"Add-(Computer|Content|History|Member|PSSnapin|Type)|Checkpoint-Computer|Clear-(Content|EventLog|History|Host|Item(Property)?|Variable)|Compare-Object|Complete-Transaction|Connect-PSSession|ConvertFrom-(Csv|Json|SecureString|StringData)|Convert-Path|ConvertTo-(Csv|Html|Json|SecureString|Xml)|Copy-Item(Property)?|Debug-Process|Disable-(ComputerRestore|PSBreakpoint|PSRemoting|PSSessionConfiguration)|Disconnect-PSSession|Enable-(ComputerRestore|PSBreakpoint|PSRemoting|PSSessionConfiguration)|(Enter|Exit)-PSSession|Export-(Alias|Clixml|Console|Counter|Csv|FormatData|ModuleMember|PSSession)|ForEach-Object|Format-(Custom|List|Table|Wide)|",
        r"Get-(Acl|Alias|AuthenticodeSignature|ChildItem|Command|ComputerRestorePoint|Content|ControlPanelItem|Counter|Credential|Culture|Date|Event|EventLog|EventSubscriber|ExecutionPolicy|FormatData|Help|History|Host|HotFix|Item|ItemProperty|Job|Location|Member|Module|PfxCertificate|Process|PSBreakpoint|PSCallStack|PSDrive|PSProvider|PSSession|PSSessionConfiguration|PSSnapin|Random|Service|TraceSource|Transaction|TypeData|UICulture|Unique|Variable|Verb|WinEvent|WmiObject)|",
        r"Group-Object|Import-(Alias|Clixml|Counter|Csv|LocalizedData|Module|PSSession)|ImportSystemModules|Invoke-(Command|Expression|History|Item|RestMethod|WebRequest|WmiMethod)|Join-Path|Limit-EventLog|Measure-(Command|Object)|Move-Item(Property)?|",
        r"New-(Alias|Event|EventLog|Item(Property)?|Module|ModuleManifest|Object|PSDrive|PSSession|PSSessionConfigurationFile|PSSessionOption|PSTransportOption|Service|TimeSpan|Variable|WebServiceProxy|WinEvent)|",
        r"Out-(Default|File|GridView|Host|Null|Printer|String)|Pause|(Pop|Push)-Location|Read-Host|Receive-(Job|PSSession)|Register-(EngineEvent|ObjectEvent|PSSessionConfiguration|WmiEvent)|Remove-(Computer|Event|EventLog|Item(Property)?|Job|Module|PSBreakpoint|PSDrive|PSSession|PSSnapin|TypeData|Variable|WmiObject)|Rename-(Computer|Item(Property)?)|Reset-ComputerMachinePassword|Resolve-Path|Restart-(Computer|Service)|Restore-Computer|Resume-(Job|Service)|Save-Help|Select-(Object|String|Xml)|Send-MailMessage|",
        r"Set-(Acl|Alias|AuthenticodeSignature|Content|Date|ExecutionPolicy|Item(Property)?|Location|PSBreakpoint|PSDebug|PSSessionConfiguration|Service|StrictMode|TraceSource|Variable|WmiInstance)|",
        r"Show-(Command|ControlPanelItem|EventLog)|Sort-Object|Split-Path|Start-(Job|Process|Service|Sleep|Transaction|Transcript)|Stop-(Computer|Job|Process|Service|Transcript)|Suspend-(Job|Service)|TabExpansion2|Tee-Object|Test-(ComputerSecureChannel|Connection|ModuleManifest|Path|PSSessionConfigurationFile)|Trace-Command|Unblock-File|Undo-Transaction|Unregister-(Event|PSSessionConfiguration)|Update-(FormatData|Help|List|TypeData)|Use-Transaction|Wait-(Event|Job|Process)|Where-Object|Write-(Debug|Error|EventLog|Host|Output|Progress|Verbose|Warning)|",
        r"cd|help|mkdir|more|oss|prompt|",
        r"ac|asnp|cat|cd|chdir|clc|clear|clhy|cli|clp|cls|clv|cnsn|compare|copy|cp|cpi|cpp|cvpa|dbp|del|diff|dir|dnsn|ebp|",
        r"echo|epal|epcsv|epsn|erase|etsn|exsn|fc|fl|foreach|ft|fw|gal|gbp|gc|gci|gcm|gcs|gdr|ghy|gi|gjb|gl|gm|gmo|gp|gps|",
        r"group|gsn|gsnp|gsv|gu|gv|gwmi|h|history|icm|iex|ihy|ii|ipal|ipcsv|ipmo|ipsn|irm|ise|iwmi|iwr|kill|lp|ls|man|md|",
        r"measure|mi|mount|move|mp|mv|nal|ndr|ni|nmo|npssc|nsn|nv|ogv|oh|popd|ps|pushd|pwd|r|rbp|rcjb|rcsn|rd|rdr|ren|ri|",
        r"rjb|rm|rmdir|rmo|rni|rnp|rp|rsn|rsnp|rujb|rv|rvpa|rwmi|sajb|sal|saps|sasv|sbp|sc|select|set|shcm|si|sl|sleep|sls|",
        r"sort|sp|spjb|spps|spsv|start|sujb|sv|swmi|tee|trcm|type|where|wjb|write",
        r")|\$(",
        r"[$?^_]|Args|ConfirmPreference|ConsoleFileName|DebugPreference|Error|ErrorActionPreference|ErrorView|ExecutionContext|",
        r"FormatEnumerationLimit|Home|Host|Input|MaximumAliasCount|MaximumDriveCount|MaximumErrorCount|MaximumFunctionCount|",
        r"MaximumHistoryCount|MaximumVariableCount|MyInvocation|NestedPromptLevel|OutputEncoding|Pid|Profile|ProgressPreference|",
        r"PSBoundParameters|PSCommandPath|PSCulture|PSDefaultParameterValues|PSEmailServer|PSHome|PSScriptRoot|PSSessionApplicationName|",
        r"PSSessionConfigurationName|PSSessionOption|PSUICulture|PSVersionTable|Pwd|ShellId|StackTrace|VerbosePreference|",
        r"WarningPreference|WhatIfPreference|",
        r"Event|EventArgs|EventSubscriber|Sender|",
        r"Matches|Ofs|ForEach|LastExitCode|PSCmdlet|PSItem|PSSenderInfo|This|",
        r"true|false|null",
        r"))",
        not_character_or_dash!()
    ))) {
        return Some("builtin");
    }
    // punctuation
    if stream.matches(re!(r"[\[\]{},;`\\.]|@[({]")) {
        return Some("punctuation");
    }
    // identifier
    if stream.matches(re!(concat!(r"^[A-Za-z_][A-Za-z\-_0-9]*", wb!()))) {
        return Some("identifier");
    }
    None
}

fn tokenize(stream: &mut StringStream, state: &mut PsState) -> Option<&'static str> {
    match state.tokenize {
        Tokenize::Base => token_base(stream, state),
        Tokenize::DoubleQuoteString => token_double_quote_string(stream, state),
        Tokenize::StringInterpolation => token_string_interpolation(stream, state),
        Tokenize::MultiStringReturn => token_multi_string_return(stream, state),
        Tokenize::HereStringInterpolation => token_here_string_interpolation(stream, state),
        Tokenize::Comment => token_comment(stream, state),
        Tokenize::Variable => token_variable(stream, state),
        Tokenize::VariableWithBraces => token_variable_with_braces(stream, state),
        Tokenize::MultiString => token_multi_string(stream, state),
    }
}

// tokenBase
fn token_base(stream: &mut StringStream, state: &mut PsState) -> Option<&'static str> {
    if let Some(parent) = state.return_stack.last()
        && parent
            .nesting
            .is_none_or(|saved| state.bracket_nesting == saved)
    {
        state.tokenize = parent.tokenize;
        state.return_stack.pop();
        return tokenize(stream, state);
    }

    if stream.eat_space() {
        return None;
    }

    if stream.eat('(').is_some() {
        state.bracket_nesting += 1;
        return Some("punctuation");
    }

    if stream.eat(')').is_some() {
        state.bracket_nesting -= 1;
        return Some("punctuation");
    }

    if let Some(style) = match_grammar(stream) {
        return Some(style);
    }

    let ch = stream.next();

    // single-quote string
    if ch == Some('\'') {
        return Some(token_single_quote_string(stream, state));
    }

    if ch == Some('$') {
        return token_variable(stream, state);
    }

    // double-quote string
    if ch == Some('"') {
        return token_double_quote_string(stream, state);
    }

    if ch == Some('<') && stream.eat('#').is_some() {
        state.tokenize = Tokenize::Comment;
        return token_comment(stream, state);
    }

    if ch == Some('#') {
        stream.skip_to_end();
        return Some("comment");
    }

    if ch == Some('@') {
        let quote_match = stream.eat_if(|c| c == '"' || c == '\'');
        if let Some(quote) = quote_match
            && stream.eol()
        {
            state.tokenize = Tokenize::MultiString;
            state.start_quote = quote;
            return token_multi_string(stream, state);
        } else if stream.eol() {
            return Some("error");
        } else if matches!(stream.peek(), Some('(' | '{')) {
            return Some("punctuation");
        } else if stream.peek().is_some_and(is_var_name) {
            // splatted variable
            return token_variable(stream, state);
        }
    }
    Some("error")
}

// tokenSingleQuoteString
fn token_single_quote_string(stream: &mut StringStream, state: &mut PsState) -> &'static str {
    while let Some(ch) = stream.next() {
        if ch == '\'' && stream.eat('\'').is_none() {
            state.tokenize = Tokenize::Base;
            return "string";
        }
    }
    "error"
}

// tokenDoubleQuoteString
fn token_double_quote_string(
    stream: &mut StringStream,
    state: &mut PsState,
) -> Option<&'static str> {
    while let Some(ch) = stream.peek() {
        if ch == '$' {
            state.tokenize = Tokenize::StringInterpolation;
            return Some("string");
        }

        stream.next();
        if ch == '`' {
            stream.next();
            continue;
        }

        if ch == '"' && stream.eat('"').is_none() {
            state.tokenize = Tokenize::Base;
            return Some("string");
        }
    }
    Some("error")
}

// tokenStringInterpolation
fn token_string_interpolation(
    stream: &mut StringStream,
    state: &mut PsState,
) -> Option<&'static str> {
    token_interpolation(stream, state, Tokenize::DoubleQuoteString)
}

// tokenMultiStringReturn
fn token_multi_string_return(
    stream: &mut StringStream,
    state: &mut PsState,
) -> Option<&'static str> {
    state.tokenize = Tokenize::MultiString;
    state.start_quote = '"';
    token_multi_string(stream, state)
}

// tokenHereStringInterpolation
fn token_here_string_interpolation(
    stream: &mut StringStream,
    state: &mut PsState,
) -> Option<&'static str> {
    token_interpolation(stream, state, Tokenize::MultiStringReturn)
}

// tokenInterpolation
fn token_interpolation(
    stream: &mut StringStream,
    state: &mut PsState,
    parent_tokenize: Tokenize,
) -> Option<&'static str> {
    if stream.match_str("$(", true, false) {
        state.return_stack.push(Return {
            nesting: Some(state.bracket_nesting),
            tokenize: parent_tokenize,
        });
        state.tokenize = Tokenize::Base;
        state.bracket_nesting += 1;
        Some("punctuation")
    } else {
        stream.next();
        state.return_stack.push(Return {
            nesting: None,
            tokenize: parent_tokenize,
        });
        state.tokenize = Tokenize::Variable;
        token_variable(stream, state)
    }
}

// tokenComment
fn token_comment(stream: &mut StringStream, state: &mut PsState) -> Option<&'static str> {
    let mut maybe_end = false;
    while let Some(ch) = stream.next() {
        if maybe_end && ch == '>' {
            state.tokenize = Tokenize::Base;
            break;
        }
        maybe_end = ch == '#';
    }
    Some("comment")
}

// tokenVariable
fn token_variable(stream: &mut StringStream, state: &mut PsState) -> Option<&'static str> {
    let ch = stream.peek();
    if stream.eat('{').is_some() {
        state.tokenize = Tokenize::VariableWithBraces;
        token_variable_with_braces(stream, state)
    } else if ch.is_some_and(is_var_name) {
        stream.eat_while_if(is_var_name);
        state.tokenize = Tokenize::Base;
        Some("variable-2")
    } else {
        state.tokenize = Tokenize::Base;
        Some("error")
    }
}

// tokenVariableWithBraces
fn token_variable_with_braces(
    stream: &mut StringStream,
    state: &mut PsState,
) -> Option<&'static str> {
    while let Some(ch) = stream.next() {
        if ch == '}' {
            state.tokenize = Tokenize::Base;
            break;
        }
    }
    Some("variable-2")
}

// tokenMultiString
fn token_multi_string(stream: &mut StringStream, state: &mut PsState) -> Option<&'static str> {
    let quote = state.start_quote;
    if stream.sol() && stream.peek() == Some(quote) && stream.char_at(stream.pos + 1) == Some('@') {
        stream.pos += 2;
        state.tokenize = Tokenize::Base;
    } else if quote == '"' {
        while !stream.eol() {
            let ch = stream.peek();
            if ch == Some('$') {
                state.tokenize = Tokenize::HereStringInterpolation;
                return Some("string");
            }

            stream.next();
            if ch == Some('`') {
                stream.next();
            }
        }
    } else {
        stream.skip_to_end();
    }

    Some("string")
}

pub struct PowerShell;

impl Mode for PowerShell {
    fn name(&self) -> &'static str {
        "powershell"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(PsState {
            return_stack: Vec::new(),
            bracket_nesting: 0,
            tokenize: Tokenize::Base,
            start_quote: '"',
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = state::<PsState>(st);
        tokenize(stream, state).map(str::to_string)
    }
}
