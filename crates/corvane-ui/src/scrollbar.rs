//! Chromium's macOS scrollbars and wheel smooth scrolling.
//!
//! GHD is Electron, so on macOS its scrollbars are Chromium's native
//! scrollers (`app/styles/ui/_scroll.scss` only restyles Windows and Linux).
//! This element reproduces them over any GPUI scroll container, in the style
//! `NSScroller.preferredScrollerStyle` asks for:
//!
//! - overlay (trackpad, "automatically"/"when scrolling"): 16 px strip,
//!   12 → 16 px thumb box, 2 px inset plus a 1 px border on the content
//!   side, 26 px minimum thumb (`scrollbar_theme_mac.cc`
//!   `overlay_regular_values`). The thumb appears at full opacity on scroll
//!   and fades after 500 ms over 250 ms; hovering the strip fades the track
//!   in and widens the thumb over 250 ms, all ease-in-out
//!   (`ui/native_theme/scrollbar_animator_mac.cc`).
//! - legacy ("Always", or a mouse without a trackpad): a 15 px track that
//!   takes layout space whenever the content overflows, 3 px inset, 20 px
//!   minimum thumb (`legacy_regular_values`), always shown; the thumb darkens
//!   or lightens while the pointer is over the scrollbar.
//!
//! Colours are `NativeThemeMac::GetScrollbarColor` (`native_theme_mac.mm`).
//!
//! Wheel smoothing follows `cc/animation/scroll_offset_animation_curve.cc`:
//! 40 px per wheel tick (`kScrollbarPixelsPerCocoaTick`), the inverse-delta
//! duration (200 ms for ≤ 120 px down to 100 ms for ≥ 480 px) on
//! `cubic-bezier(0.42, 0, 0.58, 1)`, and velocity-preserving retargeting when
//! ticks arrive mid-animation. Trackpad (precise) deltas are applied directly,
//! as in Chromium; the OS supplies momentum.
//!
//! Deviations: Chromium only animates wheel ticks on macOS when
//! `NSScrollAnimationEnabled` is set; Corvane always does (except under
//! Reduce Motion).
//!
//! Track clicks follow System Settings › "Click in the scroll bar to"
//! (`AppleScrollerPagingBehavior`, Blink `ShouldCenterOnThumb`): "Jump to the
//! next page" pages by 87.5 % of the viewport (`kMinFractionToStepWhenPaging`),
//! "Jump to the spot that's clicked" centres the thumb there and drags; ⌥
//! swaps the two.
//!
//! Usage: `.with_scrollbar()` on an `overflow_y_scroll` div or a
//! `uniform_list` (it keeps the scroll handle and the legacy gutter). For a
//! container that already tracks a handle (`ListState`, a shared
//! `UniformListScrollHandle`), add `scrollbar(id, handle)` as a later
//! sibling in the same view and pad the container's trailing edge by
//! `gutter(&handle)`.

use std::cell::Cell;
use std::panic::Location;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::base::ScrollbarHandle;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;

/// `track_box_width_unexpanded` / `track_box_width_expanded`.
const BOX_IDLE: f32 = 12.;
const BOX_EXPANDED: f32 = 16.;
/// Track border, on each long side.
const BORDER: f32 = 1.;
/// `kFadeOutDelay`, `kAnimationDuration`.
const FADE_DELAY: Duration = Duration::from_millis(500);
const ANIMATION: Duration = Duration::from_millis(250);
/// `kScrollbarPixelsPerCocoaTick`.
const PIXELS_PER_TICK: f32 = 40.;
/// `kMinFractionToStepWhenPaging`.
const PAGE_FRACTION: f32 = 0.875;
/// `kInitialAutoscrollTimerDelay`, `kAutoscrollTimerDelay`: holding the
/// mouse on the track keeps paging.
const AUTOSCROLL_DELAY: Duration = Duration::from_millis(250);
const AUTOSCROLL_REPEAT: Duration = Duration::from_millis(50);

/// `NSScroller.preferredScrollerStyle`: overlay (fading, over the content)
/// or legacy (System Settings › Show scroll bars › Always, or a mouse without
/// a trackpad: always visible, taking layout space).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Metrics {
    legacy: bool,
    /// `track_width`: the strip that reacts to the mouse.
    strip: f32,
    /// Thumb inset from the track edges and ends.
    inset: f32,
    /// `knob_min_length`.
    min_thumb: f32,
    /// Shortest track that still shows a thumb.
    min_track: f32,
}

impl Metrics {
    /// `overlay_regular_values = {16, 16, 26, 0, 0, 0, 1}`, inset 2.
    const OVERLAY: Self = Self {
        legacy: false,
        strip: 16.,
        inset: 2.,
        min_thumb: 26.,
        min_track: 28.,
    };
    /// `legacy_regular_values = {15, 15, 20, 0, 0, 0, 2}`, inset 3.
    const LEGACY: Self = Self {
        legacy: true,
        strip: 15.,
        inset: 3.,
        min_thumb: 20.,
        min_track: 24.,
    };

    fn current() -> Self {
        if legacy_scrollers() {
            Self::LEGACY
        } else {
            Self::OVERLAY
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)] // objc 0.2 macros check `cargo-clippy`
fn legacy_scrollers() -> bool {
    use objc::{class, msg_send, sel, sel_impl};
    // NSScrollerStyleLegacy = 0, NSScrollerStyleOverlay = 1
    let style: isize = unsafe { msg_send![class!(NSScroller), preferredScrollerStyle] };
    style == 0
}

#[cfg(not(target_os = "macos"))]
fn legacy_scrollers() -> bool {
    false
}

/// "Click in the scroll bar to: Jump to the spot that's clicked"
/// (`AppleScrollerPagingBehavior`), read per click like Blink does.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)] // objc 0.2 macros check `cargo-clippy`
fn jump_on_track_click() -> bool {
    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let defaults: *mut Object = msg_send![class!(NSUserDefaults), standardUserDefaults];
        let key: *mut Object = msg_send![
            class!(NSString),
            stringWithUTF8String: c"AppleScrollerPagingBehavior".as_ptr()
        ];
        let jump: BOOL = msg_send![defaults, boolForKey: key];
        jump != NO
    }
}

