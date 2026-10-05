use eframe::egui::{
    self, ecolor::Hsva, Align2, Color32, CornerRadius, FontFamily, FontId, Mesh, Pos2, Rect, Response,
    Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
};

pub mod theme {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x0d, 0x0f, 0x14);
    pub const CARD: Color32 = Color32::from_rgb(0x15, 0x18, 0x20);
    pub const RAISED: Color32 = Color32::from_rgb(0x1d, 0x21, 0x2b);
    pub const HOVER: Color32 = Color32::from_rgb(0x25, 0x2a, 0x36);
    pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x2b, 0x37);
    pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xea, 0xf0);
    pub const MUTED: Color32 = Color32::from_rgb(0x84, 0x8c, 0x9e);
    pub const ACCENT: Color32 = Color32::from_rgb(0x8b, 0x7c, 0xff);
    pub const ACCENT_HI: Color32 = Color32::from_rgb(0xa8, 0x9c, 0xff);
    pub const OK: Color32 = Color32::from_rgb(0x4a, 0xde, 0x80);
    pub const BAD: Color32 = Color32::from_rgb(0xf8, 0x71, 0x71);
}

use theme::*;

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn body(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn hsv_color(h: [f32; 3]) -> Color32 {
    let [r, g, b] = Hsva::new(h[0], h[1], h[2], 1.0).to_srgb();
    Color32::from_rgb(r, g, b)
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::same(18))
        .show(ui, add)
        .inner
}

pub fn section_title(ui: &mut Ui, title: &str, hint: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).font(semibold(15.0)).color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(hint).font(body(12.5)).color(MUTED));
        });
    });
    ui.add_space(12.0);
}

pub fn chip(ui: &mut Ui, size: Vec2, text: &str, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let (fill, stroke, fg) = if selected {
        (ACCENT, ACCENT_HI, Color32::WHITE)
    } else if resp.hovered() {
        (HOVER, BORDER, TEXT)
    } else {
        (RAISED, BORDER, MUTED)
    };
    p.rect(rect, CornerRadius::same(9), fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
    p.text(rect.center(), Align2::CENTER_CENTER, text, semibold(13.5), fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn step_slider(ui: &mut Ui, label: &str, value: &mut u8, max: u8, enabled: bool) -> bool {
    let before = *value;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).font(body(13.5)).color(if enabled { TEXT } else { MUTED }));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{value} / {max}")).font(mono(12.5)).color(MUTED));
        });
    });
    ui.add_space(6.0);
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click_and_drag());
    let pad = 9.0;
    let x0 = rect.left() + pad;
    let x1 = rect.right() - pad;
    if enabled {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
            *value = (t * max as f32).round() as u8;
        }
    }
    let p = ui.painter();
    let cy = rect.center().y;
    let track = Rect::from_min_max(Pos2::new(x0, cy - 3.0), Pos2::new(x1, cy + 3.0));
    p.rect_filled(track, CornerRadius::same(3), RAISED);
    let t = *value as f32 / max.max(1) as f32;
    let kx = x0 + t * (x1 - x0);
    let accent = if enabled { ACCENT } else { BORDER };
    p.rect_filled(
        Rect::from_min_max(track.min, Pos2::new(kx, track.max.y)),
        CornerRadius::same(3),
        accent,
    );
    for i in 0..=max {
        let x = x0 + i as f32 / max as f32 * (x1 - x0);
        let c = if i <= *value && enabled { ACCENT_HI } else { HOVER };
        p.circle_filled(Pos2::new(x, cy), 2.0, c);
    }
    let r = if resp.hovered() || resp.dragged() { 9.0 } else { 8.0 };
    p.circle_filled(Pos2::new(kx, cy), r + 3.0, accent.gamma_multiply(0.25));
    p.circle(Pos2::new(kx, cy), r, if enabled { Color32::WHITE } else { MUTED }, Stroke::new(2.0, accent));
    *value != before
}

pub fn segmented(ui: &mut Ui, options: &[&str], selected: &mut usize, enabled: bool) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(10), RAISED, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    let w = (rect.width() - 6.0) / options.len() as f32;
    for (i, name) in options.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(rect.left() + 3.0 + i as f32 * w, rect.top() + 3.0), Vec2::new(w, 28.0));
        let resp = ui.interact(r, ui.id().with(("seg", i)), Sense::click());
        if enabled && resp.clicked() {
            *selected = i;
        }
        let on = *selected == i;
        if on {
            ui.painter().rect_filled(r, CornerRadius::same(8), if enabled { ACCENT } else { HOVER });
        } else if resp.hovered() && enabled {
            ui.painter().rect_filled(r, CornerRadius::same(8), HOVER);
        }
        let fg = if on { Color32::WHITE } else { MUTED };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *name, semibold(13.0), fg);
    }
}

