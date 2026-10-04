//! One session: its ordered items, drag to reorder, and the run controls.

use std::time::Instant;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use roshan_core::engine::{ItemStatus, SkipReason};
use roshan_core::{ItemKind, LaunchItem};

use crate::app::Roshan;
use crate::i18n::{I18n, i18n};
use crate::screens::sessions::summary_text;
use crate::theme::{Palette, palette};
use crate::ui::{
    ButtonKind, button, describe_error, hrow, icon, item_tile, progress_bar, rtl, spinner, vstack,
};

/// What travels with the pointer while an item is dragged.
#[derive(Clone)]
pub struct DraggedItem {
    from: usize,
    item: LaunchItem,
    icon: Option<std::sync::Arc<gpui_kit::Image>>,
}

impl Render for DraggedItem {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        hrow(cx)
            .gap(px(10.))
            .px(px(10.))
            .py(px(8.))
            .w(px(240.))
            .rounded(px(12.))
            .bg(p.surface)
            .border_1()
            .border_color(p.accent)
            .shadow_lg()
            .opacity(0.95)
            .child(item_tile(&self.item, self.icon.clone(), px(28.), &p))
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(self.item.name.clone()),
            )
    }
}

impl Roshan {
    pub fn render_session(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = palette(cx);
        let items = self.config.sessions[ix].items.clone();
        let body: AnyElement = if items.is_empty() {
            self.render_session_empty(cx).into_any_element()
        } else {
            let rows: Vec<AnyElement> = items
                .iter()
                .enumerate()
                .map(|(n, item)| {
                    self.item_row(ix, n, item, items.len(), cx)
                        .into_any_element()
                })
                .collect();
            div()
                .id("items")
                .size_full()
                .overflow_y_scroll()
                .px(px(12.))
                .pt(px(2.))
                .pb(px(12.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(rows)
                .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().child(body))
            .child(self.session_footer(ix, cx))
            .bg(p.bg)
    }

    fn render_session_empty(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .px(px(32.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(56.))
                    .rounded(px(18.))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border)
                    .child(icon("plus", px(24.), p.faint)),
            )
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(i.t("session.empty_title")),
            )
            .child(
                div()
                    .text_center()
                    .text_size(px(12.5))
                    .line_height(px(19.))
                    .text_color(p.muted)
                    .child(i.t("session.empty_body")),
            )
    }

