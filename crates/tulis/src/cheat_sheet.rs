//! The cheat sheet: every listed command from the command table, grouped, with a filter box,
//! shown in a gpui-component dialog.

use gpui::{
    App, Context, Entity, Focusable, FontWeight, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Window, div, prelude::*, px,
};
use gpui_component::WindowExt;
use gpui_component::input::{Input, InputEvent, InputState};

use crate::actions::ShowCheatSheet;
use crate::commands::{CheatRow, Group, cheat_sheet};
use crate::pills;
use crate::theme::Theme;
use crate::workspace::Workspace;

pub struct CheatSheet {
    pub filter: Entity<InputState>,
    theme: Theme,
    /// The page font, for the vim keys.
    font: SharedString,
    _subscription: Subscription,
}

impl CheatSheet {
    pub fn new(
        theme: Theme,
        font: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter commands…"));
        let subscription = cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            filter,
            theme,
            font,
            _subscription: subscription,
        }
    }

    /// The sections shown for the current filter text.
    pub fn sections(&self, cx: &App) -> Vec<(Group, Vec<CheatRow>)> {
        cheat_sheet(&self.filter.read(cx).value())
    }

    /// A row's keys as pills, the older bindings after "or".
    fn keys(&self, pills: Vec<String>) -> impl IntoElement {
        let t = self.theme;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .when(pills.is_empty(), |d| d.text_color(t.text_muted).child("—"))
            .children(pills.into_iter().enumerate().flat_map(move |(ix, key)| {
                let or = (ix > 0).then(|| {
                    div()
                        .text_color(t.text_muted)
                        .child("or")
                        .into_any_element()
                });
                or.into_iter()
                    .chain([pills::pill(key, 13., &t).into_any_element()])
            }))
    }
}

impl Render for CheatSheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let sections = self.sections(cx);
        div()
            // The dialog lives outside the workspace's element tree, so the toggle key must be
            // handled here for it to close the sheet while the filter has focus.
            .on_action(cx.listener(|_, _: &ShowCheatSheet, window, cx| window.close_dialog(cx)))
            .flex()
            .flex_col()
            .gap(px(14.))
            .text_size(px(14.))
            .text_color(t.text)
            .child(Input::new(&self.filter))
            .child(
                div()
                    .id("cheat-sheet-list")
                    .max_h(px(440.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .when(sections.is_empty(), |list| {
                        list.child(div().text_color(t.text_muted).child("No matching commands"))
                    })
                    .children(sections.into_iter().map(|(group, rows)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .font_family(crate::theme::SCRIPT_FONT)
                                    .text_size(px(crate::theme::SCRIPT_SIZE))
                                    .text_color(t.accent)
                                    .child(group.label()),
                            )
                            .children(rows.into_iter().map(|row| {
                                div()
                                    .id(row.id)
                                    .min_h(px(30.))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(div().flex_1().child(row.label))
                                    .child(div().w(px(330.)).child(self.keys(row.pills)))
                                    .child(
                                        div()
                                            .w(px(100.))
                                            .font_family(self.font.clone())
                                            .text_color(t.accent)
                                            .child(row.vim),
                                    )
                            }))
                    })),
            )
    }
}

impl Workspace {
    /// Open the cheat sheet dialog, or close it if it is open. Needs a `Root` window (the app
    /// always has one; tests that skip `Root` must not call this).
    pub(crate) fn show_cheat_sheet(
        &mut self,
        _: &ShowCheatSheet,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.root::<gpui_component::Root>().flatten().is_none() {
            return;
        }
        if window.has_active_dialog(cx) {
            window.close_dialog(cx);
            return;
        }
        let editor = self.editor.read(cx);
        let (theme, font) = (editor.theme, editor.font_family.clone());
        let sheet = cx.new(|cx| CheatSheet::new(theme, font, window, cx));
        self.cheat_sheet = Some(sheet.clone());
        // The builder runs on every rebuild; focus the filter only the first time.
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                let filter = sheet.read(cx).filter.focus_handle(cx);
                window.focus(&filter, cx);
            }
            dialog
                .title(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .text_size(px(20.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text)
                        .child("Keyboard shortcuts")
                        .child(pills::pill("Esc", 13., &theme).font_weight(FontWeight::NORMAL)),
                )
                .close_button(false)
                .w(px(840.))
                .child(sheet.clone())
        });
    }
}
