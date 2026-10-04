//! Small building blocks shared by every screen. Everything is drawn with
//! plain GPUI elements so the look stays consistent and fully ours.

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Animation, AnimationExt, App, Div, ElementId, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, StyledImage, Svg,
    Transformation, div, percentage, px, svg,
};
use roshan_core::{ItemKind, LaunchItem};

use crate::assets::icon_path;
use crate::i18n::I18n;
use crate::theme::{Palette, palette};

static LOGO_SMALL: LazyLock<Arc<gpui_kit::Image>> =
    LazyLock::new(|| logo_image(include_bytes!(concat!(env!("OUT_DIR"), "/roshan-64.png"))));
static LOGO_LARGE: LazyLock<Arc<gpui_kit::Image>> =
    LazyLock::new(|| logo_image(include_bytes!(concat!(env!("OUT_DIR"), "/roshan-192.png"))));

fn logo_image(bytes: &[u8]) -> Arc<gpui_kit::Image> {
    Arc::new(gpui_kit::Image::from_bytes(
        gpui_kit::ImageFormat::Png,
        bytes.to_vec(),
    ))
}

/// Roshan's logo, rendered from the pre-scaled copy closest to `size`.
pub fn logo(size: Pixels) -> gpui_kit::Img {
    let image = if size <= px(48.) {
        LOGO_SMALL.clone()
    } else {
        LOGO_LARGE.clone()
    };
    gpui_kit::img(image).size(size).flex_none()
}

pub fn rtl(cx: &App) -> bool {
    cx.global::<I18n>().rtl()
}

/// A horizontal row that follows the reading direction.
pub fn hrow(cx: &App) -> Div {
    let base = div().flex().items_center();
    if rtl(cx) {
        base.flex_row_reverse()
    } else {
        base.flex_row()
    }
}

/// A vertical stack whose text follows the reading direction.
pub fn vstack(cx: &App) -> Div {
    // Children keep stretching (so inputs stay full width); only text aligns.
    div().flex().flex_col().when(rtl(cx), |d| d.text_right())
}

pub fn icon(name: &str, size: Pixels, color: Hsla) -> Svg {
    svg()
        .path(icon_path(name))
        .size(size)
        .flex_none()
        .text_color(color)
}

/// An arrow that points "back" in the current reading direction.
pub fn back_icon(cx: &App) -> &'static str {
    if rtl(cx) { "arrow-right" } else { "arrow-left" }
}

pub fn spinner(id: impl Into<ElementId>, size: Pixels, color: Hsla) -> impl IntoElement {
    icon("loader-circle", size, color).with_animation(
        id,
        Animation::new(Duration::from_millis(900)).repeat(),
        |svg, delta| svg.with_transformation(Transformation::rotate(percentage(delta))),
    )
}

/// A quiet square button holding one icon.
pub fn icon_button(id: impl Into<ElementId>, name: &str, cx: &App) -> gpui_kit::Stateful<Div> {
    let p = palette(cx);
    let id = id.into();
    let group = SharedString::from(format!("{id:?}"));
    div()
        .id(id)
        .group(group.clone())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(30.))
        .rounded(px(8.))
        .cursor_pointer()
        .hover(|s| s.bg(p.surface_hover))
        .active(|s| s.bg(p.surface_active))
        .child(icon(name, px(16.), p.muted).group_hover(group, |s| s.text_color(p.text)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Solid charcoal (light) or cream (dark): the one main action on a screen.
    Primary,
    /// Neutral surface button.
    Secondary,
    /// Text-only button.
    Ghost,
    /// Destructive, neutral until hovered.
    Danger,
    /// Destructive confirmation: solid red.
    DangerFilled,
}

pub fn button(
    id: impl Into<ElementId>,
    kind: ButtonKind,
    icon_name: Option<&str>,
    label: impl Into<SharedString>,
    cx: &App,
) -> gpui_kit::Stateful<Div> {
    let p = palette(cx);
    let (fg, icon_fg) = match kind {
        ButtonKind::Primary => (p.on_primary, p.on_primary),
        ButtonKind::Secondary | ButtonKind::Ghost => (p.text, p.muted),
        ButtonKind::Danger => (p.danger, p.danger),
        ButtonKind::DangerFilled => (gpui_kit::white(), gpui_kit::white()),
    };
    hrow(cx)
        .id(id)
        .flex_none()
        .justify_center()
        .gap(px(6.))
        .h(px(34.))
        .px(px(14.))
        .rounded(px(9.))
        .cursor_pointer()
        .text_size(px(13.))
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .text_color(fg)
        .map(|b| match kind {
            ButtonKind::Primary => b
                .bg(p.primary)
                .hover(|s| s.bg(p.primary_hover))
                .active(|s| s.opacity(0.88)),
            ButtonKind::Secondary => b
                .bg(p.surface)
                .border_1()
                .border_color(p.border_strong)
                .hover(|s| s.bg(p.surface_hover))
                .active(|s| s.bg(p.surface_active)),
            ButtonKind::Ghost => b
                .hover(|s| s.bg(p.surface_hover))
                .active(|s| s.bg(p.surface_active)),
            ButtonKind::Danger => b
                .hover(|s| s.bg(p.danger_soft))
                .active(|s| s.bg(p.danger_soft)),
            ButtonKind::DangerFilled => b
                .bg(p.danger)
                .hover(|s| s.opacity(0.9))
                .active(|s| s.opacity(0.8)),
        })
        .when_some(icon_name, |b, name| b.child(icon(name, px(15.), icon_fg)))
        .child(label.into())
}

/// A pill-shaped segmented control. `options` are (key, label).
pub fn segmented<K: Clone + PartialEq + 'static>(
    id: &str,
    options: Vec<(K, SharedString)>,
    selected: &K,
    on_select: impl Fn(&K, &mut gpui_kit::Window, &mut App) + Clone + 'static,
    cx: &App,
) -> Div {
    let p = palette(cx);
    hrow(cx)
        .p(px(3.))
        .gap(px(2.))
        .rounded(px(10.))
        .bg(p.sunken)
        .children(options.into_iter().enumerate().map(|(i, (key, label))| {
            let active = &key == selected;
            let on_select = on_select.clone();
            div()
                .id(SharedString::from(format!("{id}-{i}")))
                .flex_1()
                .flex()
                .justify_center()
                .items_center()
                .h(px(28.))
                .px(px(10.))
                .rounded(px(7.))
                .border_1()
                .text_size(px(12.5))
                .cursor_pointer()
                .map(|d| {
                    if active {
                        d.bg(p.surface)
                            .border_color(p.border_strong)
                            .text_color(p.text)
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    } else {
                        d.border_color(gpui_kit::transparent_black())
                            .text_color(p.muted)
                            .hover(|s| s.text_color(p.text))
                    }
                })
                .on_click(move |_, window, cx| on_select(&key, window, cx))
                .child(label)
        }))
}

