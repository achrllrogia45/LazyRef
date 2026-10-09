//! LazyRef board shell: egui harness driving `lazyref_board::Board`.
//! Board.canvas is the camera (world->screen). Stickers render via
//! final_matrix -> screen quads. Pointer feeds Down/Move/Up in
//! viewport-local coords (global minus viewport.min).

use lazyref::egui::{self, pos2, Color32, CornerRadius, Key, Pos2, Rect, Sense, Shape, Stroke};
use lazyref_board::board::{Board, BoardEvent, Pointer};
use lazyref_board::math::Mat3;
use lazyref_board::settings::Settings;
use lazyref_board::sticker::{Sticker, StickerKind};

struct BoardApp {
    board: Board,
    settings: Settings,
    settings_path: std::path::PathBuf,
    open_path: String,
    save_path: String,
    add_path: String,
    add_text: String,
    show_settings: bool,
    status: String,
    // ponytail: last central-panel size, reused by toolbar Fit (no ctx rect needed).
    last_viewport: (f32, f32),
}

impl BoardApp {
    fn new() -> Self {
        let settings_path = std::env::temp_dir().join("lazyref-settings.cfg");
        let settings = Settings::load(&settings_path);
        Self {
            board: Board::new(),
            settings,
            settings_path,
            open_path: String::new(),
            save_path: String::new(),
            add_path: String::new(),
            add_text: "hello".to_string(),
            show_settings: false,
            status: String::new(),
            last_viewport: (1280.0, 800.0),
        }
    }

    fn content_bounds(&self) -> (f32, f32, f32, f32) {
        let mut first = true;
        let (mut l, mut t, mut r, mut b) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for s in self.board.stickers() {
            let pts = s.mapped_points(&s.bound_points());
            for p in pts.chunks_exact(2) {
                if first {
                    (l, t, r, b) = (p[0], p[1], p[0], p[1]);
                    first = false;
                } else {
                    l = l.min(p[0]); t = t.min(p[1]);
                    r = r.max(p[0]); b = b.max(p[1]);
                }
            }
        }
        if first { (0.0, 0.0, 0.0, 0.0) } else { (l, t, r, b) }
    }
}

