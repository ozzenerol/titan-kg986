use crate::device::Device;
use crate::protocol::{Config, Lighting};
use crate::widgets::{self as w, theme};
use crate::{icon, preview};
use eframe::egui::{self, ecolor::Hsva, Align, FontData, FontDefinitions, FontFamily, Layout, RichText, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

const MAX_BRIGHTNESS: u8 = 4;
const MAX_SPEED: u8 = 4;
const EFFECTS: u8 = 18;

enum Job {
    Read,
    Apply(Lighting, Option<[u8; 3]>),
    Restore(PathBuf),
}

enum Reply {
    Config(Config, String),
    Error(String),
}

fn data_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".local/share/titan-kg986")
}

fn backup_path() -> PathBuf {
    data_dir().join("original.hex")
}

fn color_path() -> PathBuf {
    data_dir().join("color")
}

fn worker(jobs: Receiver<Job>, replies: Sender<Reply>, ctx: egui::Context) {
    for job in jobs {
        let result = Device::open().and_then(|mut dev| match job {
            Job::Read => {
                let c = dev.read_config()?;
                let backup = backup_path();
                if !backup.exists() {
                    std::fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
                    std::fs::write(&backup, c.to_hex()).map_err(|e| e.to_string())?;
                }
                Ok((c, "Connected".to_string()))
            }
            Job::Apply(l, color) => {
                let c = dev.apply(l, color)?;
                if let Some([r, g, b]) = color {
                    std::fs::create_dir_all(data_dir()).ok();
                    std::fs::write(color_path(), format!("{r:02x}{g:02x}{b:02x}")).ok();
                }
                Ok((c, "Applied and verified".to_string()))
            }
            Job::Restore(path) => {
                let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let saved = Config::from_hex(&text)?;
                dev.write(&saved.write_frames(None))?;
                Ok((dev.read_config()?, "Original settings restored".to_string()))
            }
        });
        let reply = match result {
            Ok((c, msg)) => Reply::Config(c, msg),
            Err(e) => Reply::Error(e),
        };
        if replies.send(reply).is_err() {
            break;
        }
        ctx.request_repaint();
    }
}

fn load_color() -> [f32; 3] {
    let rgb = std::fs::read_to_string(color_path())
        .ok()
        .and_then(|s| u32::from_str_radix(s.trim(), 16).ok())
        .map(|v| [(v >> 16) as u8, (v >> 8) as u8, v as u8])
        .unwrap_or([139, 124, 255]);
    let h = Hsva::from_srgb(rgb);
    [h.h, h.s, h.v]
}

fn setup_style(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("inter".into(), Arc::new(FontData::from_static(include_bytes!("../assets/Inter-Regular.ttf"))));
    fonts.font_data.insert("inter-semibold".into(), Arc::new(FontData::from_static(include_bytes!("../assets/Inter-SemiBold.ttf"))));
    fonts.font_data.insert("jbmono".into(), Arc::new(FontData::from_static(include_bytes!("../assets/JetBrainsMono-Medium.ttf"))));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "inter".into());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "jbmono".into());
    fonts.families.insert(FontFamily::Name("semibold".into()), vec!["inter-semibold".into(), "inter".into()]);
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Dark);
    let mut v = egui::Visuals::dark();
    v.panel_fill = theme::BG;
    v.window_fill = theme::CARD;
    v.override_text_color = Some(theme::TEXT);
    v.selection.bg_fill = theme::ACCENT;
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| s.spacing.item_spacing = Vec2::new(10.0, 8.0));
}

