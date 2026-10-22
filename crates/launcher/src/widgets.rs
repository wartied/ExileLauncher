//! Small set of custom widgets so every screen uses the same pieces.

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Frame, Margin, Rect, Response, RichText, Sense, Stroke,
    TextureHandle, Ui, Vec2, pos2, vec2,
};

use crate::theme::{self, *};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Danger,
}

/// A solid, filled button for the one main action on a screen.
pub fn filled_button(ui: &mut Ui, label: &str, size: Vec2, kind: Kind, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let (normal, hover, pressed) = match kind {
            Kind::Primary => (BUTTON, BUTTON_HOVER, BUTTON_PRESSED),
            Kind::Danger => (
                Color32::from_rgb(0xc2, 0x40, 0x39),
                Color32::from_rgb(0xd4, 0x4a, 0x42),
                Color32::from_rgb(0xad, 0x38, 0x32),
            ),
        };
        let (fill, text) = if !enabled {
            (SURFACE_ACTIVE, TEXT_FAINT)
        } else if response.is_pointer_button_down_on() {
            (pressed, Color32::WHITE)
        } else if response.hovered() {
            (hover, Color32::WHITE)
        } else {
            (normal, Color32::WHITE)
        };
        ui.painter().rect_filled(rect, CornerRadius::same(RADIUS), fill);
        let font = FontId::new(if size.y >= 40.0 { 16.0 } else { 14.0 }, theme::semibold());
        paint_text(ui, rect.center().x, egui::Align::Center, rect.center().y, label, font, text);
    }
    if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

pub const BUTTON_HEIGHT: f32 = 32.0;

/// Plain bordered button, sized to its label. Same height as filled buttons
/// so rows of mixed buttons line up exactly.
pub fn button(ui: &mut Ui, label: &str) -> Response {
    button_enabled(ui, true, label)
}

pub fn button_enabled(ui: &mut Ui, enabled: bool, label: &str) -> Response {
    let width = text_width(ui, label, &FontId::proportional(14.0)) + 32.0;
    outline_button(ui, label, vec2(width, BUTTON_HEIGHT), enabled, TEXT)
}

/// Bordered button with an explicit size and label colour.
pub fn outline_button(ui: &mut Ui, label: &str, size: Vec2, enabled: bool, color: Color32) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let (fill, stroke) = if !enabled {
            (SURFACE, BORDER)
        } else if response.is_pointer_button_down_on() {
            (SURFACE_ACTIVE, BORDER_STRONG)
        } else if response.hovered() {
            (SURFACE_HOVER, BORDER_STRONG)
        } else {
            (SURFACE, BORDER)
        };
        let painter = ui.painter();
        // Inset by half a pixel so the 1px border lands exactly on pixel edges.
        painter.rect(
            rect.shrink(0.5),
            CornerRadius::same(RADIUS),
            fill,
            Stroke::new(1.0, stroke),
            egui::StrokeKind::Inside,
        );
        let text = if enabled { color } else { TEXT_FAINT };
        paint_text(ui, rect.center().x, egui::Align::Center, rect.center().y, label, FontId::proportional(14.0), text);
    }
    if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

pub fn text_width(ui: &Ui, text: &str, font: &FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font.clone(), TEXT).size().x)
}

/// Paints one line of text whose capital letters are vertically centred on
/// `center_y`. Centring the line box instead (what `Painter::text` does)
/// leaves Segoe UI visibly low because of its tall ascender.
/// `x` is the left edge, centre or right edge depending on `align`.
pub fn paint_text(ui: &Ui, x: f32, align: egui::Align, center_y: f32, text: &str, font: FontId, color: Color32) {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), color);
    // Baseline and cap top of an "H", relative to the top of the line box.
    let probe = painter.layout_no_wrap("H".to_owned(), font, color);
    let cap_mid = probe
        .rows
        .first()
        .and_then(|row| {
            let g = row.glyphs.first()?;
            let baseline = row.pos.y + g.pos.y;
            Some(baseline + g.uv_rect.offset.y / 2.0)
        })
        .unwrap_or(galley.size().y / 2.0);
    let left = match align {
        egui::Align::Min => x,
        egui::Align::Center => x - galley.size().x / 2.0,
        egui::Align::Max => x - galley.size().x,
    };
    let ppp = ui.ctx().pixels_per_point();
    let snap = |v: f32| (v * ppp).round() / ppp;
    painter.galley(pos2(snap(left), snap(center_y - cap_mid)), galley, color);
}