impl eframe::App for BoardApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Fit").clicked() {
                    // Fit uses the last central-panel size (toolbar has no viewport).
                    let b = self.content_bounds();
                    // ponytail: fixed 24px margin, per-canvas fit lives in Board.
                    self.board.fit(b, self.last_viewport, 24.0);
                }
                if ui.button("Clear").clicked() { self.board.remove_all(); }
                if ui.button("Dup").clicked() { self.board.duplicate_handling(); }
                if ui.button("FlipH").clicked() { self.board.flip_handling(true, false); }
                if ui.button("FlipV").clicked() { self.board.flip_handling(false, true); }
                if ui.button("RotL").clicked() {
                    if let Some(i) = self.board.handling {
                        if let Some(s) = self.board.stickers.get_mut(i) {
                            lazyref_board::icons::apply_op(&mut s.transform, &lazyref_board::icons::IconOp::RotateLeft);
                        }
                    }
                }
                if ui.button("RotR").clicked() {
                    if let Some(i) = self.board.handling {
                        if let Some(s) = self.board.stickers.get_mut(i) {
                            lazyref_board::icons::apply_op(&mut s.transform, &lazyref_board::icons::IconOp::RotateRight);
                        }
                    }
                }
                if ui.button("Del").clicked() { self.board.remove_handling(); }
                let mut lock = self.board.locked;
                if ui.checkbox(&mut lock, "Lock").changed() { self.board.locked = lock; }
                if ui.button("Settings").clicked() { self.show_settings = !self.show_settings; }
            });
            ui.horizontal(|ui| {
                ui.label("Open:");
                ui.text_edit_singleline(&mut self.open_path);
                if ui.button("Open").clicked() {
                    match std::fs::read(&self.open_path) {
                        Ok(bytes) => {
                            let r: Result<Board, String> = match lazyref_board::io::detect(&bytes) {
                                lazyref_board::io::FileFormat::Ref => lazyref_board::io::open_ref(&bytes),
                                lazyref_board::io::FileFormat::Pur => {
                                    let scratch = std::env::temp_dir().join("lazyref-open");
                                    std::fs::create_dir_all(&scratch).ok();
                                    lazyref_board::io::open_pur(&bytes, &scratch)
                                }
                            };
                            match r {
                                Ok(b) => { self.board = b; self.status.clear(); }
                                Err(e) => { self.status = e; }
                            }
                        }
                        Err(e) => { self.status = e.to_string(); }
                    }
                }
                ui.label("Save:");
                ui.text_edit_singleline(&mut self.save_path);
                if ui.button("Save .ref").clicked() {
                    let bytes = lazyref_board::io::save_ref(&self.board);
                    // ponytail: direct write, atomic rename when a path dir exists.
                    self.status = match std::fs::write(&self.save_path, &bytes) {
                        Ok(()) => String::new(),
                        Err(e) => e.to_string(),
                    };
                }
                if ui.button("Save .pur").clicked() {
                    let scratch = std::env::temp_dir().join("lazyref-save");
                    std::fs::create_dir_all(&scratch).ok();
                    let bytes = lazyref_board::io::save_pur(&self.board, &scratch, &[]);
                    self.status = match std::fs::write(&self.save_path, &bytes) {
                        Ok(()) => String::new(),
                        Err(e) => e.to_string(),
                    };
                }
            });
            ui.horizontal(|ui| {
                ui.label("Img:");
                ui.text_edit_singleline(&mut self.add_path);
                if ui.button("Add img").clicked() {
                    match std::fs::read(&self.add_path) {
                        Ok(data) => {
                            let s = Sticker::new(StickerKind::Image { data, width: 0, height: 0 });
                            self.board.add_sticker(s);
                            self.status.clear();
                        }
                        Err(e) => { self.status = e.to_string(); }
                    }
                }
                ui.label("Txt:");
                ui.text_edit_singleline(&mut self.add_text);
                if ui.button("Add txt").clicked() {
                    let mut s = Sticker::new(StickerKind::Text { text: self.add_text.clone(), size_px: 24.0, color: (255,255,255), background: None, bold: false });
                    s.width = 200.0; s.height = 40.0; s.crop = (0.0, 0.0, 200.0, 40.0);
                    self.board.add_sticker(s);
                }
            });
            if !self.status.is_empty() {
                ui.label(&self.status);
            }
        });
        if self.show_settings {
            // borrow fix: .open() holds &mut open across show(); closure writes
            // locals only, applied after the borrow ends.
            let mut open = true;
            let mut saved: Option<Settings> = None;
            let mut save_failed = false;
            egui::Window::new("Settings").open(&mut open).show(ui.ctx(), |ui| {
                let mut s = self.settings.clone();
                ui.checkbox(&mut s.downscale_on_import, "Downscale on import");
                ui.checkbox(&mut s.multistep_scale, "Multi-step downscale");
                for opt in Settings::max_options() {
                    let sel = s.max_resolution == opt;
                    if ui.radio(sel, format!("{} px", opt)).clicked() { s.max_resolution = opt; }
                }
                if ui.button("Save").clicked() {
                    // ponytail: direct write; corrupt/missing loads defaults anyway.
                    if s.save(&self.settings_path).is_ok() { saved = Some(s); }
                    else { save_failed = true; }
                }
            });
            if let Some(s) = saved { self.settings = s; open = false; }
            if save_failed { self.status = "settings save failed".to_string(); }
            self.show_settings = open;
        }
        egui::CentralPanel::default().show(ui, |ui| {
            let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let viewport = resp.rect;
            self.last_viewport = (viewport.width(), viewport.height());
            self.board.set_viewport(viewport.width(), viewport.height());
            // i.time is seconds; Board clock is ms (DOUBLE_TAP_MS = 200.0).
            self.board.set_now_ms(ui.ctx().input(|i| i.time * 1000.0));
            let to_local = |p: Pos2| (p.x - viewport.min.x, p.y - viewport.min.y);
            if resp.drag_started() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let (x, y) = to_local(p);
                    self.board.handle_event(BoardEvent::Down(Pointer { id: 0, x, y }));
                }
            } else if resp.dragged() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let (x, y) = to_local(p);
                    self.board.handle_event(BoardEvent::Move(Pointer { id: 0, x, y }));
                }
            }
            if resp.drag_stopped() {
                if let Some(p) = resp.interact_pointer_pos().or(resp.hover_pos()) {
                    let (x, y) = to_local(p);
                    self.board.handle_event(BoardEvent::Up(Pointer { id: 0, x, y }));
                }
            }
            if resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos().or(resp.hover_pos()) {
                    let (x, y) = to_local(p);
                    self.board.handle_event(BoardEvent::Down(Pointer { id: 0, x, y }));
                    self.board.handle_event(BoardEvent::Up(Pointer { id: 0, x, y }));
                }
            }
            if ui.input(|i| i.key_pressed(Key::F)) {
                let b = self.content_bounds();
                self.board.fit(b, (viewport.width(), viewport.height()), 24.0);
            }
            if ui.input(|i| i.key_pressed(Key::Delete)) {
                self.board.remove_handling();
            }
            painter.rect_filled(viewport, CornerRadius::ZERO, Color32::from_rgb(24, 24, 28));
            draw_grid(&painter, self.board.canvas_matrix(), viewport);
            let handling = self.board.handling;
            for (i, s) in self.board.stickers().iter().enumerate() {
                draw_sticker(&painter, s, Some(i) == handling);
            }
        });
    }
}

