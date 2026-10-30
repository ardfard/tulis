mod actions;
mod assets;
mod cheat_sheet;
mod chrome;
mod commands;
mod editor;
mod header_dim;
mod keymap;
mod runs;
mod scratchpad;
mod sidebar;
mod theme;
mod toast;
mod utf16;
mod workspace;

use std::path::PathBuf;

use gpui::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_component::Root;
use tulis_core::config::Config;

use workspace::Workspace;

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            let (config, warning) = Config::load(Config::default_path().as_deref());
            keymap::bind(cx);
            let bounds = Bounds::centered(None, size(px(1100.), px(820.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Tulis".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let window = cx
                .open_window(options, |window, cx| {
                    let workspace = cx.new(|cx| Workspace::new(config, warning, path, window, cx));
                    workspace.update(cx, |ws, cx| ws.focus_editor(window, cx));
                    cx.new(|cx| Root::new(workspace, window, cx))
                })
                .expect("failed to open the Tulis window");
            window.update(cx, |_, _, cx| cx.activate(true)).ok();
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
        });
}
