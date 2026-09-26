//! Menu-bar icon renderer.
//!
//! Five icon styles, three colour modes, an optional pace marker and an
//! optional multi-profile layout. Everything is drawn into a small RGBA
//! buffer with plain raster code — no image or font crate — so the output is
//! deterministic and unit-testable.

use chrono::DateTime;

#[derive(Debug, Clone, Default)]
pub struct TrayMetric {
    /// Rolling (5-hour) used percent, when the provider reports it.
    pub rolling: Option<f64>,
    pub weekly: Option<f64>,
    pub monthly: Option<f64>,
    /// Projected end-of-window percent, when computable.
    pub pace: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TrayProfile {
    pub id: String,
    pub label: String,
    pub active: bool,
    pub metrics: TrayMetric,
    /// Token count for the profile when the user picked the tokens metric.
    pub tokens: f64,
    pub cost: f64,
}

#[derive(Debug, Clone)]
pub struct TraySnapshot {
    pub profiles: Vec<TrayProfile>,
    pub style: String,
    pub color_mode: String,
    pub custom_color: (u8, u8, u8),
    pub used_mode: String,
    pub show_remaining: bool,
    pub show_labels: bool,
    pub show_pace: bool,
    pub metrics: Vec<String>,
    /// Which window the single-value styles speak for: `auto`, `rolling`,
    /// `weekly`, `monthly`, or `worst`.
    pub primary: String,
    pub active_indicator: bool,
}

const GREEN: (u8, u8, u8) = (0x34, 0xc7, 0x59);
const AMBER: (u8, u8, u8) = (0xff, 0x9f, 0x0a);
const RED: (u8, u8, u8) = (0xff, 0x3b, 0x30);

/// Window lengths in seconds, matching the quota endpoints we consume.
pub fn window_seconds(window: &str) -> Option<f64> {
    match window {
        "rolling" => Some(5.0 * 3600.0),
        "weekly" => Some(7.0 * 24.0 * 3600.0),
        "monthly" => Some(30.0 * 24.0 * 3600.0),
        _ => None,
    }
}

/// Linear projection of where this window lands if usage keeps its pace.
pub fn projected(percent: f64, resets_at: Option<&str>, window: &str) -> Option<f64> {
    let duration = window_seconds(window)?;
    let resets = resets_at.and_then(|iso| DateTime::parse_from_rfc3339(iso).ok())?;
    let remaining = (resets.timestamp() - chrono::Utc::now().timestamp()) as f64;
    if remaining <= 0.0 || remaining >= duration {
        return None;
    }
    let elapsed = duration - remaining;
    if elapsed < duration * 0.05 {
        return None;
    }
    Some((percent / (elapsed / duration)).max(percent))
}

pub fn level_color(percent: f64) -> (u8, u8, u8) {
    if percent >= 90.0 {
        RED
    } else if percent >= 70.0 {
        AMBER
    } else {
        GREEN
    }
}

fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let value = hex.trim_start_matches('#');
    if value.len() != 6 {
        return (0x0a, 0x84, 0xff);
    }
    let parse = |range: std::ops::Range<usize>| u8::from_str_radix(&value[range], 16).unwrap_or(0);
    (parse(0..2), parse(2..4), parse(4..6))
}

// ── canvas ─────────────────────────────────────────────────────────────────
//
// The macOS status item always displays the icon 18 points tall, scaling
// whatever raster we hand it. So we draw on an 18-point design grid rendered
// at `PT` device pixels per point, and every shape is expressed in points.
// Drawing at the real display resolution (instead of a large canvas that
// macOS then crushes down) is what keeps thin strokes — key teeth, battery
// walls, pace markers — from disappearing.
//
// Shapes are anti-aliased with a distance-field coverage estimate, and every
// paint call carries its own alpha. The alpha channel is what makes template
// (monochrome) icons legible: macOS tints a template image by its alpha, so
// an empty track must be drawn *fainter* than the fill, never the same black.

/// Device pixels per point in the rendered raster.
const PT: i32 = 2;
/// macOS menu-bar content height, in points.
pub const MENUBAR_PT: f32 = 18.0;
/// Canvas height in device pixels.
const CANVAS_H: i32 = MENUBAR_PT as i32 * PT;
/// One profile's design width, in points.
const CELL_PT: f32 = 20.0;
/// Gap between profile cells, in points.
const GAP_PT: f32 = 1.5;
/// Horizontal padding either side of the cells, in points.
const PAD_PT: f32 = 1.0;

/// An axis-aligned rectangle in point space.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Rect {
    fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Rect {
            x0: x0.min(x1),
            y0: y0.min(y1),
            x1: x0.max(x1),
            y1: y0.max(y1),
        }
    }
}

