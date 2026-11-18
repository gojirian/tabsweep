//! Colours and text styles, taken from ironterm's `theme.rs`.
//!
//! Graphite surfaces with hairline borders, copper as the one action colour
//! with dark text on it, green for ok, red for danger, amber for warning.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, ThemePreference};

use crate::fonts::{medium_family, semibold_family};

pub const CANVAS: Color32 = Color32::from_rgb(0x0f, 0x11, 0x14);
pub const SURFACE_CARD: Color32 = Color32::from_rgb(0x16, 0x18, 0x1c);
pub const SURFACE_RAISED: Color32 = Color32::from_rgb(0x1f, 0x22, 0x27);
pub const SURFACE_HOVER: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 13);
pub const SURFACE_ACTIVE: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 22);

pub const BORDER_SUBTLE: Color32 = Color32::from_rgba_unmultiplied_const(58, 62, 70, 128);
pub const BORDER_DEFAULT: Color32 = Color32::from_rgb(0x2c, 0x30, 0x36);
pub const BORDER_HOVER: Color32 = Color32::from_rgba_unmultiplied_const(150, 147, 141, 128);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf0);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xc9, 0xc8, 0xc4);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x8f, 0x8d, 0x89);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x64, 0x63, 0x5f);

pub const ACCENT: Color32 = Color32::from_rgb(0xe0, 0x8a, 0x45);
pub const ACCENT_ON: Color32 = Color32::from_rgb(0x16, 0x12, 0x0e);
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(224, 138, 69, 28);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0xec, 0xa5, 0x6e);

pub const INFO: Color32 = Color32::from_rgb(0x4f, 0xa3, 0xff);
pub const DANGER: Color32 = Color32::from_rgb(0xe5, 0x53, 0x4b);
pub const WARN_TEXT: Color32 = Color32::from_rgb(0xe9, 0xc4, 0x6a);

pub const RADIUS: u8 = 8;

/// Dark visuals and the Fira Sans text styles. Call after [`crate::fonts::install`].
pub fn install(ctx: &egui::Context) {
    ctx.set_theme(ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        style.visuals.panel_fill = SURFACE_CARD;
        style.visuals.window_fill = SURFACE_RAISED;
        style.visuals.extreme_bg_color = CANVAS;
        style.visuals.faint_bg_color = SURFACE_CARD;
        style.visuals.code_bg_color = CANVAS;
        style.visuals.text_edit_bg_color = Some(Color32::TRANSPARENT);
        style.visuals.override_text_color = None;
        style.visuals.weak_text_color = Some(TEXT_MUTED);
        style.visuals.hyperlink_color = INFO;
        style.visuals.warn_fg_color = WARN_TEXT;
        style.visuals.error_fg_color = DANGER;
        style.visuals.window_corner_radius = CornerRadius::same(12);
        style.visuals.window_stroke = Stroke::new(1.0, BORDER_SUBTLE);
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_DEFAULT);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
        style.visuals.widgets.hovered.weak_bg_fill = SURFACE_HOVER;
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_HOVER);
        style.visuals.widgets.active.weak_bg_fill = SURFACE_ACTIVE;
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        style.visuals.selection.bg_fill = ACCENT_SOFT;
        style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        for widgets in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widgets.corner_radius = CornerRadius::same(RADIUS);
        }
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        style.spacing.window_margin = Margin::same(16);
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::new(18.0, semibold_family()));
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::new(13.0, medium_family()));
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        );
    });
}
