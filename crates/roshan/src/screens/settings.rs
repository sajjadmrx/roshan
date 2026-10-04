//! Settings: language, appearance, behaviour, and where data lives.

use gpui_kit::component::switch::Switch;
use gpui_kit::{
    AnyElement, App, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use roshan_core::ThemePreference;

use crate::app::Roshan;
use crate::i18n::{LANGUAGES, i18n};
use crate::theme::palette;
use crate::ui::{ButtonKind, button, hint, hrow, segmented, vstack};

fn section(title: SharedString, body: impl IntoElement, cx: &App) -> impl IntoElement {
    let p = palette(cx);
    vstack(cx)
        .w_full()
        .gap(px(8.))
        .child(
            div()
                .px(px(4.))
                .text_size(px(12.))
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(p.muted)
                .child(title),
        )
        .child(
            vstack(cx)
                .w_full()
                .p(px(12.))
                .gap(px(10.))
                .rounded(px(14.))
                .bg(p.surface)
                .border_1()
                .border_color(p.border)
                .child(body),
        )
}

impl Roshan {
    pub fn render_settings(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        let entity = cx.entity();

        let language = segmented(
            "lang",
            LANGUAGES
                .iter()
                .map(|l| (l.code, SharedString::from(l.native_name)))
                .collect(),
            &self.language_code(),
            {
                let entity = entity.clone();
                move |code, window, cx| {
                    entity.update(cx, |this, cx| this.set_language(code, window, cx))
                }
            },
            cx,
        )
        .w_full();

        let theme = segmented(
            "theme",
            vec![
                (ThemePreference::System, i.t("settings.theme_system")),
                (ThemePreference::Light, i.t("settings.theme_light")),
                (ThemePreference::Dark, i.t("settings.theme_dark")),
            ],
            &self.config.settings.theme,
            {
                let entity = entity.clone();
                move |pref, window, cx| {
                    entity.update(cx, |this, cx| this.set_theme(*pref, window, cx))
                }
            },
            cx,
        )
        .w_full();

        let start_at_login = hrow(cx)
            .w_full()
            .gap(px(12.))
            .child(
                vstack(cx)
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .child(i.t("settings.start_at_login")),
                    )
                    .child(hint(i.t("settings.start_at_login_hint"), cx)),
            )
            .child(
                Switch::new("start-at-login")
                    .checked(self.start_at_login)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.set_start_at_login(*checked, cx);
                    })),
            );

        let close_after_run = hrow(cx)
            .w_full()
            .gap(px(12.))
            .child(
                vstack(cx)
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .child(i.t("settings.close_after_run")),
                    )
                    .child(hint(i.t("settings.close_after_run_hint"), cx)),
            )
            .child(
                Switch::new("close-after-run")
                    .checked(self.config.settings.close_after_run)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.config.settings.close_after_run = *checked;
                        this.save(cx);
                    })),
            );

        let path = self.config_path.clone();
        let config_file = hrow(cx)
            .w_full()
            .gap(px(10.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(11.5))
                    .text_color(p.muted)
                    .font_family("Cascadia Mono")
                    .text_ellipsis_start()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(path.display().to_string()),
            )
            .child(
                button(
                    "reveal",
                    ButtonKind::Secondary,
                    Some("folder-open"),
                    i.t("settings.open_config"),
                    cx,
                )
                .h(px(30.))
                .on_click(move |_, _, cx| {
                    if path.exists() {
                        cx.reveal_path(&path);
                    } else if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                        cx.reveal_path(dir);
                    }
                }),
            );

        let updates = self.render_update_settings(cx).into_any_element();
        let sections: Vec<AnyElement> = vec![
            section(i.t("settings.language"), language, cx).into_any_element(),
            section(i.t("settings.theme"), theme, cx).into_any_element(),
            section(
                i.t("settings.behavior"),
                vstack(cx)
                    .w_full()
                    .gap(px(14.))
                    .child(start_at_login)
                    .child(div().h(px(1.)).bg(p.border))
                    .child(close_after_run),
                cx,
            )
            .into_any_element(),
            section(i.t("update.section"), updates, cx).into_any_element(),
            section(i.t("settings.config"), config_file, cx).into_any_element(),
        ];

        div()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .px(px(12.))
            .pb(px(16.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .children(sections)
            .child(
                div()
                    .pt(px(4.))
                    .text_center()
                    .text_size(px(11.))
                    .line_height(px(17.))
                    .text_color(p.faint)
                    .child(i.tf(
                        "settings.about",
                        &[("version", &i.num(env!("CARGO_PKG_VERSION")))],
                    )),
            )
    }
}