#[cfg(not(target_os = "macos"))]
fn jump_on_track_click() -> bool {
    false
}

/// Space a legacy scrollbar takes beside `handle`'s content (0 for overlay
/// scrollers or when nothing overflows). Containers that add `scrollbar` as
/// a sibling pad their trailing edge by this; `with_scrollbar` does it itself.
pub fn gutter(handle: &impl ScrollbarHandle) -> Pixels {
    reserved(handle, Axis::Vertical)
}

fn reserved(handle: &dyn ScrollbarHandle, axis: Axis) -> Pixels {
    let metrics = Metrics::current();
    let overflow = handle.content_size().along(axis) - handle.viewport_bounds().size.along(axis);
    if metrics.legacy && f32::from(overflow) >= 0.5 {
        px(metrics.strip)
    } else {
        px(0.)
    }
}

/// An overlay scrollbar for `handle` along `axis` (vertical by default).
pub fn scrollbar(id: impl Into<ElementId>, handle: impl ScrollbarHandle) -> Scrollbar {
    Scrollbar {
        id: id.into(),
        handle: Rc::new(handle),
        axis: Axis::Vertical,
    }
}

pub struct Scrollbar {
    id: ElementId,
    handle: Rc<dyn ScrollbarHandle>,
    axis: Axis,
}

impl Scrollbar {
    pub fn horizontal(mut self) -> Self {
        self.axis = Axis::Horizontal;
        self
    }
}

/// Containers that can report their scroll position to a handle.
pub trait TrackScroll: IntoElement + Styled + Sized + 'static {
    type Handle: ScrollbarHandle + Clone + Default;

    fn track(self, handle: &Self::Handle) -> Self;
}

impl TrackScroll for Stateful<Div> {
    type Handle = ScrollHandle;

    fn track(self, handle: &ScrollHandle) -> Self {
        self.track_scroll(handle)
    }
}

impl TrackScroll for UniformList {
    type Handle = UniformListScrollHandle;

    fn track(self, handle: &UniformListScrollHandle) -> Self {
        self.track_scroll(handle)
    }
}

pub trait ScrollbarExt: TrackScroll {
    /// Overlay a vertical scrollbar, with a scroll handle kept for as long as
    /// this element keeps rendering. The element's layout is unchanged. For a
    /// container that already tracks a handle, add `scrollbar` as a sibling.
    fn with_scrollbar(self) -> WithScrollbar<Self> {
        WithScrollbar {
            element: Some(self),
            handle: None,
        }
    }

    /// `with_scrollbar` with a caller-owned handle, for lists that scroll a
    /// row into view (`scroll_to_item`).
    fn with_scrollbar_handle(self, handle: &Self::Handle) -> WithScrollbar<Self> {
        WithScrollbar {
            element: Some(self),
            handle: Some(handle.clone()),
        }
    }
}

impl<E: TrackScroll> ScrollbarExt for E {}

pub struct WithScrollbar<E: TrackScroll> {
    element: Option<E>,
    handle: Option<E::Handle>,
}

/// Per-element state behind `WithScrollbar`.
struct Tracked<H> {
    handle: H,
    state: Entity<State>,
}

impl<H: Clone> Clone for Tracked<H> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            state: self.state.clone(),
        }
    }
}

impl<E: TrackScroll> WithScrollbar<E> {
    fn tracked(
        id: &GlobalElementId,
        handle: Option<&E::Handle>,
        window: &mut Window,
        cx: &mut App,
    ) -> Tracked<E::Handle> {
        window.with_element_state(id, |tracked: Option<Tracked<E::Handle>>, _| {
            let mut tracked = tracked.unwrap_or_else(|| Tracked {
                handle: E::Handle::default(),
                state: cx.new(|_| State::default()),
            });
            if let Some(handle) = handle {
                tracked.handle = handle.clone();
            }
            (tracked.clone(), tracked)
        })
    }
}

impl<E: TrackScroll> IntoElement for WithScrollbar<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E: TrackScroll> Element for WithScrollbar<E> {
    type RequestLayoutState = Option<AnyElement>;
    type PrepaintState = Option<Prepaint>;

