//! Theme presets (Midnight, Paper, Sepia, Black), font fallback, and syncing gpui-component.

use gpui::{App, Hsla, hsla, rgb};

/// Every color the editor and its chrome paint with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub id: ThemeId,
    pub dark: bool,
    pub background: Hsla,
    /// Header, footer, panels and dialogs.
    pub panel: Hsla,
    /// Borders and separators.
    pub rule: Hsla,
    /// Error banner background.
    pub banner: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub accent: Hsla,
    pub ai_accent: Hsla,
    pub selection: Hsla,
    pub cursor: Hsla,
    /// Text alpha of ghosted text.
    pub ghost_alpha: f32,
    /// Text alpha of passages a Lab trim would cut.
    pub fade_alpha: f32,
    /// Background of Lab marks.
    pub mark_tint: Hsla,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeId {
    #[default]
    Midnight,
    Paper,
    Sepia,
    Black,
}

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

fn with_alpha(mut color: Hsla, alpha: f32) -> Hsla {
    color.a = alpha;
    color
}

impl ThemeId {
    pub const ALL: [ThemeId; 4] = [
        ThemeId::Midnight,
        ThemeId::Paper,
        ThemeId::Sepia,
        ThemeId::Black,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Midnight => "Midnight",
            ThemeId::Paper => "Paper",
            ThemeId::Sepia => "Sepia",
            ThemeId::Black => "Black",
        }
    }

    /// Case-insensitive; surrounding whitespace is ignored.
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|id| id.name().eq_ignore_ascii_case(name))
    }

    pub fn next(self) -> Self {
        let ix = Self::ALL.iter().position(|&id| id == self).unwrap_or(0);
        Self::ALL[(ix + 1) % Self::ALL.len()]
    }

    pub fn theme(self) -> Theme {
        match self {
            ThemeId::Midnight => {
                let accent = hex(0x8b7cf6);
                Theme {
                    id: self,
                    dark: true,
                    background: hex(0x070b1f),
                    panel: hex(0x0c1230),
                    rule: hex(0x1d2448),
                    banner: hex(0x3a1730),
                    text: hex(0xe8cdb5),
                    text_muted: hex(0x8a8199),
                    accent,
                    ai_accent: hex(0xc49cff),
                    selection: with_alpha(accent, 0.32),
                    cursor: hex(0xf2b38a),
                    ghost_alpha: 0.06,
                    fade_alpha: 0.35,
                    mark_tint: with_alpha(hex(0xf2c94c), 0.18),
                }
            }
            ThemeId::Paper => Theme {
                id: self,
                dark: false,
                background: hex(0xfbf8f1),
                panel: hex(0xf3efe6),
                rule: hex(0xe6e0d4),
                banner: hex(0xf6dcd2),
                text: hex(0x2b2a27),
                text_muted: hex(0x8a857a),
                accent: hsla(0.58, 0.35, 0.5, 1.),
                ai_accent: hsla(0.76, 0.55, 0.55, 1.),
                selection: hsla(0.11, 0.7, 0.6, 0.28),
                cursor: hex(0xb5522d),
                ghost_alpha: 0.12,
                fade_alpha: 0.35,
                mark_tint: hsla(0.14, 0.9, 0.6, 0.3),
            },
            ThemeId::Sepia => Theme {
                id: self,
                dark: false,
                background: hex(0xf4ecd8),
                panel: hex(0xebe0c6),
                rule: hex(0xdccfb0),
                banner: hex(0xeac9b5),
                text: hex(0x5b4636),
                text_muted: hex(0x9a8468),
                accent: hex(0xa0522d),
                ai_accent: hex(0x8e5ba8),
                selection: with_alpha(hex(0xc8a96e), 0.35),
                cursor: hex(0xa0522d),
                ghost_alpha: 0.12,
                fade_alpha: 0.35,
                mark_tint: with_alpha(hex(0xe0b040), 0.3),
            },
            ThemeId::Black => {
                let accent = hex(0x8b7cf6);
                Theme {
                    id: self,
                    dark: true,
                    background: hex(0x000000),
                    panel: hex(0x0b0b0b),
                    rule: hex(0x222222),
                    banner: hex(0x3a1010),
                    text: hex(0xd4d4d4),
                    text_muted: hex(0x7a7a7a),
                    accent,
                    ai_accent: hex(0xc49cff),
                    selection: with_alpha(accent, 0.3),
                    cursor: hex(0xe5e5e5),
                    ghost_alpha: 0.08,
                    fade_alpha: 0.35,
                    mark_tint: with_alpha(hex(0xf2c94c), 0.18),
                }
            }
        }
    }
}