fn draw_grid(painter: &egui::Painter, canvas: Mat3, viewport: Rect) {
    let inv = match canvas.invert() { Some(m) => m, None => return };
    let (x0, y0) = inv.map_point(viewport.min.x, viewport.min.y);
    let (x1, y1) = inv.map_point(viewport.max.x, viewport.max.y);
    let (lx, rx) = (x0.min(x1), x0.max(x1));
    let (ty, by) = (y0.min(y1), y0.max(y1));
    let zoom = canvas.0[0].abs().max(1e-6);
    let mut step = 50.0f32;
    while step * zoom < 30.0 { step *= 2.0; }
    while step * zoom > 240.0 { step *= 0.5; }
    while (rx - lx) / step > 200.0 { step *= 2.0; }
    let line = Stroke::new(1.0, Color32::from_gray(52));
    let mut x = (lx / step).floor() * step;
    while x <= rx {
        let (sx, _) = canvas.map_point(x, 0.0);
        painter.line_segment([pos2(sx, viewport.top()), pos2(sx, viewport.bottom())], line);
        x += step;
    }
    let mut y = (ty / step).floor() * step;
    while y <= by {
        let (_, sy) = canvas.map_point(0.0, y);
        painter.line_segment([pos2(viewport.left(), sy), pos2(viewport.right(), sy)], line);
        y += step;
    }
}

fn draw_sticker(painter: &egui::Painter, s: &Sticker, selected: bool) {
    let v = s.mapped_points(&s.bound_points());
    let pts = vec![pos2(v[0], v[1]), pos2(v[2], v[3]), pos2(v[6], v[7]), pos2(v[4], v[5])];
    let (fill, label) = match &s.kind {
        StickerKind::Image { width, height, .. } => (Color32::from_rgb(90, 120, 150), format!("{width}x{height}")),
        StickerKind::Text { text, .. } => (Color32::from_rgb(60, 60, 64), text.clone()),
    };
    let stroke = if selected { Stroke::new(2.0, Color32::WHITE) } else { Stroke::new(1.0, Color32::from_black_alpha(140)) };
    painter.add(Shape::convex_polygon(pts.clone(), fill, stroke));
    if let StickerKind::Text { color, .. } = &s.kind {
        painter.text(pts[0], egui::Align2::LEFT_TOP, label, egui::FontId::default(), Color32::from_rgb(color.0, color.1, color.2));
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("LazyRef"),
        ..Default::default()
    };
    eframe::run_native("LazyRef", options, Box::new(|_cc| Ok(Box::new(BoardApp::new()))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_bounds_covers_board_stickers() {
        let mut app = BoardApp::new();
        app.board.remove_all();
        let mut s = Sticker::new(StickerKind::Text { text: "t".into(), size_px: 12.0, color: (0,0,0), background: None, bold: false });
        s.width = 100.0; s.height = 50.0;
        s.transform.pos = (10.0, 20.0);
        app.board.add_sticker(s);
        assert_eq!(app.content_bounds(), (10.0, 20.0, 110.0, 70.0));
    }

    #[test]
    fn toolbar_delete_uses_remove_handling() {
        let mut app = BoardApp::new();
        let mut s = Sticker::new(StickerKind::Text { text: "x".into(), size_px: 12.0, color: (0,0,0), background: None, bold: false });
        s.width = 20.0; s.height = 10.0;
        app.board.add_sticker(s);
        assert_eq!(app.board.stickers().len(), 1);
        assert_eq!(app.board.remove_handling(), Some(0));
        assert!(app.board.stickers().is_empty());
    }
}
