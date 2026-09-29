//! `codemirror/mode/puppet/puppet.js` (`text/x-puppet`), ported line by
//! line. GHD maps `.pp` to it.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct Puppet;

#[derive(Clone)]
struct PuppetState {
    in_definition: bool,
    in_include: bool,
    continue_string: bool,
    /// `state.pending`: the open quote (`false` → `'\0'`)
    pending: char,
}

/// `words`, filled by `define(style, string)`; later calls win, so `file`
/// ends up `builtin`, not `atom`.
fn word_style(w: &str) -> Option<&'static str> {
    Some(match w {
        "class" | "define" | "site" | "node" | "include" | "import" | "inherits" | "case"
        | "if" | "else" | "in" | "and" | "elsif" | "default" | "or" => "keyword",
        "false" | "true" | "running" | "present" | "absent" | "directory" | "undef" => "atom",
        "action"
        | "augeas"
        | "burst"
        | "chain"
        | "computer"
        | "cron"
        | "destination"
        | "dport"
        | "exec"
        | "file"
        | "filebucket"
        | "group"
        | "host"
        | "icmp"
        | "iniface"
        | "interface"
        | "jump"
        | "k5login"
        | "limit"
        | "log_level"
        | "log_prefix"
        | "macauthorization"
        | "mailalias"
        | "maillist"
        | "mcx"
        | "mount"
        | "nagios_command"
        | "nagios_contact"
        | "nagios_contactgroup"
        | "nagios_host"
        | "nagios_hostdependency"
        | "nagios_hostescalation"
        | "nagios_hostextinfo"
        | "nagios_hostgroup"
        | "nagios_service"
        | "nagios_servicedependency"
        | "nagios_serviceescalation"
        | "nagios_serviceextinfo"
        | "nagios_servicegroup"
        | "nagios_timeperiod"
        | "name"
        | "notify"
        | "outiface"
        | "package"
        | "proto"
        | "reject"
        | "resources"
        | "router"
        | "schedule"
        | "scheduled_task"
        | "selboolean"
        | "selmodule"
        | "service"
        | "source"
        | "sport"
        | "ssh_authorized_key"
        | "sshkey"
        | "stage"
        | "state"
        | "table"
        | "tidy"
        | "todest"
        | "toports"
        | "tosource"
        | "user"
        | "vlan"
        | "yumrepo"
        | "zfs"
        | "zone"
        | "zpool" => "builtin",
        _ => return None,
    })
}

// tokenString
fn token_string(stream: &mut StringStream, s: &mut PuppetState) -> &'static str {
    let mut current = None;
    let mut prev = None;
    let mut found_var = false;
    while !stream.eol() {
        current = stream.next();
        if current == Some(s.pending) {
            break;
        }
        if current == Some('$') && prev != Some('\\') && s.pending == '"' {
            found_var = true;
            break;
        }
        prev = current;
    }
    if found_var {
        stream.back_up(1);
    }
    s.continue_string = current != Some(s.pending);
    "string"
}

