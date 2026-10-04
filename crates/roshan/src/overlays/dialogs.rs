//! Small modal pieces: delete confirmation and the badge chooser.

use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};

use super::dialog;
use crate::app::Roshan;
use crate::assets::BADGES;
use crate::i18n::i18n;
use crate::theme::palette;
use crate::ui::{ButtonKind, button, hrow, icon, vstack};

impl Roshan {
    pub fn render_confirm_delete(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        let name = self
            .current_session()
            .map(|ix| self.config.sessions[ix].name.clone())
            .unwrap_or_default();
        dialog(
            cx.listener(|this, _, _, cx| {
                this.overlay = None;
                cx.notify();
            }),
            vstack(cx)
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(40.))
                        .rounded(px(12.))
                        .bg(p.danger_soft)
                        .child(icon("trash", px(18.), p.danger)),
                )
                .child(
                    div()
                        .pt(px(4.))
                        .text_size(px(15.))
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .child(i.tf("session.delete_title", &[("name", &name)])),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .line_height(px(19.))
                        .text_color(p.muted)
                        .child(i.t("session.delete_body")),
                )
                .child(
                    hrow(cx)
                        .w_full()
                        .pt(px(10.))
                        .gap(px(8.))
                        .child(div().flex_1())
                        .child(
                            button(
                                "cancel",
                                ButtonKind::Secondary,
                                None,
                                i.t("common.cancel"),
                                cx,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.overlay = None;
                                cx.notify();
                            })),
                        )
                        .child(
                            button(
                                "confirm-delete",
                                ButtonKind::DangerFilled,
                                Some("trash"),
                                i.t("common.delete"),
                                cx,
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| this.delete_current_session(window, cx),
                            )),
                        ),
                ),
            cx,
        )
    }

    pub fn render_badges(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        let current = self
            .current_session()
            .and_then(|ix| self.config.sessions[ix].badge.clone())
            .unwrap_or_default();
        // Selected: the primary fill; others: neutral, like the badge tiles.
        let tint = (p.primary, p.on_primary);
        dialog(
            cx.listener(|this, _, _, cx| {
                this.overlay = None;
                cx.notify();
            }),
            vstack(cx)
                .gap(px(12.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .child(i.t("session.badge")),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(BADGES.iter().map(|&badge| {
                            let selected = badge == current;
                            div()
                                .id(SharedString::from(badge))
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(44.))
                                .rounded(px(12.))
                                .cursor_pointer()
                                .bg(if selected { tint.0 } else { p.sunken })
                                .border_2()
                                .border_color(if selected { tint.0 } else { p.sunken })
                                .hover(|s| s.bg(p.surface_active))
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.set_badge(badge, cx)),
                                )
                                .child(icon(
                                    badge,
                                    px(20.),
                                    if selected { tint.1 } else { p.muted },
                                ))
                        })),
                ),
            cx,
        )
    }
}