    fn id(&self) -> Option<ElementId> {
        Some("scrollbar".into())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Option<AnyElement>) {
        let (Some(id), Some(element)) = (id, self.element.take()) else {
            return (window.request_layout(Style::default(), None, cx), None);
        };
        let tracked = Self::tracked(id, self.handle.as_ref(), window, cx);
        let mut element = element.track(&tracked.handle);
        // a legacy scrollbar sits between the border and the padding
        let gutter = reserved(&tracked.handle, Axis::Vertical);
        if gutter > px(0.) {
            let rem = window.rem_size();
            let padding = &mut element.style().padding.right;
            let existing = match padding {
                Some(DefiniteLength::Absolute(length)) => length.to_pixels(rem),
                _ => px(0.),
            };
            *padding = Some((existing + gutter).into());
        }
        let mut child = element.into_any_element();
        (child.request_layout(window, cx), Some(child))
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut Option<AnyElement>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Prepaint> {
        child.as_mut()?.prepaint(window, cx);
        let tracked = Self::tracked(id?, self.handle.as_ref(), window, cx);
        let handle: Rc<dyn ScrollbarHandle> = Rc::new(tracked.handle);
        prepaint_bar(&handle, Axis::Vertical, tracked.state, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut Option<AnyElement>,
        prepaint: &mut Option<Prepaint>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(child) = child else {
            return;
        };
        child.paint(window, cx);
        if let (Some(id), Some(p)) = (id, prepaint.take()) {
            let handle: Rc<dyn ScrollbarHandle> =
                Rc::new(Self::tracked(id, self.handle.as_ref(), window, cx).handle);
            paint_bar(&handle, Axis::Vertical, p, window, cx);
        }
    }
}

// ---------------------------------------------------------------------------
// Curves

/// `gfx::CubicBezier`: a unit bezier from (0, 0) to (1, 1).
#[derive(Clone, Copy, Debug)]
struct CubicBezier {
    ax: f64,
    bx: f64,
    cx: f64,
    ay: f64,
    by: f64,
    cy: f64,
}

impl CubicBezier {
    fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        let cx = 3. * x1;
        let bx = 3. * (x2 - x1) - cx;
        let cy = 3. * y1;
        let by = 3. * (y2 - y1) - cy;
        Self {
            ax: 1. - cx - bx,
            bx,
            cx,
            ay: 1. - cy - by,
            by,
            cy,
        }
    }

    /// `kEaseInOutControlPoints`.
    fn ease_in_out() -> Self {
        Self::new(0.42, 0., 0.58, 1.)
    }

    /// `EaseInOutWithInitialSlope`: ease-in-out whose start speed is `slope`
    /// times the average speed.
    fn ease_in_out_with_slope(slope: f64) -> Self {
        Self::new(0.42, 0.42 * slope, 0.58, 1.)
    }

    fn x(&self, t: f64) -> f64 {
        ((self.ax * t + self.bx) * t + self.cx) * t
    }

    fn y(&self, t: f64) -> f64 {
        ((self.ay * t + self.by) * t + self.cy) * t
    }

    fn dx(&self, t: f64) -> f64 {
        (3. * self.ax * t + 2. * self.bx) * t + self.cx
    }

    fn dy(&self, t: f64) -> f64 {
        (3. * self.ay * t + 2. * self.by) * t + self.cy
    }

    /// The curve parameter whose x is `x` (Newton, then bisection).
    fn t_for(&self, x: f64) -> f64 {
        let x = x.clamp(0., 1.);
        let mut t = x;
        for _ in 0..8 {
            let err = self.x(t) - x;
            if err.abs() < 1e-7 {
                return t;
            }
            let d = self.dx(t);
            if d.abs() < 1e-6 {
                break;
            }
            t -= err / d;
        }
        let (mut lo, mut hi) = (0., 1.);
        t = x;
        while hi - lo > 1e-7 {
            if self.x(t) < x {
                lo = t;
            } else {
                hi = t;
            }
            t = (lo + hi) / 2.;
        }
        t
    }

    fn solve(&self, x: f64) -> f64 {
        self.y(self.t_for(x))
    }

    fn slope(&self, x: f64) -> f64 {
        let t = self.t_for(x);
        let dx = self.dx(t);
        if dx.abs() < 1e-6 { 0. } else { self.dy(t) / dx }
    }
}

/// `kInverseDelta` duration for a wheel scroll of `delta` px, in seconds:
/// 12 frames up to 120 px, 6 frames from 480 px, linear in between.
fn wheel_duration(delta: f64) -> f64 {
    (14. - delta.abs() / 60.).clamp(6., 12.) / 60.
}

/// `ScrollOffsetAnimationCurve` along one axis, in scroll-offset space.
#[derive(Clone, Copy, Debug)]
struct Curve {
    from: f32,
    to: f32,
    start: Instant,
    duration: f64,
    bezier: CubicBezier,
}

impl Curve {
    fn new(from: f32, to: f32, now: Instant) -> Self {
        Self {
            from,
            to,
            start: now,
            duration: wheel_duration((to - from) as f64),
            bezier: CubicBezier::ease_in_out(),
        }
    }

    fn progress(&self, now: Instant) -> f64 {
        if self.duration <= 0. {
            return 1.;
        }
        (now.saturating_duration_since(self.start).as_secs_f64() / self.duration).min(1.)
    }

    fn value(&self, now: Instant) -> f32 {
        let p = self.progress(now);
        if p >= 1. {
            return self.to;
        }
        self.from + (self.to - self.from) * self.bezier.solve(p) as f32
    }

    /// px/s.
    fn velocity(&self, now: Instant) -> f64 {
        let p = self.progress(now);
        if p >= 1. || self.duration <= 0. {
            return 0.;
        }
        self.bezier.slope(p) * (self.to - self.from) as f64 / self.duration
    }

    fn done(&self, now: Instant) -> bool {
        self.progress(now) >= 1.
    }

