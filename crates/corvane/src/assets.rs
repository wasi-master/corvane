//! Asset source: Corvane's own assets (Primer Octicons) layered over gpui-kit's
//! bundled Lucide icons, which the kit components need for their own chrome.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "octicons/*.svg"]
#[include = "illustrations/*.svg"]
struct Embedded;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(file) = Embedded::get(path) {
            return Ok(Some(file.data));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> = Embedded::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| SharedString::from(p.to_string()))
            .collect();
        out.extend(gpui_kit::assets::Assets.list(path)?);
        Ok(out)
    }
}