/// Row of text tabs with an accent underline on the active one.
/// Returns the index clicked this frame, if any.
pub fn tabs(ui: &mut Ui, labels: &[&str], active: usize) -> Option<usize> {
    let mut clicked = None;
    let font = FontId::new(14.0, theme::semibold());
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    // Hairline under the whole row; the active tab's underline sits on top.
    ui.painter().hline(row.x_range(), row.bottom() - 0.5, Stroke::new(1.0, BORDER));
    let mut x = row.left();
    for (i, label) in labels.iter().enumerate() {
        let w = text_width(ui, label, &font);
        let rect = Rect::from_min_max(pos2(x, row.top()), pos2(x + w, row.bottom()));
        let response = ui.interact(rect, ui.scope_id().with(("tab", i)), Sense::click());
        let color = if i == active {
            TEXT
        } else if response.hovered() {
            TEXT_DIM
        } else {
            TEXT_FAINT
        };
        paint_text(ui, x, egui::Align::Min, row.top() + 13.0, label, font.clone(), color);
        if i == active {
            let bar = Rect::from_min_max(pos2(rect.left(), row.bottom() - 2.0), pos2(rect.right(), row.bottom()));
            ui.painter().rect_filled(bar, CornerRadius::same(1), ACCENT);
        }
        if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            clicked = Some(i);
        }
        x += w + 24.0;
    }
    clicked.filter(|&i| i != active)
}

/// On/off switch. Returns the response; `on` is flipped when clicked.
pub fn toggle(ui: &mut Ui, on: &mut bool, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(34.0, 20.0), sense);
    let mut response = response;
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool(response.id, *on);
    let track = if !enabled {
        SURFACE_ACTIVE
    } else {
        lerp_color(BORDER_STRONG, BUTTON, t)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), track);
    let knob_x = rect.left() + 10.0 + t * (rect.width() - 20.0);
    let knob = if enabled { Color32::WHITE } else { TEXT_FAINT };
    painter.circle_filled(pos2(knob_x, rect.center().y), 7.0, knob);
    if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Compact segmented picker. Disabled options show `disabled_hint` on hover.
/// Returns the newly picked index, if it changed.
pub fn segmented(ui: &mut Ui, options: &[(&str, bool)], active: usize, disabled_hint: &str) -> Option<usize> {
    let font = FontId::proportional(13.0);
    let widths: Vec<f32> = options.iter().map(|(l, _)| text_width(ui, l, &font) + 24.0).collect();
    let total = widths.iter().sum::<f32>() + 4.0;
    let (outer, _) = ui.allocate_exact_size(vec2(total, 30.0), Sense::hover());
    ui.painter().rect(outer.shrink(0.5), CornerRadius::same(RADIUS), SURFACE, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
    let mut x = outer.left() + 2.0;
    let mut picked = None;
    for (i, ((label, enabled), w)) in options.iter().zip(&widths).enumerate() {
        let rect = Rect::from_min_size(pos2(x, outer.top() + 2.0), vec2(*w, outer.height() - 4.0));
        let sense = if *enabled { Sense::click() } else { Sense::hover() };
        let response = ui.interact(rect, ui.scope_id().with(("seg", i)), sense);
        if i == active {
            ui.painter().rect_filled(rect, CornerRadius::same(RADIUS - 2), SURFACE_ACTIVE);
        } else if *enabled && response.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(RADIUS - 2), SURFACE_HOVER);
        }
        let color = if !enabled {
            TEXT_FAINT
        } else if i == active {
            TEXT
        } else {
            TEXT_DIM
        };
        paint_text(ui, rect.center().x, egui::Align::Center, rect.center().y, label, font.clone(), color);
        let response = if *enabled {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response.on_hover_text(disabled_hint)
        };
        if response.clicked() && i != active {
            picked = Some(i);
        }
        x += w;
    }
    picked
}

/// "1.2M", "34K", "980".
pub fn short_count(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.0}K", n as f64 / 1e3),
        _ if n < 10_000_000 => format!("{:.1}M", n as f64 / 1e6),
        _ => format!("{:.0}M", n as f64 / 1e6),
    }
}

/// A notice with a coloured bar on the left and an optional action button
/// on the right. Returns true when the action was clicked.
pub fn notice(ui: &mut Ui, color: Color32, text: &str, action: Option<&str>) -> bool {
    let mut clicked = false;
    let rect = Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin { left: 16, right: 8, top: 8, bottom: 8 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let font = FontId::proportional(14.0);
            let button_w = action.map(|a| text_width(ui, a, &font) + 32.0).unwrap_or(0.0);
            let text_w = (ui.available_width() - button_w - 12.0).max(80.0);
            let galley = ui.painter().layout(text.to_owned(), font.clone(), TEXT, text_w);
            let single_line = galley.rows.len() <= 1;
            let height = galley.size().y.max(if action.is_some() { BUTTON_HEIGHT } else { 0.0 });
            let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
            if single_line {
                // Same optical centring as button labels, so text and button line up.
                paint_text(ui, row.left(), egui::Align::Min, row.center().y, text, font, TEXT);
            } else {
                ui.painter().galley(row.left_top(), galley, TEXT);
            }
            if let Some(a) = action {
                let r = Rect::from_min_size(
                    pos2(row.right() - button_w, row.center().y - BUTTON_HEIGHT / 2.0),
                    vec2(button_w, BUTTON_HEIGHT),
                );
                // A detached child: placing it must not move this layout's cursor.
                clicked = button(&mut ui.new_child(egui::UiBuilder::new().max_rect(r)), a).clicked();
            }
        })
        .response
        .rect;
    let bar = Rect::from_min_max(rect.min, pos2(rect.left() + 3.0, rect.bottom()));
    ui.painter().rect_filled(bar, CornerRadius { nw: RADIUS, sw: RADIUS, ne: 0, se: 0 }, color);
    clicked
}

