//! Colors. A warm paper look; one place to tweak.

use gpui::{Hsla, hsla, rgb};

pub const GHOST_ALPHA: f32 = 0.1;

pub fn paper() -> Hsla {
    rgb(0xfbf8f1).into()
}
pub fn panel() -> Hsla {
    rgb(0xf3efe6).into()
}
pub fn ink() -> Hsla {
    rgb(0x2b2a27).into()
}
pub fn muted() -> Hsla {
    rgb(0x8a857a).into()
}
pub fn rule() -> Hsla {
    rgb(0xe6e0d4).into()
}
pub fn banner() -> Hsla {
    rgb(0xf6dcd2).into()
}
pub fn cursor() -> Hsla {
    rgb(0xb5522d).into()
}
pub fn selection() -> Hsla {
    hsla(0.11, 0.7, 0.6, 0.28)
}
pub fn alt_underline() -> Hsla {
    hsla(0.58, 0.35, 0.5, 0.7)
}
pub fn ai_underline() -> Hsla {
    hsla(0.76, 0.55, 0.55, 0.9)
}
pub fn trim() -> Hsla {
    hsla(0.0, 0.7, 0.5, 0.9)
}