/// The preset named in config, or Midnight plus a notice when the name is unknown.
pub fn theme_from_config(name: &str) -> (ThemeId, Option<String>) {
    match ThemeId::from_name(name) {
        Some(id) => (id, None),
        None => (
            ThemeId::Midnight,
            Some(format!("Unknown theme \"{}\"; using Midnight", name.trim())),
        ),
    }
}

/// Monospace families tried, in order, after the configured one.
pub const FONT_FALLBACKS: [&str; 2] = ["IBM Plex Mono", "JetBrains Mono"];

/// The handwriting face for small labels (sidebar tabs, Overflow title, Lab guide, cheat-sheet groups).
/// Bundled, so it looks the same on every machine. Caveat is licensed under the SIL OFL (see
/// `assets/fonts/OFL-Caveat.txt`).
pub const SCRIPT_FONT: &str = "Caveat";
/// Caveat runs small for its point size; labels using it are set a little larger.
pub const SCRIPT_SIZE: f32 = 21.0;
const CAVEAT_TTF: &[u8] = include_bytes!("../assets/fonts/Caveat.ttf");

/// Register the bundled fonts with the text system. Call once at startup, before the first window.
pub fn register_fonts(cx: &mut App) {
    if let Err(error) = cx
        .text_system()
        .add_fonts(vec![std::borrow::Cow::Borrowed(CAVEAT_TTF)])
    {
        eprintln!("could not register the bundled Caveat font: {error}");
    }
}

/// The platform's monospace font, used when no preferred family is installed.
pub fn platform_mono(cx: &App) -> String {
    if cx.has_global::<gpui_component::Theme>() {
        // gpui-component already swapped its default for an installed family.
        return gpui_component::Theme::global(cx)
            .mono_font_family
            .to_string();
    }
    if cfg!(target_os = "macos") {
        "Menlo".into()
    } else if cfg!(target_os = "windows") {
        "Consolas".into()
    } else {
        "DejaVu Sans Mono".into()
    }
}

/// The configured family if installed, else IBM Plex Mono, then JetBrains Mono, then
/// `platform_mono`.
pub fn resolve_font(configured: &str, installed: &[String], platform_mono: &str) -> String {
    std::iter::once(configured.trim())
        .chain(FONT_FALLBACKS)
        .find(|family| !family.is_empty() && installed.iter().any(|name| name == family))
        .unwrap_or(platform_mono)
        .to_string()
}