/// Uppercase-free small label above a field.
pub fn field_label(text: impl Into<SharedString>, cx: &App) -> Div {
    let p = palette(cx);
    vstack(cx)
        .text_size(px(12.))
        .font_weight(gpui_kit::FontWeight::MEDIUM)
        .text_color(p.muted)
        .child(text.into())
}

pub fn hint(text: impl Into<SharedString>, cx: &App) -> Div {
    let p = palette(cx);
    vstack(cx)
        .text_size(px(11.5))
        .line_height(px(16.))
        .text_color(p.faint)
        .child(text.into())
}

/// A rounded tile holding an item's picture: its real app icon, or a glyph
/// for commands and links.
pub fn item_tile(
    item: &LaunchItem,
    app_icon: Option<std::sync::Arc<gpui_kit::Image>>,
    size: Pixels,
    p: &Palette,
) -> Div {
    let tile = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size)
        .rounded(size * 0.28);
    match (&item.kind, app_icon) {
        (ItemKind::App { .. }, Some(image)) => tile.child(
            gpui_kit::img(image)
                .size(size * 0.86)
                .object_fit(gpui_kit::ObjectFit::Contain),
        ),
        (ItemKind::App { .. }, None) => {
            tile.bg(p.sunken)
                .child(icon("app-window", size * 0.5, p.faint))
        }
        // Commands get the logo's charcoal tile with a cream glyph.
        (ItemKind::Command { .. }, _) => tile
            .bg(gpui_kit::rgb(if p.dark { 0x0e0d0c } else { 0x22201d }))
            .border_1()
            .border_color(p.border)
            .child(icon(
                "terminal",
                size * 0.52,
                gpui_kit::rgb(0xeee5d6).into(),
            )),
        (ItemKind::Open { target }, _) => {
            let is_web = target.contains("://") || target.starts_with("www.");
            let path = std::path::Path::new(target);
            let glyph = if is_web {
                "globe"
            } else if path.is_dir() {
                "folder"
            } else {
                "file"
            };
            tile.bg(p.sunken)
                .border_1()
                .border_color(p.border)
                .child(icon(glyph, size * 0.5, p.muted))
        }
    }
}

/// A thin progress bar; the filled part is the amber "on" light.
pub fn progress_bar(fraction: f32, p: &Palette, rtl: bool) -> Div {
    let fraction = fraction.clamp(0., 1.);
    div()
        .w_full()
        .h(px(4.))
        .rounded_full()
        .bg(p.sunken)
        .flex()
        .when(rtl, |d| d.flex_row_reverse())
        .child(
            div()
                .h_full()
                .w(gpui_kit::relative(fraction))
                .rounded_full()
                .bg(p.accent),
        )
}

/// Turns an engine error into a translated sentence.
pub fn describe_error(err: &roshan_core::LaunchError, i: &I18n) -> SharedString {
    use roshan_core::LaunchError as E;
    match err {
        E::NotFound(what) if what.is_empty() => i.t("error.not_found"),
        E::NotFound(what) => i.tf("error.not_found_what", &[("what", what)]),
        E::AccessDenied => i.t("error.access_denied"),
        E::ElevationCancelled => i.t("error.elevation_cancelled"),
        E::NoHandler => i.t("error.no_handler"),
        E::NoTerminal => i.t("error.no_terminal"),
        E::Blocked(what) => i.tf("error.blocked", &[("what", what)]),
        E::Unsupported => i.t("error.unsupported"),
        E::Os { message, .. } => i.tf("error.os", &[("message", message)]),
    }
}
