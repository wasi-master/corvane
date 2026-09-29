//! `codemirror/mode/dart/dart.js`: `application/dart`, a clike config
//! (`CodeMirror.defineMIME("application/dart", {name: "clike", …})`) whose
//! hooks add triple-quoted / raw strings with `$name` and `${…}`
//! interpolation, nested block comments, `@` metadata and capitalised
//! identifiers as variable-2. The mode's name stays `clike` (`m-clike`).
//!
//! The hooks and tokenizers here run on [`clike`]'s state: the
//! `state.tokenize` closures are the `Tokenize::Dart*` variants and
//! `state.interpolationStack` is `State::interpolation_stack`.

use std::sync::{Arc, OnceLock};

use super::clike::{self, Config, Hook, State, Style, TokenHook, Tokenize, Words};
use crate::cm::{Mode, StringStream};

/// The `application/dart` config.
fn config() -> Config {
    Config {
        keywords: Words::of(&[
            "this super static final const abstract class extends external factory \
implements mixin get native set typedef with enum throw rethrow assert break case \
continue default in return new deferred async await covariant try catch finally \
do else for if switch while import library export part of show hide is as extension \
on yield late required sealed base interface when",
        ]),
        block_keywords: Words::of(&["try catch finally do else for if switch while"]),
        builtin: Words::of(&["void bool num int double dynamic var String Null Never"]),
        atoms: Words::of(&["true false null"]),
        hooks: |ch| match ch {
            '@' => Some(Hook::DartAt),
            '\'' | '"' => Some(Hook::DartQuote),
            'r' => Some(Hook::DartRaw),
            '}' => Some(Hook::DartCloseBrace),
            '/' => Some(Hook::NestedComment),
            _ => None,
        },
        token_hook: Some(TokenHook::Dart),
        ..Config::default()
    }
}

/// `application/dart`
pub fn dart() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| Arc::new(clike::with_config(config())))
        .clone()
}

/// `hooks.token`: `/^[_$]*[A-Z][a-zA-Z0-9_$]*$/` ("assume uppercase
/// symbols are classes").
pub(super) fn is_upper(word: &str) -> bool {
    let rest = word.trim_start_matches(['_', '$']);
    let mut chars = rest.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// `tokenString(quote, stream, state, raw)`
pub(super) fn token_string(
    quote: char,
    stream: &mut StringStream,
    state: &mut State,
    raw: bool,
) -> Style {
    let mut triple = false;
    if stream.eat(quote).is_some() {
        if stream.eat(quote).is_some() {
            triple = true;
        } else {
            // empty string
            return Some("string");
        }
    }
    state.tokenize = Tokenize::DartString { quote, raw, triple };
    token_string_helper(quote, raw, triple, stream, state)
}

/// `tokenStringHelper` (the closure `tokenString` installs)
pub(super) fn token_string_helper(
    quote: char,
    raw: bool,
    triple: bool,
    stream: &mut StringStream,
    state: &mut State,
) -> Style {
    let mut escaped = false;
    while !stream.eol() {
        if !raw && !escaped && stream.peek() == Some('$') {
            // pushInterpolationStack
            state.interpolation_stack.push(state.tokenize);
            state.tokenize = Tokenize::DartInterpolation;
            return Some("string");
        }
        let next = stream.next();
        if next == Some(quote)
            && !escaped
            && (!triple
                || (stream.peek() == Some(quote)
                    && stream.char_at(stream.pos + 1) == Some(quote)
                    && {
                        stream.pos += 2;
                        true
                    }))
        {
            state.tokenize = Tokenize::Base;
            break;
        }
        escaped = !raw && !escaped && next == Some('\\');
    }
    Some("string")
}

/// `tokenInterpolation`: `${` hands back to clike until the matching `}`
/// hook, `$name` reads one identifier.
pub(super) fn token_interpolation(stream: &mut StringStream, state: &mut State) -> Style {
    stream.eat('$');
    state.tokenize = if stream.eat('{').is_some() {
        Tokenize::Base
    } else {
        Tokenize::DartInterpolationIdentifier
    };
    None
}

/// `tokenInterpolationIdentifier`
pub(super) fn token_interpolation_identifier(
    stream: &mut StringStream,
    state: &mut State,
) -> Style {
    stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_');
    // popInterpolationStack (an empty stack gives `undefined` = tokenBase)
    state.tokenize = state.interpolation_stack.pop().unwrap_or(Tokenize::Base);
    Some("variable")
}

#[cfg(test)]
mod tests {
    use super::is_upper;

    #[test]
    fn capitalised_identifiers() {
        assert!(is_upper("Widget"));
        assert!(is_upper("_\u{24}Foo_1"));
        assert!(!is_upper("widget"));
        assert!(!is_upper("_"));
        assert!(!is_upper("Wid-get"));
    }
}
