//! First run: choose a language.

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use crate::app::Roshan;
use crate::i18n::{LANGUAGES, i18n};
use crate::theme::palette;
use crate::ui::{ButtonKind, button, hrow};

impl Roshan {
    pub fn render_welcome(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        let current = self.language_code();

        let options = LANGUAGES.iter().map(|lang| {
            let selected = lang.code == current;
            let code = lang.code;
            hrow(cx)
                .id(lang.code)
                .w_full()
                .gap(px(12.))
                .h(px(52.))
                .px(px(16.))
                .rounded(px(14.))
                .border_1()
                .cursor_pointer()
                .map(|d| {
                    if selected {
                        d.border_color(p.accent).bg(p.accent_soft)
                    } else {
                        d.border_color(p.border_strong)
                            .bg(p.surface)
                            .hover(|s| s.bg(p.surface_hover))
                    }
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    // Preview the language immediately; it is saved on Continue.
                    this.config.settings.language = Some(code.to_owned());
                    this.apply_language(window, cx);
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(18.))
                        .rounded_full()
                        .border_2()
                        .border_color(if selected { p.accent } else { p.border_strong })
                        .when(selected, |d| {
                            d.child(div().size(px(8.)).rounded_full().bg(p.accent))
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(15.))
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .when(lang.code == "fa", |d| {
                            d.font_family(crate::assets::PERSIAN_FONT)
                        })
                        .child(lang.native_name),
                )
        });

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .px(px(24.))
            .pb(px(22.))
            .child(div().flex_1().max_h(px(48.)))
            .child(crate::ui::logo(px(88.)))
            .child(div().h(px(18.)))
            .child(
                div()
                    .text_size(px(22.))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .child(i.t("welcome.title")),
            )
            .child(
                div()
                    .pt(px(6.))
                    .text_color(p.muted)
                    .text_size(px(13.))
                    .text_center()
                    .child(i.t("app.tagline")),
            )
            .child(div().flex_1().min_h(px(20.)))
            .child(
                div()
                    .w_full()
                    .pb(px(10.))
                    .text_size(px(12.))
                    .text_color(p.muted)
                    .text_center()
                    .child(i.t("welcome.subtitle")),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(options),
            )
            .child(div().h(px(16.)))
            .child(
                button(
                    "continue",
                    ButtonKind::Primary,
                    None,
                    i.t("welcome.continue"),
                    cx,
                )
                .w_full()
                .h(px(42.))
                .on_click(cx.listener(|this, _, window, cx| this.finish_welcome(window, cx))),
            )
    }
}
