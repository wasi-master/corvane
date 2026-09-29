//! Image diffs (GHD `ui/diff/image-diffs/*` + `styles/ui/_diff.scss`
//! `.panel.image`): a new or deleted image, or the modified-image switcher
//! with 2-up, Swipe, Onion Skin and Difference. Images are decoded once for
//! their dimensions; "Difference" is a CPU blend (GPUI has no `mix-blend-mode`).
//! Deviation: overlaid images are letterboxed with `ObjectFit::Contain` inside
//! the shared box rather than top-left aligned.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use corvane_core::{Dispatcher, FileStatusKind, ImageBlob, ImageDiffType};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::format::format_bytes;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

/// GHD `.tab-bar` inside `.panel.image`.
const TAB_BAR_WIDTH: Pixels = px(350.);
/// GHD swipe `SliderOverflow`.
const SLIDER_OVERFLOW: Pixels = px(14.);

struct Side {
    image: Arc<Image>,
    bytes: usize,
    /// Natural size in pixels (`None` when the format could not be decoded).
    size: Option<(u32, u32)>,
}

impl Side {
    fn from_blob(blob: &ImageBlob) -> Self {
        let format = match blob.media_type.as_str() {
            "image/jpg" | "image/jpeg" => ImageFormat::Jpeg,
            "image/gif" => ImageFormat::Gif,
            "image/webp" => ImageFormat::Webp,
            "image/bmp" => ImageFormat::Bmp,
            "image/x-icon" => ImageFormat::Ico,
            _ => ImageFormat::Png,
        };
        let size = image::ImageReader::new(std::io::Cursor::new(&blob.bytes))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok());
        Self {
            image: Arc::new(Image::from_bytes(format, blob.bytes.clone())),
            bytes: blob.bytes.len(),
            size,
        }
    }
}

pub struct ImageDiff {
    previous: Option<Side>,
    current: Option<Side>,
    status: FileStatusKind,
    swipe: Entity<SliderState>,
    onion: Entity<SliderState>,
    /// Measured size of the sizing container (one frame behind).
    container: Rc<Cell<Size<Pixels>>>,
    difference: Option<Arc<Image>>,
    difference_pending: bool,
}

