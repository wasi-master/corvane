//! "Corvane quit unexpectedly last time" (Corvane addition, no GHD
//! counterpart: GHD reports crashes itself and shows `crash/crash-app.tsx`).
//! Lists the file names of the reports the previous session left
//! (`corvane_core::crash_reports`); "Reveal in Finder" (GHD
//! `RevealInFileManagerLabel`, per platform) shows the newest one.
//! Nothing is read from the reports and nothing is uploaded.

use std::path::PathBuf;

use corvane_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::labels;
use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::code_ref;

pub struct CrashReportFoundDialog {
    reports: Vec<PathBuf>,
}

impl CrashReportFoundDialog {
    pub fn new(reports: Vec<PathBuf>) -> Self {
        Self { reports }
    }
}

impl Render for CrashReportFoundDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let names: Vec<String> = self
            .reports
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        let intro = if names.len() == 1 {
            "A crash report was saved on this Mac:"
        } else {
            "Crash reports were saved on this Mac:"
        };
        let newest = self.reports.first().cloned();
        let content = div()
            .w(zpx(420.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(intro)
            .child(
                div().flex().flex_col().gap(SPACING_HALF()).children(
                    names
                        .into_iter()
                        .map(|n| div().flex().child(code_ref(n, cx))),
                ),
            )
            .child(
                div()
                    .text_color(t.text_secondary)
                    .child("Reports stay on this Mac; Corvane never uploads them."),
            );
        dialog_with_kind(
            "crash-report-found",
            DialogKind::Warning,
            "Corvane quit unexpectedly last time",
            content,
            vec![
                DialogButton {
                    id: "crash-report-dismiss",
                    label: "Dismiss".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(|_, cx| Dispatcher::close_popup(cx)),
                },
                DialogButton {
                    id: "crash-report-reveal",
                    label: labels::REVEAL_IN_FILE_MANAGER.into(),
                    primary: true,
                    disabled: newest.is_none(),
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        if let Some(path) = &newest {
                            Dispatcher::show_in_finder(path, cx);
                        }
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