struct App {
    jobs: Sender<Job>,
    replies: Receiver<Reply>,
    busy: bool,
    connected: bool,
    status: String,
    error: bool,
    lighting: Lighting,
    hsv: [f32; 3],
    mode: usize,
    dirty: bool,
    start: Instant,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        setup_style(&cc.egui_ctx);
        let (job_tx, job_rx) = channel();
        let (reply_tx, reply_rx) = channel();
        let ctx = cc.egui_ctx.clone();
        thread::spawn(move || worker(job_rx, reply_tx, ctx));
        job_tx.send(Job::Read).ok();
        App {
            jobs: job_tx,
            replies: reply_rx,
            busy: true,
            connected: false,
            status: "Looking for the receiver".into(),
            error: false,
            lighting: Lighting { effect: 1, brightness: MAX_BRIGHTNESS, speed: 2, colorful: true },
            hsv: load_color(),
            mode: 0,
            dirty: false,
            start: Instant::now(),
        }
    }

    fn submit(&mut self, job: Job, status: &str) {
        if self.jobs.send(job).is_ok() {
            self.busy = true;
            self.error = false;
            self.status = status.into();
        }
    }

    fn poll(&mut self) {
        while let Ok(reply) = self.replies.try_recv() {
            self.busy = false;
            match reply {
                Reply::Config(c, msg) => {
                    let l = c.lighting();
                    self.lighting = Lighting {
                        brightness: l.brightness.min(MAX_BRIGHTNESS),
                        speed: l.speed.min(MAX_SPEED),
                        ..l
                    };
                    self.mode = if l.colorful || l.effect == 0 { 0 } else { 1 };
                    self.connected = true;
                    self.dirty = false;
                    self.status = msg;
                    self.error = false;
                }
                Reply::Error(e) => {
                    self.connected = !e.contains("not found");
                    self.status = e;
                    self.error = true;
                }
            }
        }
    }

    fn apply(&mut self) {
        let mut l = self.lighting;
        l.colorful = self.mode == 0;
        if l.effect == 0 {
            l = Lighting { effect: 0, brightness: 0, speed: 0, colorful: false };
        }
        let color = (self.mode == 1 && l.effect != 0).then(|| {
            let c = w::hsv_color(self.hsv);
            [c.r(), c.g(), c.b()]
        });
        self.submit(Job::Apply(l, color), "Writing to keyboard");
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Titan KG986").font(w::semibold(22.0)).color(theme::TEXT));
                ui.label(RichText::new("Marvo Titan 98  ·  2.4 GHz receiver").font(w::body(13.0)).color(theme::MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (text, color) = if self.busy && !self.connected {
                    ("Searching", theme::MUTED)
                } else if self.connected {
                    ("Connected", theme::OK)
                } else {
                    ("Receiver not found", theme::BAD)
                };
                egui::Frame::new()
                    .fill(color.gamma_multiply(0.12))
                    .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.4)))
                    .corner_radius(egui::CornerRadius::same(255))
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                            ui.painter().circle_filled(r.center(), 4.0, color);
                            ui.label(RichText::new(text).font(w::semibold(12.5)).color(color));
                        });
                    });
            });
        });
    }

    fn effects(&mut self, ui: &mut egui::Ui, on: bool) {
        w::card(ui, |ui| {
            w::section_title(ui, "Effect", "Fn + \\ cycles on the keyboard");
            let cols = 7;
            let gap = 8.0;
            let size = Vec2::new((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32, 36.0);
            egui::Grid::new("effects").spacing([gap, gap]).show(ui, |ui| {
                for id in 0..=EFFECTS {
                    let label = if id == 0 { "Off".to_string() } else { id.to_string() };
                    if w::chip(ui, size, &label, self.lighting.effect == id).clicked() {
                        self.lighting.effect = id;
                        self.dirty = true;
                    }
                    if (id + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
            ui.add_space(18.0);
            self.dirty |= w::step_slider(ui, "Brightness", &mut self.lighting.brightness, MAX_BRIGHTNESS, on);
            ui.add_space(14.0);
            self.dirty |= w::step_slider(ui, "Speed", &mut self.lighting.speed, MAX_SPEED, on);
        });
    }

    fn colour(&mut self, ui: &mut egui::Ui, on: bool) {
        w::card(ui, |ui| {
            let c = w::hsv_color(self.hsv);
            let hex = format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
            w::section_title(ui, "Colour", if self.mode == 1 && on { &hex } else { "" });
            let before = self.mode;
            w::segmented(ui, &["Effect colours", "Single colour"], &mut self.mode, on);
            self.dirty |= before != self.mode;
            ui.add_space(14.0);
            let pick = on && self.mode == 1;
            self.dirty |= w::color_picker(ui, &mut self.hsv, pick);
            ui.add_space(14.0);
            self.dirty |= w::swatches(ui, &mut self.hsv, pick);
        });
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.busy {
                ui.add(egui::Spinner::new().size(14.0).color(theme::ACCENT));
            }
            let color = if self.error {
                theme::BAD
            } else if self.dirty {
                theme::ACCENT_HI
            } else {
                theme::MUTED
            };
            let text = if self.dirty && !self.busy && !self.error { "Unsaved changes" } else { &self.status };
            ui.label(RichText::new(text).font(w::body(13.0)).color(color));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ready = !self.busy && self.connected;
                if w::button(ui, "Apply", true, ready).clicked() {
                    self.apply();
                }
                if w::button(ui, "Reload", false, !self.busy).clicked() {
                    self.submit(Job::Read, "Reading keyboard");
                }
                if w::button(ui, "Restore original", false, ready && backup_path().exists()).clicked() {
                    self.submit(Job::Restore(backup_path()), "Restoring original settings");
                }
            });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let on = self.lighting.effect != 0;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::same(24)))
            .show(ui, |ui| {
                self.header(ui);
                ui.add_space(18.0);

                let t = self.start.elapsed().as_secs_f32();
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 170.0), egui::Sense::hover());
                let look = preview::Look {
                    on,
                    brightness: self.lighting.brightness as f32 / MAX_BRIGHTNESS as f32,
                    speed: self.lighting.speed as f32 / MAX_SPEED as f32,
                    single: (self.mode == 1).then_some(self.hsv),
                    effect: self.lighting.effect,
                };
                preview::paint(ui.painter(), rect, &look, t);
                ui.add_space(18.0);

                ui.columns(2, |cols| {
                    self.effects(&mut cols[0], on);
                    self.colour(&mut cols[1], on);
                });

                ui.add_space(18.0);
                self.footer(ui);
                ui.add_space(10.0);
                credits(ui);
                ui.add_space(2.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Unofficial software. Not made, affiliated with or endorsed by MARVO.")
                            .font(w::body(11.5))
                            .color(theme::MUTED.gamma_multiply(0.8)),
                    );
                });
            });
        ui.ctx().request_repaint();
    }
}

