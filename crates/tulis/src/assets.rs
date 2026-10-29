//! Icon assets: gpui-component's default icons plus the few extra Lucide icons Tulis uses.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

gpui_kit_assets::icon_assets!(ExtraIcons, [Keyboard, Save, Share2]);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match ExtraIcons.load(path)? {
            Some(data) => Ok(Some(data)),
            None => gpui_kit_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names = ExtraIcons.list(path)?;
        names.extend(gpui_kit_assets::Assets.list(path)?);
        names.sort();
        names.dedup();
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use gpui::AssetSource;
    use gpui_kit_assets::IconName;

    use super::Assets;

    #[test]
    fn extra_and_default_icons_load() {
        for icon in [
            IconName::Keyboard,
            IconName::Save,
            IconName::Share2,
            IconName::Eye,
        ] {
            let path = icon.path();
            assert!(Assets.load(&path).unwrap().is_some(), "{path}");
        }
    }
}