    /// `UpdateTarget`: aim at `target` from where the curve is now, keeping
    /// the current speed.
    fn retarget(&mut self, target: f32, now: Instant) {
        if (target - self.to).abs() < 0.01 {
            return;
        }
        let current = self.value(now);
        let velocity = self.velocity(now);
        let delta = (target - current) as f64;
        let mut duration = wheel_duration(delta);
        if velocity.abs() >= 0.01 {
            let bound = delta / velocity * 2.5;
            if bound >= 0. {
                duration = duration.min(bound);
            }
        }
        if delta.abs() < 0.01 || duration < 0.01 {
            *self = Self {
                from: target,
                to: target,
                start: now,
                duration: 0.,
                bezier: CubicBezier::ease_in_out(),
            };
            return;
        }
        let slope = (velocity * duration / delta).clamp(-1000., 1000.);
        *self = Self {
            from: current,
            to: target,
            start: now,
            duration,
            bezier: CubicBezier::ease_in_out_with_slope(slope),
        };
    }
}

fn ease(p: f32) -> f32 {
    CubicBezier::ease_in_out().solve(p.clamp(0., 1.) as f64) as f32
}

fn fraction(since: Instant, now: Instant, over: Duration) -> f32 {
    (now.saturating_duration_since(since).as_secs_f32() / over.as_secs_f32()).min(1.)
}

/// Thumb start and length along a track of `track` px showing `viewport` px
/// of `content` px scrolled by `position` px (`ScrollbarTheme::ThumbLength`,
/// `ThumbPosition`). `None` when there is nothing to scroll or the track is
/// too short for the minimum thumb.
fn thumb_extent(
    metrics: Metrics,
    track: f32,
    viewport: f32,
    content: f32,
    position: f32,
) -> Option<(f32, f32)> {
    let max = content - viewport;
    if max < 0.5 || track < metrics.min_track {
        return None;
    }
    let length = (track * viewport / content)
        .round()
        .clamp(metrics.min_thumb, track);
    let start = (track - length) * (position / max).clamp(0., 1.);
    Some((start, length))
}

// ---------------------------------------------------------------------------
// State

// Which scroller took the current wheel event in the capture phase: the
// innermost one under the pointer that can move that way (Chromium's scroll
// latching, per event). Keyed by the event's address, which both phases share.
thread_local! {
    static WHEEL_CLAIM: Cell<Option<(usize, EntityId)>> = const { Cell::new(None) };
}

#[derive(Default)]
struct State {
    /// Offset along the axis at the last frame, to notice scrolling.
    last_offset: Option<f32>,
    /// Whether the content overflowed at the last frame.
    last_scrollable: Option<bool>,
    /// Start of the fade-out countdown (last scroll, hover exit, drag end);
    /// `None` = hidden.
    shown_at: Option<Instant>,
    /// Pointer is over the strip (geometrically, even while hidden).
    in_strip: bool,
    /// The strip counts as hovered: entered while visible, or scrolled under
    /// the pointer.
    hovered: bool,
    /// When the thumb started widening; `None` = idle width.
    expand_start: Option<Instant>,
    /// Thumb drag: pointer offset from the thumb start.
    drag: Option<f32>,
    /// Track press: paging towards the pointer (`None` = not pressed).
    paging: Option<Paging>,
    /// Wheel animation along the axis.
    curve: Option<Curve>,
    /// Offset the animation last wrote, to notice other writers.
    written: Option<f32>,
    frame_pending: bool,
    /// Fade timer and the moment it fires for.
    timer: Option<(Instant, Task<()>)>,
}

#[derive(Clone, Copy)]
struct Paging {
    /// Pointer position along the axis, relative to the track start.
    pointer: f32,
}

impl State {
    fn thumb_alpha(&self, now: Instant) -> f32 {
        if self.drag.is_some() || self.paging.is_some() || self.hovered {
            return if self.shown_at.is_some() { 1. } else { 0. };
        }
        let Some(shown) = self.shown_at else {
            return 0.;
        };
        let since = now.saturating_duration_since(shown);
        if since < FADE_DELAY {
            1.
        } else {
            1. - ease(fraction(shown + FADE_DELAY, now, ANIMATION))
        }
    }

    fn expansion(&self, now: Instant) -> f32 {
        self.expand_start
            .map_or(0., |start| ease(fraction(start, now, ANIMATION)))
    }

    /// The thumb appears (or stays) at full opacity.
    fn show(&mut self, now: Instant) {
        self.shown_at = Some(now);
        if self.in_strip && !self.hovered {
            // scrolled under the pointer: expanded at once
            self.hovered = true;
            self.expand_start = Some(now - ANIMATION);
        }
    }

    fn set_in_strip(&mut self, inside: bool, now: Instant) {
        if self.in_strip == inside {
            return;
        }
        self.in_strip = inside;
        if inside {
            if self.thumb_alpha(now) > 0. {
                self.hovered = true;
                self.shown_at = Some(now);
                if self.expand_start.is_none() {
                    self.expand_start = Some(now);
                }
            }
        } else if self.hovered {
            self.hovered = false;
            self.shown_at = Some(now);
        }
    }

    /// Settle after the fade: hidden, idle width.
    fn settle(&mut self, now: Instant) {
        if self.shown_at.is_some() && self.thumb_alpha(now) <= 0. {
            self.shown_at = None;
            self.hovered = false;
            self.expand_start = None;
        }
    }