impl ImageDiff {
    pub fn new(
        previous: Option<&ImageBlob>,
        current: Option<&ImageBlob>,
        status: FileStatusKind,
        cx: &mut Context<Self>,
    ) -> Self {
        let swipe = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(0.1)
                .default_value(0.)
        });
        // GHD starts the onion skin at 1 %
        let onion = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(0.1)
                .default_value(1.)
        });
        for s in [&swipe, &onion] {
            cx.subscribe(s, |_, _, _: &SliderEvent, cx| cx.notify())
                .detach();
        }
        Self {
            previous: previous.map(Side::from_blob),
            current: current.map(Side::from_blob),
            status,
            swipe,
            onion,
            container: Rc::new(Cell::new(Size::default())),
            difference: None,
            difference_pending: false,
        }
    }

    /// GHD `getAspectFitSize`: shrink (never grow) to fit the container.
    fn aspect_fit(image: (u32, u32), container: Size<Pixels>) -> Size<Pixels> {
        let (w, h) = (image.0 as f32, image.1 as f32);
        let (cw, ch) = (f32::from(container.width), f32::from(container.height));
        let height_ratio = if ch < h && ch > 0. { h / ch } else { 1. };
        let width_ratio = if cw < w && cw > 0. { w / cw } else { 1. };
        let mut ratio = width_ratio.max(1.);
        if width_ratio < height_ratio {
            ratio = height_ratio.max(1.);
        }
        size(px(w / ratio), px(h / ratio))
    }

    /// GHD `getMaxFitSize`: the box both images share.
    fn max_fit(&self, container: Size<Pixels>) -> Size<Pixels> {
        let fit = |side: &Option<Side>| {
            side.as_ref()
                .and_then(|s| s.size)
                .map(|s| Self::aspect_fit(s, container))
                .unwrap_or_default()
        };
        let (p, c) = (fit(&self.previous), fit(&self.current));
        size(p.width.max(c.width), p.height.max(c.height))
    }

    /// Captures the sizing container's bounds for the next frame.
    fn measure(&self, cx: &Context<Self>) -> AnyElement {
        let cell = self.container.clone();
        let entity = cx.entity();
        canvas(
            move |bounds, window, _| {
                if cell.get() != bounds.size {
                    cell.set(bounds.size);
                    window.on_next_frame(move |_, cx| entity.update(cx, |_, cx| cx.notify()));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    fn image_element(side: &Side, box_size: Size<Pixels>, border: Hsla) -> AnyElement {
        let fit = side
            .size
            .map(|s| Self::aspect_fit(s, box_size))
            .unwrap_or(box_size);
        div()
            .relative()
            .w(fit.width)
            .h(fit.height)
            .child(checkerboard())
            .child(
                img(side.image.clone())
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .border_1()
                    .border_color(border),
            )
            .into_any_element()
    }

    fn footer(side: &Side, cx: &App) -> AnyElement {
        let t = cx.ghd();
        let (w, h) = side.size.unwrap_or((0, 0));
        let strong = |s: &str| div().font_weight(FontWeight::SEMIBOLD).child(s.to_string());
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .justify_center()
            .gap(px(3.))
            .text_size(FONT_SIZE)
            .text_color(t.text_secondary)
            .child(strong("W:"))
            .child(format!("{w}px |"))
            .child(strong("H:"))
            .child(format!("{h}px |"))
            .child(strong("Size:"))
            .child(format_bytes(side.bytes as i64, 2))
            .into_any_element()
    }

    /// `TwoUp`
    fn two_up(&self, previous: &Side, current: &Side, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let container = self.container.get();
        // room for the headers / footers / summary rows
        let image_box = size(
            ((container.width - SPACING_DOUBLE * 2.) / 2. - SPACING_HALF).max(px(0.)),
            (container.height - px(90.)).max(px(0.)),
        );
        let max_fit = self.max_fit(image_box);
        let column = |side: &Side, label: &str, color: Hsla| {
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .min_w_0()
                .min_h_0()
                .max_w(max_fit.width.max(px(200.)))
                .mb(SPACING_HALF)
                .text_color(color)
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .pb(px(10.))
                        .child(label.to_string()),
                )
                .child(Self::image_element(side, image_box, color))
                .child(Self::footer(side, cx))
        };
        let diff_bytes = current.bytes as i64 - previous.bytes as i64;
        let percent = if previous.bytes == 0 {
            0
        } else {
            ((current.bytes as f64 / previous.bytes as f64) * 100.)
                .round()
                .abs() as i64
        };
        let summary = if diff_bytes != 0 {
            format!(
                "{}{} ({percent}%)",
                if diff_bytes >= 0 { "+" } else { "" },
                format_bytes(diff_bytes, 2)
            )
        } else {
            "No size difference".to_string()
        };
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .min_h_0()
            .min_w_0()
            .child(self.measure(cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .max_h_full()
                    .min_h_0()
                    .min_w_0()
                    .px(SPACING_DOUBLE)
                    .gap(SPACING)
                    .child(column(previous, "Deleted", t.color_deleted))
                    .child(column(current, "Added", t.color_new)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.))
                    .text_color(t.text_secondary)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Diff:")
                    .child(
                        div()
                            .when(diff_bytes > 0, |d| d.text_color(t.color_new))
                            .when(diff_bytes < 0, |d| d.text_color(t.color_deleted))
                            .child(summary),
                    ),
            )
            .into_any_element()
    }

    /// The overlay box shared by Swipe / Onion Skin / Difference.
    fn overlay_box(&self, cx: &Context<Self>) -> Size<Pixels> {
        let container = self.container.get();
        let _ = cx;
        self.max_fit(size(container.width, container.height))
    }

    /// `Swipe`: the slider reveals the new image from the right.
    fn swipe(&self, previous: &Side, current: &Side, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let box_size = self.overlay_box(cx);
        let percentage = self.swipe.read(cx).value().start();
        let swiper_width = box_size.width * (1. - percentage / 100.);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .min_h_0()
            .child(
                div()
                    .w(box_size.width + SLIDER_OVERFLOW)
                    .mb(SPACING)
                    .child(Slider::new(&self.swipe).horizontal()),
            )
            .child(
                div()
                    .id("image-diff-sizing")
                    .relative()
                    .flex_1()
                    .w_full()
                    .flex()
                    .justify_center()
                    .items_center()
                    .min_h_0()
                    .child(self.measure(cx))
                    .child(
                        div()
                            .relative()
                            .w(box_size.width)
                            .h(box_size.height)
                            .child(
                                // previous: clipped on the right by the swiper
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .h(box_size.height)
                                    .w((box_size.width - swiper_width).max(px(0.)))
                                    .overflow_hidden()
                                    .child(div().w(box_size.width).h(box_size.height).child(
                                        Self::image_element(previous, box_size, t.color_deleted),
                                    )),
                            )
                            .child(
                                // current: clipped on the left
                                div()
                                    .absolute()
                                    .top_0()
                                    .right_0()
                                    .h(box_size.height)
                                    .w(swiper_width.max(px(0.)))
                                    .overflow_hidden()
                                    .flex()
                                    .justify_end()
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(box_size.width)
                                            .h(box_size.height)
                                            .child(Self::image_element(
                                                current,
                                                box_size,
                                                t.color_new,
                                            )),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// `OnionSkin`: the slider cross-fades the new image over the old one.
    fn onion_skin(&self, previous: &Side, current: &Side, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let box_size = self.overlay_box(cx);
        let crossfade = self.onion.read(cx).value().start() / 100.;
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .min_h_0()
            .child(
                div()
                    .w(box_size.width / 2.)
                    .mb(SPACING)
                    .child(Slider::new(&self.onion).horizontal()),
            )
            .child(
                div()
                    .id("image-diff-sizing")
                    .relative()
                    .flex_1()
                    .w_full()
                    .flex()
                    .justify_center()
                    .items_center()
                    .min_h_0()
                    .child(self.measure(cx))
                    .child(
                        div()
                            .relative()
                            .w(box_size.width)
                            .h(box_size.height)
                            .child(div().absolute().top_0().left_0().child(Self::image_element(
                                previous,
                                box_size,
                                t.color_deleted,
                            )))
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .opacity(crossfade)
                                    .child(Self::image_element(current, box_size, t.color_new)),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// `DifferenceBlend`: |current − previous| per pixel.
    fn difference(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let box_size = self.overlay_box(cx);
        if self.difference.is_none() && !self.difference_pending {
            self.difference_pending = true;
            let a = self.previous.as_ref().map(|s| s.image.clone());
            let b = self.current.as_ref().map(|s| s.image.clone());
            let task = cx.background_executor().spawn(async move {
                let (a, b) = (a?, b?);
                difference_image(&a.bytes, &b.bytes)
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                this.update(cx, |this, cx| {
                    this.difference_pending = false;
                    this.difference = result.map(Arc::new);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        div()
            .id("image-diff-sizing")
            .relative()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .min_h_0()
            .child(self.measure(cx))
            .when_some(self.difference.clone(), |d, image| {
                d.child(
                    div()
                        .relative()
                        .w(box_size.width)
                        .h(box_size.height)
                        .child(checkerboard())
                        .child(
                            img(image)
                                .absolute()
                                .inset_0()
                                .size_full()
                                .object_fit(ObjectFit::Contain)
                                .border_1()
                                .border_color(t.color_modified),
                        ),
                )
            })
            .into_any_element()
    }

    /// `NewImageDiff` / `DeletedImageDiff`: one image with its header.
    fn single(&self, side: &Side, label: &str, color: Hsla, cx: &Context<Self>) -> AnyElement {
        let container = self.container.get();
        let image_box = size(
            (container.width - SPACING_DOUBLE).max(px(0.)),
            (container.height - px(40.)).max(px(0.)),
        );
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .min_h_0()
            .text_color(color)
            .child(self.measure(cx))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .pb(px(10.))
                    .child(label.to_string()),
            )
            .child(Self::image_element(side, image_box, color))
            .into_any_element()
    }
}

impl Render for ImageDiff {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let kind = corvane_core::AppState::try_global(cx)
            .map(|s| s.read(cx).settings.image_diff_type)
            .unwrap_or_default();
        let body: AnyElement = match (&self.previous, &self.current) {
            (Some(previous), Some(current)) => {
                let tabs = ["2-up", "Swipe", "Onion Skin", "Difference"];
                let selected = match kind {
                    ImageDiffType::TwoUp => 0,
                    ImageDiffType::Swipe => 1,
                    ImageDiffType::OnionSkin => 2,
                    ImageDiffType::Difference => 3,
                };
                let switcher = div()
                    .id("image-diff-tabs")
                    .w(TAB_BAR_WIDTH)
                    .h(px(29.))
                    .flex_none()
                    .flex()
                    .flex_row()
                    .gap(SPACING)
                    .mt(SPACING)
                    .mx_auto()
                    .mb(SPACING)
                    .children(tabs.iter().enumerate().map(|(ix, label)| {
                        let is_selected = ix == selected;
                        div()
                            .id(("image-diff-tab", ix))
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(BORDER_RADIUS)
                            .cursor_pointer()
                            .text_size(FONT_SIZE)
                            .when(is_selected, |d| {
                                d.bg(t.tab_bar_active)
                                    .text_color(t.box_selected_active_text)
                            })
                            .when(!is_selected, |d| {
                                let hover = t.tab_bar_hover_background;
                                d.hover(move |s| s.bg(hover))
                            })
                            .on_click(move |_, _, cx| {
                                let next = match ix {
                                    0 => ImageDiffType::TwoUp,
                                    1 => ImageDiffType::Swipe,
                                    2 => ImageDiffType::OnionSkin,
                                    _ => ImageDiffType::Difference,
                                };
                                Dispatcher::set_image_diff_type(next, cx);
                            })
                            .child(label.to_string())
                    }));
                let content = match kind {
                    ImageDiffType::TwoUp => self.two_up(previous, current, cx),
                    ImageDiffType::Swipe => self.swipe(previous, current, cx),
                    ImageDiffType::OnionSkin => self.onion_skin(previous, current, cx),
                    ImageDiffType::Difference => {
                        // the borrow of `previous`/`current` ends before the mutable call
                        let _ = (previous, current);
                        self.difference(cx)
                    }
                };
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .child(switcher)
                    .child(div().flex_1().min_h_0().child(content))
                    .into_any_element()
            }
            (None, Some(current))
                if matches!(self.status, FileStatusKind::New | FileStatusKind::Untracked) =>
            {
                self.single(current, "Added", t.color_new, cx)
            }
            (Some(previous), None) if self.status == FileStatusKind::Deleted => {
                self.single(previous, "Deleted", t.color_deleted, cx)
            }
            (_, Some(current)) => self.single(current, "Added", t.color_new, cx),
            (Some(previous), None) => self.single(previous, "Deleted", t.color_deleted, cx),
            (None, None) => div().into_any_element(),
        };
        // `.panel.image`
        div()
            .id("image-diff")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .p(SPACING)
            .bg(t.background)
            .text_size(FONT_SIZE)
            .child(body)
    }
}

/// GHD `checkboard-background` mixin behind transparent images.
fn checkerboard() -> AnyElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            let light = rgb(0xffffff);
            let dark = rgb(0xcccccc);
            window.paint_quad(fill(bounds, light));
            let cell = px(10.);
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                let mut y = bounds.top();
                let mut row = 0;
                while y < bounds.bottom() {
                    let mut x = bounds.left() + if row % 2 == 0 { px(0.) } else { cell };
                    while x < bounds.right() {
                        window.paint_quad(fill(Bounds::new(point(x, y), size(cell, cell)), dark));
                        x += cell * 2.;
                    }
                    y += cell;
                    row += 1;
                }
            });
        },
    )
    .absolute()
    .inset_0()
    .into_any_element()
}

/// |a − b| per channel over the larger of the two sizes (the smaller image is
/// resized to match), encoded as PNG for GPUI.
fn difference_image(a: &[u8], b: &[u8]) -> Option<Image> {
    use image::imageops::FilterType;
    let a = image::load_from_memory(a).ok()?.to_rgba8();
    let b = image::load_from_memory(b).ok()?.to_rgba8();
    let width = a.width().max(b.width());
    let height = a.height().max(b.height());
    let fit = |img: image::RgbaImage| {
        if img.width() == width && img.height() == height {
            img
        } else {
            image::imageops::resize(&img, width, height, FilterType::Triangle)
        }
    };
    let (a, b) = (fit(a), fit(b));
    let mut out = image::RgbaImage::new(width, height);
    for (x, y, px_out) in out.enumerate_pixels_mut() {
        let pa = a.get_pixel(x, y).0;
        let pb = b.get_pixel(x, y).0;
        *px_out = image::Rgba([
            pa[0].abs_diff(pb[0]),
            pa[1].abs_diff(pb[1]),
            pa[2].abs_diff(pb[2]),
            255,
        ]);
    }
    let mut bytes = Vec::new();
    out.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .ok()?;
    Some(Image::from_bytes(ImageFormat::Png, bytes))
}
