//! The update status: a section in Settings and, when there is something to
//! act on, a notice at the top of the home screen.

use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};

use crate::app::{Roshan, UpdateState, current_version};
use crate::i18n::i18n;
use crate::theme::palette;
use crate::ui::{ButtonKind, button, hint, hrow, icon, spinner, vstack};

/// What the status line says, and which action it offers.
enum Action {
    Check,
    Restart,
    OpenPage,
    None,
}

impl Roshan {
    fn update_status(
        &self,
        cx: &Context<Self>,
    ) -> (SharedString, Option<&'static str>, bool, Action) {
        let i = i18n(cx);
        let v = |version: &roshan_core::Version| i.num(version);
        // (text, icon, busy spinner, action)
        match &self.update {
            UpdateState::Idle => (
                i.tf("update.current", &[("version", &v(&current_version()))]),
                None,
                false,
                Action::Check,
            ),
            UpdateState::Checking => (i.t("update.checking"), None, true, Action::None),
            UpdateState::UpToDate => (
                i.t("update.up_to_date"),
                Some("circle-check"),
                false,
                Action::Check,
            ),
            UpdateState::Available(release) => (
                i.tf("update.available", &[("version", &v(&release.version))]),
                None,
                false,
                Action::OpenPage,
            ),
            UpdateState::Downloading(version) => (
                i.tf("update.downloading", &[("version", &v(version))]),
                None,
                true,
                Action::None,
            ),
            UpdateState::Ready { version, .. } => (
                i.tf("update.ready", &[("version", &v(version))]),
                None,
                false,
                Action::Restart,
            ),
            UpdateState::Failed(_) => (
                i.t("update.failed"),
                Some("circle-alert"),
                false,
                Action::Check,
            ),
        }
    }

    fn update_action(&self, action: Action, cx: &mut Context<Self>) -> Option<AnyElement> {
        let i = i18n(cx);
        let element = match action {
            Action::Check => button(
                "update-check",
                ButtonKind::Secondary,
                Some("rotate-cw"),
                i.t("update.check"),
                cx,
            )
            .h(px(30.))
            .on_click(cx.listener(|this, _, _, cx| this.check_for_updates(cx))),
            Action::Restart => button(
                "update-restart",
                ButtonKind::Primary,
                None,
                i.t("update.restart"),
                cx,
            )
            .h(px(30.))
            .on_click(cx.listener(|this, _, _, cx| this.restart_to_update(cx))),
            Action::OpenPage => button(
                "update-page",
                ButtonKind::Primary,
                None,
                i.t("update.open_page"),
                cx,
            )
            .h(px(30.))
            .on_click(cx.listener(|this, _, _, cx| this.open_release_page(cx))),
            Action::None => return None,
        };
        Some(element.into_any_element())
    }

    /// The "Updates" card body in Settings.
    pub fn render_update_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        let (text, glyph, busy, action) = self.update_status(cx);
        let status_color = match glyph {
            Some("circle-check") => p.success,
            Some(_) => p.warning,
            None => p.text,
        };

        let auto = hrow(cx)
            .w_full()
            .gap(px(12.))
            .child(
                vstack(cx)
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(div().text_size(px(13.)).child(i.t("update.auto")))
                    .child(hint(i.t("update.auto_hint"), cx)),
            )
            .child(
                Switch::new("check-updates")
                    .checked(self.config.settings.check_updates)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.set_check_updates(*checked, cx);
                    })),
            );

        let status = hrow(cx)
            .w_full()
            .gap(px(8.))
            .min_h(px(30.))
            .when(busy, |d| d.child(spinner("update-spin", px(14.), p.accent)))
            .when_some(glyph, |d, g| d.child(icon(g, px(15.), status_color)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.5))
                    .text_color(if glyph.is_some() {
                        status_color
                    } else {
                        p.muted
                    })
                    .child(text),
            )
            .children(self.update_action(action, cx));

        vstack(cx)
            .w_full()
            .gap(px(14.))
            .child(auto)
            .child(div().h(px(1.)).bg(p.border))
            .child(status)
            // Say why a check failed (offline, blocked, rate limited…).
            .when_some(
                match &self.update {
                    UpdateState::Failed(reason) => Some(reason.clone()),
                    _ => None,
                },
                |d, reason| d.child(hint(reason, cx)),
            )
    }

    /// A notice on the home screen when an update can be installed or
    /// downloaded.
    pub fn render_update_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let i = i18n(cx);
        let (text, action) = match &self.update {
            UpdateState::Ready { version, .. } => (
                i.tf("update.banner_ready", &[("version", &i.num(version))]),
                Action::Restart,
            ),
            UpdateState::Available(release) => (
                i.tf(
                    "update.banner_available",
                    &[("version", &i.num(&release.version))],
                ),
                Action::OpenPage,
            ),
            _ => return None,
        };
        let p = palette(cx);
        Some(
            hrow(cx)
                .id("update-banner")
                .w_full()
                .gap(px(10.))
                .px(px(12.))
                .py(px(8.))
                .rounded(px(12.))
                .bg(p.surface)
                .border_1()
                .border_color(p.border_strong)
                // The amber dot: something is ready, i.e. "on".
                .child(div().size(px(8.)).flex_none().rounded_full().bg(p.accent))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(text),
                )
                .children(self.update_action(action, cx))
                .into_any_element(),
        )
    }
}
