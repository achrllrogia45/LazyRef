//! LazyRef board prototype: infinite grid + placeholder stickers on a
//! pan/zoom canvas. This binary is the desktop dev harness; the Android app
//! will build on the same core.
//!
//! World space is the board. Each sticker is rendered with the same pipeline
//! as the DroidRef app's `StickerTransform`: translate(position) then rotate
//! about the sticker's center (scale folded into w/h for now).

use lazyref::camera::Camera;
use lazyref::egui::emath::Rot2;
use lazyref::egui::{self, pos2, Color32, CornerRadius, Key, Pos2, Rect, Sense, Shape, Stroke, Vec2};

/// (x, y) is the sticker's center in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PlaceholderSticker {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    angle_deg: f32,
    color: Color32,
}

impl PlaceholderSticker {
    fn center(&self) -> Pos2 {
        pos2(self.x, self.y)
    }

    /// World-space corners: local rect -> rotate about center -> translate.
    fn corners(&self) -> [Pos2; 4] {
        let rot = Rot2::from_angle(self.angle_deg.to_radians());
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]
            .map(|(x, y)| self.center() + rot * Vec2::new(x, y))
    }

    /// Inverse transform: is this world point inside the (rotated) sticker?
    fn contains(&self, world: Pos2) -> bool {
        let rot = Rot2::from_angle(-self.angle_deg.to_radians());
        let local = rot * (world - self.center());
        local.x.abs() <= self.w / 2.0 && local.y.abs() <= self.h / 2.0
    }
}

struct BoardApp {
    camera: Camera,
    stickers: Vec<PlaceholderSticker>,
    fit_pending: bool,
}

impl BoardApp {
    fn new() -> Self {
        let rgb = Color32::from_rgb;
        Self {
            camera: Camera::default(),
            stickers: vec![
                PlaceholderSticker { x: 0.0, y: 0.0, w: 320.0, h: 200.0, angle_deg: 0.0, color: rgb(216, 92, 82) },
                PlaceholderSticker { x: 420.0, y: -60.0, w: 240.0, h: 300.0, angle_deg: 25.0, color: rgb(88, 156, 216) },
                PlaceholderSticker { x: -380.0, y: 140.0, w: 280.0, h: 280.0, angle_deg: -40.0, color: rgb(118, 196, 118) },
                PlaceholderSticker { x: 60.0, y: 380.0, w: 360.0, h: 160.0, angle_deg: 10.0, color: rgb(228, 188, 88) },
            ],
            fit_pending: true,
        }
    }

    fn content_bounds(&self) -> Rect {
        let mut r = Rect::NOTHING;
        for s in &self.stickers {
            for c in s.corners() {
                r.extend_with(c);
            }
        }
        r
    }
}

impl eframe::App for BoardApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let viewport = resp.rect;

        // First frame: frame the whole board (no-op unless resized to zero).
        if self.fit_pending && viewport.is_positive() {
            self.camera.fit(self.content_bounds(), viewport, 0.9);
            self.fit_pending = false;
        }

        // Pan: drag anywhere on the canvas.
        self.camera.pan(resp.drag_delta());

        // Zoom: scroll wheel, pinned to the cursor.
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            if let Some(cursor) = resp.hover_pos() {
                self.camera.zoom_at(cursor, (scroll * 0.0015).exp());
            }
        }
        if ui.input(|i| i.key_pressed(Key::F)) {
            self.camera.fit(self.content_bounds(), viewport, 0.9);
        }

        painter.rect_filled(viewport, CornerRadius::ZERO, Color32::from_rgb(24, 24, 28));
        draw_grid(&painter, &self.camera, viewport);

        let hover_world = resp.hover_pos().map(|p| self.camera.screen_to_world(p));
        for s in &self.stickers {
            let hovered = hover_world.is_some_and(|w| s.contains(w));
            draw_sticker(&painter, &self.camera, s, hovered);
        }
    }
}

/// "Infinite" grid: world-space steps of a power-of-two multiple of 50, kept
/// roughly 30..240 px apart, so line count stays bounded at any zoom.
fn draw_grid(painter: &egui::Painter, cam: &Camera, viewport: Rect) {
    let vis = cam.visible_world(viewport);
    let mut step = 50.0_f32;
    while step * cam.zoom < 30.0 {
        step *= 2.0;
    }
    while step * cam.zoom > 240.0 {
        step *= 0.5;
    }
    while vis.width() / step > 200.0 {
        step *= 2.0;
    }

    let line = Stroke::new(1.0, Color32::from_gray(52));
    let axis = Stroke::new(1.5, Color32::from_gray(92));

    let mut x = (vis.min.x / step).floor() * step;
    while x <= vis.max.x {
        let sx = cam.world_to_screen(pos2(x, 0.0)).x;
        let stroke = if x == 0.0 { axis } else { line };
        painter.line_segment([pos2(sx, viewport.top()), pos2(sx, viewport.bottom())], stroke);
        x += step;
    }
    let mut y = (vis.min.y / step).floor() * step;
    while y <= vis.max.y {
        let sy = cam.world_to_screen(pos2(0.0, y)).y;
        let stroke = if y == 0.0 { axis } else { line };
        painter.line_segment([pos2(viewport.left(), sy), pos2(viewport.right(), sy)], stroke);
        y += step;
    }
}

fn draw_sticker(painter: &egui::Painter, cam: &Camera, s: &PlaceholderSticker, hovered: bool) {
    let pts: Vec<Pos2> = s.corners().iter().map(|w| cam.world_to_screen(*w)).collect();
    let stroke = if hovered {
        Stroke::new(2.0, Color32::WHITE)
    } else {
        Stroke::new(1.0, Color32::from_black_alpha(140))
    };
    painter.add(Shape::convex_polygon(pts, s.color, stroke));
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

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()))
    }

    #[test]
    fn corners_match_translate_then_rotate_about_center() {
        let s = PlaceholderSticker { x: 10.0, y: 20.0, w: 100.0, h: 50.0, angle_deg: 90.0, color: Color32::RED };
        let c = s.corners();
        // 90° rotation about the center: local (x, y) -> (-y, x)
        let expect = [
            (10.0 + 25.0, 20.0 - 50.0),
            (10.0 + 25.0, 20.0 + 50.0),
            (10.0 - 25.0, 20.0 + 50.0),
            (10.0 - 25.0, 20.0 - 50.0),
        ];
        for (p, (ex, ey)) in c.iter().zip(expect) {
            assert!(close(p.x, ex, 1e-5) && close(p.y, ey, 1e-5), "{p:?} != ({ex},{ey})");
        }
    }

    #[test]
    fn contains_is_the_inverse_of_corners() {
        let s = PlaceholderSticker { x: -40.0, y: 15.0, w: 200.0, h: 80.0, angle_deg: 30.0, color: Color32::RED };
        assert!(s.contains(s.center()));
        // A point just inside the rotated top edge, and its mirror just outside.
        let rot = Rot2::from_angle(30.0_f32.to_radians());
        let inside = s.center() + rot * Vec2::new(99.0, -39.0);
        let outside = s.center() + rot * Vec2::new(101.0, -39.0);
        assert!(s.contains(inside));
        assert!(!s.contains(outside));
    }
}