/// Make gpui-component's widgets use `theme`'s colors. Does nothing before
/// `gpui_component::init` (editor tests run without it).
pub fn apply_to_components(theme: &Theme, cx: &mut App) {
    use gpui_component::{Theme as ComponentTheme, ThemeMode};

    if !cx.has_global::<ComponentTheme>() {
        return;
    }
    let mode = if theme.dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    if ComponentTheme::global(cx).mode != mode {
        ComponentTheme::change(mode, None, cx);
    }
    let t = *theme;
    ComponentTheme::update(cx, |c| {
        let hover = with_alpha(t.accent, 0.14);
        c.background = t.background;
        c.foreground = t.text;
        c.border = t.rule;
        c.input = t.rule;
        c.ring = t.accent;
        c.caret = t.cursor;
        c.selection = t.selection;
        c.muted = t.panel;
        c.muted_foreground = t.text_muted;
        c.accent = hover;
        c.accent_foreground = t.text;
        c.primary = t.accent;
        c.primary_hover = with_alpha(t.accent, 0.85);
        c.primary_active = with_alpha(t.accent, 0.7);
        c.primary_foreground = t.background;
        c.secondary = t.panel;
        c.secondary_hover = hover;
        c.secondary_active = with_alpha(t.accent, 0.22);
        c.secondary_foreground = t.text;
        c.popover = t.panel;
        c.popover_foreground = t.text;
        c.list_hover = hover;
        c.list_active = with_alpha(t.accent, 0.22);
        c.list_active_border = t.accent;
        c.tab_bar = t.panel;
        c.tab = t.panel;
        c.tab_active = t.background;
        c.tab_foreground = t.text_muted;
        c.tab_active_foreground = t.accent;
        c.title_bar = t.panel;
        c.title_bar_border = t.rule;
        c.scrollbar = t.background;
        c.scrollbar_thumb = with_alpha(t.text_muted, 0.4);
        c.scrollbar_thumb_hover = with_alpha(t.text_muted, 0.6);
        c.overlay = with_alpha(gpui::black(), if t.dark { 0.55 } else { 0.25 });
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_bundled_script_font_is_a_caveat_truetype_file() {
        assert_eq!(
            &super::CAVEAT_TTF[..4],
            &[0, 1, 0, 0],
            "not a TrueType file"
        );
        assert!(super::CAVEAT_TTF.windows(6).any(|w| w == b"Caveat"));
        assert_eq!(super::SCRIPT_FONT, "Caveat");
    }

    use super::*;

    #[test]
    fn names_round_trip_case_insensitively() {
        for id in ThemeId::ALL {
            assert_eq!(ThemeId::from_name(&id.name().to_uppercase()), Some(id));
        }
        assert_eq!(ThemeId::from_name(" sepia "), Some(ThemeId::Sepia));
        assert_eq!(ThemeId::from_name("solarized"), None);
    }

    #[test]
    fn next_cycles_through_all_presets() {
        let mut id = ThemeId::Midnight;
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(id);
            id = id.next();
        }
        assert_eq!(seen, ThemeId::ALL.to_vec());
        assert_eq!(id, ThemeId::Midnight);
    }

    #[test]
    fn presets_follow_the_spec() {
        let midnight = ThemeId::Midnight.theme();
        assert_eq!(midnight.background, hex(0x070b1f));
        assert_eq!(midnight.text, hex(0xe8cdb5));
        assert_eq!(midnight.accent, hex(0x8b7cf6));
        assert_eq!(midnight.ghost_alpha, 0.06);
        assert_eq!(ThemeId::Paper.theme().ghost_alpha, 0.12);
        assert_eq!(ThemeId::Black.theme().background, hex(0x000000));
        for id in ThemeId::ALL {
            let t = id.theme();
            assert_eq!(t.id, id);
            assert!(
                (t.text.l - t.background.l).abs() > 0.4,
                "{id:?} lacks contrast"
            );
        }
    }

    #[test]
    fn unknown_config_theme_falls_back_to_midnight_with_a_notice() {
        assert_eq!(theme_from_config("Paper"), (ThemeId::Paper, None));
        let (id, notice) = theme_from_config("neon");
        assert_eq!(id, ThemeId::Midnight);
        assert_eq!(
            notice.as_deref(),
            Some("Unknown theme \"neon\"; using Midnight")
        );
    }

    #[test]
    fn font_falls_back_in_order() {
        let installed = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let platform = "DejaVu Sans Mono";
        assert_eq!(
            resolve_font(
                "Fira Code",
                &installed(&["Fira Code", "IBM Plex Mono"]),
                platform
            ),
            "Fira Code"
        );
        assert_eq!(
            resolve_font(
                "Fira Code",
                &installed(&["JetBrains Mono", "IBM Plex Mono"]),
                platform
            ),
            "IBM Plex Mono"
        );
        assert_eq!(
            resolve_font("Fira Code", &installed(&["JetBrains Mono"]), platform),
            "JetBrains Mono"
        );
        assert_eq!(resolve_font("Fira Code", &[], platform), platform);
        assert_eq!(
            resolve_font("", &installed(&["IBM Plex Mono"]), platform),
            "IBM Plex Mono"
        );
    }
}
