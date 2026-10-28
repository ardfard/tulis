mod actions;
mod editor;
mod keymap;
mod runs;
mod theme;
mod utf16;
mod workspace;

use std::path::PathBuf;

use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size,
};
use tulis_core::config::Config;

use workspace::Workspace;

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    Application::new().run(move |cx: &mut App| {
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
                cx.new(|cx| Workspace::new(config, warning, path, window, cx))
            })
            .expect("failed to open the Tulis window");
        window
            .update(cx, |workspace, window, cx| {
                workspace.focus_editor(window, cx);
                cx.activate(true);
            })
            .ok();
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
    });
}