/// Signed distance from `(x, y)` to a rounded rectangle (negative inside).
fn rounded_rect_sdf(x: f32, y: f32, r: &Rect, radius: f32) -> f32 {
    let half_w = (r.x1 - r.x0) / 2.0;
    let half_h = (r.y1 - r.y0) / 2.0;
    let radius = radius.min(half_w).min(half_h).max(0.0);
    let cx = (r.x0 + r.x1) / 2.0;
    let cy = (r.y0 + r.y1) / 2.0;
    let qx = (x - cx).abs() - (half_w - radius);
    let qy = (y - cy).abs() - (half_h - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

struct Canvas {
    px: Vec<u8>,
    w: i32,
    h: i32,
}

impl Canvas {
    fn new_pt(w_pt: f32) -> Self {
        let w = (w_pt * PT as f32).round() as i32;
        Canvas {
            px: vec![0u8; (w * CANVAS_H * 4) as usize],
            w,
            h: CANVAS_H,
        }
    }

    fn blend(&mut self, x: i32, y: i32, rgb: (u8, u8, u8), alpha: u8) {
        if !(0..self.w).contains(&x) || !(0..self.h).contains(&y) || alpha == 0 {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let a = alpha as u32;
        let existing = self.px[i + 3] as u32;
        let out_a = a + existing * (255 - a) / 255;
        if out_a == 0 {
            return;
        }
        for (offset, channel) in [rgb.0, rgb.1, rgb.2].into_iter().enumerate() {
            let old = self.px[i + offset] as u32;
            let new = (channel as u32 * a + old * existing * (255 - a) / 255) / out_a;
            self.px[i + offset] = new.min(255) as u8;
        }
        self.px[i + 3] = out_a.min(255) as u8;
    }

    fn clear_px(&mut self, x: i32, y: i32, coverage: f32) {
        if !(0..self.w).contains(&x) || !(0..self.h).contains(&y) || coverage <= 0.0 {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let keep = (1.0 - coverage.clamp(0.0, 1.0)) * self.px[i + 3] as f32;
        if keep <= 0.5 {
            self.px[i] = 0;
            self.px[i + 1] = 0;
            self.px[i + 2] = 0;
        }
        self.px[i + 3] = keep.clamp(0.0, 255.0) as u8;
    }

    /// Paint one device pixel with a coverage in 0..=1.
    fn put(&mut self, x: i32, y: i32, rgb: (u8, u8, u8), alpha: u8, coverage: f32) {
        let coverage = coverage.clamp(0.0, 1.0);
        if coverage <= 0.0 {
            return;
        }
        let a = (alpha as f32 * coverage).round() as u8;
        self.blend(x, y, rgb, a);
    }
}

/// Two geometry helpers that convert a scalar distance to pixel coverage.
fn coverage_from_sdf(d: f32) -> f32 {
    // Half a device pixel of softness gives a clean 2x anti-aliased edge.
    (0.5 - d * PT as f32).clamp(0.0, 1.0)
}

fn paint_round_rect(canvas: &mut Canvas, r: Rect, radius: f32, rgb: (u8, u8, u8), alpha: u8) {
    let x0 = (r.x0 * PT as f32).floor().max(0.0) as i32;
    let x1 = (r.x1 * PT as f32).ceil().min(canvas.w as f32) as i32;
    let y0 = (r.y0 * PT as f32).floor().max(0.0) as i32;
    let y1 = (r.y1 * PT as f32).ceil().min(canvas.h as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let fx = (x as f32 + 0.5) / PT as f32;
            let fy = (y as f32 + 0.5) / PT as f32;
            let d = rounded_rect_sdf(fx, fy, &r, radius);
            canvas.put(x, y, rgb, alpha, coverage_from_sdf(d));
        }
    }
}

fn erase_round_rect(canvas: &mut Canvas, r: Rect, radius: f32) {
    let x0 = (r.x0 * PT as f32).floor().max(0.0) as i32;
    let x1 = (r.x1 * PT as f32).ceil().min(canvas.w as f32) as i32;
    let y0 = (r.y0 * PT as f32).floor().max(0.0) as i32;
    let y1 = (r.y1 * PT as f32).ceil().min(canvas.h as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let fx = (x as f32 + 0.5) / PT as f32;
            let fy = (y as f32 + 0.5) / PT as f32;
            let d = rounded_rect_sdf(fx, fy, &r, radius);
            canvas.clear_px(x, y, coverage_from_sdf(d));
        }
    }
}

fn paint_rect(canvas: &mut Canvas, r: Rect, rgb: (u8, u8, u8), alpha: u8) {
    paint_round_rect(canvas, r, 0.0, rgb, alpha);
}

fn paint_disc(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32, rgb: (u8, u8, u8), alpha: u8) {
    let bounds = Rect::new(cx - radius, cy - radius, cx + radius, cy + radius);
    let x0 = (bounds.x0 * PT as f32).floor().max(0.0) as i32;
    let x1 = (bounds.x1 * PT as f32).ceil().min(canvas.w as f32) as i32;
    let y0 = (bounds.y0 * PT as f32).floor().max(0.0) as i32;
    let y1 = (bounds.y1 * PT as f32).ceil().min(canvas.h as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let fx = (x as f32 + 0.5) / PT as f32;
            let fy = (y as f32 + 0.5) / PT as f32;
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() - radius;
            canvas.put(x, y, rgb, alpha, coverage_from_sdf(d));
        }
    }
}

fn erase_disc(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32) {
    let bounds = Rect::new(cx - radius, cy - radius, cx + radius, cy + radius);
    let x0 = (bounds.x0 * PT as f32).floor().max(0.0) as i32;
    let x1 = (bounds.x1 * PT as f32).ceil().min(canvas.w as f32) as i32;
    let y0 = (bounds.y0 * PT as f32).floor().max(0.0) as i32;
    let y1 = (bounds.y1 * PT as f32).ceil().min(canvas.h as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let fx = (x as f32 + 0.5) / PT as f32;
            let fy = (y as f32 + 0.5) / PT as f32;
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() - radius;
            canvas.clear_px(x, y, coverage_from_sdf(d));
        }
    }
}

