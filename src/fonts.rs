// ============================================================================
// fonts.rs — Embedded Studio Typography Engine (Segoe UI Semibold & Bold)
// ============================================================================

use std::ffi::{c_char, c_int, c_void, CString};
use std::fs;
use std::path::PathBuf;

use wxdragon::prelude::*;

pub const UI_FONT_SEMIBOLD: &str = "Segoe UI Semibold";
pub const UI_FONT_BASE: &str = "Segoe UI";

const SEGOE_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/SegoeUI-Semibold.ttf");
const SEGOE_BOLD: &[u8] = include_bytes!("../assets/fonts/SegoeUI-Bold.ttf");

#[cfg(target_os = "linux")]
#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcConfigGetCurrent() -> *mut c_void;
    fn FcInitLoadConfigAndFonts() -> *mut c_void;
    fn FcConfigSetCurrent(config: *mut c_void) -> c_int;
    fn FcConfigAppFontAddFile(config: *mut c_void, file: *const c_char) -> c_int;
}

fn font_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .or_else(dirs::config_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("audio_engine_v10")
        .join("fonts")
}

#[cfg(target_os = "linux")]
fn register_font_file_linux(path_str: &str) {
    if let Ok(c_path) = CString::new(path_str) {
        unsafe {
            let mut cfg = FcConfigGetCurrent();
            if cfg.is_null() {
                cfg = FcInitLoadConfigAndFonts();
                if !cfg.is_null() {
                    let _ = FcConfigSetCurrent(cfg);
                }
            }
            if !cfg.is_null() {
                let _ = FcConfigAppFontAddFile(cfg, c_path.as_ptr());
            }
        }
    }
}

/// Extracts bundled fonts to cache and registers them in Fontconfig before GTK/Pango starts.
pub fn init_embedded_fonts() {
    let dir = font_cache_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    let files = [
        ("SegoeUI-Semibold.ttf", SEGOE_SEMIBOLD),
        ("SegoeUI-Bold.ttf", SEGOE_BOLD),
    ];

    for (name, bytes) in files {
        let dest = dir.join(name);
        let needs_write = fs::metadata(&dest)
            .map(|m| m.len() != bytes.len() as u64)
            .unwrap_or(true);

        if needs_write {
            let _ = fs::write(&dest, bytes);
        }

        if let Some(path_str) = dest.to_str() {
            #[cfg(target_os = "linux")]
            register_font_file_linux(path_str);

            #[cfg(not(target_os = "linux"))]
            let _ = Font::add_private_font(path_str);
        }
    }
}

/// Crisp Segoe UI Semibold font for general UI controls, buttons, and playlist rows.
pub fn ui_semibold(point_size: i32) -> Option<Font> {
    Font::builder()
        .with_face_name(UI_FONT_SEMIBOLD)
        .with_family(FontFamily::Swiss)
        .with_point_size(point_size)
        .with_weight(FontWeight::SemiBold)
        .build()
}

/// Heavy Segoe UI Bold font for main track titles, section headers, and hero buttons.
pub fn ui_bold(point_size: i32) -> Option<Font> {
    Font::builder()
        .with_face_name(UI_FONT_BASE)
        .with_family(FontFamily::Swiss)
        .with_point_size(point_size)
        .with_weight(FontWeight::Bold)
        .build()
}
