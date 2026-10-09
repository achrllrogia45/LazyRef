//! App settings (import downscaling) + image fitting. No platform types:
//! the shell loads/saves the file and passes pixel buffers.

use std::path::Path;

/// Import downscale preferences (mirrors the app's BoardSettings keys).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub downscale_on_import: bool,
    pub multistep_scale: bool,
    /// Max total pixels on import (e.g. 2048*2048).
    pub max_resolution: u32,
}

impl Settings {
    /// true, true, 2048².
    pub fn defaults() -> Self {
        Self {
            downscale_on_import: true,
            multistep_scale: true,
            max_resolution: 2048 * 2048,
        }
    }

    /// Missing or corrupt file = defaults.
    pub fn load(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if text.trim().is_empty() {
            return Self::defaults();
        }
        let mut s = Self::defaults();
        let mut ok = false;
        for line in text.lines() {
            let (k, v) = match line.split_once('=') {
                Some(p) => (p.0.trim(), p.1.trim()),
                None => continue,
            };
            match k {
                "downscale_on_import" => {
                    s.downscale_on_import = parse_bool(v).unwrap_or(s.downscale_on_import);
                    ok = true;
                }
                "multistep_scale" => {
                    s.multistep_scale = parse_bool(v).unwrap_or(s.multistep_scale);
                    ok = true;
                }
                "max_resolution" => {
                    if let Ok(n) = v.parse::<u32>() {
                        if n > 0 {
                            s.max_resolution = n;
                            ok = true;
                        }
                    }
                }
                _ => {}
            }
        }
        if ok { s } else { Self::defaults() }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(
            path,
            format!(
                "downscale_on_import={}\nmultistep_scale={}\nmax_resolution={}\n",
                self.downscale_on_import, self.multistep_scale, self.max_resolution
            ),
        )
    }

    /// 1024², 2048², 3072², 4096².
    pub fn max_options() -> [u32; 4] {
        [1024 * 1024, 2048 * 2048, 3072 * 3072, 4096 * 4096]
    }
}

