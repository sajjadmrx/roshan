//! "Add" sheet: pick a real installed app, or describe a command or link.

use std::rc::Rc;

use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px, uniform_list,
};
use roshan_platform::DiscoveredApp;

use super::sheet;
use crate::app::{AppsState, PickerTab, Roshan};
use crate::i18n::i18n;
use crate::theme::palette;
use crate::ui::{
    ButtonKind, button, field_label, hint, hrow, icon, icon_button, segmented, spinner, vstack,
};

impl Roshan {
    pub fn render_picker(
        &mut self,
        tab: PickerTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let i = i18n(cx);
        let entity = cx.entity();
        let tabs = segmented(
            "picker-tab",
            vec![
                (PickerTab::Apps, i.t("picker.tab_apps")),
                (PickerTab::Command, i.t("picker.tab_command")),
                (PickerTab::Open, i.t("picker.tab_open")),
            ],
            &tab,
            move |tab, window, cx| {
                entity.update(cx, |this, cx| this.set_picker_tab(*tab, window, cx))
            },
            cx,
        );
        let header = hrow(cx)
            .w_full()
            .gap(px(8.))
            .px(px(12.))
            .pt(px(8.))
            .pb(px(10.))
            .child(tabs.flex_1())
            .child(icon_button("close-picker", "x", cx).on_click(cx.listener(
                |this, _, window, cx| {
                    this.overlay = None;
                    window.focus(&this.focus, cx);
                    cx.notify();
                },
            )));
        let body: AnyElement = match tab {
            PickerTab::Apps => self.render_apps_tab(window, cx).into_any_element(),
            PickerTab::Command => self.render_command_tab(cx).into_any_element(),
            PickerTab::Open => self.render_open_tab(cx).into_any_element(),
        };
        sheet(
            "picker",
            true,
            cx.listener(|this, _, window, cx| {
                this.overlay = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }),
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(header)
                .child(div().flex_1().min_h_0().flex().flex_col().child(body)),
            cx,
        )
    }