    /// Anything moving this frame (the fade delay is a timer, not frames).
    fn animating(&self, now: Instant) -> bool {
        let fading = self.shown_at.is_some_and(|shown| {
            !self.hovered
                && self.drag.is_none()
                && self.paging.is_none()
                && now.saturating_duration_since(shown) >= FADE_DELAY
        });
        let expanding = self
            .expand_start
            .is_some_and(|start| now.saturating_duration_since(start) < ANIMATION);
        fading || expanding
    }
}

// ---------------------------------------------------------------------------
// Element

pub struct Prepaint {
    metrics: Metrics,
    state: Entity<State>,
    viewport: Bounds<Pixels>,
    content: Size<Pixels>,
    /// The whole viewport, for wheel routing.
    area: Hitbox,
    /// The strip, while the scrollbar is visible.
    strip: Option<Hitbox>,
}

impl IntoElement for Scrollbar {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Scrollbar {
    type RequestLayoutState = ();
    type PrepaintState = Option<Prepaint>;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            position: Position::Absolute,
            ..Style::default()
        };
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Prepaint> {
        let state = window.with_element_state(id?, |state: Option<Entity<State>>, _| {
            let state = state.unwrap_or_else(|| cx.new(|_| State::default()));
            (state.clone(), state)
        });
        prepaint_bar(&self.handle, self.axis, state, window, cx)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Option<Prepaint>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(p) = prepaint.take() {
            paint_bar(&self.handle, self.axis, p, window, cx);
        }
    }
}

fn prepaint_bar(
    handle: &Rc<dyn ScrollbarHandle>,
    axis: Axis,
    state: Entity<State>,
    window: &mut Window,
    cx: &mut App,
) -> Option<Prepaint> {
    let viewport = handle.viewport_bounds();
    if viewport.size.width <= px(0.) || viewport.size.height <= px(0.) {
        return None;
    }
    let content = handle.content_size();
    let offset = f32::from(handle.offset().along(axis));
    let now = Instant::now();
    let metrics = Metrics::current();
    let scrollable = f32::from(content.along(axis) - viewport.size.along(axis)) >= 0.5;
    let (visible, relayout) = state.update(cx, |s, _| {
        if let Some(last) = s.last_offset
            && (last - offset).abs() > 0.01
        {
            s.show(now);
        }
        s.last_offset = Some(offset);
        s.settle(now);
        // a legacy scrollbar appearing or going away changes the layout
        let relayout = metrics.legacy && s.last_scrollable != Some(scrollable);
        s.last_scrollable = Some(scrollable);
        (metrics.legacy || s.thumb_alpha(now) > 0., relayout)
    });
    if relayout {
        window.request_animation_frame();
    }
    let area = window.insert_hitbox(viewport, HitboxBehavior::Normal);
    let strip = (visible && scrollable).then(|| {
        window.insert_hitbox(
            strip_bounds(viewport, axis, metrics.strip),
            HitboxBehavior::BlockMouseExceptScroll,
        )
    });
    Some(Prepaint {
        metrics,
        state,
        viewport,
        content,
        area,
        strip,
    })
}

fn paint_bar(
    source: &Rc<dyn ScrollbarHandle>,
    axis: Axis,
    p: Prepaint,
    window: &mut Window,
    cx: &mut App,
) {
    let view = window.current_view();
    let now = Instant::now();
    let dark = cx.ghd().is_dark();

    // ---- paint ----
    let track_len = f32::from(p.viewport.size.along(axis));
    let viewport_len = track_len;
    let content_len = f32::from(p.content.along(axis));
    let position = -f32::from(source.offset().along(axis));
    let metrics = p.metrics;
    let inset = metrics.inset;
    let extent = thumb_extent(metrics, track_len, viewport_len, content_len, position);
    let (alpha, expansion, hovered) = {
        let s = p.state.read(cx);
        if metrics.legacy {
            (1., 1., s.in_strip || s.drag.is_some())
        } else {
            (s.thumb_alpha(now), s.expansion(now), false)
        }
    };
    if let Some((start, length)) = extent
        && alpha > 0.
    {
        let (box_width, palette) = if metrics.legacy {
            (metrics.strip, Palette::legacy(dark, hovered))
        } else {
            (
                BOX_IDLE + ((BOX_EXPANDED - BOX_IDLE) * expansion).round(),
                Palette::overlay(dark),
            )
        };
        let track = strip_bounds(p.viewport, axis, box_width);
        let track_alpha = alpha * expansion;
        window.with_content_mask(Some(ContentMask { bounds: p.viewport }), |window| {
            if track_alpha > 0. {
                paint_track(window, track, axis, &palette, track_alpha);
            }
            // thumb: inset from the box, plus the content-side border
            let across = box_width - BORDER - 2. * inset;
            let thumb = match axis {
                Axis::Vertical => Bounds::new(
                    point(
                        track.origin.x + px(BORDER + inset),
                        p.viewport.origin.y + px(start + inset),
                    ),
                    size(px(across), px(length - 2. * inset)),
                ),
                Axis::Horizontal => Bounds::new(
                    point(
                        p.viewport.origin.x + px(start + inset),
                        track.origin.y + px(BORDER + inset),
                    ),
                    size(px(length - 2. * inset), px(across)),
                ),
            };
            window.paint_quad(
                fill(thumb, with_alpha(palette.thumb, alpha))
                    .corner_radii(Corners::all(px(across / 2.))),
            );
        });
    }
    if let Some(strip) = &p.strip {
        window.set_cursor_style(CursorStyle::Arrow, strip);
    }

    // ---- frames and the fade timer ----
    let (animating, fade_at) = {
        let s = p.state.read(cx);
        let pending = s
            .shown_at
            .filter(|_| !s.hovered && s.drag.is_none() && s.paging.is_none());
        (s.animating(now), pending.map(|shown| shown + FADE_DELAY))
    };
    if metrics.legacy {
        // always shown: nothing fades
    } else if animating {
        window.request_animation_frame();
    } else if let Some(at) = fade_at.filter(|at| *at > now) {
        let scheduled = p.state.read(cx).timer.as_ref().map(|(when, _)| *when);
        if scheduled != Some(at) {
            let wait = at - now;
            let task = window.spawn(cx, async move |cx| {
                cx.background_executor().timer(wait).await;
                cx.update(|_, cx| cx.notify(view)).ok();
            });
            p.state.update(cx, |s, _| s.timer = Some((at, task)));
        }
    }

    // ---- wheel ----
    let handle = source.clone();
    let state = p.state.clone();
    let area = p.area.clone();
    let viewport = p.viewport;
    let content = p.content;
    window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
        let key = event as *const ScrollWheelEvent as usize;
        let me = state.entity_id();
        match phase {
            DispatchPhase::Capture => {
                if !area.should_handle_scroll(window) {
                    return;
                }
                let ScrollDelta::Lines(lines) = event.delta else {
                    // trackpad: applied directly by the container
                    state.update(cx, |s, _| {
                        s.curve = None;
                        s.written = None;
                    });
                    return;
                };
                let ticks = lines.along(axis);
                if ticks == 0. || WHEEL_CLAIM.get().is_some_and(|(k, _)| k == key) {
                    return;
                }
                let max = f32::from(content.along(axis) - viewport.size.along(axis));
                let from = state
                    .read(cx)
                    .curve
                    .map_or(f32::from(handle.offset().along(axis)), |c| c.to);
                let target = (from + ticks * PIXELS_PER_TICK).clamp(-max.max(0.), 0.);
                if (target - from).abs() >= 0.5 {
                    WHEEL_CLAIM.set(Some((key, me)));
                }
            }
            DispatchPhase::Bubble => {
                if WHEEL_CLAIM.get() != Some((key, me)) {
                    return;
                }
                WHEEL_CLAIM.set(None);
                cx.stop_propagation();
                let ScrollDelta::Lines(lines) = event.delta else {
                    return;
                };
                let max = f32::from(content.along(axis) - viewport.size.along(axis)).max(0.);
                let ticks = lines.along(axis);
                let current = f32::from(handle.offset().along(axis));
                let now = Instant::now();
                if cx.reduce_motion() {
                    let target = (current + ticks * PIXELS_PER_TICK).clamp(-max, 0.);
                    set_offset(&*handle, axis, target);
                    cx.notify(view);
                    return;
                }
                let schedule = state.update(cx, |s, _| {
                    match &mut s.curve {
                        Some(curve) if !curve.done(now) => {
                            let target = (curve.to + ticks * PIXELS_PER_TICK).clamp(-max, 0.);
                            curve.retarget(target, now);
                        }
                        _ => {
                            let target = (current + ticks * PIXELS_PER_TICK).clamp(-max, 0.);
                            s.curve = Some(Curve::new(current, target, now));
                            s.written = Some(current);
                        }
                    }
                    !std::mem::replace(&mut s.frame_pending, true)
                });
                if schedule {
                    let (handle, state) = (handle.clone(), state.clone());
                    window.on_next_frame(move |window, cx| {
                        animate(handle, state, axis, view, window, cx)
                    });
                }
            }
        }
    });

    // ---- pointer: hover, drag, track paging ----
    let state = p.state.clone();
    let area = p.area.clone();
    let strip_hitbox = p.strip.clone();
    let strip_rect = strip_bounds(viewport, axis, metrics.strip);
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        let over = match &strip_hitbox {
            Some(strip) => strip.is_hovered(window),
            None => area.is_hovered(window) && strip_rect.contains(&event.position),
        };
        let now = Instant::now();
        let changed = state.update(cx, |s, _| {
            let before = (s.hovered, s.in_strip);
            s.set_in_strip(over, now);
            before != (s.hovered, s.in_strip)
        });
        if changed {
            cx.notify(view);
        }
    });

    let Some(strip) = p.strip.clone() else {
        return;
    };
    let Some((thumb_start, thumb_len)) = extent else {
        return;
    };
    let origin = f32::from(viewport.origin.along(axis));
    let max = (content_len - viewport_len).max(0.);
    let handle = source.clone();
    let state = p.state.clone();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || event.button != MouseButton::Left
            || !strip.is_hovered(window)
        {
            return;
        }
        cx.stop_propagation();
        let pointer = f32::from(event.position.along(axis)) - origin;
        let now = Instant::now();
        if pointer >= thumb_start && pointer < thumb_start + thumb_len {
            handle.start_drag();
            state.update(cx, |s, _| {
                s.drag = Some(pointer - thumb_start);
                s.curve = None;
                s.shown_at = Some(now);
            });
        } else if event.modifiers.alt != jump_on_track_click() {
            // jump so the thumb centres on the pointer, then drag
            let grab = thumb_len / 2.;
            drag_to(&*handle, axis, pointer - grab, track_len, thumb_len, max);
            handle.start_drag();
            state.update(cx, |s, _| {
                s.drag = Some(grab);
                s.curve = None;
                s.shown_at = Some(now);
            });
        } else {
            state.update(cx, |s, _| {
                s.paging = Some(Paging { pointer });
                s.shown_at = Some(now);
            });
            page(&handle, &state, axis, view, viewport_len, max, window, cx);
            let (handle, state) = (handle.clone(), state.clone());
            window
                .spawn(cx, async move |cx| {
                    let executor = cx.background_executor().clone();
                    executor.timer(AUTOSCROLL_DELAY).await;
                    loop {
                        let paging = cx
                            .update(|window, cx| {
                                if state.read(cx).paging.is_none() {
                                    return false;
                                }
                                page(&handle, &state, axis, view, viewport_len, max, window, cx)
                            })
                            .unwrap_or(false);
                        if !paging {
                            break;
                        }
                        executor.timer(AUTOSCROLL_REPEAT).await;
                    }
                })
                .detach();
        }
        cx.notify(view);
    });

    let handle = source.clone();
    let state = p.state.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        let pointer = f32::from(event.position.along(axis)) - origin;
        let (drag, paging) = {
            let s = state.read(cx);
            (s.drag, s.paging.is_some())
        };
        if let Some(grab) = drag {
            if !event.dragging() {
                return;
            }
            drag_to(&*handle, axis, pointer - grab, track_len, thumb_len, max);
            state.update(cx, |s, _| s.shown_at = Some(Instant::now()));
            cx.notify(view);
        } else if paging {
            state.update(cx, |s, _| {
                if let Some(paging) = &mut s.paging {
                    paging.pointer = pointer;
                }
            });
        }
    });

    let handle = source.clone();
    let state = p.state.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
        if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
            return;
        }
        let was_dragging = state.update(cx, |s, _| {
            let active = s.drag.is_some() || s.paging.is_some();
            let dragging = s.drag.take().is_some();
            s.paging = None;
            if active {
                s.shown_at = Some(Instant::now());
            }
            dragging
        });
        if was_dragging {
            handle.end_drag();
        }
        cx.notify(view);
    });
}