fn parse_bool(v: &str) -> Option<bool> {
    match v {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Max per-step scale when stepping (matches the app's 0.6 quality cap).
const MAX_SCALE_PER_STEP: f64 = 0.6;

/// Downscale decoded RGBA to fit `limit` px total (multi-step halving when
/// `multistep`); returns `(rgba, width, height)`.
pub fn fit_image(rgba: &[u8], w: u32, h: u32, limit: u32, multistep: bool) -> (Vec<u8>, u32, u32) {
    let total = w as u64 * h as u64;
    if total <= limit as u64 || w == 0 || h == 0 || limit == 0 {
        return (rgba.to_vec(), w, h);
    }
    if rgba.len() < w as usize * h as usize * 4 {
        return (rgba.to_vec(), w, h);
    }
    // Target factor matches the app: sqrt(limit / total).
    let mut factor = (limit as f64 / total as f64).sqrt();
    let mut px: Vec<u8> = rgba[..w as usize * h as usize * 4].to_vec();
    let (mut cw, mut ch) = (w, h);
    loop {
        let mut step = factor;
        if multistep && factor < MAX_SCALE_PER_STEP {
            factor /= MAX_SCALE_PER_STEP;
            step = MAX_SCALE_PER_STEP;
        } else {
            // Final step this pass.
            let (nw, nh) = target_dims(cw, ch, step);
            px = scale_box(&px, cw, ch, nw, nh);
            cw = nw;
            ch = nh;
            if multistep && cw as u64 * ch as u64 > limit as u64 && step >= 1.0 {
                // Degenerate guard; cannot happen with step < 1, but keeps the
                // loop total-bounded if rounding ever stalls it.
                break;
            }
            break;
        }
        let (nw, nh) = target_dims(cw, ch, step);
        // Rounding stall guard: box step must shrink, else finish directly.
        if nw >= cw && nh >= ch {
            let (fw, fh) = target_dims(cw, ch, factor);
            px = scale_box(&px, cw, ch, fw, fh);
            cw = fw;
            ch = fh;
            break;
        }
        px = scale_box(&px, cw, ch, nw, nh);
        cw = nw;
        ch = nh;
    }
    (px, cw, ch)
}

fn target_dims(w: u32, h: u32, f: f64) -> (u32, u32) {
    ((w as f64 * f).max(1.0) as u32, (h as f64 * f).max(1.0) as u32)
}

/// Box-average resample of RGBA `src` (w×h) to nw×nh.
fn scale_box(src: &[u8], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u8> {
    let (w, h, nw, nh) = (w as usize, h as usize, nw as usize, nh as usize);
    let mut out = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        let y0 = y * h / nh;
        let y1 = ((y + 1) * h / nh).max(y0 + 1);
        for x in 0..nw {
            let x0 = x * w / nw;
            let x1 = ((x + 1) * w / nw).max(x0 + 1);
            let mut acc = [0u64; 4];
            let mut n = 0u64;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let o = (sy * w + sx) * 4;
                    for c in 0..4 {
                        acc[c] += src[o + c] as u64;
                    }
                    n += 1;
                }
            }
            let o = (y * nw + x) * 4;
            for c in 0..4 {
                out[o + c] = (acc[c] / n) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
        let mut v = Vec::with_capacity(w as usize * h as usize * 4);
        for _ in 0..w as usize * h as usize {
            v.extend_from_slice(&px);
        }
        v
    }

    #[test]
    fn defaults_match_app() {
        let s = Settings::defaults();
        assert!(s.downscale_on_import && s.multistep_scale);
        assert_eq!(s.max_resolution, 2048 * 2048);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = std::env::temp_dir();
        let p = dir.join(format!("lazyref_settings_{}_rt.cfg", std::process::id()));
        let s = Settings {
            downscale_on_import: false,
            multistep_scale: false,
            max_resolution: 1024 * 1024,
        };
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn corrupt_or_missing_is_defaults() {
        assert_eq!(
            Settings::load(Path::new("/nonexistent-lazyref-settings.cfg")),
            Settings::defaults()
        );
        let dir = std::env::temp_dir();
        let p = dir.join(format!("lazyref_settings_{}_bad.cfg", std::process::id()));
        std::fs::write(&p, "garbage {{{{ not kv\n").unwrap();
        assert_eq!(Settings::load(&p), Settings::defaults());
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn max_options_values() {
        assert_eq!(
            Settings::max_options(),
            [1024 * 1024, 2048 * 2048, 3072 * 3072, 4096 * 4096]
        );
    }

    #[test]
    fn fit_noop_when_under_limit() {
        let src = solid(4, 4, [9, 8, 7, 255]);
        let (out, w, h) = fit_image(&src, 4, 4, 1024, true);
        assert_eq!((w, h), (4, 4));
        assert_eq!(out, src);
    }

    #[test]
    fn fit_output_fits_limit() {
        let src = solid(3000, 2000, [200, 100, 50, 255]);
        let limit = 2048 * 2048;
        let (out, w, h) = fit_image(&src, 3000, 2000, limit, true);
        assert!(w as u64 * h as u64 <= limit as u64);
        assert_eq!(out.len(), w as usize * h as usize * 4);
        // Aspect roughly preserved.
        let r0 = 3000f64 / 2000f64;
        let r1 = w as f64 / h as f64;
        assert!((r0 - r1).abs() < 0.05, "{} vs {}", r0, r1);
    }

    #[test]
    fn multistep_matches_single_step_dims() {
        let src = solid(3000, 2000, [1, 2, 3, 255]);
        let limit = 1024 * 1024;
        let (_, w1, h1) = fit_image(&src, 3000, 2000, limit, true);
        let (_, w2, h2) = fit_image(&src, 3000, 2000, limit, false);
        assert_eq!((w1, h1), (w2, h2));
    }
}
