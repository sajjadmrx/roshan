//! Item editor: name, timing, and the details of commands and links.

use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use roshan_core::{ItemKind, MAX_WAIT_SECS};

use super::sheet;
use crate::app::Roshan;
use crate::i18n::i18n;
use crate::theme::palette;
use crate::ui::{
    ButtonKind, button, field_label, hint, hrow, icon, icon_button, item_tile, vstack,
};

const PRESETS: [u32; 7] = [0, 1, 2, 3, 5, 10, 30];

impl Roshan {
    pub fn render_editor(
        &mut self,
        n: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let ix = self.current_session()?;
        let count = self.config.sessions[ix].items.len();
        let item = self.config.sessions[ix].items.get(n)?.clone();
        let p = palette(cx);
        let i = i18n(cx);
        let image = match &item.kind {
            ItemKind::App { target } => self.icon(target, cx),
            _ => None,
        };
        let is_last = n + 1 == count;
        let wait = item.wait_after_secs;

        let header = hrow(cx)
            .w_full()
            .gap(px(10.))
            .child(item_tile(&item, image, px(36.), &p))
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&self.inputs.item_name).h(px(36.))),
            )
            .child(
                icon_button("close-editor", "x", cx)
                    .on_click(cx.listener(|this, _, window, cx| this.close_editor(window, cx))),
            );

        let details = match &item.kind {
            ItemKind::Command { terminal, .. } => {
                let terminal = *terminal;
                let cwd_input = self.inputs.item_cwd.clone();
                Some(
                    vstack(cx)
                        .gap(px(12.))
                        .child(
                            vstack(cx)
                                .gap(px(6.))
                                .child(field_label(i.t("command.line"), cx))
                                .child(
                                    Input::new(&self.inputs.item_line)
                                        .prefix(icon("terminal", px(15.), p.faint))
                                        .h(px(36.)),
                                ),
                        )
                        .child(
                            vstack(cx)
                                .gap(px(6.))
                                .child(field_label(i.t("command.cwd"), cx))
                                .child(
                                    hrow(cx)
                                        .gap(px(6.))
                                        .child(
                                            div().flex_1().child(
                                                Input::new(&self.inputs.item_cwd).h(px(36.)),
                                            ),
                                        )
                                        .child(
                                            icon_button("edit-cwd", "folder", cx)
                                                .size(px(36.))
                                                .border_1()
                                                .border_color(p.border_strong)
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.browse_into(
                                                            cwd_input.clone(),
                                                            true,
                                                            window,
                                                            cx,
                                                        )
                                                    },
                                                )),
                                        ),
                                ),
                        )
                        .child(
                            hrow(cx)
                                .w_full()
                                .gap(px(12.))
                                .child(
                                    vstack(cx)
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div().text_size(px(13.)).child(i.t("command.terminal")),
                                        )
                                        .child(hint(
                                            if terminal {
                                                i.t("command.terminal_hint_on")
                                            } else {
                                                i.t("command.terminal_hint_off")
                                            },
                                            cx,
                                        )),
                                )
                                .child(Switch::new("edit-terminal").checked(terminal).on_click(
                                    cx.listener(move |this, checked: &bool, _, cx| {
                                        this.set_item_terminal(n, *checked, cx)
                                    }),
                                )),
                        )
                        .into_any_element(),
                )
            }
            ItemKind::Open { .. } => Some(
                vstack(cx)
                    .gap(px(6.))
                    .child(field_label(i.t("open.target"), cx))
                    .child(
                        Input::new(&self.inputs.item_target)
                            .prefix(icon("link", px(15.), p.faint))
                            .h(px(36.)),
                    )
                    .into_any_element(),
            ),
            ItemKind::App { .. } => None,
        };

        let presets = hrow(cx)
            .w_full()
            .gap(px(4.))
            .children(PRESETS.iter().map(|&secs| {
                let selected = secs == wait;
                div()
                    .id(SharedString::from(format!("preset-{secs}")))
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(30.))
                    .rounded(px(8.))
                    .text_size(px(12.))
                    .cursor_pointer()
                    .map(|d| {
                        if selected {
                            d.bg(p.primary)
                                .text_color(p.on_primary)
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        } else {
                            d.bg(p.sunken)
                                .text_color(p.muted)
                                .hover(|s| s.text_color(p.text).bg(p.surface_active))
                        }
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.set_wait(n, secs, cx)))
                    .child(i.num(secs))
            }));

        let stepper = hrow(cx)
            .flex_none()
            .gap(px(2.))
            .rounded(px(10.))
            .bg(p.sunken)
            .p(px(2.))
            .child(
                icon_button("wait-down", "minus", cx)
                    .size(px(28.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_wait(n, wait.saturating_sub(1), cx)
                    })),
            )
            .child(
                div()
                    .min_w(px(64.))
                    .text_center()
                    .text_size(px(13.))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(i.seconds(wait)),
            )
            .child(
                icon_button("wait-up", "plus", cx)
                    .size(px(28.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_wait(n, (wait + 1).min(MAX_WAIT_SECS), cx)
                    })),
            );

        let timing = vstack(cx)
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(14.))
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
            .child(
                hrow(cx)
                    .w_full()
                    .gap(px(8.))
                    .child(icon("timer", px(15.), p.accent_text))
                    .child(div().flex_1().text_size(px(13.)).child(i.t("editor.wait")))
                    .child(stepper),
            )
            .child(presets)
            .child(hint(
                if is_last {
                    i.t("editor.wait_last")
                } else {
                    i.t("editor.wait_hint")
                },
                cx,
            ));

        let footer = hrow(cx)
            .w_full()
            .gap(px(6.))
            .child(
                icon_button("move-up", "arrow-up", cx)
                    .when(n == 0, |b| b.opacity(0.35))
                    .when(n > 0, |b| {
                        b.on_click(cx.listener(move |this, _, _, cx| this.move_item(n, n - 1, cx)))
                    }),
            )
            .child(
                icon_button("move-down", "arrow-down", cx)
                    .when(is_last, |b| b.opacity(0.35))
                    .when(!is_last, |b| {
                        b.on_click(cx.listener(move |this, _, _, cx| this.move_item(n, n + 1, cx)))
                    }),
            )
            .child(div().flex_1())
            .child(
                button(
                    "remove",
                    ButtonKind::Danger,
                    Some("trash"),
                    i.t("editor.remove"),
                    cx,
                )
                .h(px(32.))
                .on_click(cx.listener(move |this, _, window, cx| this.remove_item(n, window, cx))),
            )
            .child(
                button("done", ButtonKind::Primary, None, i.t("editor.done"), cx)
                    .h(px(32.))
                    .on_click(cx.listener(|this, _, window, cx| this.close_editor(window, cx))),
            );

        Some(sheet(
            "editor",
            false,
            cx.listener(|this, _, window, cx| this.close_editor(window, cx)),
            vstack(cx)
                .id("editor-body")
                .w_full()
                .max_h(px(520.))
                .overflow_y_scroll()
                .gap(px(14.))
                .px(px(14.))
                .pt(px(8.))
                .pb(px(14.))
                .child(header)
                .children(details)
                .child(timing)
                .child(footer),
            cx,
        ))
    }
}
