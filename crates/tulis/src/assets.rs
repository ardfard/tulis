//! Icon assets: gpui-component's default icons, the few extra Lucide icons Tulis uses, and the
//! X and LinkedIn logos for the Share menu.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

gpui_kit_assets::icon_assets!(ExtraIcons, [FlaskConical, Keyboard, Save, Share]);

/// Logos Lucide does not have, as `Icon` paths.
pub const X_LOGO: &str = "icons/x-logo.svg";
pub const LINKEDIN_LOGO: &str = "icons/linkedin.svg";

const LOGOS: [(&str, &[u8]); 2] = [
    (X_LOGO, include_bytes!("../assets/icons/x-logo.svg")),
    (
        LINKEDIN_LOGO,
        include_bytes!("../assets/icons/linkedin.svg"),
    ),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, data)) = LOGOS.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(data)));
        }
        match ExtraIcons.load(path)? {
            Some(data) => Ok(Some(data)),
            None => gpui_kit_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names = ExtraIcons.list(path)?;
        names.extend(
            LOGOS
                .iter()
                .map(|(p, _)| *p)
                .filter(|p| p.starts_with(path))
                .map(SharedString::from),
        );
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

    use super::{Assets, LINKEDIN_LOGO, X_LOGO};

    #[test]
    fn extra_and_default_icons_load() {
        for icon in [
            IconName::FlaskConical,
            IconName::Keyboard,
            IconName::Save,
            IconName::Share,
            IconName::Eye,
            IconName::PanelRight,
            IconName::ChevronDown,
        ] {
            let path = icon.path();
            assert!(Assets.load(&path).unwrap().is_some(), "{path}");
        }
        for path in [X_LOGO, LINKEDIN_LOGO] {
            assert!(Assets.load(path).unwrap().is_some(), "{path}");
            assert!(Assets.list("icons/").unwrap().iter().any(|p| p == path));
        }
    }
}
