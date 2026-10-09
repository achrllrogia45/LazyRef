//! Integration: drive the camera the way the app's event loop does and check
//! the invariants hold end to end (fit -> pan -> repeated zoom-at-cursor).

use lazyref::camera::Camera;
use lazyref::egui::{pos2, Pos2, Rect, Vec2};

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-3 * (1.0 + a.abs().max(b.abs()))
}

fn assert_pos_close(a: Pos2, b: Pos2) {
    assert!(close(a.x, b.x) && close(a.y, b.y), "expected {a:?} ≈ {b:?}");
}

const VIEWPORT: Vec2 = Vec2::new(1280.0, 800.0); // const Vec2::new is const fn in emath

#[test]
fn session_pan_zoom_keeps_anchors_and_roundtrips() {
    let viewport = Rect::from_min_size(pos2(0.0, 0.0), VIEWPORT);
    let board = Rect::from_min_size(pos2(-500.0, -300.0), Vec2::new(1000.0, 600.0));

    let mut cam = Camera::default();
    cam.fit(board, viewport, 0.8); // start: whole board visible

    // User pans, then zooms twice with the wheel over a board corner.
    cam.pan(Vec2::new(-40.0, 25.0));
    let corner_screen = cam.world_to_screen(board.min);
    cam.zoom_at(corner_screen, 1.25);
    cam.zoom_at(corner_screen, 1.25);

    // The world point under the cursor is still under the cursor.
    assert_pos_close(cam.world_to_screen(board.min), corner_screen);

    // The board was bigger than the viewport*0.8 after the fit... zoomed in.
    assert!(cam.zoom > 1.0);

    // Arbitrary world points survive screen->world->screen.
    for p in [pos2(0.0, 0.0), pos2(999.9, -300.0), pos2(-123.4, 567.8)] {
        assert_pos_close(cam.screen_to_world(cam.world_to_screen(p)), p);
    }
}

#[test]
fn fit_keeps_whole_board_visible_and_centered() {
    let viewport = Rect::from_min_size(pos2(130.0, 70.0), VIEWPORT);
    let board = Rect::from_min_size(pos2(-2000.0, 300.0), Vec2::new(320.0, 2400.0));
    let mut cam = Camera::default();
    cam.fit(board, viewport, 0.9);

    assert_pos_close(cam.world_to_screen(board.center()), viewport.center());
    for corner in [board.min, board.right_top(), board.left_bottom(), board.max] {
        assert!(viewport.contains(cam.world_to_screen(corner)), "{corner:?} outside viewport");
    }
    // Visible world rect covers the whole board.
    let vis = cam.visible_world(viewport);
    assert!(vis.contains(board.min) && vis.contains(board.max));
}
