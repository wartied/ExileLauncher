//! Visual style: one dark palette, flat surfaces, a single accent colour.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle, Vec2,
};

// Everything is derived from the brand blue #678FC5 (hue ~215°): the neutrals
// are the same hue at very low saturation, so greys read as "cool" rather
// than flat.
pub const BG: Color32 = Color32::from_rgb(0x11, 0x14, 0x19);
pub const SIDEBAR: Color32 = Color32::from_rgb(0x15, 0x19, 0x20);
pub const SURFACE: Color32 = Color32::from_rgb(0x1b, 0x20, 0x29);
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(0x22, 0x28, 0x33);
pub const SURFACE_ACTIVE: Color32 = Color32::from_rgb(0x29, 0x30, 0x3d);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x2c, 0x37);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x34, 0x3c, 0x4a);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xea, 0xf0);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x97, 0xa2, 0xb3);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x66, 0x71, 0x85);
/// The brand colour: indicators, progress, selection bar, status text.
pub const ACCENT: Color32 = Color32::from_rgb(0x67, 0x8f, 0xc5);
/// Lighter tint for links on dark backgrounds.
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0x8e, 0xae, 0xdb);
/// Deeper shades for filled buttons, so white labels stay readable
/// (4.5:1 contrast; white on #678FC5 itself is only ~3.3:1).
pub const BUTTON: Color32 = Color32::from_rgb(0x4f, 0x79, 0xb2);
pub const BUTTON_HOVER: Color32 = Color32::from_rgb(0x5a, 0x84, 0xbd);
pub const BUTTON_PRESSED: Color32 = Color32::from_rgb(0x46, 0x6c, 0xa0);
/// Selected rows and the filled part of sliders.
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x2b, 0x3e, 0x5a);
const SELECTION: Color32 = Color32::from_rgb(0x46, 0x67, 0x95);
pub const DANGER: Color32 = Color32::from_rgb(0xe5, 0x53, 0x4b);
pub const WARN: Color32 = Color32::from_rgb(0xd9, 0xa2, 0x3b);

pub const RADIUS: u8 = 6;

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Iosevka Aile (SIL OFL 1.1, see assets/fonts/LICENSE-Iosevka.md), subset
/// to Latin, Greek, Cyrillic and common symbols to keep the binary small.
/// egui's bundled fonts stay behind it as fallback for any other glyph.
static UI_REGULAR: &[u8] = include_bytes!("../assets/fonts/IosevkaAile-Regular.ttf");
static UI_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IosevkaAile-SemiBold.ttf");

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("ui".into(), Arc::new(FontData::from_static(UI_REGULAR)));
    fonts.font_data.insert("ui-semibold".into(), Arc::new(FontData::from_static(UI_SEMIBOLD)));

    let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let proportional: Vec<String> = std::iter::once("ui".to_owned()).chain(fallback.iter().cloned()).collect();
    let bold: Vec<String> = std::iter::once("ui-semibold".to_owned()).chain(fallback).collect();
    fonts.families.insert(FontFamily::Proportional, proportional);
    fonts.families.insert(semibold(), bold);
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(22.0, semibold())),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();

        let s = &mut style.spacing;
        s.item_spacing = Vec2::new(8.0, 8.0);
        s.button_padding = Vec2::new(14.0, 6.0);
        s.interact_size = Vec2::new(40.0, 30.0);
        s.window_margin = Margin::same(20);
        s.menu_margin = Margin::same(6);
        s.slider_width = 260.0;
        s.slider_rail_height = 6.0;
        s.text_edit_width = 280.0;
        s.icon_width = 16.0;
        s.icon_width_inner = 9.0;
        s.icon_spacing = 8.0;
        // Floating bars overlay the content instead of taking width, so rows
        // and their buttons don't shift when a list becomes scrollable.
        s.scroll = egui::style::ScrollStyle::floating();

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = None;
        // Also the placeholder colour of text fields.
        v.weak_text_color = Some(TEXT_FAINT);
        v.panel_fill = BG;
        v.window_fill = SURFACE;
        v.window_stroke = Stroke::new(1.0, BORDER_STRONG);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(110) };
        v.popup_shadow = Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(90) };
        v.extreme_bg_color = Color32::from_rgb(0x0d, 0x10, 0x15);
        v.faint_bg_color = SURFACE;
        v.code_bg_color = SURFACE;
        v.hyperlink_color = ACCENT_LIGHT;
        v.warn_fg_color = WARN;
        v.error_fg_color = DANGER;
        v.selection.bg_fill = SELECTION;
        v.selection.stroke = Stroke::new(1.0, TEXT);
        v.slider_trailing_fill = true;
        v.handle_shape = egui::style::HandleShape::Circle;
        v.interact_cursor = Some(CursorIcon::PointingHand);
        v.window_highlight_topmost = false;
        v.striped = false;
        v.indent_has_left_vline = false;

        let radius = CornerRadius::same(RADIUS);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = BG;
        w.noninteractive.weak_bg_fill = SURFACE;
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        w.noninteractive.corner_radius = radius;

        w.inactive.bg_fill = SURFACE_ACTIVE;
        w.inactive.weak_bg_fill = SURFACE;
        w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.inactive.corner_radius = radius;
        w.inactive.expansion = 0.0;

        w.hovered.bg_fill = BORDER_STRONG;
        w.hovered.weak_bg_fill = SURFACE_HOVER;
        w.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
        w.hovered.fg_stroke = Stroke::new(1.5, TEXT);
        w.hovered.corner_radius = radius;
        // No growth on hover: widgets must never shift or overlap neighbours.
        w.hovered.expansion = 0.0;

        w.active.bg_fill = BORDER_STRONG;
        w.active.weak_bg_fill = SURFACE_ACTIVE;
        w.active.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
        w.active.fg_stroke = Stroke::new(1.5, TEXT);
        w.active.corner_radius = radius;
        w.active.expansion = 0.0;

        w.open = w.active;
    });
}
