//! The home screen: every session as a compact card with a Run button.

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use roshan_core::{ItemKind, RunSummary, Session};

use crate::app::{Roshan, Screen};
use crate::i18n::i18n;
use crate::theme::palette;
use crate::ui::{ButtonKind, button, hrow, icon, progress_bar, rtl, spinner, vstack};

impl Roshan {
    pub fn badge_tile(&self, session: &Session, size: Pixels, cx: &Context<Self>) -> Div {
        let p = palette(cx);
        let glyph = session.badge.as_deref().unwrap_or("sunrise");
        // One neutral tile for every badge: the icon carries the meaning.
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(size)
            .rounded(size * 0.28)
            .bg(p.sunken)
            .border_1()
            .border_color(p.border)
            .child(icon(glyph, size * 0.52, p.text))
    }

    pub fn render_sessions(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if self.config.sessions.is_empty() {
            return self.render_sessions_empty(cx).into_any_element();
        }
        let p = palette(cx);
        let i = i18n(cx);
        let banner = self.render_update_banner(cx);
        let cards: Vec<AnyElement> = (0..self.config.sessions.len())
            .map(|ix| self.session_card(ix, cx).into_any_element())
            .collect();
        div()
            .id("sessions")
            .size_full()
            .overflow_y_scroll()
            .px(px(12.))
            .pb(px(14.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(banner)
            .children(cards)
            .child(
                hrow(cx)
                    .id("new-session")
                    .justify_center()
                    .gap(px(6.))
                    .h(px(44.))
                    .rounded(px(14.))
                    .border_1()
                    .border_dashed()
                    .border_color(p.border_strong)
                    .text_color(p.muted)
                    .text_size(px(13.))
                    .cursor_pointer()
                    .hover(|s| s.bg(p.surface).text_color(p.text))
                    .on_click(cx.listener(|this, _, window, cx| this.create_session(window, cx)))
                    .child(icon("plus", px(15.), p.muted))
                    .child(i.t("sessions.new")),
            )
            .into_any_element()
    }

    fn render_sessions_empty(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .px(px(28.))
            .pb(px(30.))
            .child(crate::ui::logo(px(64.)))
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .child(i.t("sessions.empty_title")),
            )
            .child(
                div()
                    .text_center()
                    .text_color(p.muted)
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .child(i.t("sessions.empty_body")),
            )
            .child(div().h(px(4.)))
            .child(
                button(
                    "create",
                    ButtonKind::Primary,
                    Some("plus"),
                    i.t("sessions.empty_action"),
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.create_session(window, cx))),
            )
    }

    fn session_card(&mut self, ix: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let session = self.config.sessions[ix].clone();
        let run = self
            .run_for(ix)
            .map(|r| (r.is_active(), r.done_count(), r.summary));
        let running = run.is_some_and(|(active, ..)| active);
        let busy_elsewhere = self.any_running() && !running;

        // Up to three real app icons as a preview of what the session starts.
        let mut previews = Vec::new();
        for item in &session.items {
            if previews.len() == 3 {
                break;
            }
            if let ItemKind::App { target } = &item.kind
                && let Some(image) = self.icon(target, cx)
            {
                previews.push(image);
            }
        }

        let i = i18n(cx);
        let count = session.items.len();
        let wait: u32 = session
            .items
            .iter()
            .take(count.saturating_sub(1))
            .map(|it| it.wait_after_secs)
            .sum();
        let subtitle = if count == 0 {
            i.t("sessions.empty_items")
        } else if wait > 0 {
            i.join(&[i.items(count), i.seconds(wait)])
        } else {
            i.items(count)
        };

        let play = div()
            .id(("play", ix))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(36.))
            .rounded_full()
            .cursor_pointer()
            .map(|d| {
                if running {
                    d.bg(p.accent_soft)
                        .border_1()
                        .border_color(p.accent)
                        .hover(|s| s.bg(p.danger_soft))
                        .child(icon("stop-fill", px(12.), p.accent_text))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.stop_run(cx);
                        }))
                } else if count == 0 || busy_elsewhere {
                    d.bg(p.sunken).child(icon("play-fill", px(15.), p.faint))
                } else {
                    d.bg(p.primary)
                        .hover(|s| s.bg(p.primary_hover))
                        .child(icon("play-fill", px(14.), p.on_primary))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.start_run(ix, cx);
                        }))
                }
            });

        let preview_row = hrow(cx)
            .flex_none()
            .children(previews.into_iter().enumerate().map(|(n, image)| {
                div()
                    .size(px(22.))
                    .rounded(px(6.))
                    .bg(p.surface)
                    .p(px(1.5))
                    .when(n > 0, |d| {
                        if rtl(cx) {
                            d.mr(px(-6.))
                        } else {
                            d.ml(px(-6.))
                        }
                    })
                    .child(gpui_kit::img(image).size_full())
            }));

        let status_line = run.map(|(active, done, summary)| {
            let total = count.max(1);
            let fraction = done as f32 / total as f32;
            let label = if active {
                format!("{} / {}", i.num(done), i.num(count)).into()
            } else {
                summary_text(summary.unwrap_or_default(), count, cx)
            };
            vstack(cx)
                .gap(px(6.))
                .pt(px(10.))
                .child(progress_bar(
                    if active { fraction } else { 1. },
                    &p,
                    rtl(cx),
                ))
                .child(
                    hrow(cx)
                        .gap(px(6.))
                        .text_size(px(11.5))
                        .text_color(p.muted)
                        .when(active, |d| {
                            d.child(spinner(("card-spin", ix), px(12.), p.accent))
                        })
                        .child(label),
                )
        });

        vstack(cx)
            .id(("session", ix))
            .w_full()
            .p(px(10.))
            .rounded(px(16.))
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
            .cursor_pointer()
            .hover(|s| s.border_color(p.border_strong).bg(p.surface_hover))
            .on_click(
                cx.listener(move |this, _, window, cx| this.go(Screen::Session(ix), window, cx)),
            )
            .child(
                hrow(cx)
                    .w_full()
                    .gap(px(12.))
                    .child(self.badge_tile(&session, px(40.), cx))
                    .child(
                        vstack(cx)
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .w_full()
                                    .truncate()
                                    .text_size(px(14.5))
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child(session.name.clone()),
                            )
                            .child(div().text_size(px(12.)).text_color(p.muted).child(subtitle)),
                    )
                    .child(preview_row)
                    .child(play),
            )
            .children(status_line)
    }
}

pub fn summary_text(
    summary: RunSummary,
    count: usize,
    cx: &gpui_kit::App,
) -> gpui_kit::SharedString {
    let i = i18n(cx);
    if summary.cancelled {
        i.t("run.stopped")
    } else if summary.all_ok() {
        if count == 1 {
            i.t("run.all_ok_one")
        } else {
            i.tf("run.all_ok", &[("n", &i.num(count))])
        }
    } else {
        i.tf(
            "run.partial",
            &[("ok", &i.num(summary.launched)), ("n", &i.num(count))],
        )
    }
}