fn credits(ui: &mut egui::Ui) {
    let font = w::body(12.0);
    let parts = ["Made with ", "\u{2764}", " by ozzenerol  \u{b7}  ", "github.com/ozzenerol"];
    let width: f32 = ui.fonts_mut(|f| {
        parts.iter().map(|p| f.layout_no_wrap(p.to_string(), font.clone(), theme::MUTED).size().x).sum()
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        ui.label(RichText::new(parts[0]).font(font.clone()).color(theme::MUTED));
        ui.label(RichText::new(parts[1]).font(font.clone()).color(theme::BAD));
        ui.label(RichText::new(parts[2]).font(font.clone()).color(theme::MUTED));
        ui.hyperlink_to(RichText::new(parts[3]).font(font).color(theme::ACCENT_HI), "https://github.com/ozzenerol");
    });
}

pub fn run() -> Result<(), String> {
    let icon = egui::IconData { rgba: icon::rgba(), width: icon::SIZE, height: icon::SIZE };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Titan KG986")
            .with_app_id("titan-kg986")
            .with_icon(Arc::new(icon))
            .with_inner_size([880.0, 790.0])
            .with_min_inner_size([780.0, 770.0]),
        ..Default::default()
    };
    eframe::run_native("Titan KG986", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
        .map_err(|e| e.to_string())
}
