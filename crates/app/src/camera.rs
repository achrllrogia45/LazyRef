//! Pure world <-> screen camera: uniform zoom + pan offset.
//!
//! `screen = world * zoom + offset`
//!
//! Same shape as the DroidRef app's `canvasMatrix` (a single uniform scale +
//! translate applied over every sticker's own transform). No rendering or
//! window dependency here on purpose — everything is unit-testable.

use eframe::egui::{Pos2, Rect, Vec2};

pub const MIN_ZOOM: f32 = 0.02;
pub const MAX_ZOOM: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Screen position (px) of the world origin.
    pub offset: Vec2,
    /// Screen pixels per world unit.
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { offset: Vec2::ZERO, zoom: 1.0 }
    }
}

impl Camera {
    pub fn world_to_screen(&self, world: Pos2) -> Pos2 {
        (world.to_vec2() * self.zoom + self.offset).to_pos2()
    }

    pub fn screen_to_world(&self, screen: Pos2) -> Pos2 {
        ((screen.to_vec2() - self.offset) / self.zoom).to_pos2()
    }

    /// Drag the canvas by a screen-space delta.
    pub fn pan(&mut self, screen_delta: Vec2) {
        self.offset += screen_delta;
    }

    /// Zoom by `factor` keeping the world point under `cursor` pinned to `cursor`.
    pub fn zoom_at(&mut self, cursor: Pos2, factor: f32) {
        let world = self.screen_to_world(cursor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.offset = cursor.to_vec2() - world.to_vec2() * self.zoom;
    }

    /// Zoom + center so `world` fits `viewport` with a `margin` fraction (e.g. 0.9).
    /// Degenerate (zero-area) world rects are seeded at `self.zoom` but still centered.
    pub fn fit(&mut self, world: Rect, viewport: Rect, margin: f32) {
        let (w, h) = (world.width(), world.height());
        if w > 0.0 && h > 0.0 {
            self.zoom = (viewport.width() * margin / w)
                .min(viewport.height() * margin / h)
                .clamp(MIN_ZOOM, MAX_ZOOM);
        }
        self.offset = viewport.center().to_vec2() - world.center().to_vec2() * self.zoom;
    }

    /// World rect visible inside `viewport` (grid range / culling).
    pub fn visible_world(&self, viewport: Rect) -> Rect {
        Rect::from_min_max(self.screen_to_world(viewport.min), self.screen_to_world(viewport.max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()))
    }

    fn assert_pos_close(a: Pos2, b: Pos2, tol: f32) {
        assert!(
            close(a.x, b.x, tol) && close(a.y, b.y, tol),
            "expected {a:?} ≈ {b:?} (tol {tol})"
        );
    }

    #[test]
    fn roundtrip_at_various_zooms_and_offsets() {
        let world = pos2(123.0, -456.25);
        for zoom in [MIN_ZOOM, 0.05, 0.5, 1.0, 2.0, 7.5, MAX_ZOOM] {
            for offset in [Vec2::ZERO, Vec2::new(-320.0, 77.0), Vec2::new(1000.5, -0.25)] {
                let cam = Camera { offset, zoom };
                assert_pos_close(cam.screen_to_world(cam.world_to_screen(world)), world, 1e-3);
            }
        }
    }

    #[test]
    fn zoom_at_cursor_pins_the_world_point() {
        let mut cam = Camera { offset: Vec2::new(40.0, -15.0), zoom: 1.0 };
        let cursor = pos2(300.0, 200.0);
        let anchored = cam.screen_to_world(cursor); // world point under cursor before
        for factor in [1.3, 1.3, 0.5, 2.0] {
            cam.zoom_at(cursor, factor);
            assert_pos_close(cam.world_to_screen(anchored), cursor, 1e-3);
        }
    }

    #[test]
    fn zoom_clamps_but_still_pins() {
        let mut cam = Camera::default();
        let cursor = pos2(100.0, 50.0);
        let anchored = cam.screen_to_world(cursor);
        cam.zoom_at(cursor, 1e9);
        assert_eq!(cam.zoom, MAX_ZOOM);
        assert_pos_close(cam.world_to_screen(anchored), cursor, 1e-3);
        cam.zoom_at(cursor, 1e-9);
        assert_eq!(cam.zoom, MIN_ZOOM);
        assert_pos_close(cam.world_to_screen(anchored), cursor, 1e-3);
    }

    #[test]
    fn pan_translates_exactly() {
        let mut cam = Camera::default();
        let before = cam.world_to_screen(pos2(10.0, 20.0));
        let delta = Vec2::new(30.0, -12.0);
        cam.pan(delta);
        assert_pos_close(cam.world_to_screen(pos2(10.0, 20.0)), before + delta, 1e-4);
        assert_pos_close(cam.screen_to_world(before + delta), pos2(10.0, 20.0), 1e-4);
    }

    #[test]
    fn fit_centers_and_contains() {
        let mut cam = Camera::default();
        let viewport = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(800.0, 600.0));
        let world = Rect::from_min_size(pos2(-100.0, 50.0), Vec2::new(400.0, 200.0));
        cam.fit(world, viewport, 0.9);
        assert_pos_close(cam.world_to_screen(world.center()), viewport.center(), 1e-3);
        assert!(viewport.contains(cam.world_to_screen(world.min)));
        assert!(viewport.contains(cam.world_to_screen(world.max)));
    }

    #[test]
    fn fit_handles_degenerate_rect() {
        let mut cam = Camera { offset: Vec2::new(5.0, 5.0), zoom: 3.0 };
        let viewport = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(800.0, 600.0));
        let point_rect = Rect::from_min_size(pos2(10.0, 10.0), Vec2::ZERO);
        cam.fit(point_rect, viewport, 0.9);
        assert_eq!(cam.zoom, 3.0); // zoom untouched, still centered
        assert_pos_close(cam.world_to_screen(point_rect.center()), viewport.center(), 1e-3);
    }

    #[test]
    fn visible_world_matches_viewport_corners() {
        let cam = Camera { offset: Vec2::new(-100.0, 60.0), zoom: 2.5 };
        let viewport = Rect::from_min_size(pos2(20.0, 10.0), Vec2::new(640.0, 480.0));
        let vis = cam.visible_world(viewport);
        assert_pos_close(cam.world_to_screen(vis.min), viewport.min, 1e-3);
        assert_pos_close(cam.world_to_screen(vis.max), viewport.max, 1e-3);
    }
}
