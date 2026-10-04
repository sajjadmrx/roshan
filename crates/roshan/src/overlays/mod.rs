mod dialogs;
mod editor;
mod picker;

use std::time::Duration;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Animation, AnimationExt, AnyElement, App, ClickEvent, InteractiveElement, IntoElement,
    MouseButton, ParentElement, StatefulInteractiveElement, Styled, Window, div, ease_out_quint,
    px,
};

use crate::theme::palette;

/// A sheet that slides up from the bottom over a dimmed backdrop.
/// `full` sheets take the whole height; others size to their content.
pub fn sheet(
    id: &'static str,
    full: bool,
    on_dismiss: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    content: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    let p = palette(cx);
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .justify_end()
        .child(
            div()
                .id("scrim")
                .absolute()
                .inset_0()
                .bg(p.scrim)
                .on_click(on_dismiss)
                .with_animation(
                    "scrim-in",
                    Animation::new(Duration::from_millis(160)),
                    |el, delta| el.opacity(delta),
                ),
        )
        .child(
            div()
                .id("sheet")
                .relative()
                .w_full()
                .when(full, |d| d.h_full().mt(px(6.)))
                .flex()
                .flex_col()
                .bg(p.bg)
                .rounded_t(px(18.))
                .border_t_1()
                .border_color(p.border_strong)
                .shadow_2xl()
                // Swallow clicks so they don't reach the scrim.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div().w_full().flex().justify_center().pt(px(6.)).child(
                        div()
                            .w(px(36.))
                            .h(px(4.))
                            .rounded_full()
                            .bg(p.border_strong),
                    ),
                )
                .child(content)
                .with_animation(
                    "sheet-in",
                    Animation::new(Duration::from_millis(260)).with_easing(ease_out_quint()),
                    |el, delta| el.top(px(40. * (1. - delta))).opacity(0.4 + 0.6 * delta),
                ),
        )
}

/// A small centered card over a dimmed backdrop.
pub fn dialog(
    on_dismiss: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    content: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let p = palette(cx);
    div()
        .id("dialog")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .px(px(20.))
        .child(
            div()
                .id("dialog-scrim")
                .absolute()
                .inset_0()
                .bg(p.scrim)
                .on_click(on_dismiss),
        )
        .child(
            div()
                .id("dialog-card")
                .relative()
                .w_full()
                .p(px(18.))
                .rounded(px(18.))
                .bg(p.surface)
                .border_1()
                .border_color(p.border_strong)
                .shadow_2xl()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(content)
                .with_animation(
                    "dialog-in",
                    Animation::new(Duration::from_millis(200)).with_easing(ease_out_quint()),
                    |el, delta| el.opacity(delta).top(px(12. * (1. - delta))),
                ),
        )
        .into_any_element()
}