/// The key mark: round head with a hole, shaft and two teeth — drawn to read
/// at 18pt, so every stroke is at least one point thick.
fn draw_key_glyph(canvas: &mut Canvas, cx: f32, cy: f32, scale: f32, rgb: (u8, u8, u8), alpha: u8) {
    let s = |v: f32| v * scale;
    paint_disc(canvas, cx - s(3.6), cy, s(3.0), rgb, alpha);
    erase_disc(canvas, cx - s(3.6), cy, s(1.2));
    paint_round_rect(
        canvas,
        Rect::new(cx - s(1.2), cy - s(1.2), cx + s(7.8), cy + s(1.2)),
        s(1.2),
        rgb,
        alpha,
    );
    paint_round_rect(
        canvas,
        Rect::new(cx + s(3.4), cy + s(1.2), cx + s(5.0), cy + s(3.6)),
        s(0.6),
        rgb,
        alpha,
    );
    paint_round_rect(
        canvas,
        Rect::new(cx + s(6.2), cy + s(1.2), cx + s(7.8), cy + s(3.6)),
        s(0.6),
        rgb,
        alpha,
    );
}

/// The projected-pace tick: a white bar over the track, which overhangs it so
/// it reads as a marker rather than a fill edge.
///
/// A template icon is tinted by the system, so it always contrasts with the
/// menu bar and needs nothing extra. A coloured icon keeps its own pixels, so
/// on a light menu bar a bare white tick would disappear — it gets a dark halo.
fn draw_pace_marker(canvas: &mut Canvas, x: f32, y0: f32, y1: f32, template: bool) {
    if !template {
        paint_rect(canvas, Rect::new(x - 0.5, y0, x + 1.3, y1), (0, 0, 0), 160);
    }
    paint_rect(
        canvas,
        Rect::new(x, y0 + 0.4, x + 0.8, y1 - 0.4),
        (255, 255, 255),
        255,
    );
}

/// Rounded track + level fill + optional pace marker, laid out horizontally.
#[allow(clippy::too_many_arguments)]
fn draw_hbar(
    canvas: &mut Canvas,
    r: Rect,
    percent: f64,
    fill: (u8, u8, u8),
    track: (u8, u8, u8),
    track_alpha: u8,
    pace: Option<f64>,
    template: bool,
) {
    let radius = ((r.y1 - r.y0) / 2.0).min(2.2);
    paint_round_rect(canvas, r, radius, track, track_alpha);
    let clamped = percent.clamp(0.0, 100.0) / 100.0;
    let width = r.x1 - r.x0;
    let filled = (width as f64 * clamped) as f32;
    if filled > 0.0 {
        paint_round_rect(
            canvas,
            Rect::new(r.x0, r.y0, r.x0 + filled.max(radius * 2.0), r.y1),
            radius,
            fill,
            255,
        );
    }
    if let Some(pace) = pace {
        let x = (r.x0 + width * (pace.clamp(0.0, 130.0) / 100.0) as f32).min(r.x1 - 1.0);
        // Keep the tick proportional to the bar so stacked bars don't collide.
        let overhang = ((r.y1 - r.y0) * 0.35).min(1.2);
        draw_pace_marker(canvas, x, r.y0 - overhang, r.y1 + overhang, template);
    }
}

