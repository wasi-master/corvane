//! `codemirror/mode/rpm/rpm.js`: the `rpm-spec` mode (`text/x-rpm-spec`,
//! which GHD maps `.rpm` to) and the `rpm-changes` mode
//! (`text/x-rpm-changes`, no GHD extension), ported line by line.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

/// `CodeMirror.defineMode("rpm-changes", ...)`
pub struct RpmChanges;

impl Mode for RpmChanges {
    fn name(&self) -> &'static str {
        "rpm-changes"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(())
    }

    // token
    fn token(&self, stream: &mut StringStream, _state: &mut dyn ModeState) -> Option<String> {
        if stream.sol() {
            // headerSeparator
            if stream.matches(re!(r"^-+$")) {
                return Some("tag".into());
            }
            // headerLine
            if stream.matches(re!(
                r"^(Mon|Tue|Wed|Thu|Fri|Sat|Sun) (Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)  ?[0-9]{1,2} [0-9]{2}:[0-9]{2}(:[0-9]{2})? [A-Z]{3,4} [0-9]{4} - "
            )) {
                return Some("tag".into());
            }
        }
        // simpleEmail
        if stream.matches(re!(r"^[A-Za-z0-9_+.-]+@[A-Za-z0-9_.-]+")) {
            return Some("string".into());
        }
        stream.next();
        None
    }
}

/// `CodeMirror.defineMode("rpm-spec", ...)`
pub struct RpmSpec;

#[derive(Clone)]
struct SpecState {
    control_flow: bool,
    macro_parameters: bool,
}

impl Mode for RpmSpec {
    fn name(&self) -> &'static str {
        "rpm-spec"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(SpecState {
            control_flow: false,
            macro_parameters: false,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<SpecState>(st);
        let style = spec_token(stream, s);
        style.map(Into::into)
    }
}

fn spec_token(stream: &mut StringStream, s: &mut SpecState) -> Option<&'static str> {
    if stream.peek() == Some('#') {
        stream.skip_to_end();
        return Some("comment");
    }
    if stream.sol() {
        // preamble
        if stream.matches(re!(r"^[a-zA-Z0-9()]+:")) {
            return Some("header");
        }
        // section
        if stream.matches(re!(
            r"^%(debug_package|package|description|prep|build|install|files|clean|changelog|preinstall|preun|postinstall|postun|pretrans|posttrans|pre|post|triggerin|triggerun|verifyscript|check|triggerpostun|triggerprein|trigger)"
        )) {
            return Some("atom");
        }
    }
    // Variables like '$RPM_BUILD_ROOT'
    if stream.matches(re!(r"^\$[A-Za-z0-9_]+")) {
        return Some("def");
    }
    // Variables like '${RPM_BUILD_ROOT}'
    if stream.matches(re!(r"^\$\{[A-Za-z0-9_]+\}")) {
        return Some("def");
    }
    // control_flow_simple
    if stream.matches(re!(r"^%(else|endif)")) {
        return Some("keyword");
    }
    // control_flow_complex
    if stream.matches(re!(r"^%(ifnarch|ifarch|if)")) {
        s.control_flow = true;
        return Some("keyword");
    }
    if s.control_flow {
        // operators
        if stream.matches(re!(r"^(!|\?|<=|<|>=|>|==|&&|\|\|)")) {
            return Some("operator");
        }
        if stream.matches(re!(r"^([0-9]+)")) {
            return Some("number");
        }
        if stream.eol() {
            s.control_flow = false;
        }
    }
    // arch
    if stream.matches(re!(
        r"^(i386|i586|i686|x86_64|ppc64le|ppc64|ppc|ia64|s390x|s390|sparc64|sparcv9|sparc|noarch|alphaev6|alpha|hppa|mipsel)"
    )) {
        if stream.eol() {
            s.control_flow = false;
        }
        return Some("number");
    }
    // Macros like '%make_install' or '%attr(0775,root,root)'
    if stream.matches(re!(r"^%[A-Za-z0-9_]+")) {
        if stream.match_str("(", true, false) {
            s.macro_parameters = true;
        }
        return Some("keyword");
    }
    if s.macro_parameters {
        if stream.matches(re!(r"^[0-9]+")) {
            return Some("number");
        }
        if stream.match_str(")", true, false) {
            s.macro_parameters = false;
            return Some("keyword");
        }
    }
    // Macros like '%{defined fedora}'
    if stream.matches(re!(r"^%\{\??[A-Za-z0-9_ \-:!]+\}")) {
        if stream.eol() {
            s.control_flow = false;
        }
        return Some("def");
    }
    stream.next();
    None
}
