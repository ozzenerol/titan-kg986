use crate::widgets::{hsv_color, theme};
use eframe::egui::{Color32, CornerRadius, Painter, Rect, Stroke, StrokeKind, Vec2};

const GAP: f32 = -1.0;

const ROWS: [&[f32]; 6] = [
    &[1.0, GAP, 1.0, 1.0, 1.0, 1.0, GAP, 1.0, 1.0, 1.0, 1.0, GAP, 1.0, 1.0, 1.0, 1.0, GAP, 1.0, 1.0, 1.0, 1.0],
    &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, GAP, 1.0, 1.0, 1.0, 1.0],
    &[1.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.5, GAP, 1.0, 1.0, 1.0, 1.0],
    &[1.75, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.25, GAP, 1.0, 1.0, 1.0, 1.0],
    &[2.25, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.75, 1.0, GAP, 1.0, 1.0, 1.0, 1.0],
    &[1.25, 1.25, 1.25, 6.25, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, GAP, 2.0, 1.0, 1.0],
];

const GAP_UNITS: f32 = 0.35;

pub struct Look {
    pub on: bool,
    pub brightness: f32,
    pub speed: f32,
    pub single: Option<[f32; 3]>,
    pub effect: u8,
}

fn row_units(row: &[f32]) -> f32 {
    row.iter().map(|w| if *w == GAP { GAP_UNITS } else { *w }).sum()
}

pub fn paint(p: &Painter, area: Rect, look: &Look, t: f32) {
    let units = ROWS.iter().map(|r| row_units(r)).fold(0.0, f32::max);
    let unit = (area.width() / (units + 1.2)).min(area.height() / (ROWS.len() as f32 + 1.4));
    let size = Vec2::new(units * unit, ROWS.len() as f32 * unit);
    let origin = area.center() - size / 2.0;
    let body = Rect::from_min_size(origin, size).expand(unit * 0.45);

    let level = if look.on { 0.18 + 0.82 * look.brightness } else { 0.0 };
    let glow_color = match look.single {
        Some(h) => hsv_color(h),
        None => hsv_color([(t * 0.05 * (0.5 + look.speed)).fract(), 0.8, 1.0]),
    };
    for i in 0..6 {
        let k = i as f32;
        p.rect_filled(
            body.expand(4.0 + k * 5.0),
            CornerRadius::same((16.0 + k * 5.0) as u8),
            glow_color.gamma_multiply(0.035 * level * (6.0 - k) / 6.0),
        );
    }
    p.rect(body, CornerRadius::same(14), Color32::from_rgb(0x10, 0x12, 0x18), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);

    let phase = t * (0.35 + look.speed * 0.45);
    let seed = look.effect as f32 * 1.618;
    for (r, row) in ROWS.iter().enumerate() {
        let mut x = 0.0;
        for w in row.iter() {
            if *w == GAP {
                x += GAP_UNITS;
                continue;
            }
            let rect = Rect::from_min_size(
                origin + Vec2::new(x * unit, r as f32 * unit),
                Vec2::new(w * unit, unit),
            )
            .shrink(unit * 0.07);
            let u = (x + w / 2.0) / units;
            let v = r as f32 / ROWS.len() as f32;
            let wave = 0.55 + 0.45 * ((u * 6.0 + v * 2.0 - phase * 2.0 + seed).sin());
            let color = if !look.on {
                Color32::from_rgb(0x1c, 0x1f, 0x27)
            } else {
                let c = match look.single {
                    Some(h) => hsv_color([h[0], h[1], h[2] * wave]),
                    None => hsv_color([(u * 0.9 + v * 0.1 - phase * 0.15 + seed).rem_euclid(1.0), 0.85, wave]),
                };
                lerp(Color32::from_rgb(0x1c, 0x1f, 0x27), c, level)
            };
            p.rect_filled(rect.translate(Vec2::new(0.0, unit * 0.05)), CornerRadius::same(5), Color32::from_black_alpha(120));
            p.rect(rect, CornerRadius::same(5), color, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
            let cap = Rect::from_min_max(rect.min + Vec2::splat(unit * 0.12), rect.max - Vec2::new(unit * 0.12, unit * 0.2));
            p.rect_filled(cap, CornerRadius::same(4), Color32::from_white_alpha(if look.on { 10 } else { 4 }));
            x += w;
        }
    }
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}