fn track_color(mode: &str) -> (u8, u8, u8) {
    match mode {
        "mono" => (0, 0, 0),
        _ => (120, 120, 128),
    }
}

/// Template (mono) icons are tinted through their alpha channel, so the empty
/// part of a track has to be translucent or the level would be invisible.
fn track_alpha(mode: &str) -> u8 {
    if is_template(mode) {
        72
    } else {
        120
    }
}

fn fill_color(mode: &str, custom: (u8, u8, u8), percent: f64) -> (u8, u8, u8) {
    match mode {
        "mono" => (0, 0, 0),
        "custom" => custom,
        _ => level_color(percent),
    }
}

fn is_template(mode: &str) -> bool {
    mode == "mono"
}

// ── style renderers ────────────────────────────────────────────────────────

fn raw_metric(profile: &TrayProfile, name: &str) -> Option<f64> {
    match name {
        "rolling" => profile.metrics.rolling,
        "weekly" => profile.metrics.weekly,
        "monthly" => profile.metrics.monthly,
        _ => None,
    }
}

/// Which metric drives the single-value styles (battery, percent, key, and the
/// single-number title). The user picks it: `auto` follows the first selected
/// window, `worst` follows whichever selected window is closest to its limit,
/// and the rest name a window explicitly. An unavailable window falls back to
/// `auto` so the icon never goes blank.
pub fn primary_metric(snapshot: &TraySnapshot, profile: &TrayProfile) -> Option<(String, f64)> {
    let selected = |name: &str| snapshot.metrics.iter().any(|m| m == name);

    if snapshot.primary == "worst" {
        if let Some((name, value)) = ["rolling", "weekly", "monthly"]
            .iter()
            .filter(|name| selected(name))
            .filter_map(|name| raw_metric(profile, name).map(|value| (*name, value)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            return Some((name.to_string(), value));
        }
    } else if snapshot.primary != "auto" && selected(&snapshot.primary) {
        if let Some(value) = raw_metric(profile, &snapshot.primary) {
            return Some((snapshot.primary.clone(), value));
        }
    }

    for name in ["rolling", "weekly", "monthly"] {
        if !selected(name) {
            continue;
        }
        if let Some(value) = raw_metric(profile, name) {
            return Some((name.to_string(), value));
        }
    }
    profile
        .metrics
        .rolling
        .or(profile.metrics.weekly)
        .or(profile.metrics.monthly)
        .map(|value| ("rolling".to_string(), value))
}

fn display_percent(snapshot: &TraySnapshot, percent: f64) -> f64 {
    let value = if snapshot.show_remaining {
        100.0 - percent
    } else {
        percent
    };
    value.clamp(0.0, 100.0)
}

/// The pace projection is computed against *used* percent, so it has to be
/// mirrored when the bars are drawing remaining percent instead.
fn pace_in_display_space(snapshot: &TraySnapshot, pace: f64) -> f64 {
    if snapshot.show_remaining {
        (100.0 - pace).clamp(0.0, 100.0)
    } else {
        pace
    }
}

fn metric_value(snapshot: &TraySnapshot, profile: &TrayProfile, metric: &str) -> Option<f64> {
    match metric {
        "rolling" => profile.metrics.rolling,
        "weekly" => profile.metrics.weekly,
        "monthly" => profile.metrics.monthly,
        _ => None,
    }
    .map(|value| display_percent(snapshot, value))
}

fn render_profile_cell(
    canvas: &mut Canvas,
    x0: f32,
    profile: &TrayProfile,
    snapshot: &TraySnapshot,
) {
    let (primary_name, primary) = primary_metric(snapshot, profile).unwrap_or_default();
    let fill = fill_color(&snapshot.color_mode, snapshot.custom_color, primary);
    let track = track_color(&snapshot.color_mode);
    let base_track_alpha = track_alpha(&snapshot.color_mode);
    let template = is_template(&snapshot.color_mode);
    // Pace is projected from the 5-hour window only, so it is meaningless on a
    // bar that is speaking for the weekly or monthly window.
    let pace = if snapshot.show_pace && primary_name == "rolling" {
        profile
            .metrics
            .pace
            .map(|pace| pace_in_display_space(snapshot, pace))
    } else {
        None
    };
    // Multi-profile: the inactive cells dim so the active one reads without a
    // separate badge (which at 18pt would only be a smudge).
    let dim = snapshot.profiles.len() > 1 && snapshot.active_indicator && !profile.active;
    let alpha = if dim { 130 } else { 255 };
    let track_a = if dim {
        base_track_alpha / 2
    } else {
        base_track_alpha
    };
    let center = x0 + CELL_PT / 2.0;
    match snapshot.style.as_str() {
        "bar" => {
            // One horizontal bar per metric, stacked top to bottom. Length
            // follows the display mode (used or remaining) but the colour
            // always follows the *used* level, so a nearly-exhausted window
            // stays red whether the user reads used or remaining.
            let mut values: Vec<(String, f64, f64)> = snapshot
                .metrics
                .iter()
                .filter_map(|metric| {
                    raw_metric(profile, metric)
                        .map(|used| (metric.clone(), used, display_percent(snapshot, used)))
                })
                .collect();
            if values.is_empty() {
                values.push((String::new(), primary, display_percent(snapshot, primary)));
            }
            let count = values.len();
            let bar_h = match count {
                1 => 6.0,
                2 => 4.0,
                _ => 3.2,
            };
            let gap = 2.0;
            let total = count as f32 * bar_h + (count - 1) as f32 * gap;
            let mut y = (MENUBAR_PT - total) / 2.0;
            for (metric, used, shown) in &values {
                let bar_fill = fill_color(&snapshot.color_mode, snapshot.custom_color, *used);
                let bar_pace = if metric == "rolling" { pace } else { None };
                draw_hbar(
                    canvas,
                    Rect::new(x0 + 2.5, y, x0 + CELL_PT - 2.5, y + bar_h),
                    *shown,
                    bar_fill,
                    track,
                    track_a,
                    bar_pace,
                    template,
                );
                y += bar_h + gap;
            }
        }
        "iconbar" => {
            draw_key_glyph(canvas, center, 5.4, 0.9, fill, alpha);
            draw_hbar(
                canvas,
                Rect::new(x0 + 2.5, 12.2, x0 + CELL_PT - 2.5, 15.6),
                primary,
                fill,
                track,
                track_a,
                pace,
                template,
            );
        }
        "percent" | "compact" => {
            draw_key_glyph(canvas, center, 9.0, 1.05, fill, alpha);
        }
        _ => {
            // battery: a 9pt-tall body with a tip, filled from the left.
            let pct = display_percent(snapshot, primary);
            let body = Rect::new(x0 + 2.5, 4.5, x0 + CELL_PT - 3.5, 13.5);
            paint_round_rect(canvas, body, 2.4, track, track_a);
            let inner = Rect::new(body.x0 + 1.4, body.y0 + 1.4, body.x1 - 1.4, body.y1 - 1.4);
            erase_round_rect(canvas, inner, 1.4);
            let filled = ((inner.x1 - inner.x0) as f64 * (pct / 100.0)) as f32;
            if filled > 0.0 {
                paint_round_rect(
                    canvas,
                    Rect::new(
                        inner.x0,
                        inner.y0,
                        (inner.x0 + filled).max(inner.x0 + 2.4).min(inner.x1),
                        inner.y1,
                    ),
                    1.2,
                    fill,
                    alpha,
                );
            }
            // Tip.
            paint_round_rect(
                canvas,
                Rect::new(body.x1 + 0.6, 7.4, body.x1 + 2.0, 10.6),
                0.7,
                track,
                track_a,
            );
            if let Some(pace) = pace {
                let width = inner.x1 - inner.x0;
                let x = (inner.x0 + width * (pace.clamp(0.0, 130.0) / 100.0) as f32)
                    .min(inner.x1 - 1.2);
                draw_pace_marker(canvas, x, 3.2, 14.8, template);
            }
        }
    }
}

/// Menu-bar title text built from the selected metrics.
pub fn title(snapshot: &TraySnapshot) -> String {
    if snapshot.profiles.is_empty() {
        return String::new();
    }
    let compact = snapshot.style == "compact";
    let multi = snapshot.profiles.len() > 1;
    let mut parts = Vec::new();
    for profile in &snapshot.profiles {
        let (primary_name, primary_value) = primary_metric(snapshot, profile).unwrap_or_default();
        let text = if snapshot.used_mode == "tokens" && profile.tokens > 0.0 {
            format_tokens(profile.tokens)
        } else {
            format!(
                "{}%",
                display_percent(snapshot, primary_value).round() as i64
            )
        };
        let mut piece = if compact && snapshot.used_mode != "tokens" {
            snapshot
                .metrics
                .iter()
                .filter_map(|metric| metric_value(snapshot, profile, metric))
                .map(|value| format!("{}", value.round() as i64))
                .collect::<Vec<_>>()
                .join("|")
        } else {
            let prefix = match primary_name.as_str() {
                "weekly" => "W".to_string(),
                "monthly" => "M".to_string(),
                _ => String::new(),
            };
            format!("{prefix}{text}")
        };
        if piece.is_empty() {
            piece = text;
        }
        // The pace projection belongs to the 5-hour window, so it only rides
        // along while that window is the one being shown.
        let pace = if snapshot.show_pace && primary_name == "rolling" {
            profile.metrics.pace
        } else {
            None
        };
        if let Some(pace) = pace {
            if pace > 100.0 {
                piece.push('▲');
            } else if !compact && pace > 85.0 {
                piece.push('•');
            }
        }
        if !compact && snapshot.show_labels && multi {
            let initial = profile
                .label
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase().to_string())
                .unwrap_or_default();
            if !initial.is_empty() {
                piece = format!("{initial}{piece}");
            }
        }
        if multi {
            let marker = if profile.active && snapshot.active_indicator {
                "●"
            } else {
                "○"
            };
            piece = format!("{marker}{piece}");
        }
        parts.push(piece);
    }
    parts.join(" ")
}