    fn render_apps_tab(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        let search = div().px(px(12.)).pb(px(8.)).child(
            Input::new(&self.inputs.search)
                .prefix(icon("search", px(15.), p.faint))
                .cleanable(true)
                .h(px(36.)),
        );

        let list: AnyElement = match &self.apps {
            AppsState::Loaded(_) => {
                let apps = Rc::new(self.filtered_apps(cx));
                if apps.is_empty() {
                    let query = self.inputs.search.read(cx).value();
                    let text = if query.trim().is_empty() {
                        i.t("picker.no_apps")
                    } else {
                        i.tf("picker.no_results", &[("q", query.trim())])
                    };
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(p.muted)
                        .text_size(px(13.))
                        .child(text)
                        .into_any_element()
                } else {
                    let count = apps.len();
                    uniform_list(
                        "apps",
                        count,
                        cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                            range
                                .map(|ix| this.app_row(ix, &apps[ix], cx).into_any_element())
                                .collect::<Vec<_>>()
                        }),
                    )
                    .flex_1()
                    .px(px(8.))
                    .into_any_element()
                }
            }
            _ => vstack(cx)
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .text_color(p.muted)
                .text_size(px(13.))
                .child(spinner("apps-loading", px(20.), p.accent))
                .child(i.t("picker.loading"))
                .into_any_element(),
        };

        let count = match &self.apps {
            AppsState::Loaded(apps) => Some(apps.len()),
            _ => None,
        };
        let footer = hrow(cx)
            .flex_none()
            .w_full()
            .gap(px(6.))
            .px(px(10.))
            .py(px(8.))
            .border_t_1()
            .border_color(p.border)
            .child(
                button(
                    "browse",
                    ButtonKind::Ghost,
                    Some("folder-open"),
                    i.t("picker.browse"),
                    cx,
                )
                .h(px(30.))
                .px(px(10.))
                .on_click(cx.listener(|this, _, window, cx| this.browse_program(window, cx))),
            )
            .child(div().flex_1())
            .when_some(count, |d, n| {
                d.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(p.faint)
                        .child(i.tf("picker.count", &[("n", &i.num(n))])),
                )
                .child(
                    icon_button("refresh", "rotate-cw", cx)
                        .on_click(cx.listener(|this, _, _, cx| this.load_apps(true, cx))),
                )
            });

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(search)
            .child(div().flex_1().min_h_0().flex().flex_col().child(list))
            .child(footer)
    }

    fn app_row(
        &mut self,
        ix: usize,
        app: &DiscoveredApp,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = palette(cx);
        let image = self.icon(&app.target, cx);
        let chosen = app.clone();
        hrow(cx)
            .id(("app", ix))
            .w_full()
            .h(px(44.))
            .gap(px(10.))
            .px(px(8.))
            .rounded(px(10.))
            .cursor_pointer()
            .hover(|s| s.bg(p.surface))
            .active(|s| s.bg(p.surface_active))
            .on_click(
                cx.listener(move |this, _, window, cx| this.add_app(chosen.clone(), window, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(30.))
                    .map(|d| match image {
                        Some(image) => d.child(gpui_kit::img(image).size(px(28.))),
                        None => d.child(div().size(px(24.)).rounded(px(7.)).bg(p.sunken)),
                    }),
            )
            .child(
                vstack(cx)
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_size(px(13.))
                            .child(app.name.clone()),
                    )
                    .when_some(app.detail.clone(), |d, detail| {
                        d.child(
                            div()
                                .w_full()
                                .text_size(px(11.))
                                .text_color(p.faint)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis_start()
                                .child(detail),
                        )
                    }),
            )
    }

    fn render_command_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        let has_line = !self.inputs.cmd_line.read(cx).value().trim().is_empty();
        let cwd_input = self.inputs.cmd_cwd.clone();
        let terminal = self.cmd_terminal;
        vstack(cx)
            .flex_1()
            .min_h_0()
            .child(
                vstack(cx)
                    .id("command-form")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(14.))
                    .gap(px(14.))
                    .child(
                        vstack(cx)
                            .gap(px(6.))
                            .child(field_label(i.t("command.line"), cx))
                            .child(
                                Input::new(&self.inputs.cmd_line)
                                    .prefix(icon("terminal", px(15.), p.faint))
                                    .h(px(36.)),
                            )
                            .child(hint(i.t("command.shell_note"), cx)),
                    )
                    .child(
                        vstack(cx)
                            .gap(px(6.))
                            .child(field_label(i.t("command.name"), cx))
                            .child(Input::new(&self.inputs.cmd_name).h(px(36.))),
                    )
                    .child(
                        vstack(cx)
                            .gap(px(6.))
                            .child(field_label(i.t("command.cwd"), cx))
                            .child(
                                hrow(cx)
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&self.inputs.cmd_cwd).h(px(36.))),
                                    )
                                    .child(
                                        button(
                                            "cwd-browse",
                                            ButtonKind::Secondary,
                                            Some("folder"),
                                            i.t("common.browse"),
                                            cx,
                                        )
                                        .h(px(36.))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.browse_into(
                                                    cwd_input.clone(),
                                                    true,
                                                    window,
                                                    cx,
                                                )
                                            }),
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        hrow(cx)
                            .w_full()
                            .gap(px(12.))
                            .p(px(12.))
                            .rounded(px(12.))
                            .bg(p.surface)
                            .border_1()
                            .border_color(p.border)
                            .child(
                                vstack(cx)
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(div().text_size(px(13.)).child(i.t("command.terminal")))
                                    .child(hint(
                                        if terminal {
                                            i.t("command.terminal_hint_on")
                                        } else {
                                            i.t("command.terminal_hint_off")
                                        },
                                        cx,
                                    )),
                            )
                            .child(Switch::new("cmd-terminal").checked(terminal).on_click(
                                cx.listener(|this, checked: &bool, _, cx| {
                                    this.cmd_terminal = *checked;
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(form_footer(
                button(
                    "add-command",
                    ButtonKind::Primary,
                    Some("plus"),
                    i.t("command.add"),
                    cx,
                )
                .when(!has_line, |b| b.opacity(0.45))
                .on_click(cx.listener(|this, _, window, cx| this.submit_picker_form(window, cx))),
                cx,
            ))
    }

    fn render_open_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let i = i18n(cx);
        let has_target = !self.inputs.open_target.read(cx).value().trim().is_empty();
        let target_for_file = self.inputs.open_target.clone();
        let target_for_dir = self.inputs.open_target.clone();
        vstack(cx)
            .flex_1()
            .min_h_0()
            .child(
                vstack(cx)
                    .id("open-form")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(14.))
                    .gap(px(14.))
                    .child(
                        vstack(cx)
                            .gap(px(6.))
                            .child(field_label(i.t("open.target"), cx))
                            .child(
                                Input::new(&self.inputs.open_target)
                                    .prefix(icon("link", px(15.), p.faint))
                                    .h(px(36.)),
                            )
                            .when(self.open_error, |d| {
                                d.child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(p.danger)
                                        .child(i.t("open.invalid")),
                                )
                            })
                            .child(
                                hrow(cx)
                                    .gap(px(6.))
                                    .child(
                                        button(
                                            "open-file",
                                            ButtonKind::Secondary,
                                            Some("file"),
                                            i.t("open.file"),
                                            cx,
                                        )
                                        .h(px(30.))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.browse_into(
                                                    target_for_file.clone(),
                                                    false,
                                                    window,
                                                    cx,
                                                )
                                            }),
                                        ),
                                    )
                                    .child(
                                        button(
                                            "open-folder",
                                            ButtonKind::Secondary,
                                            Some("folder"),
                                            i.t("open.folder"),
                                            cx,
                                        )
                                        .h(px(30.))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.browse_into(
                                                    target_for_dir.clone(),
                                                    true,
                                                    window,
                                                    cx,
                                                )
                                            }),
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        vstack(cx)
                            .gap(px(6.))
                            .child(field_label(i.t("open.name"), cx))
                            .child(Input::new(&self.inputs.open_name).h(px(36.))),
                    ),
            )
            .child(form_footer(
                button(
                    "add-open",
                    ButtonKind::Primary,
                    Some("plus"),
                    i.t("open.add"),
                    cx,
                )
                .when(!has_target, |b| b.opacity(0.45))
                .on_click(cx.listener(|this, _, window, cx| this.submit_picker_form(window, cx))),
                cx,
            ))
    }
}

fn form_footer(action: impl IntoElement, cx: &gpui_kit::App) -> impl IntoElement {
    let p = palette(cx);
    hrow(cx)
        .flex_none()
        .w_full()
        .px(px(12.))
        .py(px(10.))
        .border_t_1()
        .border_color(p.border)
        .child(div().flex_1())
        .child(action)
}