/// Small capitalised heading above a group of controls.
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).size(11.0).color(TEXT_FAINT).extra_letter_spacing(1.2));
}

/// A flat panel used to group related controls.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn status_dot(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(8.0, 16.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

/// Player face, drawn crisp (nearest-neighbour) with slightly rounded corners.
pub fn avatar(ui: &mut Ui, face: Option<&TextureHandle>, name: &str, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_avatar(ui, rect, face, name);
    response
}

/// Falls back to the name's initial while the skin is loading or unavailable.
pub fn paint_avatar(ui: &Ui, rect: Rect, face: Option<&TextureHandle>, name: &str) {
    match face {
        Some(tex) => {
            egui::Image::new((tex.id(), rect.size()))
                .corner_radius(CornerRadius::same(4))
                .paint_at(ui, rect);
        }
        None => {
            ui.painter().rect_filled(rect, CornerRadius::same(4), SURFACE_ACTIVE);
            let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            let font = FontId::new(rect.height() * 0.5, theme::semibold());
            paint_text(ui, rect.center().x, egui::Align::Center, rect.center().y, &initial, font, TEXT_DIM);
        }
    }
}

/// Row in the instance list. Returns the click response.
pub fn instance_row(
    ui: &mut Ui,
    selected: bool,
    title: &str,
    subtitle: &str,
    badge: Option<(String, Color32)>,
) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 52.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let bg = if selected {
            SURFACE_ACTIVE
        } else if response.hovered() {
            SURFACE_HOVER
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, CornerRadius::same(RADIUS), bg);
        if selected {
            let bar = Rect::from_min_size(pos2(rect.left(), rect.top() + 12.0), vec2(3.0, rect.height() - 24.0));
            painter.rect_filled(bar, CornerRadius::same(2), ACCENT);
        }

        let left = rect.left() + 16.0;
        let right_reserved = if badge.is_some() { 76.0 } else { 12.0 };
        let max_text = (rect.right() - right_reserved - left).max(20.0);
        let title_galley = painter.layout_no_wrap(
            truncate(ui, title, FontId::new(14.0, theme::semibold()), max_text),
            FontId::new(14.0, theme::semibold()),
            TEXT,
        );
        painter.galley(pos2(left, rect.top() + 8.0), title_galley, TEXT);
        let sub_galley = painter.layout_no_wrap(
            truncate(ui, subtitle, FontId::proportional(12.0), max_text),
            FontId::proportional(12.0),
            TEXT_DIM,
        );
        painter.galley(pos2(left, rect.top() + 29.0), sub_galley, TEXT_DIM);

        if let Some((text, color)) = badge {
            let font = FontId::proportional(12.0);
            paint_text(ui, rect.right() - 12.0, egui::Align::Max, rect.center().y, &text, font, color);
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Shortens `text` with an ellipsis so it fits in `max_width`.
pub fn truncate(ui: &Ui, text: &str, font: FontId, max_width: f32) -> String {
    let fits = |s: &str| ui.fonts_mut(|f| f.layout_no_wrap(s.to_owned(), font.clone(), TEXT).size().x) <= max_width;
    if fits(text) {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let candidate: String = chars[..mid].iter().collect::<String>() + "…";
        if fits(&candidate) { lo = mid } else { hi = mid - 1 }
    }
    chars[..lo].iter().collect::<String>().trim_end().to_owned() + "…"
}

/// Frame used by every dialog.
pub fn dialog_frame() -> Frame {
    Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(24))
        .shadow(egui::Shadow { offset: [0, 10], blur: 32, spread: 0, color: Color32::from_black_alpha(120) })
}

pub fn dialog_title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(18.0).family(theme::semibold()).color(TEXT));
    ui.add_space(4.0);
}

/// Right-aligned row of dialog buttons, flush with the right edge of the
/// content above. Buttons are added right to left. The row has a fixed
/// height: a bare right-to-left layout would claim all remaining height and
/// stretch the dialog to the size of the window.
pub fn dialog_buttons(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.add_space(16.0);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), BUTTON_HEIGHT),
        egui::Layout::right_to_left(egui::Align::Center),
        add,
    );
}

pub fn relative_time(then: u64, now: u64) -> String {
    let secs = now.saturating_sub(then);
    match secs {
        0..60 => "just now".into(),
        60..3600 => plural(secs / 60, "minute"),
        3600..86_400 => plural(secs / 3600, "hour"),
        86_400..2_592_000 => plural(secs / 86_400, "day"),
        _ => plural(secs / 2_592_000, "month"),
    }
}

fn plural(n: u64, unit: &str) -> String {
    if n == 1 { format!("1 {unit} ago") } else { format!("{n} {unit}s ago") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_times() {
        assert_eq!(relative_time(100, 100), "just now");
        assert_eq!(relative_time(0, 60), "1 minute ago");
        assert_eq!(relative_time(0, 7200), "2 hours ago");
        assert_eq!(relative_time(0, 86_400 * 3), "3 days ago");
    }
}
