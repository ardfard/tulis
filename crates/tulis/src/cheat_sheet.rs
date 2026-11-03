//! The cheat sheet: every listed command from the command table, grouped, with a filter box,
//! shown in a gpui-component dialog.

use gpui::{
    App, Context, Entity, Focusable, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div, prelude::*, px,
};
use gpui_component::WindowExt;
use gpui_component::input::{Input, InputEvent, InputState};

use crate::actions::ShowCheatSheet;
use crate::commands::{CheatRow, Group, cheat_sheet};
use crate::theme::Theme;
use crate::workspace::Workspace;

pub struct CheatSheet {
    pub filter: Entity<InputState>,
    theme: Theme,
    _subscription: Subscription,
}

impl CheatSheet {
    pub fn new(theme: Theme, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter commands…"));
        let subscription = cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            filter,
            theme,
            _subscription: subscription,
        }
    }

    /// The sections shown for the current filter text.
    pub fn sections(&self, cx: &App) -> Vec<(Group, Vec<CheatRow>)> {
        cheat_sheet(&self.filter.read(cx).value())
    }

    fn pill(&self, text: impl Into<SharedString>) -> impl IntoElement {
        let t = self.theme;
        div()
            .px_1()
            .rounded_sm()
            .border_1()
            .border_color(t.rule)
            .bg(t.background)
            .text_color(t.text)
            .child(text.into())
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
            .gap_3()
            .text_sm()
            .child(Input::new(&self.filter))
            .child(
                div()
                    .id("cheat-sheet-list")
                    .max_h(px(440.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .when(sections.is_empty(), |list| {
                        list.child(div().text_color(t.text_muted).child("No matching commands"))
                    })
                    .children(sections.into_iter().map(|(group, rows)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
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
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(div().flex_1().child(row.label))
                                    .child(div().w(px(150.)).child(self.pill(row.key)))
                                    .child(div().w(px(48.)).when(!row.vim.is_empty(), |cell| {
                                        cell.child(self.pill(row.vim))
                                    }))
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
        let theme = self.editor.read(cx).theme;
        let sheet = cx.new(|cx| CheatSheet::new(theme, window, cx));
        self.cheat_sheet = Some(sheet.clone());
        // The builder runs on every rebuild; focus the filter only the first time.
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                let filter = sheet.read(cx).filter.focus_handle(cx);
                window.focus(&filter, cx);
            }
            dialog
                .title("Keyboard shortcuts")
                .w(px(600.))
                .child(sheet.clone())
        });
    }
}