/// One wheel-animation frame, run before the frame draws so the content and
/// the thumb move together.
fn animate(
    handle: Rc<dyn ScrollbarHandle>,
    state: Entity<State>,
    axis: Axis,
    view: EntityId,
    window: &mut Window,
    cx: &mut App,
) {
    let now = Instant::now();
    let current = f32::from(handle.offset().along(axis));
    let step = state.update(cx, |s, _| {
        s.frame_pending = false;
        // another writer (drag, keyboard, reveal) moved it: stop
        if s.written.is_some_and(|w| (w - current).abs() > 1.) {
            s.curve = None;
        }
        let curve = s.curve?;
        let value = curve.value(now);
        s.written = Some(value);
        if curve.done(now) {
            s.curve = None;
        } else {
            s.frame_pending = true;
        }
        Some((value, s.frame_pending))
    });
    let Some((value, more)) = step else {
        return;
    };
    set_offset(&*handle, axis, value);
    cx.notify(view);
    if more {
        window.on_next_frame(move |window, cx| animate(handle, state, axis, view, window, cx));
    }
}

/// Page once towards the pressed pointer; `false` once the thumb reached it.
#[allow(clippy::too_many_arguments)]
fn page(
    handle: &Rc<dyn ScrollbarHandle>,
    state: &Entity<State>,
    axis: Axis,
    view: EntityId,
    viewport_len: f32,
    max: f32,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(paging) = state.read(cx).paging else {
        return false;
    };
    let content = viewport_len + max;
    let current = f32::from(handle.offset().along(axis));
    let from = state.read(cx).curve.map_or(current, |c| c.to);
    let Some((start, length)) = thumb_extent(
        Metrics::current(),
        viewport_len,
        viewport_len,
        content,
        -from,
    ) else {
        return false;
    };
    let direction = if paging.pointer < start {
        1.
    } else if paging.pointer >= start + length {
        -1.
    } else {
        return false;
    };
    let target = (from + direction * viewport_len * PAGE_FRACTION).clamp(-max, 0.);
    let now = Instant::now();
    if cx.reduce_motion() {
        set_offset(&**handle, axis, target);
        cx.notify(view);
        return true;
    }
    let schedule = state.update(cx, |s, _| {
        match &mut s.curve {
            Some(curve) if !curve.done(now) => curve.retarget(target, now),
            _ => {
                s.curve = Some(Curve::new(current, target, now));
                s.written = Some(current);
            }
        }
        s.shown_at = Some(now);
        !std::mem::replace(&mut s.frame_pending, true)
    });
    if schedule {
        let (handle, state) = (handle.clone(), state.clone());
        window.on_next_frame(move |window, cx| animate(handle, state, axis, view, window, cx));
    }
    true
}

