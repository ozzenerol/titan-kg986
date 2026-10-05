use eframe::egui::ecolor::Hsva;

pub const SIZE: u32 = 256;

fn rrect_sdf(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - hw + r;
    let qy = (py - cy).abs() - hh + r;
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

fn srgb(h: Hsva) -> [f32; 3] {
    h.to_srgb().map(|v| v as f32 / 255.0)
}

fn coverage(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

fn over(dst: &mut [f32; 4], c: [f32; 3], a: f32) {
    let out_a = a + dst[3] * (1.0 - a);
    if out_a <= 0.0 {
        return;
    }
    for i in 0..3 {
        dst[i] = (c[i] * a + dst[i] * dst[3] * (1.0 - a)) / out_a;
    }
    dst[3] = out_a;
}

struct Key {
    cx: f32,
    cy: f32,
    hw: f32,
    hh: f32,
}

fn keys() -> Vec<Key> {
    let rows: [&[f32]; 5] = [
        &[1.0; 10],
        &[1.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.5],
        &[1.75, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.25],
        &[2.25, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.75],
        &[1.5, 1.5, 5.0, 1.5, 1.5],
    ];
    let (left, top, unit, gap) = (40.0, 76.0, 17.6, 4.0);
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        let mut x = left;
        for w in row.iter() {
            let width = w * unit;
            out.push(Key {
                cx: x + width / 2.0,
                cy: top + r as f32 * (unit + 4.0) + unit / 2.0,
                hw: (width - gap) / 2.0,
                hh: (unit - gap) / 2.0,
            });
            x += width;
        }
    }
    out
}

pub fn rgba() -> Vec<u8> {
    let s = SIZE as f32;
    let keys = keys();
    let mut buf = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut c = [0.0f32; 4];
            let tile = coverage(rrect_sdf(px, py, s / 2.0, s / 2.0, 116.0, 116.0, 52.0));
            if tile > 0.0 {
                let t = py / s;
                let bg = [0.10 - 0.04 * t, 0.11 - 0.04 * t, 0.15 - 0.05 * t];
                over(&mut c, bg, tile);
                let rim = coverage(rrect_sdf(px, py, s / 2.0, s / 2.0, 116.0, 116.0, 52.0).abs() - 1.0);
                over(&mut c, [0.32, 0.34, 0.42], rim * 0.6 * tile);
                let glow_d = rrect_sdf(px, py, s / 2.0, 128.0, 92.0, 52.0, 14.0).max(0.0);
                let hue = ((px - 40.0) / 176.0).clamp(0.0, 1.0) * 0.83;
                let glow = srgb(Hsva::new(hue, 0.9, 1.0, 1.0));
                over(&mut c, glow, (-glow_d / 16.0).exp() * 0.35 * tile);
                for k in &keys {
                    let d = rrect_sdf(px, py, k.cx, k.cy, k.hw, k.hh, 3.5);
                    let a = coverage(d);
                    if a > 0.0 {
                        let hue = ((k.cx - 40.0) / 176.0 + (k.cy - 76.0) / 900.0).clamp(0.0, 1.0) * 0.83;
                        let top = 1.0 - ((py - (k.cy - k.hh)) / (2.0 * k.hh)).clamp(0.0, 1.0) * 0.35;
                        let col = srgb(Hsva::new(hue, 0.78, top, 1.0));
                        over(&mut c, col, a * tile);
                    }
                }
            }
            for v in &c[..3] {
                buf.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
            buf.push((c[3].clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    buf
}

pub fn write_png(path: &std::path::Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), SIZE, SIZE);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(&rgba()).map_err(|e| e.to_string())
}
