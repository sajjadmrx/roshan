//! The root view: title bar, current screen and overlays.

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, Context, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, WindowControlArea, div, px,
};

use crate::app::{Overlay, Roshan, Screen};
use crate::i18n::i18n;
use crate::theme::palette;
use crate::ui::{back_icon, hrow, icon, icon_button};

pub const TITLE_BAR_HEIGHT: f32 = 44.;

impl Render for Roshan {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let content: AnyElement = match self.screen {
            Screen::Welcome => self.render_welcome(window, cx).into_any_element(),
            Screen::Sessions => self.render_sessions(window, cx).into_any_element(),
            Screen::Session(ix) if ix < self.config.sessions.len() => {
                self.render_session(ix, window, cx).into_any_element()
            }
            Screen::Session(_) => self.render_sessions(window, cx).into_any_element(),
            Screen::Settings => self.render_settings(window, cx).into_any_element(),
        };
        let overlay: Option<AnyElement> = match &self.overlay {
            Some(Overlay::Picker(tab)) => {
                Some(self.render_picker(*tab, window, cx).into_any_element())
            }
            Some(Overlay::Editor { item }) => {
                let item = *item;
                self.render_editor(item, window, cx)
                    .map(IntoElement::into_any_element)
            }
            Some(Overlay::ConfirmDelete) => Some(self.render_confirm_delete(cx).into_any_element()),
            Some(Overlay::Badges) => Some(self.render_badges(cx).into_any_element()),
            None => None,
        };

        div()
            .id("roshan")
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(self.font_family.clone())
            .text_size(px(13.5))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.on_escape(window, cx);
                }
            }))
            .child(self.render_titlebar(window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(content)
                    .children(overlay)
                    .children(self.render_notice(cx)),
            )
    }
}

impl Roshan {
    fn on_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming {
            self.finish_rename(window, cx);
        } else if matches!(self.overlay, Some(Overlay::Editor { .. })) {
            self.close_editor(window, cx);
        } else if self.overlay.is_some() {
            self.overlay = None;
            window.focus(&self.focus, cx);
            cx.notify();
        } else if matches!(self.screen, Screen::Session(_) | Screen::Settings) {
            self.go(Screen::Sessions, window, cx);
        }
    }

    fn render_titlebar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let i = i18n(cx);
        let mut leading: Vec<AnyElement> = Vec::new();
        let mut trailing: Vec<AnyElement> = Vec::new();
        let mut title: Option<AnyElement> = None;

        match self.screen {
            Screen::Welcome | Screen::Sessions => {
                leading.push(crate::ui::logo(px(24.)).into_any_element());
                title = Some(
                    div()
                        .text_size(px(14.))
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .child(i.t("app.name"))
                        .into_any_element(),
                );
                if self.screen == Screen::Sessions {
                    trailing.push(
                        icon_button("settings", "settings", cx)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.go(Screen::Settings, window, cx)
                            }))
                            .into_any_element(),
                    );
                }
            }
            Screen::Session(ix) if ix < self.config.sessions.len() => {
                let session = &self.config.sessions[ix];
                let running = self.is_running(ix);
                leading.push(
                    icon_button("back", back_icon(cx), cx)
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.go(Screen::Sessions, window, cx)
                            }),
                        )
                        .into_any_element(),
                );
                leading.push(
                    self.badge_tile(session, px(26.), cx)
                        .id("badge")
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.8))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.overlay = Some(Overlay::Badges);
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                title = Some(if self.renaming {
                    div()
                        .flex_1()
                        .child(
                            gpui_kit::component::input::Input::new(&self.inputs.rename).h(px(30.)),
                        )
                        .into_any_element()
                } else {
                    div()
                        .id("title")
                        .min_w_0()
                        .truncate()
                        .text_size(px(14.))
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .child(session.name.clone())
                        .into_any_element()
                });
                if !running && !self.renaming {
                    trailing.push(
                        icon_button("rename", "pencil", cx)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.start_rename(window, cx)),
                            )
                            .into_any_element(),
                    );
                    trailing.push(
                        icon_button("delete", "trash", cx)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.overlay = Some(Overlay::ConfirmDelete);
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
            }
            Screen::Session(_) => {}
            Screen::Settings => {
                leading.push(
                    icon_button("back", back_icon(cx), cx)
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.go(Screen::Sessions, window, cx)
                            }),
                        )
                        .into_any_element(),
                );
                title = Some(
                    div()
                        .text_size(px(14.))
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .child(i.t("settings.title"))
                        .into_any_element(),
                );
            }
        }

        let controls = hrow(cx)
            .flex_none()
            .gap(px(2.))
            .child(window_control(
                "min",
                "minus",
                WindowControlArea::Min,
                false,
                cx,
            ))
            .child(window_control(
                "close",
                "x",
                WindowControlArea::Close,
                true,
                cx,
            ));

        hrow(cx)
            .flex_none()
            .h(px(TITLE_BAR_HEIGHT))
            .px(px(8.))
            .gap(px(6.))
            .children(leading)
            .child(
                // Only this part drags the window, so buttons stay clickable.
                hrow(cx)
                    .id("drag")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .gap(px(6.))
                    .when(cfg!(windows), |d| {
                        d.window_control_area(WindowControlArea::Drag)
                    })
                    .when(!cfg!(windows), |d| {
                        d.on_mouse_down(MouseButton::Left, |_, window, _| {
                            window.start_window_move()
                        })
                    })
                    .children(title),
            )
            .children(trailing)
            .child(div().w(px(4.)))
            .child(controls)
    }

    fn render_notice(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let notice = self.notice.clone()?;
        let p = palette(cx);
        Some(
            div()
                .absolute()
                .bottom(px(12.))
                .left(px(12.))
                .right(px(12.))
                .child(
                    hrow(cx)
                        .gap(px(10.))
                        .p(px(12.))
                        .rounded(px(12.))
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.border_strong)
                        .shadow_lg()
                        .child(icon("triangle-alert", px(16.), p.warning))
                        .child(
                            crate::ui::vstack(cx)
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.))
                                .child(notice),
                        )
                        .child(icon_button("dismiss-notice", "x", cx).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.notice = None;
                                cx.notify();
                            },
                        ))),
                ),
        )
    }
}

fn window_control(
    id: &'static str,
    glyph: &str,
    area: WindowControlArea,
    close: bool,
    cx: &Context<Roshan>,
) -> impl IntoElement {
    let p = palette(cx);
    let (hover_bg, hover_fg) = if close {
        (
            gpui_kit::rgb(0xe5484d).into(),
            gpui_kit::rgb(0xffffff).into(),
        )
    } else {
        (p.surface_hover, p.text)
    };
    div()
        .id(id)
        .group(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(30.))
        .rounded(px(8.))
        .hover(move |s| s.bg(hover_bg))
        .when(cfg!(windows), |d| d.window_control_area(area))
        .when(!cfg!(windows), move |d| {
            d.on_click(move |_, window, cx| match area {
                WindowControlArea::Min => window.minimize_window(),
                _ => cx.quit(),
            })
        })
        .child(
            gpui_kit::svg()
                .path(crate::assets::icon_path(glyph))
                .size(px(15.))
                .text_color(p.muted)
                .group_hover(id, move |s| s.text_color(hover_fg)),
        )
}