pub fn format_tokens(tokens: f64) -> String {
    if tokens >= 1_000_000.0 {
        let value = tokens / 1_000_000.0;
        if value >= 10.0 {
            format!("{value:.0}M")
        } else {
            format!("{value:.1}M")
        }
    } else if tokens >= 1_000.0 {
        format!("{:.0}K", tokens / 1_000.0)
    } else {
        format!("{tokens:.0}")
    }
}

// ── public entry points ────────────────────────────────────────────────────

/// Build the snapshot's custom colour from its packed form.
pub fn custom_color(hex: &str) -> (u8, u8, u8) {
    hex_to_rgb(hex)
}

/// (rgba pixels, width, height, use_template).
///
/// The canvas is `PT`x the menu bar's 18pt height, so the raster lands 1:1 on
/// a Retina display and halves cleanly on a 1x one.
pub fn render(snapshot: &TraySnapshot) -> (Vec<u8>, u32, u32, bool) {
    let count = snapshot.profiles.len().max(1);
    let width_pt =
        PAD_PT * 2.0 + CELL_PT * count as f32 + GAP_PT * (count.saturating_sub(1)) as f32;
    let mut canvas = Canvas::new_pt(width_pt);
    for (index, profile) in snapshot.profiles.iter().enumerate() {
        let x0 = PAD_PT + index as f32 * (CELL_PT + GAP_PT);
        render_profile_cell(&mut canvas, x0, profile, snapshot);
    }
    let width = canvas.w as u32;
    let height = canvas.h as u32;
    (canvas.px, width, height, is_template(&snapshot.color_mode))
}