// tokenize
fn tokenize(stream: &mut StringStream, s: &mut PuppetState) -> Option<&'static str> {
    // Matches one whole word
    let word = stream.match_re(re!(r"^(?:[A-Za-z0-9_]+)"), false);
    // Matches attributes (i.e. ensure => present ; 'ensure' would be matched)
    let attribute = stream
        .match_re(
            re!(r"^(?:(\s+)?[A-Za-z0-9_]+\s+=>[^\n\r\x{2028}\x{2029}]*)"),
            false,
        )
        .is_some();
    // Matches non-builtin resource declarations
    let resource = stream
        .match_re(re!(r"^(?:(\s+)?[A-Za-z0-9_:]+(\s+)?\{)"), false)
        .is_some();
    // Matches virtual and exported resources (i.e. @@user { ; and the like)
    let special_resource = stream
        .match_re(re!(r"^(?:(\s+)?[@]{1,2}[A-Za-z0-9_:]+(\s+)?\{)"), false)
        .is_some();

    // Finally advance the stream
    let ch = stream.next()?;

    // Have we found a variable?
    if ch == '$' {
        if stream.matches(re!(
            r"^(?:(\{)?([a-z][a-z0-9_]*)?((::[a-z][a-z0-9_]*)*::)?[a-zA-Z0-9_]+(\})?)"
        )) {
            return Some(if s.continue_string {
                "variable-2"
            } else {
                "variable"
            });
        }
        return Some("error");
    }
    // Should we still be looking for the end of a string?
    if s.continue_string {
        stream.back_up(1);
        return Some(token_string(stream, s));
    }
    // Are we in a definition (class, node, define)?
    if s.in_definition {
        if stream.matches(re!(r"^(?:(\s+)?[A-Za-z0-9_:]+(\s+)?)")) {
            return Some("def");
        }
        // Match the rest it the next time around
        stream.matches(re!(r"^(?:\s+\{)"));
        s.in_definition = false;
    }
    // Are we in an 'include' statement?
    if s.in_include {
        stream.matches(re!(r"^(?:(\s+)?\S+(\s+)?)"));
        s.in_include = false;
        return Some("def");
    }
    // Do we just have a function on our hands?
    if stream.matches(re!(r"^(?:(\s+)?[A-Za-z0-9_]+\()")) {
        stream.back_up(1);
        return Some("def");
    }
    // Have we matched the prior attribute regex?
    if attribute {
        stream.matches(re!(r"^(?:(\s+)?[A-Za-z0-9_]+)"));
        return Some("tag");
    }
    let word = word.map(|m| m.text);
    // Do we have Puppet specific words?
    if let Some(style) = word.as_deref().and_then(word_style) {
        stream.back_up(1);
        stream.matches(re!(r"^(?:[A-Za-z0-9_]+)"));
        if stream.match_re(re!(r"^(?:\s+\S+\s+\{)"), false).is_some() {
            s.in_definition = true;
        }
        if word.as_deref() == Some("include") {
            s.in_include = true;
        }
        return Some(style);
    }
    // Is there a match on a reference? (`test(null)` tests "null")
    if re!(r"^(?:(^|\s+)[A-Z][A-Za-z0-9_:]+)")
        .is_match(word.as_deref().unwrap_or("null"))
        .unwrap_or(false)
    {
        stream.back_up(1);
        stream.matches(re!(r"^(?:(^|\s+)[A-Z][A-Za-z0-9_:]+)"));
        return Some("def");
    }
    // Have we matched the prior resource regex?
    if resource {
        stream.matches(re!(r"^(?:(\s+)?[A-Za-z0-9_:]+)"));
        return Some("def");
    }
    // Have we matched the prior special_resource regex?
    if special_resource {
        stream.matches(re!(r"^(?:(\s+)?[@]{1,2})"));
        return Some("special");
    }
    // Match all the comments. All of them.
    if ch == '#' {
        stream.skip_to_end();
        return Some("comment");
    }
    // Have we found a string?
    if ch == '\'' || ch == '"' {
        s.pending = ch;
        return Some(token_string(stream, s));
    }
    if ch == '{' || ch == '}' {
        return Some("bracket");
    }
    // Match characters that we are going to assume are trying to be regex
    if ch == '/' {
        stream.matches(re!(r"^[^/]*/"));
        return Some("variable-3");
    }
    if ch.is_ascii_digit() {
        stream.eat_while_if(|c| c.is_ascii_digit());
        return Some("number");
    }
    // Match the '=' and '=>' operators
    if ch == '=' {
        if stream.peek() == Some('>') {
            stream.next();
        }
        return Some("operator");
    }
    // Keep advancing through all the rest
    stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    None
}

impl Mode for Puppet {
    fn name(&self) -> &'static str {
        "puppet"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(PuppetState {
            in_definition: false,
            in_include: false,
            continue_string: false,
            pending: '\0',
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<PuppetState>(st);
        // Strip the spaces, but regex will account for them eitherway
        if stream.eat_space() {
            return None;
        }
        tokenize(stream, s).map(Into::into)
    }
}
