// ============================================================================
// theme.rs — Skin & Color Palette Engine for Audio Engine V10
// ============================================================================

use serde::{Deserialize, Serialize};
use wxdragon::Colour;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    #[default]
    DarkElegance,
    CyberpunkNeon,
    WarmVintage,
    MatrixEmerald,
    FrostLight,
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub mode: ThemeMode,
    pub name: &'static str,
    pub panel_bg: Colour,
    pub card_bg: Colour,
    pub btn_bg: Colour,
    pub accent: Colour,
    pub accent_fg: Colour,
    pub text_main: Colour,
    pub text_muted: Colour,
    /// Low-frequency bar color in the 64-band spectrum analyzer
    pub spec_low: (u8, u8, u8),
    /// High-frequency bar color in the 64-band spectrum analyzer
    pub spec_high: (u8, u8, u8),
    /// Subtle background slot color behind each spectrum bar
    pub spec_slot: Colour,
}

impl Theme {
    pub fn get(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::DarkElegance => Self {
                mode,
                name: "Dark Elegance",
                panel_bg: Colour::new(18, 24, 38, 255),
                card_bg: Colour::new(26, 35, 52, 255),
                btn_bg: Colour::new(36, 48, 70, 255),
                accent: Colour::new(56, 189, 248, 255),
                accent_fg: Colour::new(12, 18, 30, 255),
                text_main: Colour::new(241, 245, 249, 255),
                text_muted: Colour::new(148, 163, 184, 255),
                spec_low: (35, 165, 245),
                spec_high: (180, 75, 235),
                spec_slot: Colour::new(38, 50, 72, 255),
            },
            ThemeMode::CyberpunkNeon => Self {
                mode,
                name: "Cyberpunk Neon",
                panel_bg: Colour::new(11, 11, 20, 255),
                card_bg: Colour::new(20, 18, 36, 255),
                btn_bg: Colour::new(32, 26, 56, 255),
                accent: Colour::new(0, 240, 255, 255),
                accent_fg: Colour::new(10, 10, 18, 255),
                text_main: Colour::new(250, 250, 255, 255),
                text_muted: Colour::new(217, 70, 239, 255),
                spec_low: (0, 245, 255),
                spec_high: (255, 20, 180),
                spec_slot: Colour::new(34, 28, 58, 255),
            },
            ThemeMode::WarmVintage => Self {
                mode,
                name: "Warm Vintage Studio",
                panel_bg: Colour::new(28, 22, 18, 255),
                card_bg: Colour::new(40, 31, 25, 255),
                btn_bg: Colour::new(58, 45, 36, 255),
                accent: Colour::new(245, 158, 11, 255),
                accent_fg: Colour::new(24, 16, 10, 255),
                text_main: Colour::new(254, 243, 199, 255),
                text_muted: Colour::new(196, 160, 126, 255),
                spec_low: (245, 158, 11),
                spec_high: (239, 68, 68),
                spec_slot: Colour::new(58, 45, 36, 255),
            },
            ThemeMode::MatrixEmerald => Self {
                mode,
                name: "Matrix Emerald",
                panel_bg: Colour::new(10, 18, 14, 255),
                card_bg: Colour::new(16, 29, 22, 255),
                btn_bg: Colour::new(24, 44, 34, 255),
                accent: Colour::new(16, 185, 129, 255),
                accent_fg: Colour::new(8, 16, 12, 255),
                text_main: Colour::new(220, 252, 231, 255),
                text_muted: Colour::new(110, 231, 183, 255),
                spec_low: (16, 185, 129),
                spec_high: (163, 230, 53),
                spec_slot: Colour::new(24, 44, 34, 255),
            },
            ThemeMode::FrostLight => Self {
                mode,
                name: "Frost Light",
                panel_bg: Colour::new(241, 245, 249, 255),
                card_bg: Colour::new(255, 255, 255, 255),
                btn_bg: Colour::new(226, 232, 240, 255),
                accent: Colour::new(37, 99, 235, 255),
                accent_fg: Colour::new(255, 255, 255, 255),
                text_main: Colour::new(15, 23, 42, 255),
                text_muted: Colour::new(100, 116, 139, 255),
                spec_low: (37, 99, 235),
                spec_high: (124, 58, 237),
                spec_slot: Colour::new(226, 232, 240, 255),
            },
        }
    }

    pub fn is_dark(&self) -> bool {
        self.mode != ThemeMode::FrostLight
    }

    /// Interpolates the spectrum bar color for bar `index` out of `total`.
    pub fn bar_color(&self, index: usize, total: usize) -> Colour {
        let t = index as f32 / total.max(1) as f32;
        let r = (self.spec_low.0 as f32 + t * (self.spec_high.0 as f32 - self.spec_low.0 as f32)) as u8;
        let g = (self.spec_low.1 as f32 + t * (self.spec_high.1 as f32 - self.spec_low.1 as f32)) as u8;
        let b = (self.spec_low.2 as f32 + t * (self.spec_high.2 as f32 - self.spec_low.2 as f32)) as u8;
        Colour::new(r, g, b, 255)
    }
}