pub fn update(app: &tauri::AppHandle, snapshot: &TraySnapshot) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let (rgba, width, height, template) = render(snapshot);
    let image = tauri::image::Image::new_owned(rgba, width, height);
    let _ = tray.set_icon(Some(image));
    let _ = tray.set_icon_as_template(template);
    let title = title(snapshot);
    let _ = tray.set_title(if title.is_empty() { None } else { Some(title) });
    let tooltip = snapshot
        .profiles
        .iter()
        .map(|p| {
            let (_, primary) = primary_metric(snapshot, p).unwrap_or_default();
            format!("{} · {:.0}%", p.label, display_percent(snapshot, primary))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let _ = tray.set_tooltip(Some(&tooltip));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> TraySnapshot {
        TraySnapshot {
            profiles: vec![TrayProfile {
                id: "default".to_string(),
                label: "Default".to_string(),
                active: true,
                metrics: TrayMetric {
                    rolling: Some(42.0),
                    weekly: Some(18.0),
                    monthly: Some(7.0),
                    pace: Some(120.0),
                },
                tokens: 0.0,
                cost: 1.0,
            }],
            style: "battery".to_string(),
            color_mode: "multi".to_string(),
            custom_color: (10, 132, 255),
            used_mode: "percent".to_string(),
            show_remaining: false,
            show_labels: false,
            show_pace: true,
            metrics: vec!["rolling".to_string(), "weekly".to_string()],
            primary: "auto".to_string(),
            active_indicator: true,
        }
    }

    #[test]
    fn remaining_mode_flips_the_length_but_not_the_colour() {
        // 85% used is dangerous. With "show remaining" the bar must get short,
        // but it must stay red — colouring by the remaining figure would paint
        // a nearly-exhausted window green.
        let sample = |show_remaining: bool| {
            let mut snap = snapshot();
            snap.style = "bar".to_string();
            snap.color_mode = "multi".to_string();
            snap.metrics = vec!["rolling".to_string()];
            snap.profiles[0].metrics.rolling = Some(85.0);
            snap.show_pace = false;
            snap.show_remaining = show_remaining;
            let (px, w, _, _) = render(&snap);
            let at = |x: u32, y: u32| {
                let i = ((y * w + x) * 4) as usize;
                (px[i], px[i + 1], px[i + 2], px[i + 3])
            };
            // A single bar is 6pt tall, centred: y 6..12pt → px 12..24.
            // 10% along the 15pt bar is 4pt → px 8.
            (at(8, 18), at(28, 18))
        };

        let (used_start, used_end) = sample(false);
        let (rem_start, rem_end) = sample(true);

        // 85% used is the amber level; the same pixel must hold that colour
        // whichever way the length is drawn.
        assert_eq!(used_start, (255, 159, 10, 255), "85% used is amber");
        assert_eq!(rem_start, used_start, "colour must not follow remaining");
        // 85% used → long bar; 85% remaining → short bar.
        assert!(used_end.3 > 200, "used mode should fill the far end");
        assert!(
            rem_end.3 < 200,
            "remaining mode should leave the far end empty"
        );
    }

    #[test]
    fn reported_window_follows_the_setting() {
        let mut snap = snapshot();
        // rolling 42, weekly 18, monthly 7.
        let pick = |snap: &TraySnapshot| primary_metric(snap, &snap.profiles[0]).unwrap();

        snap.primary = "auto".to_string();
        assert_eq!(pick(&snap), ("rolling".to_string(), 42.0));

        snap.primary = "weekly".to_string();
        assert_eq!(pick(&snap), ("weekly".to_string(), 18.0));

        snap.primary = "worst".to_string();
        assert_eq!(pick(&snap), ("rolling".to_string(), 42.0));

        // Worst only considers the selected windows.
        snap.metrics = vec!["weekly".to_string(), "monthly".to_string()];
        assert_eq!(pick(&snap), ("weekly".to_string(), 18.0));

        // A window that is not selected falls back to the first selected one.
        snap.primary = "monthly".to_string();
        snap.metrics = vec!["rolling".to_string(), "weekly".to_string()];
        assert_eq!(pick(&snap), ("rolling".to_string(), 42.0));

        // A selected window with no reported value also falls back.
        snap.primary = "weekly".to_string();
        snap.profiles[0].metrics.weekly = None;
        assert_eq!(pick(&snap), ("rolling".to_string(), 42.0));
    }

    #[test]
    fn worst_window_drives_the_colour() {
        // rolling is calm, weekly is over the red line: the icon must warn.
        let mut snap = snapshot();
        snap.metrics = vec!["rolling".to_string(), "weekly".to_string()];
        snap.profiles[0].metrics.rolling = Some(10.0);
        snap.profiles[0].metrics.weekly = Some(95.0);
        let calm = fill_color("multi", snap.custom_color, 10.0);
        let hot = fill_color("multi", snap.custom_color, 95.0);

        snap.primary = "auto".to_string();
        assert_eq!(
            fill_color(
                "multi",
                snap.custom_color,
                primary_metric(&snap, &snap.profiles[0]).unwrap().1
            ),
            calm
        );
        snap.primary = "worst".to_string();
        assert_eq!(
            fill_color(
                "multi",
                snap.custom_color,
                primary_metric(&snap, &snap.profiles[0]).unwrap().1
            ),
            hot
        );
    }

    #[test]
    fn projected_extrapolates_and_rejects_edge_cases() {
        let resets = (chrono::Utc::now() + chrono::Duration::seconds(3600))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        // 4 of 5 hours elapsed, 40% used → projects to 50%.
        let value = projected(40.0, Some(&resets), "rolling").expect("projection");
        assert!((value - 50.0).abs() < 0.5, "got {value}");
        assert!(projected(40.0, Some("nonsense"), "rolling").is_none());
        assert!(projected(40.0, Some(&resets), "custom").is_none());
        assert!(projected(0.0, None, "rolling").is_none());
    }

    #[test]
    fn level_and_custom_colors_pick_the_right_tint() {
        assert_eq!(level_color(10.0), GREEN);
        assert_eq!(level_color(75.0), AMBER);
        assert_eq!(level_color(99.0), RED);
        assert_eq!(fill_color("custom", (1, 2, 3), 99.0), (1, 2, 3));
        assert_eq!(fill_color("mono", (1, 2, 3), 99.0), (0, 0, 0));
        assert_eq!(custom_color("#0a84ff"), (10, 132, 255));
    }

    #[test]
    fn titles_cover_single_multi_and_remaining_modes() {
        let mut snap = snapshot();
        assert_eq!(title(&snap), "42%▲");

        snap.show_remaining = true;
        assert_eq!(title(&snap), "58%▲");
        snap.show_remaining = false;

        // Pace is a 5-hour projection, so it rides only on the rolling window.
        snap.metrics = vec!["weekly".to_string()];
        assert_eq!(title(&snap), "W18%");

        snap.primary = "weekly".to_string();
        snap.metrics = vec!["rolling".to_string(), "weekly".to_string()];
        assert_eq!(title(&snap), "W18%");
        snap.primary = "auto".to_string();

        snap.metrics = vec!["rolling".to_string(), "weekly".to_string()];
        snap.style = "compact".to_string();
        assert_eq!(title(&snap), "42|18▲");

        snap.style = "battery".to_string();
        snap.show_labels = true;
        snap.profiles.push(TrayProfile {
            id: "work".to_string(),
            label: "Work".to_string(),
            active: false,
            metrics: TrayMetric {
                rolling: Some(87.0),
                pace: Some(88.0),
                ..TrayMetric::default()
            },
            tokens: 0.0,
            cost: 0.0,
        });
        assert_eq!(title(&snap), "●D42%▲ ○W87%•");
    }

    #[test]
    fn token_formatting_is_readable() {
        assert_eq!(format_tokens(950.0), "950");
        assert_eq!(format_tokens(600_000.0), "600K");
        assert_eq!(format_tokens(1_000_000.0), "1.0M");
        assert_eq!(format_tokens(12_600_000.0), "13M");
    }

    #[test]
    fn every_style_renders_a_non_empty_icon() {
        for style in ["battery", "bar", "percent", "iconbar", "compact"] {
            let mut snap = snapshot();
            snap.style = style.to_string();
            let (px, w, h, template) = render(&snap);
            assert_eq!(px.len(), (w * h * 4) as usize, "{style}");
            assert!(px.chunks(4).any(|p| p[3] > 0), "{style} drew nothing");
            assert!(!template);
        }
        let mut mono = snapshot();
        mono.color_mode = "mono".to_string();
        assert!(render(&mono).3);
    }

    #[test]
    fn canvas_matches_the_menu_bar_height() {
        let (_, w, h, _) = render(&snapshot());
        assert_eq!(h, (MENUBAR_PT as u32) * PT as u32);
        // One profile cell plus the padding, in points → pixels.
        assert_eq!(w, ((PAD_PT * 2.0 + CELL_PT) * PT as f32) as u32);
    }

    #[test]
    fn mono_icons_still_show_the_level() {
        // Template icons are tinted by alpha; if track and fill shared one
        // alpha the icon would be a solid blob and the level invisible.
        for style in ["battery", "bar", "iconbar"] {
            let mut low = snapshot();
            low.style = style.to_string();
            low.color_mode = "mono".to_string();
            low.profiles[0].metrics.rolling = Some(12.0);
            low.metrics = vec!["rolling".to_string()];
            let mut high = low.clone();
            high.profiles[0].metrics.rolling = Some(94.0);
            assert_ne!(
                render(&low).0,
                render(&high).0,
                "{style}: mono icon ignores the level"
            );
        }
    }

    #[test]
    fn active_profile_is_brighter_than_the_others() {
        let mut snap = snapshot();
        snap.color_mode = "mono".to_string();
        snap.metrics = vec!["rolling".to_string()];
        snap.active_indicator = true;
        snap.profiles.push(TrayProfile {
            id: "work".to_string(),
            label: "Work".to_string(),
            active: false,
            metrics: TrayMetric {
                rolling: Some(42.0),
                ..TrayMetric::default()
            },
            tokens: 0.0,
            cost: 0.0,
        });
        let (px, w, h, _) = render(&snap);
        let sum_alpha = |from: u32, to: u32| {
            let mut total = 0u32;
            for y in 0..h {
                for x in from..to {
                    total += px[((y * w + x) * 4 + 3) as usize] as u32;
                }
            }
            total
        };
        // Each cell occupies CELL_PT + GAP_PT, so the second cell starts at
        // (PAD_PT + CELL_PT + GAP_PT) * PT device pixels.
        let split = ((PAD_PT + CELL_PT + GAP_PT) * PT as f32) as u32;
        let active = sum_alpha(0, split);
        let other = sum_alpha(split, w);
        assert!(
            active > other,
            "active cell {active} should out-alpha {other}"
        );
    }
}
