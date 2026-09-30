//! Repository › Add License… - a Corvane addition with no GHD counterpart
//! (flag `455-add-license`, see `.docs/deviations.md`). Picks one
//! of the license templates Create a New Repository offers
//! (`ui/add-repository/create-repository.tsx` `renderLicenses`) and writes
//! it to `LICENSE` in the repository (`Dispatcher::add_license`), which
//! never replaces an existing license file.

use corvane_core::Dispatcher;
use corvane_core::templates::License;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{SelectHandler, SelectItem, labeled, select_button_items};

pub struct AddLicenseDialog {
    repo: u64,
    licenses: Vec<License>,
    /// Index into `licenses`.
    selected: usize,
}

impl AddLicenseDialog {
    pub fn new(repo: u64) -> Self {
        Self {
            repo,
            licenses: corvane_core::templates::licenses(),
            selected: 0,
        }
    }

    /// The featured licenses, a separator, the rest (`renderLicenses`
    /// without "None").
    fn license_select(&self, cx: &Context<Self>) -> impl IntoElement {
        let featured = self.licenses.iter().filter(|l| l.featured).count();
        let mut items = Vec::new();
        for (ix, license) in self.licenses.iter().enumerate() {
            if ix == featured && featured > 0 {
                items.push(SelectItem::Separator);
            }
            items.push(SelectItem::Option(license.name.clone().into()));
        }
        let weak = cx.weak_entity();
        let on_select: SelectHandler = std::rc::Rc::new(move |ix, _, cx| {
            weak.update(cx, |this, cx| {
                this.selected = ix;
                cx.notify();
            })
            .ok();
        });
        select_button_items(
            "add-license-select",
            self.licenses
                .get(self.selected)
                .map(|l| l.name.clone())
                .unwrap_or_default(),
            items,
            Some(self.selected),
            false,
            on_select,
            cx,
        )
    }
}

impl Render for AddLicenseDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let repo = self.repo;
        let picked = self.licenses.get(self.selected).map(|l| l.name.clone());
        dialog(
            "add-license",
            "Add License",
            div()
                .w(zpx(358.))
                .flex()
                .flex_col()
                .gap(SPACING())
                .child(labeled("License", self.license_select(cx), cx))
                .child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .child(
                            "Writes the license to a LICENSE file in the repository, filled in \
                             with your Git name and this year. An existing license file is \
                             never replaced.",
                        ),
                ),
            vec![
                DialogButton {
                    id: "add-license-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "add-license-ok",
                    label: "Add License".into(),
                    primary: true,
                    disabled: picked.is_none(),
                    on_click: Box::new(move |_, cx| {
                        if let Some(name) = picked.clone() {
                            Dispatcher::add_license(repo, name, cx);
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
