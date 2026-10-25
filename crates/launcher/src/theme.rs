//! Visual style: a dark palette (Exile's blue-tinted greys, or neutral
//! greys), flat surfaces, a single accent colour.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle, Vec2,
};

/// The neutrals: everything except the accent and status colours.
struct Palette {
    bg: Color32,
    sidebar: Color32,
    surface: Color32,
    surface_hover: Color32,
    surface_active: Color32,
    border: Color32,
    border_strong: Color32,
    text: Color32,
    text_dim: Color32,
    text_faint: Color32,
    extreme: Color32,
}

// Exile: derived from the brand blue #678FC5 (hue ~215°). The neutrals are
// the same hue at very low saturation, so greys read as "cool" rather than flat.
const EXILE: Palette = Palette {
    bg: Color32::from_rgb(0x11, 0x14, 0x19),
    sidebar: Color32::from_rgb(0x15, 0x19, 0x20),
    surface: Color32::from_rgb(0x1b, 0x20, 0x29),
    surface_hover: Color32::from_rgb(0x22, 0x28, 0x33),
    surface_active: Color32::from_rgb(0x29, 0x30, 0x3d),
    border: Color32::from_rgb(0x26, 0x2c, 0x37),
    border_strong: Color32::from_rgb(0x34, 0x3c, 0x4a),
    text: Color32::from_rgb(0xe6, 0xea, 0xf0),
    text_dim: Color32::from_rgb(0x97, 0xa2, 0xb3),
    text_faint: Color32::from_rgb(0x66, 0x71, 0x85),
    extreme: Color32::from_rgb(0x0d, 0x10, 0x15),
};

// Dark: the same lightness steps in neutral grey.
const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x13, 0x13, 0x13),
    sidebar: Color32::from_rgb(0x17, 0x17, 0x17),
    surface: Color32::from_rgb(0x1e, 0x1e, 0x1e),
    surface_hover: Color32::from_rgb(0x25, 0x25, 0x25),
    surface_active: Color32::from_rgb(0x2d, 0x2d, 0x2d),
    border: Color32::from_rgb(0x29, 0x29, 0x29),
    border_strong: Color32::from_rgb(0x38, 0x38, 0x38),
    text: Color32::from_rgb(0xe8, 0xe8, 0xe8),
    text_dim: Color32::from_rgb(0xa0, 0xa0, 0xa0),
    text_faint: Color32::from_rgb(0x6c, 0x6c, 0x6c),
    extreme: Color32::from_rgb(0x0e, 0x0e, 0x0e),
};

/// 0 = Exile, 1 = Dark.
static ACTIVE: AtomicU8 = AtomicU8::new(0);

fn palette() -> &'static Palette {
    if ACTIVE.load(Ordering::Relaxed) == 1 { &DARK } else { &EXILE }
}

/// Switches the palette and restyles egui. Takes effect on the next frame.
pub fn set_dark(ctx: &egui::Context, dark: bool) {
    ACTIVE.store(dark as u8, Ordering::Relaxed);
    apply_style(ctx);
}

pub fn bg() -> Color32 {
    palette().bg
}
pub fn sidebar() -> Color32 {
    palette().sidebar
}
pub fn surface() -> Color32 {
    palette().surface
}
pub fn surface_hover() -> Color32 {
    palette().surface_hover
}
pub fn surface_active() -> Color32 {
    palette().surface_active
}
pub fn border() -> Color32 {
    palette().border
}
pub fn border_strong() -> Color32 {
    palette().border_strong
}
pub fn text() -> Color32 {
    palette().text
}
pub fn text_dim() -> Color32 {
    palette().text_dim
}
pub fn text_faint() -> Color32 {
    palette().text_faint
}
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
    apply_style(ctx);
}

fn apply_style(ctx: &egui::Context) {
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
        v.weak_text_color = Some(text_faint());
        v.panel_fill = bg();
        v.window_fill = surface();
        v.window_stroke = Stroke::new(1.0, border_strong());
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(110) };
        v.popup_shadow = Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(90) };
        v.extreme_bg_color = palette().extreme;
        v.faint_bg_color = surface();
        v.code_bg_color = surface();
        v.hyperlink_color = ACCENT_LIGHT;
        v.warn_fg_color = WARN;
        v.error_fg_color = DANGER;
        v.selection.bg_fill = SELECTION;
        v.selection.stroke = Stroke::new(1.0, text());
        v.slider_trailing_fill = true;
        v.handle_shape = egui::style::HandleShape::Circle;
        v.interact_cursor = Some(CursorIcon::PointingHand);
        v.window_highlight_topmost = false;
        v.striped = false;
        v.indent_has_left_vline = false;

        let radius = CornerRadius::same(RADIUS);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = bg();
        w.noninteractive.weak_bg_fill = surface();
        w.noninteractive.bg_stroke = Stroke::new(1.0, border());
        w.noninteractive.fg_stroke = Stroke::new(1.0, text());
        w.noninteractive.corner_radius = radius;

        w.inactive.bg_fill = surface_active();
        w.inactive.weak_bg_fill = surface();
        w.inactive.bg_stroke = Stroke::new(1.0, border());
        w.inactive.fg_stroke = Stroke::new(1.0, text());
        w.inactive.corner_radius = radius;
        w.inactive.expansion = 0.0;

        w.hovered.bg_fill = border_strong();
        w.hovered.weak_bg_fill = surface_hover();
        w.hovered.bg_stroke = Stroke::new(1.0, border_strong());
        w.hovered.fg_stroke = Stroke::new(1.5, text());
        w.hovered.corner_radius = radius;
        // No growth on hover: widgets must never shift or overlap neighbours.
        w.hovered.expansion = 0.0;

        w.active.bg_fill = border_strong();
        w.active.weak_bg_fill = surface_active();
        w.active.bg_stroke = Stroke::new(1.0, border_strong());
        w.active.fg_stroke = Stroke::new(1.5, text());
        w.active.corner_radius = radius;
        w.active.expansion = 0.0;

        w.open = w.active;
    });
}
