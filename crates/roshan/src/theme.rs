//! Roshan's visual language, taken from its logo: a charcoal tile, a cream
//! bowl and one small amber light.
//!
//! * **Primary** actions are solid: charcoal on light themes, cream on dark.
//! * **Amber** means "on": a running item, an active wait, a switch that is
//!   enabled, the focused field. It is never decoration.
//! * No gradients, no glows, no rainbow tints. Depth comes from borders and
//!   surface steps, not shadows.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Global, Hsla, SharedString, Window, WindowAppearance, px, rgb, rgba};
use roshan_core::ThemePreference;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg: Hsla,
    pub surface: Hsla,
    pub surface_hover: Hsla,
    pub surface_active: Hsla,
    pub sunken: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    /// Solid fill of the main action on a screen.
    pub primary: Hsla,
    pub primary_hover: Hsla,
    pub on_primary: Hsla,
    /// The amber "on" light.
    pub accent: Hsla,
    /// Amber dark enough to read as text on the background.
    pub accent_text: Hsla,
    pub on_accent: Hsla,
    pub accent_soft: Hsla,
    pub success: Hsla,
    pub danger: Hsla,
    pub danger_soft: Hsla,
    pub warning: Hsla,
    pub scrim: Hsla,
}

impl Global for Palette {}

impl Palette {
    pub fn light() -> Self {
        Self {
            dark: false,
            bg: rgb(0xf3f1ec).into(),
            surface: rgb(0xfdfcfa).into(),
            surface_hover: rgb(0xf8f6f2).into(),
            surface_active: rgb(0xeeebe5).into(),
            sunken: rgb(0xe9e5de).into(),
            border: rgba(0x1d1a1617).into(),
            border_strong: rgba(0x1d1a1629).into(),
            text: rgb(0x1d1a16).into(),
            muted: rgb(0x655f57).into(),
            faint: rgb(0x9c958b).into(),
            primary: rgb(0x22201d).into(),
            primary_hover: rgb(0x3a3631).into(),
            on_primary: rgb(0xf6efe3).into(),
            accent: rgb(0xf59a0e).into(),
            accent_text: rgb(0xa35f00).into(),
            on_accent: rgb(0x1d1206).into(),
            accent_soft: rgba(0xf59a0e1f).into(),
            success: rgb(0x2f7d4f).into(),
            danger: rgb(0xc8352b).into(),
            danger_soft: rgba(0xc8352b14).into(),
            warning: rgb(0xa35f00).into(),
            scrim: rgba(0x1d1a1647).into(),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: rgb(0x141312).into(),
            surface: rgb(0x1c1b19).into(),
            surface_hover: rgb(0x232220).into(),
            surface_active: rgb(0x2b2927).into(),
            sunken: rgb(0x0e0d0c).into(),
            border: rgba(0xfff6e812).into(),
            border_strong: rgba(0xfff6e821).into(),
            text: rgb(0xf1ece4).into(),
            muted: rgb(0xa29b91).into(),
            faint: rgb(0x6c675f).into(),
            primary: rgb(0xeee5d6).into(),
            primary_hover: rgb(0xfff8ec).into(),
            on_primary: rgb(0x181614).into(),
            accent: rgb(0xffab2e).into(),
            accent_text: rgb(0xffbd57).into(),
            on_accent: rgb(0x1d1206).into(),
            accent_soft: rgba(0xffab2e1c).into(),
            success: rgb(0x6cc493).into(),
            danger: rgb(0xf0766b).into(),
            danger_soft: rgba(0xf0766b1a).into(),
            warning: rgb(0xffbd57).into(),
            scrim: rgba(0x00000080).into(),
        }
    }
}

pub fn palette(cx: &App) -> Palette {
    *cx.global::<Palette>()
}

pub fn resolve_dark(preference: ThemePreference, appearance: WindowAppearance) -> bool {
    match preference {
        ThemePreference::Light => false,
        ThemePreference::Dark => true,
        ThemePreference::System => {
            matches!(
                appearance,
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            )
        }
    }
}

/// Applies the palette globally and keeps the component library (inputs,
/// switches, scrollbars) in the same colors.
pub fn apply(dark: bool, font_family: SharedString, window: Option<&mut Window>, cx: &mut App) {
    let p = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    cx.set_global(p);
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    Theme::update(cx, |t| {
        t.font_family = font_family;
        t.font_size = px(14.);
        t.radius = px(8.);
        t.radius_lg = px(12.);
        t.shadow = false;
        t.focus_ring = false;
        t.background = p.bg;
        t.foreground = p.text;
        t.border = p.border_strong;
        t.input = p.border_strong;
        t.ring = p.accent;
        t.caret = p.accent_text;
        t.selection = p.accent_soft;
        t.muted = p.sunken;
        t.muted_foreground = p.faint;
        // Switches: amber when on, like the light in the logo.
        t.primary = p.accent;
        t.primary_hover = p.accent;
        t.primary_foreground = p.on_accent;
        t.switch = p.border_strong;
        t.scrollbar_thumb = p.border_strong;
        t.scrollbar_thumb_hover = p.faint;
        t.popover = p.surface;
        t.popover_foreground = p.text;
        t.overlay = p.scrim;
    });
}