/// Move so the thumb starts at `thumb_start` px along the track.
fn drag_to(
    handle: &dyn ScrollbarHandle,
    axis: Axis,
    thumb_start: f32,
    track_len: f32,
    thumb_len: f32,
    max: f32,
) {
    let room = track_len - thumb_len;
    if room <= 0. {
        return;
    }
    let position = (thumb_start / room).clamp(0., 1.) * max;
    set_offset(handle, axis, -position);
}

fn set_offset(handle: &dyn ScrollbarHandle, axis: Axis, value: f32) {
    let offset = handle.offset().apply_along(axis, |_| px(value));
    handle.set_offset(offset);
}

/// The last `thickness` px across `axis` (right edge, or bottom edge).
fn strip_bounds(viewport: Bounds<Pixels>, axis: Axis, thickness: f32) -> Bounds<Pixels> {
    match axis {
        Axis::Vertical => Bounds::new(
            point(viewport.right() - px(thickness), viewport.origin.y),
            size(px(thickness), viewport.size.height),
        ),
        Axis::Horizontal => Bounds::new(
            point(viewport.origin.x, viewport.bottom() - px(thickness)),
            size(viewport.size.width, px(thickness)),
        ),
    }
}

// ---------------------------------------------------------------------------
// Colours

/// `NativeThemeMac::GetScrollbarColor`. Overlay: Blink paints light-on-dark
/// over a dark background (`ScrollbarOverlayColorTheme::kLight`),
/// dark-on-light otherwise. Legacy: the dark or light native palette.
struct Palette {
    thumb: Hsla,
    track_start: Hsla,
    track_end: Hsla,
    inner_border: Hsla,
    outer_border: Hsla,
}

impl Palette {
    fn legacy(dark: bool, hovered: bool) -> Self {
        if dark {
            Self {
                thumb: if hovered {
                    argb(0xFF, 0x93, 0x93, 0x93)
                } else {
                    argb(0xFF, 0x6B, 0x6B, 0x6B)
                },
                track_start: argb(0xFF, 0x2D, 0x2D, 0x2D),
                track_end: argb(0xFF, 0x2B, 0x2B, 0x2B),
                inner_border: argb(0xFF, 0x3D, 0x3D, 0x3D),
                outer_border: argb(0xFF, 0x51, 0x51, 0x51),
            }
        } else {
            Self {
                thumb: if hovered {
                    argb(0x80, 0, 0, 0)
                } else {
                    argb(0x3A, 0, 0, 0)
                },
                track_start: argb(0xFF, 0xFA, 0xFA, 0xFA),
                track_end: argb(0xFF, 0xFA, 0xFA, 0xFA),
                inner_border: argb(0xFF, 0xE8, 0xE8, 0xE8),
                outer_border: argb(0xFF, 0xED, 0xED, 0xED),
            }
        }
    }

    fn overlay(dark: bool) -> Self {
        if dark {
            Self {
                thumb: argb(0x80, 0xFF, 0xFF, 0xFF),
                track_start: rgba_f(0xD8, 0xD8, 0xD8, 0.157),
                track_end: rgba_f(0xCC, 0xCC, 0xCC, 0.149),
                inner_border: argb(0x33, 0xE5, 0xE5, 0xE5),
                outer_border: argb(0x28, 0xD8, 0xD8, 0xD8),
            }
        } else {
            Self {
                thumb: argb(0x80, 0, 0, 0),
                track_start: rgba_f(0xF8, 0xF8, 0xF8, 0.776),
                track_end: rgba_f(0xF8, 0xF8, 0xF8, 0.761),
                inner_border: argb(0xF9, 0xDF, 0xDF, 0xDF),
                outer_border: argb(0xC6, 0xE8, 0xE8, 0xE8),
            }
        }
    }
}

fn argb(a: u8, r: u8, g: u8, b: u8) -> Hsla {
    rgba_f(r, g, b, a as f32 / 255.)
}

fn rgba_f(r: u8, g: u8, b: u8, a: f32) -> Hsla {
    Rgba {
        r: r as f32 / 255.,
        g: g as f32 / 255.,
        b: b as f32 / 255.,
        a,
    }
    .into()
}

