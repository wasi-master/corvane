//! `codemirror/mode/vue/vue.js` (`text/x-vue`, `script/x-vue`): htmlmixed
//! with vue's `tagLanguages`, so `<script lang="coffee">`, `<style
//! lang="scss|less|sass|stylus">` and `<template lang="pug|handlebars">`
//! bodies go to those modes, and any other `<template>` body to
//! `vue-template`: `text/html` (htmlmixed) under a `{{ … }}` mustache
//! overlay (`CodeMirror.overlayMode`, `codemirror/addon/mode/overlay.js`).
//!
//! GHD's worker for `.vue` loads vue.js and what it requires (overlay, xml,
//! javascript, coffeescript, css, sass, stylus, pug, handlebars - and
//! through them htmlmixed, simple and multiplex), so every nested lookup
//! goes through [`vue_modes`], pug's filters included. `lang="ts"` is not
//! in the table and falls through to htmlmixed's defaults (javascript).
//!
//! `getMode` renames the htmlmixed object it returns to `vue`, but
//! htmlmixed's `innerMode` never yields itself, so tokens carry the inner
//! mode's class (`m-xml`, `m-javascript`, …). The overlay's `innerMode` is
//! its base, so mustaches inside templates come out as `m-xml meta
//! mustache`.

use std::sync::{Arc, OnceLock};

use super::super::{Mode, ModeState, StringStream, state, state_ref};
use super::htmlmixed::{HtmlMixed, HtmlMixedConfig, TagSpec, ascii_ci};
use crate::re;

/// `tagLanguages`
fn tag_languages() -> Vec<(&'static str, Vec<TagSpec>)> {
    vec![
        (
            "script",
            vec![
                TagSpec::new("lang", "coffee(script)?", "coffeescript"),
                TagSpec::new(
                    "type",
                    r"^(?:text|application)\/(?:x-)?coffee(?:script)?$",
                    "coffeescript",
                ),
                TagSpec::new("lang", "^babel$", "javascript"),
                TagSpec::new("type", r"^text\/babel$", "javascript"),
                TagSpec::new("type", r"^text\/ecmascript-[0-9]+$", "javascript"),
            ],
        ),
        (
            "style",
            vec![
                TagSpec::new("lang", &ascii_ci("^stylus$"), "stylus"),
                TagSpec::new("lang", &ascii_ci("^sass$"), "sass"),
                TagSpec::new("lang", &ascii_ci("^less$"), "text/x-less"),
                TagSpec::new("lang", &ascii_ci("^scss$"), "text/x-scss"),
                TagSpec::new("type", &ascii_ci(r"^(text\/)?(x-)?styl(us)?$"), "stylus"),
                TagSpec::new("type", &ascii_ci(r"^text\/sass"), "sass"),
                TagSpec::new("type", &ascii_ci(r"^(text\/)?(x-)?scss$"), "text/x-scss"),
                TagSpec::new("type", &ascii_ci(r"^(text\/)?(x-)?less$"), "text/x-less"),
            ],
        ),
        (
            "template",
            vec![
                TagSpec::new("lang", &ascii_ci("^vue-template$"), "vue"),
                TagSpec::new("lang", &ascii_ci("^pug$"), "pug"),
                TagSpec::new("lang", &ascii_ci("^handlebars$"), "handlebars"),
                TagSpec::new("type", &ascii_ci(r"^(text\/)?(x-)?pug$"), "pug"),
                TagSpec::new(
                    "type",
                    &ascii_ci(r"^text\/x-handlebars-template$"),
                    "handlebars",
                ),
                TagSpec::fallback("vue-template"),
            ],
        ),
    ]
}

/// `CodeMirror.defineMode("vue", …)`: htmlmixed with `tags: tagLanguages`,
/// named `vue`.
pub struct Vue(HtmlMixed);

impl Vue {
    pub fn new() -> Self {
        Self(HtmlMixed::new(
            HtmlMixedConfig {
                tags: tag_languages(),
                ..Default::default()
            },
            vue_modes,
        ))
    }
}

impl Default for Vue {
    fn default() -> Self {
        Self::new()
    }
}

impl Mode for Vue {
    fn name(&self) -> &'static str {
        "vue"
    }
    fn start_state(&self) -> Box<dyn ModeState> {
        self.0.start_state()
    }
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        self.0.token(stream, st)
    }
    fn blank_line(&self, st: &mut dyn ModeState) {
        self.0.blank_line(st)
    }
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        self.0.inner_mode_name(st)
    }
}

