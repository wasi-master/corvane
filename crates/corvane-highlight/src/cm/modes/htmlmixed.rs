//! `codemirror/mode/htmlmixed/htmlmixed.js`: HTML as the xml mode in
//! `htmlMode`, handing the bodies of `<script>` and `<style>` (and any tag
//! a configuration adds) to a nested mode chosen from the tag's `lang` /
//! `type` attributes (`defaultTags`, `findMatchingMode`).
//!
//! [`HtmlMixed::new`] takes the configuration nesting modes pass
//! (`tags`, `scriptTypes`, `allowMissingTagName`) and a resolver standing in
//! for `CodeMirror.getMode`: it maps a mode spec (name or MIME) to a port
//! only when the mode is loaded next to the nesting mode in GHD's
//! highlighter worker (each worker loads just the module GHD installs for
//! the MIME plus what it `require`s); anything else falls back to the
//! `null` mode ([`NullMode`], `text/plain`), as `getMode` does for an
//! unknown mode. `getMode` builds a fresh mode instance for every nested
//! block, so per-instance JS scratch (javascript's `type` / `content`)
//! starts over in each block; the ports keep it in their state, which is
//! created per block too.

use std::sync::Arc;

use fancy_regex::Regex;

use crate::re;

use super::super::{Mode, ModeState, StringStream, js_len, state, state_ref};
use super::xml::{XmlConfig, XmlMode, XmlState};

/// `CodeMirror.getMode(config, spec)` limited to the modes loaded with the
/// nesting mode; `None` means the spec's mode is not loaded (→ `null`).
pub type Resolver = fn(&str) -> Option<Arc<dyn Mode>>;

/// The `null` mode (`text/plain`): skips the line, no style.
pub struct NullMode;

impl Mode for NullMode {
    fn name(&self) -> &'static str {
        "null"
    }
    fn start_state(&self) -> Box<dyn ModeState> {
        // `CodeMirror.startState` of a mode without `startState` is `true`
        Box::new(())
    }
    fn token(&self, stream: &mut StringStream, _state: &mut dyn ModeState) -> Option<String> {
        stream.skip_to_end();
        None
    }
}

/// Rewrite a JS regex source written with the `i` flag so ASCII letters
/// outside classes and escapes match both cases, without `(?i)`'s Unicode
/// folding (`ſ`, the Kelvin sign) that JS's non-unicode `i` does not do.
/// Enough for the tag patterns htmlmixed and its users configure.
pub fn ascii_ci(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len() * 2);
    let mut chars = pattern.chars();
    let mut in_class = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                out.push(c);
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            }
            '[' if !in_class => {
                in_class = true;
                out.push(c);
            }
            ']' if in_class => {
                in_class = false;
                out.push(c);
            }
            c if c.is_ascii_alphabetic() && !in_class => {
                out.push('[');
                out.push(c.to_ascii_lowercase());
                out.push(c.to_ascii_uppercase());
                out.push(']');
            }
            c => out.push(c),
        }
    }
    out
}

fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("htmlmixed regex")
}

/// One `[attr, regexp, mode]` entry of a tag's list.
pub struct TagSpec {
    /// `getAttrRegexp(attr)` (`None` for the `[null, null, mode]` fallback)
    attr: Option<Regex>,
    test: Option<Regex>,
    mode: &'static str,
}

impl TagSpec {
    /// `[attr, /pattern/, mode]`; `pattern` is the regex source in Rust
    /// syntax (use [`ascii_ci`] for a JS `i` flag).
    pub fn new(attr: &str, pattern: &str, mode: &'static str) -> Self {
        Self::with_regex(attr, compile(pattern), mode)
    }
    /// `[attr, regexp, mode]` with a compiled regexp.
    pub fn with_regex(attr: &str, test: Regex, mode: &'static str) -> Self {
        Self {
            attr: Some(compile(&format!(
                r#"\s+{attr}\s*=\s*('|")?([^'"]+)('|")?\s*"#
            ))),
            test: Some(test),
            mode,
        }
    }
    /// `[null, null, mode]`
    pub fn fallback(mode: &'static str) -> Self {
        Self {
            attr: None,
            test: None,
            mode,
        }
    }
}

/// A tag whose body is handed to another mode.
struct Tag {
    name: String,
    specs: Vec<TagSpec>,
    /// `getTagRegexp(tagName, true)` / `getTagRegexp(tagName, false)`
    end_anchored: Regex,
    end: Regex,
}