    fn item_row(
        &mut self,
        session: usize,
        n: usize,
        item: &LaunchItem,
        count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = palette(cx);
        let app_icon = match &item.kind {
            ItemKind::App { target } => self.icon(target, cx),
            _ => None,
        };
        let run = self.run_for(session);
        let running = run.is_some_and(|r| r.is_active());
        let status = run.and_then(|r| r.statuses.get(n).cloned());
        let waiting = run.and_then(|r| r.waiting).filter(|w| w.index == n);
        let is_last = n + 1 == count;
        let i = i18n(cx);

        let leading = match &status {
            Some(status) => status_icon(status, n, &p).into_any_element(),
            None => icon("grip-vertical", px(14.), p.faint).into_any_element(),
        };

        let (subtitle, subtitle_color, monospace) = match &status {
            Some(ItemStatus::Failed(err)) => (describe_error(err, &i), p.danger, false),
            Some(ItemStatus::Skipped(SkipReason::Missing(_))) => {
                (i.t("session.missing"), p.warning, false)
            }
            Some(ItemStatus::Skipped(SkipReason::Cancelled)) => {
                (i.t("status.cancelled"), p.faint, false)
            }
            _ => {
                let (text, mono) = item_subtitle(item, &i);
                (text, p.muted, mono)
            }
        };

        let wait_chip = (!is_last).then(|| {
            let (label, active) = match waiting {
                Some(w) => {
                    let left = w.until.saturating_duration_since(Instant::now());
                    (i.seconds(left.as_secs_f32().ceil() as u32), true)
                }
                None if item.wait_after_secs > 0 => (i.seconds(item.wait_after_secs), false),
                None => (SharedString::default(), false),
            };
            hrow(cx)
                .flex_none()
                .gap(px(4.))
                .h(px(24.))
                .px(px(8.))
                .rounded_full()
                .text_size(px(11.5))
                .map(|d| {
                    if active {
                        d.bg(p.accent_soft).text_color(p.accent_text)
                    } else if item.wait_after_secs > 0 {
                        d.bg(p.sunken).text_color(p.muted)
                    } else {
                        d.text_color(p.faint)
                    }
                })
                .child(icon(
                    "timer",
                    px(12.),
                    if active { p.accent_text } else { p.faint },
                ))
                .when(!label.is_empty(), |d| d.child(label))
        });

        let mut row = hrow(cx)
            .id(("item", n))
            .w_full()
            .gap(px(10.))
            .pl(px(8.))
            .pr(px(10.))
            .py(px(8.))
            .rounded(px(14.))
            .bg(p.surface)
            .border_1()
            .border_color(
                if waiting.is_some() || matches!(status, Some(ItemStatus::Launching)) {
                    p.accent.opacity(0.6)
                } else {
                    p.border
                },
            )
            .child(
                div()
                    .flex_none()
                    .w(px(16.))
                    .flex()
                    .justify_center()
                    .child(leading),
            )
            .child(item_tile(item, app_icon.clone(), px(32.), &p))
            .child(
                vstack(cx)
                    .flex_1()
                    .min_w_0()
                    .gap(px(1.))
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_size(px(13.5))
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .child(item.name.clone()),
                    )
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_size(px(11.5))
                            .text_color(subtitle_color)
                            .when(monospace, |d| d.font_family("Cascadia Mono"))
                            .child(subtitle),
                    ),
            )
            .children(wait_chip);

        if !running {
            let dragged = DraggedItem {
                from: n,
                item: item.clone(),
                icon: app_icon,
            };
            row = row
                .cursor_pointer()
                .hover(|s| s.bg(p.surface_hover).border_color(p.border_strong))
                .on_click(cx.listener(move |this, _, window, cx| this.open_editor(n, window, cx)))
                .on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
                .drag_over::<DraggedItem>(move |style, _, _, _| {
                    style.border_color(p.accent).bg(p.accent_soft)
                })
                .on_drop(cx.listener(move |this, dragged: &DraggedItem, _, cx| {
                    this.move_item(dragged.from, n, cx)
                }));
        }
        row
    }

    fn session_footer(&mut self, ix: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        let count = self.config.sessions[ix].items.len();
        let run = self
            .run_for(ix)
            .map(|r| (r.is_active(), r.done_count(), r.waiting, r.summary));
        let busy_elsewhere = self.any_running() && !self.is_running(ix);

        let container = vstack(cx)
            .flex_none()
            .w_full()
            .gap(px(10.))
            .px(px(12.))
            .pt(px(10.))
            .pb(px(12.))
            .border_t_1()
            .border_color(p.border)
            .bg(p.bg);

        match run {
            Some((true, done, waiting, _)) => {
                let (label, fraction) = match waiting {
                    Some(w) => {
                        let left = w.until.saturating_duration_since(Instant::now());
                        let secs = left.as_secs_f32().ceil() as u32;
                        let within = 1. - left.as_secs_f32() / w.total.as_secs_f32().max(0.001);
                        (
                            i.tf("run.next_in", &[("s", &i.num(secs))]),
                            (done as f32 - 1. + within.clamp(0., 1.)) / count as f32 + 0.0,
                        )
                    }
                    None => (i.t("run.starting"), done as f32 / count as f32),
                };
                let fraction = fraction.max(done.saturating_sub(1) as f32 / count as f32);
                container.child(progress_bar(fraction, &p, rtl(cx))).child(
                    hrow(cx)
                        .w_full()
                        .gap(px(8.))
                        .child(spinner("footer-spin", px(14.), p.accent))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(12.5))
                                .text_color(p.muted)
                                .child(label),
                        )
                        .when(waiting.is_some(), |d| {
                            d.child(
                                button(
                                    "skip",
                                    ButtonKind::Ghost,
                                    Some("skip-forward"),
                                    i.t("run.skip"),
                                    cx,
                                )
                                .h(px(30.))
                                .on_click(cx.listener(|this, _, _, cx| this.skip_wait(cx))),
                            )
                        })
                        .child(
                            button(
                                "stop",
                                ButtonKind::Secondary,
                                Some("stop-fill"),
                                i.t("session.stop"),
                                cx,
                            )
                            .h(px(30.))
                            .on_click(cx.listener(|this, _, _, cx| this.stop_run(cx))),
                        ),
                )
            }
            other => {
                let summary = other.and_then(|(_, _, _, s)| s);
                let summary_chip = summary.map(|s| {
                    let (glyph, color) = if s.cancelled {
                        ("circle-minus", p.muted)
                    } else if s.all_ok() {
                        ("circle-check", p.success)
                    } else {
                        ("circle-alert", p.warning)
                    };
                    hrow(cx)
                        .id("summary")
                        .flex_1()
                        .min_w_0()
                        .gap(px(6.))
                        .text_size(px(12.5))
                        .text_color(color)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.dismiss_run(cx)))
                        .child(icon(glyph, px(15.), color))
                        .child(div().truncate().child(summary_text(s, count, cx)))
                });
                let can_run = count > 0 && !busy_elsewhere;
                container.child(
                    hrow(cx)
                        .w_full()
                        .gap(px(8.))
                        .child(
                            button(
                                "add",
                                ButtonKind::Secondary,
                                Some("plus"),
                                i.t("session.add"),
                                cx,
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_picker(window, cx)),
                            ),
                        )
                        .map(|d| match summary_chip {
                            Some(chip) => d.child(chip),
                            None => d.child(div().flex_1()),
                        })
                        .child(
                            button(
                                "run",
                                ButtonKind::Primary,
                                Some("play-fill"),
                                i.t("session.run"),
                                cx,
                            )
                            .px(px(20.))
                            .when(!can_run, |b| b.opacity(0.45).cursor_default())
                            .when(can_run, |b| {
                                b.on_click(
                                    cx.listener(move |this, _, _, cx| this.start_run(ix, cx)),
                                )
                            }),
                        ),
                )
            }
        }
    }
}