fn with_alpha(color: Hsla, alpha: f32) -> Hsla {
    Hsla {
        a: color.a * alpha,
        ..color
    }
}

/// Track fill (a gradient along the bar) with a 1 px border on each long
/// side: inner towards the content, outer at the window edge.
fn paint_track(
    window: &mut Window,
    track: Bounds<Pixels>,
    axis: Axis,
    palette: &Palette,
    alpha: f32,
) {
    let angle = match axis {
        Axis::Vertical => 180.,
        Axis::Horizontal => 90.,
    };
    window.paint_quad(fill(
        track,
        linear_gradient(
            angle,
            linear_color_stop(with_alpha(palette.track_start, alpha), 0.),
            linear_color_stop(with_alpha(palette.track_end, alpha), 1.),
        ),
    ));
    let (inner, outer) = match axis {
        Axis::Vertical => (
            Bounds::new(track.origin, size(px(BORDER), track.size.height)),
            Bounds::new(
                point(track.right() - px(BORDER), track.origin.y),
                size(px(BORDER), track.size.height),
            ),
        ),
        Axis::Horizontal => (
            Bounds::new(track.origin, size(track.size.width, px(BORDER))),
            Bounds::new(
                point(track.origin.x, track.bottom() - px(BORDER)),
                size(track.size.width, px(BORDER)),
            ),
        ),
    };
    window.paint_quad(fill(inner, with_alpha(palette.inner_border, alpha)));
    window.paint_quad(fill(outer, with_alpha(palette.outer_border, alpha)));
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{CubicBezier, Curve, Metrics, State, thumb_extent, wheel_duration};

    #[test]
    fn wheel_duration_matches_inverse_delta() {
        assert!((wheel_duration(40.) - 0.2).abs() < 1e-9);
        assert!((wheel_duration(120.) - 0.2).abs() < 1e-9);
        assert!((wheel_duration(300.) - 0.15).abs() < 1e-9);
        assert!((wheel_duration(-480.) - 0.1).abs() < 1e-9);
        assert!((wheel_duration(4000.) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn ease_in_out_is_symmetric_and_flat_at_the_ends() {
        let b = CubicBezier::ease_in_out();
        assert!(b.solve(0.).abs() < 1e-6);
        assert!((b.solve(1.) - 1.).abs() < 1e-6);
        assert!((b.solve(0.5) - 0.5).abs() < 1e-4);
        assert!((b.solve(0.25) + b.solve(0.75) - 1.).abs() < 1e-4);
        assert!(b.slope(0.).abs() < 1e-6);
    }

    #[test]
    fn initial_slope_is_kept() {
        let b = CubicBezier::ease_in_out_with_slope(2.);
        assert!((b.slope(0.) - 2.).abs() < 1e-3);
    }

    #[test]
    fn curve_reaches_target() {
        let now = Instant::now();
        let c = Curve::new(0., -40., now);
        assert_eq!(c.value(now), 0.);
        assert_eq!(c.value(now + Duration::from_millis(200)), -40.);
        assert!(c.done(now + Duration::from_millis(200)));
        let mid = c.value(now + Duration::from_millis(100));
        assert!((mid + 20.).abs() < 0.5);
    }

    #[test]
    fn retarget_keeps_position_and_speed() {
        let now = Instant::now();
        let mut c = Curve::new(0., -40., now);
        let t = now + Duration::from_millis(80);
        let (pos, vel) = (c.value(t), c.velocity(t));
        c.retarget(-80., t);
        assert!((c.value(t) - pos).abs() < 1e-3);
        assert!((c.velocity(t) - vel).abs() / vel.abs() < 1e-2);
        assert_eq!(c.to, -80.);
        assert_eq!(c.value(t + Duration::from_secs(1)), -80.);
    }

    #[test]
    fn thumb_geometry() {
        // nothing to scroll
        assert_eq!(thumb_extent(Metrics::OVERLAY, 400., 400., 400., 0.), None);
        // too short for the minimum thumb
        assert_eq!(thumb_extent(Metrics::OVERLAY, 27., 27., 100., 0.), None);
        // half the content visible: half-length thumb
        assert_eq!(
            thumb_extent(Metrics::OVERLAY, 400., 400., 800., 0.),
            Some((0., 200.))
        );
        assert_eq!(
            thumb_extent(Metrics::OVERLAY, 400., 400., 800., 400.),
            Some((200., 200.))
        );
        // huge content: minimum length
        assert_eq!(
            thumb_extent(Metrics::OVERLAY, 400., 400., 1_000_000., 0.).map(|t| t.1),
            Some(26.)
        );
    }

    #[test]
    fn fade_timeline() {
        let now = Instant::now();
        let mut s = State::default();
        assert_eq!(s.thumb_alpha(now), 0.);
        s.show(now);
        assert_eq!(s.thumb_alpha(now + Duration::from_millis(499)), 1.);
        let half = s.thumb_alpha(now + Duration::from_millis(625));
        assert!((half - 0.5).abs() < 0.01);
        assert_eq!(s.thumb_alpha(now + Duration::from_millis(750)), 0.);
        // hovering keeps it; leaving restarts the countdown
        let later = now + Duration::from_millis(300);
        s.set_in_strip(true, later);
        assert!(s.hovered);
        assert_eq!(s.thumb_alpha(later + Duration::from_secs(5)), 1.);
        let exit = later + Duration::from_secs(5);
        s.set_in_strip(false, exit);
        assert_eq!(s.thumb_alpha(exit + Duration::from_millis(400)), 1.);
        assert_eq!(s.thumb_alpha(exit + Duration::from_millis(800)), 0.);
    }

    #[test]
    fn hovering_a_hidden_scrollbar_is_ignored() {
        let now = Instant::now();
        let mut s = State::default();
        s.set_in_strip(true, now);
        assert!(!s.hovered);
        // scrolling under the pointer shows it expanded straight away
        s.show(now);
        assert!(s.hovered);
        assert_eq!(s.expansion(now), 1.);
    }
}