/// `parserConfig` of htmlmixed.
#[derive(Default)]
pub struct HtmlMixedConfig {
    /// `tags`: extra specs per tag, put in front of the defaults
    pub tags: Vec<(&'static str, Vec<TagSpec>)>,
    /// `scriptTypes`: `{matches, mode}` pairs for `<script type>`
    pub script_types: Vec<(Regex, &'static str)>,
    pub allow_missing_tag_name: bool,
}

pub struct HtmlMixed {
    html: XmlMode,
    tags: Vec<Tag>,
    resolve: Resolver,
}

/// The nested mode running a tag's body.
#[derive(Clone)]
struct Local {
    mode: Arc<dyn Mode>,
    state: Box<dyn ModeState>,
    /// index into `HtmlMixed::tags`
    tag: usize,
}

#[derive(Clone)]
pub struct HtmlMixedState {
    html_state: XmlState,
    /// `state.inTag`: `"<tag> <attributes text so far>"`
    in_tag: Option<String>,
    /// `state.localMode` / `state.localState`; `state.token` is the local
    /// function while this is set, `html` otherwise
    local: Option<Local>,
}

/// `defaultTags`
fn default_tags() -> Vec<(&'static str, Vec<TagSpec>)> {
    vec![
        (
            "script",
            vec![
                TagSpec::new("lang", &ascii_ci("(javascript|babel)"), "javascript"),
                TagSpec::new(
                    "type",
                    &ascii_ci(r"^(?:text|application)\/(?:x-)?(?:java|ecma)script$|^module$|^$"),
                    "javascript",
                ),
                TagSpec::new("type", ".", "text/plain"),
                TagSpec::fallback("javascript"),
            ],
        ),
        (
            "style",
            vec![
                TagSpec::new("lang", &ascii_ci("^css$"), "css"),
                TagSpec::new(
                    "type",
                    &ascii_ci(r"^(text\/)?(x-)?(stylesheet|css)$"),
                    "css",
                ),
                TagSpec::new("type", ".", "text/plain"),
                TagSpec::fallback("css"),
            ],
        ),
    ]
}

/// JS `\s` (trim in `getAttrValue`)
fn is_js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

impl HtmlMixed {
    /// `CodeMirror.defineMode("htmlmixed", …)` with `parserConfig`.
    pub fn new(config: HtmlMixedConfig, resolve: Resolver) -> Self {
        // addTags(defaultTags, tags); addTags(configTags, tags)
        let mut tags: Vec<(&'static str, Vec<TagSpec>)> = default_tags();
        for (name, specs) in config.tags {
            match tags.iter_mut().find(|(n, _)| *n == name) {
                Some((_, dest)) => {
                    let rest = std::mem::take(dest);
                    *dest = specs;
                    dest.extend(rest);
                }
                None => tags.push((name, specs)),
            }
        }
        // scriptTypes, unshifted last to first
        if !config.script_types.is_empty()
            && let Some((_, dest)) = tags.iter_mut().find(|(n, _)| *n == "script")
        {
            let rest = std::mem::take(dest);
            *dest = config
                .script_types
                .into_iter()
                .map(|(re, mode)| TagSpec::with_regex("type", re, mode))
                .collect();
            dest.extend(rest);
        }
        let tags = tags
            .into_iter()
            .map(|(name, specs)| Tag {
                name: name.to_string(),
                specs,
                end_anchored: compile(&format!("^{}", ascii_ci(&format!(r"<\/\s*{name}\s*>")))),
                end: compile(&ascii_ci(&format!(r"<\/\s*{name}\s*>"))),
            })
            .collect();
        Self {
            html: XmlMode::new(XmlConfig {
                allow_missing_tag_name: config.allow_missing_tag_name,
                ..XmlConfig::html()
            }),
            tags,
            resolve,
        }
    }

    pub fn start_html_mixed(&self) -> HtmlMixedState {
        HtmlMixedState {
            html_state: self.html.start_xml(),
            in_tag: None,
            local: None,
        }
    }

    /// `findMatchingMode(tagInfo, tagText)`
    fn find_matching_mode(specs: &[TagSpec], text: &str) -> Option<&'static str> {
        specs
            .iter()
            .find_map(|spec| match (&spec.attr, &spec.test) {
                (Some(attr), Some(test)) => {
                    // getAttrValue(text, attr)
                    let value = attr
                        .captures(text)
                        .ok()
                        .flatten()
                        .and_then(|m| m.get(2))
                        .map_or("", |g| g.as_str().trim_matches(is_js_space));
                    test.is_match(value).unwrap_or(false).then_some(spec.mode)
                }
                _ => Some(spec.mode),
            })
    }

    /// `html(stream, state)`
    fn html(&self, stream: &mut StringStream, s: &mut HtmlMixedState) -> Option<String> {
        let style = self.html.token_xml(stream, &mut s.html_state);
        let tag = style
            .as_deref()
            .is_some_and(|st| st.split_whitespace().any(|c| c == "tag"));
        let cur = stream.current();
        let tag_name = if tag
            && !cur
                .chars()
                .any(|c| matches!(c, '<' | '>' | '/') || is_js_space(c))
        {
            s.html_state
                .tag_name()
                .filter(|t| !t.is_empty())
                .map(str::to_lowercase)
                .filter(|t| self.tags.iter().any(|tag| tag.name == *t))
        } else {
            None
        };
        if let Some(tag_name) = tag_name {
            s.in_tag = Some(format!("{tag_name} "));
        } else if tag
            && cur.ends_with('>')
            && let Some(in_tag) = s.in_tag.take()
        {
            // /^([\S]+) (.*)/.exec(state.inTag)
            let (name, rest) = in_tag.split_once(' ').unwrap_or((&in_tag, ""));
            let rest = rest
                .split(['\n', '\r', '\u{2028}', '\u{2029}'])
                .next()
                .unwrap_or("");
            let ix = self.tags.iter().position(|t| t.name == name).unwrap_or(0);
            let spec = if cur == ">" {
                self.tags
                    .get(ix)
                    .and_then(|t| Self::find_matching_mode(&t.specs, rest))
            } else {
                None
            };
            let mode: Arc<dyn Mode> = spec
                .and_then(self.resolve)
                .unwrap_or_else(|| Arc::new(NullMode));
            let state = mode.start_state();
            s.local = Some(Local {
                mode,
                state,
                tag: ix,
            });
        } else if let Some(in_tag) = &mut s.in_tag {
            in_tag.push_str(cur);
            if stream.eol() {
                in_tag.push(' ');
            }
        }
        style.map(|s| s.into_owned())
    }

    /// `mode.token`
    pub fn token_html_mixed(
        &self,
        stream: &mut StringStream,
        s: &mut HtmlMixedState,
    ) -> Option<String> {
        let Some(local) = &mut s.local else {
            return self.html(stream, s);
        };
        let tag = &self.tags[local.tag];
        if stream.match_re(&tag.end_anchored, false).is_some() {
            s.local = None;
            return None;
        }
        let style = local.mode.token(stream, &mut *local.state);
        maybe_backup(stream, &tag.end, style)
    }

    /// `innerMode(state).mode.name`, followed down nested modes.
    pub fn inner_name(&self, s: &HtmlMixedState) -> &'static str {
        match &s.local {
            Some(local) => local.mode.inner_mode_name(&*local.state),
            None => "xml",
        }
    }

    /// `state.localMode` is set (the inner mode is not the html one).
    pub fn in_local(s: &HtmlMixedState) -> bool {
        s.local.is_some()
    }

    /// The html (xml) mode's state, for `htmlMode.indent`-free callers.
    pub fn html_state(s: &HtmlMixedState) -> &XmlState {
        &s.html_state
    }
}

/// `maybeBackup(stream, pat, style)`
fn maybe_backup(stream: &mut StringStream, pat: &Regex, style: Option<String>) -> Option<String> {
    let cur = stream.current();
    if let Some(close) = pat.find(cur).ok().flatten() {
        let n = js_len(cur) - js_len(&cur[..close.start()]);
        stream.back_up(n);
    } else if re!(r"<\/?$").is_match(cur).unwrap_or(false) {
        let cur = cur.to_string();
        stream.back_up(js_len(&cur));
        if stream.match_re(pat, false).is_none() {
            stream.match_str(&cur, true, false);
        }
    }
    style
}

impl Mode for HtmlMixed {
    fn name(&self) -> &'static str {
        "htmlmixed"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(self.start_html_mixed())
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        self.token_html_mixed(stream, state::<HtmlMixedState>(st))
    }

    /// `innerMode`: the local mode (and its own inner mode) or xml
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        self.inner_name(state_ref::<HtmlMixedState>(st))
    }
}