fn status_icon(status: &ItemStatus, n: usize, p: &Palette) -> AnyElement {
    match status {
        ItemStatus::Pending => icon("circle-dashed", px(15.), p.faint).into_any_element(),
        ItemStatus::Launching => spinner(("row-spin", n), px(15.), p.accent).into_any_element(),
        ItemStatus::Launched => icon("circle-check", px(15.), p.success).into_any_element(),
        ItemStatus::Failed(_) => icon("circle-x", px(15.), p.danger).into_any_element(),
        ItemStatus::Skipped(_) => icon("circle-minus", px(15.), p.faint).into_any_element(),
    }
}

/// Secondary line of an item row, and whether it is code (monospace).
fn item_subtitle(item: &LaunchItem, i: &I18n) -> (SharedString, bool) {
    match &item.kind {
        ItemKind::App { .. } => (i.t("item.kind_app"), false),
        // A command named after itself would just repeat the name.
        ItemKind::Command { line, terminal, .. } if line.trim() == item.name.trim() => {
            let mode = if *terminal {
                "item.in_terminal"
            } else {
                "item.in_background"
            };
            (i.join(&[i.t("item.kind_command"), i.t(mode)]), false)
        }
        ItemKind::Command { line, .. } => (line.clone().into(), true),
        ItemKind::Open { target } => (target.clone().into(), false),
    }
}