/// `CodeMirror.defineMode("vue-template", …)`: `overlayMode(getMode(config,
/// "text/html"), mustacheOverlay)` (no `backdrop` in the spec vue passes).
pub struct VueTemplate {
    base: Arc<dyn Mode>,
}

/// overlayMode's state; the overlay's own state is `true` (no
/// `startState`), so it has nothing to keep.
#[derive(Clone)]
struct OverlayState {
    base: Box<dyn ModeState>,
    base_pos: usize,
    base_cur: Option<String>,
    overlay_pos: usize,
    overlay_cur: Option<&'static str>,
    /// `streamSeen`: GHD makes one stream per line, so the line index
    stream_seen: Option<usize>,
}

/// `mustacheOverlay.token`
fn mustache_overlay(stream: &mut StringStream) -> Option<&'static str> {
    if stream.matches(re!(r"^\{\{[^\n\r\x{2028}\x{2029}]*?\}\}")) {
        return Some("meta mustache");
    }
    while stream.next().is_some() && !stream.match_str("{{", false, false) {}
    None
}

impl Mode for VueTemplate {
    fn name(&self) -> &'static str {
        "vue-template"
    }

    // overlayMode startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(OverlayState {
            base: self.base.start_state(),
            base_pos: 0,
            base_cur: None,
            overlay_pos: 0,
            overlay_cur: None,
            stream_seen: None,
        })
    }

    // overlayMode token (combine unset: the overlay's style wins)
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<OverlayState>(st);
        if s.stream_seen != Some(stream.line()) || s.base_pos.min(s.overlay_pos) < stream.start {
            s.stream_seen = Some(stream.line());
            s.base_pos = stream.start;
            s.overlay_pos = stream.start;
        }
        if stream.start == s.base_pos {
            s.base_cur = self.base.token(stream, &mut *s.base);
            s.base_pos = stream.pos;
        }
        if stream.start == s.overlay_pos {
            stream.pos = stream.start;
            s.overlay_cur = mustache_overlay(stream);
            s.overlay_pos = stream.pos;
        }
        stream.pos = s.base_pos.min(s.overlay_pos);
        match s.overlay_cur {
            None => s.base_cur.clone(),
            Some(overlay) => Some(overlay.to_string()),
        }
    }

    // overlayMode blankLine (the overlay has none)
    fn blank_line(&self, st: &mut dyn ModeState) {
        self.base.blank_line(&mut *state::<OverlayState>(st).base)
    }

    // overlayMode innerMode → the base
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        self.base
            .inner_mode_name(&*state_ref::<OverlayState>(st).base)
    }
}

/// `CodeMirror.getMode({}, "text/x-vue")`
pub fn vue() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| Arc::new(Vue::new())).clone()
}

/// `CodeMirror.getMode({}, "vue-template")`
pub fn vue_template() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| {
        Arc::new(VueTemplate {
            base: super::htmlmixed(),
        })
    })
    .clone()
}

/// pug as loaded by vue: its filters see vue's modes.
fn pug() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| Arc::new(super::pug::Pug::with_resolver(vue_modes)))
        .clone()
}

/// `getMode` for a spec in GHD's `.vue` worker: vue.js and the modes it
/// requires (plus htmlmixed, which pug requires); `None` is the `null`
/// mode.
pub fn vue_modes(spec: &str) -> Option<Arc<dyn Mode>> {
    match spec {
        "vue" | "text/x-vue" | "script/x-vue" => Some(vue()),
        "vue-template" => Some(vue_template()),
        "coffeescript"
        | "application/vnd.coffeescript"
        | "text/x-coffeescript"
        | "text/coffeescript" => super::mode_for_mime("text/x-coffeescript"),
        "sass" | "text/x-sass" => super::mode_for_mime("text/x-sass"),
        "stylus" | "text/x-styl" => super::mode_for_mime("text/x-styl"),
        "pug" | "text/x-pug" | "text/x-jade" => Some(pug()),
        "handlebars" | "text/x-handlebars-template" => Some(super::handlebars::handlebars()),
        "handlebars-tags" => Some(super::handlebars::handlebars_tags_mode()),
        _ => super::html_modes(spec),
    }
}