pub fn button(ui: &mut Ui, text: &str, primary: bool, enabled: bool) -> Response {
    let galley_w = ui.fonts_mut(|f| f.layout_no_wrap(text.into(), semibold(13.5), TEXT).size().x);
    let size = Vec2::new(galley_w + if primary { 44.0 } else { 28.0 }, 38.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let hovered = enabled && resp.hovered();
    let p = ui.painter();
    if primary {
        let fill = if !enabled { HOVER } else if hovered { ACCENT_HI } else { ACCENT };
        if enabled {
            p.rect_filled(rect.expand(3.0), CornerRadius::same(13), ACCENT.gamma_multiply(0.18));
        }
        p.rect_filled(rect, CornerRadius::same(10), fill);
        p.text(rect.center(), Align2::CENTER_CENTER, text, semibold(13.5), if enabled { Color32::WHITE } else { MUTED });
    } else {
        let fill = if hovered { HOVER } else { RAISED };
        p.rect(rect, CornerRadius::same(10), fill, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        p.text(rect.center(), Align2::CENTER_CENTER, text, semibold(13.5), if enabled { TEXT } else { MUTED });
    }
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

fn handle(p: &egui::Painter, pos: Pos2, fill: Color32) {
    p.circle_filled(pos, 10.0, Color32::from_black_alpha(90));
    p.circle(pos, 8.0, fill, Stroke::new(2.5, Color32::WHITE));
}

pub fn color_picker(ui: &mut Ui, hsv: &mut [f32; 3], enabled: bool) -> bool {
    let before = *hsv;
    let width = ui.available_width();
    let (sv_rect, sv) = ui.allocate_exact_size(Vec2::new(width, 132.0), Sense::click_and_drag());
    ui.add_space(10.0);
    let (hue_rect, hue) = ui.allocate_exact_size(Vec2::new(width, 16.0), Sense::click_and_drag());

    if enabled {
        if let Some(pos) = sv.interact_pointer_pos() {
            hsv[1] = ((pos.x - sv_rect.left()) / sv_rect.width()).clamp(0.0, 1.0);
            hsv[2] = 1.0 - ((pos.y - sv_rect.top()) / sv_rect.height()).clamp(0.0, 1.0);
        }
        if let Some(pos) = hue.interact_pointer_pos() {
            hsv[0] = ((pos.x - hue_rect.left()) / hue_rect.width()).clamp(0.0, 0.9999);
        }
    }

    let p = ui.painter();
    let dim = if enabled { 1.0 } else { 0.35 };
    let n = 24;
    let mut mesh = Mesh::default();
    for j in 0..=n {
        for i in 0..=n {
            let s = i as f32 / n as f32;
            let v = 1.0 - j as f32 / n as f32;
            let pos = Pos2::new(sv_rect.left() + s * sv_rect.width(), sv_rect.top() + (1.0 - v) * sv_rect.height());
            mesh.colored_vertex(pos, hsv_color([hsv[0], s, v]).gamma_multiply(dim));
        }
    }
    let row = n + 1;
    for j in 0..n {
        for i in 0..n {
            let a = j * row + i;
            mesh.add_triangle(a, a + 1, a + row);
            mesh.add_triangle(a + 1, a + row + 1, a + row);
        }
    }
    p.add(Shape::mesh(mesh));
    p.rect_stroke(sv_rect, CornerRadius::same(4), Stroke::new(1.0, BORDER), StrokeKind::Outside);

    let segs = 48;
    let mut mesh = Mesh::default();
    for i in 0..=segs {
        let t = i as f32 / segs as f32;
        let x = hue_rect.left() + t * hue_rect.width();
        let c = hsv_color([t.min(0.9999), 1.0, 1.0]).gamma_multiply(dim);
        mesh.colored_vertex(Pos2::new(x, hue_rect.top()), c);
        mesh.colored_vertex(Pos2::new(x, hue_rect.bottom()), c);
    }
    for i in 0..segs {
        let a = (i * 2) as u32;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    p.add(Shape::mesh(mesh));
    p.rect_stroke(hue_rect, CornerRadius::same(4), Stroke::new(1.0, BORDER), StrokeKind::Outside);

    if enabled {
        handle(
            p,
            Pos2::new(sv_rect.left() + hsv[1] * sv_rect.width(), sv_rect.top() + (1.0 - hsv[2]) * sv_rect.height()),
            hsv_color(*hsv),
        );
        handle(
            p,
            Pos2::new(hue_rect.left() + hsv[0] * hue_rect.width(), hue_rect.center().y),
            hsv_color([hsv[0], 1.0, 1.0]),
        );
    }
    *hsv != before
}

pub const PRESETS: [[u8; 3]; 9] = [
    [255, 255, 255],
    [255, 40, 40],
    [255, 140, 0],
    [255, 220, 0],
    [60, 230, 90],
    [0, 220, 220],
    [40, 110, 255],
    [150, 60, 255],
    [255, 60, 200],
];

pub fn swatches(ui: &mut Ui, hsv: &mut [f32; 3], enabled: bool) -> bool {
    let mut changed = false;
    let n = PRESETS.len() as f32;
    let gap = 8.0;
    let d = ((ui.available_width() - gap * (n - 1.0)) / n).min(26.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for rgb in PRESETS {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d), Sense::click());
            let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            let current = hsv_color(*hsv);
            let same = (c.r() as i32 - current.r() as i32).abs() < 4
                && (c.g() as i32 - current.g() as i32).abs() < 4
                && (c.b() as i32 - current.b() as i32).abs() < 4;
            let p = ui.painter();
            if same && enabled {
                p.circle_stroke(rect.center(), d / 2.0 + 2.5, Stroke::new(2.0, Color32::WHITE));
            }
            p.circle_filled(rect.center(), d / 2.0, if enabled { c } else { c.gamma_multiply(0.35) });
            if enabled && resp.clicked() {
                let h = Hsva::from_srgb(rgb);
                *hsv = [h.h, h.s, h.v];
                changed = true;
            }
            if enabled && resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }
    });
    changed
}
