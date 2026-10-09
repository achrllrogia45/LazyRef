//! App settings (import downscaling) + image fitting. No platform types:
//! the shell loads/saves the file and passes pixel buffers.

use std::path::Path;

pub struct Settings {
    pub downscale_on_import: bool,
    pub multistep_scale: bool,
    /// Max total pixels on import (e.g. 2048*2048).
    pub max_resolution: u32,
}

impl Settings {
    /// true, true, 2048².
    pub fn defaults() -> Self {
        unimplemented!()
    }
    /// Missing or corrupt file = defaults.
    pub fn load(path: &Path) -> Self {
        unimplemented!()
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        unimplemented!()
    }
    /// 1024², 2048², 3072², 4096².
    pub fn max_options() -> [u32; 4] {
        unimplemented!()
    }
}

/// Downscale decoded RGBA to fit `limit` px total (multi-step halving when
/// `multistep`); returns `(rgba, width, height)`.
pub fn fit_image(rgba: &[u8], w: u32, h: u32, limit: u32, multistep: bool) -> (Vec<u8>, u32, u32) {
    unimplemented!()
}
