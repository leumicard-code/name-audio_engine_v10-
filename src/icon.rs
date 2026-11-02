// ============================================================================
// icon.rs — Studio Vector Icon Engine (Loads SVGs from `assets/icons/`)
// ============================================================================

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use wxdragon::prelude::*;

use crate::theme::Theme;

// ============================================================================
// 1. Application Window Icon (64x64 RGBA)
// ============================================================================

/// Generates a crisp 64x64 anti-aliased RGBA application icon matching the SVG logo.
pub fn build_app_icon() -> Option<Bitmap> {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut rgba = vec![0u8; (W * H * 4) as usize];

    let cx = 31.5f32;
    let cy = 31.5f32;
    let half = 29.0f32;
    let radius = 13.0f32;

    for y in 0..H {
        for x in 0..W {
            let dx = (x as f32 - cx).abs() - (half - radius);
            let dy = (y as f32 - cy).abs() - (half - radius);
            let dist = (dx.max(0.0).hypot(dy.max(0.0)) + dx.min(0.0).max(dy.min(0.0))) - radius;

            if dist < 1.0 {
                let alpha = (0.5 - dist).clamp(0.0, 1.0);
                let border_mix = (1.0 - (-dist - 0.8).clamp(0.0, 1.8) / 1.8).clamp(0.0, 1.0);
                let grad = x as f32 / (W - 1) as f32;

                let br = 35.0 + grad * (180.0 - 35.0);
                let bg = 185.0 + grad * (75.0 - 185.0);
                let bb = 248.0 + grad * (235.0 - 248.0);

                let r = (18.0 * (1.0 - border_mix * 0.85) + br * (border_mix * 0.85)) as u8;
                let g = (24.0 * (1.0 - border_mix * 0.85) + bg * (border_mix * 0.85)) as u8;
                let b = (38.0 * (1.0 - border_mix * 0.85) + bb * (border_mix * 0.85)) as u8;
                let a = (alpha * 255.0) as u8;

                let idx = ((y * W + x) * 4) as usize;
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = a;
            }
        }
    }

    let bar_heights: [u32; 5] = [20, 34, 26, 38, 22];
    let bar_w: u32 = 6;
    let gap: u32 = 3;
    let start_x: u32 = 11;
    let bottom_y: u32 = 50;

    for (i, &bh) in bar_heights.iter().enumerate() {
        let t = i as f32 / 4.0;
        let r = (35.0 + t * (190.0 - 35.0)) as u8;
        let g = (190.0 + t * (75.0 - 190.0)) as u8;
        let b = (248.0 + t * (235.0 - 248.0)) as u8;

        let bx0 = start_x + i as u32 * (bar_w + gap);
        let by0 = bottom_y - bh;

        for y in by0..bottom_y {
            for x in bx0..(bx0 + bar_w) {
                let idx = ((y * W + x) * 4) as usize;
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }

        let cap_y = by0.saturating_sub(4);
        for y in cap_y..(cap_y + 2) {
            for x in bx0..(bx0 + bar_w) {
                let idx = ((y * W + x) * 4) as usize;
                rgba[idx] = 241;
                rgba[idx + 1] = 245;
                rgba[idx + 2] = 249;
                rgba[idx + 3] = 255;
            }
        }
    }

    Bitmap::from_rgba(&rgba, W, H)
}

// ============================================================================
// 2. SVG Asset Loader (`assets/icons/*.svg` + Embedded Fallback)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconName {
    Prev,
    Play,
    Pause,
    Next,
    Stop,
    Eq,
    Volume,
    Search,
    AddFile,
    Folder,
    Trash,
}

impl IconName {
    fn filename(self) -> &'static str {
        match self {
            Self::Prev => "prev.svg",
            Self::Play => "play.svg",
            Self::Pause => "pause.svg",
            Self::Next => "next.svg",
            Self::Stop => "stop.svg",
            Self::Eq => "eq.svg",
            Self::Volume => "volume.svg",
            Self::Search => "search.svg",
            Self::AddFile => "add_file.svg",
            Self::Folder => "folder.svg",
            Self::Trash => "trash.svg",
        }
    }

    fn fallback_svg(self) -> &'static str {
        match self {
            Self::Prev => include_str!("../assets/icons/prev.svg"),
            Self::Play => include_str!("../assets/icons/play.svg"),
            Self::Pause => include_str!("../assets/icons/pause.svg"),
            Self::Next => include_str!("../assets/icons/next.svg"),
            Self::Stop => include_str!("../assets/icons/stop.svg"),
            Self::Eq => include_str!("../assets/icons/eq.svg"),
            Self::Volume => include_str!("../assets/icons/volume.svg"),
            Self::Search => include_str!("../assets/icons/search.svg"),
            Self::AddFile => include_str!("../assets/icons/add_file.svg"),
            Self::Folder => include_str!("../assets/icons/folder.svg"),
            Self::Trash => include_str!("../assets/icons/trash.svg"),
        }
    }

    /// Loads the SVG markup from `assets/icons/` on disk first, falling back to embedded assets.
    fn load_svg(self) -> String {
        let fname = self.filename();
        let mut candidates = vec![
            PathBuf::from("assets").join("icons").join(fname),
            PathBuf::from("/app/share/audio_engine_v10/icons").join(fname),
        ];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("assets").join("icons").join(fname));
                if let Some(parent) = dir.parent() {
                    candidates.push(parent.join("assets").join("icons").join(fname));
                }
            }
        }
        for path in candidates {
            if let Ok(content) = fs::read_to_string(&path) {
                if content.contains("<svg") {
                    return content;
                }
            }
        }
        self.fallback_svg().to_string()
    }
}

fn hex_color(c: Colour) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

fn extract_svg_inner(raw_svg: &str) -> &str {
    let start = raw_svg.find('>').map(|i| i + 1).unwrap_or(0);
    let end = raw_svg.rfind("</svg>").unwrap_or(raw_svg.len());
    if start <= end {
        &raw_svg[start..end]
    } else {
        raw_svg
    }
}

// ============================================================================
// 3. Frameless Circular Studio Icon Control (`IconButton`)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconRole {
    /// 40x40 circular transport control (Prev, Next, Stop)
    Standard,
    /// 50x50 glowing primary hero circle (Play / Pause)
    Hero,
    /// 42x42 accented studio control (EQ Studio)
    Accent,
    /// 36x36 compact circular toolbar icon (Add Files, Add Folder, Clear)
    Compact,
}

impl IconRole {
    pub fn pixel_size(self) -> i32 {
        match self {
            Self::Standard => 40,
            Self::Hero => 50,
            Self::Accent => 42,
            Self::Compact => 36,
        }
    }

    fn icon_draw_px(self) -> f32 {
        match self {
            Self::Standard => 18.0,
            Self::Hero => 23.0,
            Self::Accent => 20.0,
            Self::Compact => 17.0,
        }
    }
}

struct IconBtnInner {
    icon: IconName,
    role: IconRole,
    hovered: bool,
    pressed: bool,
    panel_bg: Colour,
    bmp_normal: Option<Bitmap>,
    bmp_hover: Option<Bitmap>,
    bmp_pressed: Option<Bitmap>,
}

impl IconBtnInner {
    fn rebuild_bitmaps(&mut self, theme: &Theme) {
        self.panel_bg = theme.panel_bg;
        let sz = self.role.pixel_size();
        let icon_px = self.role.icon_draw_px();
        let raw = self.icon.load_svg();
        let inner = extract_svg_inner(&raw);

        let bg_canvas = hex_color(theme.panel_bg);
        let card_hex = hex_color(theme.card_bg);
        let btn_hex = hex_color(theme.btn_bg);
        let slot_hex = hex_color(theme.spec_slot);
        let accent_hex = hex_color(theme.accent);
        let accent_fg_hex = hex_color(theme.accent_fg);
        let text_hex = hex_color(theme.text_main);

        match self.role {
            IconRole::Hero => {
                self.bmp_normal = rasterize_circle_icon(
                    sz,
                    icon_px,
                    &bg_canvas,
                    &accent_hex,
                    &accent_hex,
                    0.28,
                    3.0,
                    &accent_fg_hex,
                    inner,
                );
                self.bmp_hover = rasterize_circle_icon(
                    sz,
                    icon_px + 1.0,
                    &bg_canvas,
                    &accent_hex,
                    &text_hex,
                    0.75,
                    2.4,
                    &accent_fg_hex,
                    inner,
                );
                self.bmp_pressed = rasterize_circle_icon(
                    sz,
                    icon_px - 1.0,
                    &bg_canvas,
                    &btn_hex,
                    &accent_hex,
                    0.95,
                    2.2,
                    &accent_hex,
                    inner,
                );
            }
            IconRole::Accent => {
                self.bmp_normal = rasterize_circle_icon(
                    sz,
                    icon_px,
                    &bg_canvas,
                    &btn_hex,
                    &accent_hex,
                    0.65,
                    1.6,
                    &accent_hex,
                    inner,
                );
                self.bmp_hover = rasterize_circle_icon(
                    sz,
                    icon_px,
                    &bg_canvas,
                    &accent_hex,
                    &accent_hex,
                    0.95,
                    2.0,
                    &accent_fg_hex,
                    inner,
                );
                self.bmp_pressed = rasterize_circle_icon(
                    sz,
                    icon_px - 1.0,
                    &bg_canvas,
                    &card_hex,
                    &accent_hex,
                    1.0,
                    2.0,
                    &accent_hex,
                    inner,
                );
            }
            IconRole::Standard | IconRole::Compact => {
                self.bmp_normal = rasterize_circle_icon(
                    sz,
                    icon_px,
                    &bg_canvas,
                    &card_hex,
                    &slot_hex,
                    0.90,
                    1.4,
                    &text_hex,
                    inner,
                );
                self.bmp_hover = rasterize_circle_icon(
                    sz,
                    icon_px,
                    &bg_canvas,
                    &btn_hex,
                    &accent_hex,
                    0.95,
                    1.8,
                    &accent_hex,
                    inner,
                );
                self.bmp_pressed = rasterize_circle_icon(
                    sz,
                    icon_px - 1.0,
                    &bg_canvas,
                    &accent_hex,
                    &accent_hex,
                    1.0,
                    1.8,
                    &accent_fg_hex,
                    inner,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rasterize_circle_icon(
    size: i32,
    icon_px: f32,
    canvas_bg: &str,
    circle_fill: &str,
    ring_stroke: &str,
    ring_opacity: f32,
    ring_width: f32,
    fg_hex: &str,
    inner_icon: &str,
) -> Option<Bitmap> {
    let center = size as f32 / 2.0;
    let radius = center - 2.6;
    let offset = (size as f32 - icon_px) / 2.0;
    let scale = icon_px / 24.0;
    let tinted = inner_icon.replace("#FFFFFF", fg_hex);

    let composed = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}" width="{size}" height="{size}">
  <rect x="0" y="0" width="{size}" height="{size}" fill="{canvas_bg}"/>
  <circle cx="{center:.2}" cy="{center:.2}" r="{radius:.2}" fill="{circle_fill}" stroke="{ring_stroke}" stroke-width="{ring_width:.2}" stroke-opacity="{ring_opacity:.2}"/>
  <g fill="none" transform="translate({offset:.2},{offset:.2}) scale({scale:.4})">
    {tinted}
  </g>
</svg>"#
    );

    BitmapBundle::from_svg_text(&composed, Size::new(size, size))
        .and_then(|bundle| bundle.get_bitmap(Size::new(size, size)))
}

#[derive(Clone)]
pub struct IconButton {
    panel: Panel,
    inner: Arc<Mutex<IconBtnInner>>,
}

impl IconButton {
    pub fn new(parent: &Panel, icon: IconName, role: IconRole, tooltip: &str, theme: &Theme) -> Self {
        let sz = role.pixel_size();
        let panel = Panel::builder(parent)
            .with_size(Size::new(sz, sz))
            .build();
        panel.set_min_size(Size::new(sz, sz));
        panel.set_background_style(BackgroundStyle::Paint);
        panel.set_tooltip(tooltip);

        if let Some(hand) = Cursor::from_stock(StockCursor::Hand) {
            panel.set_cursor(Some(&hand));
        }

        let mut init_inner = IconBtnInner {
            icon,
            role,
            hovered: false,
            pressed: false,
            panel_bg: theme.panel_bg,
            bmp_normal: None,
            bmp_hover: None,
            bmp_pressed: None,
        };
        init_inner.rebuild_bitmaps(theme);

        let inner = Arc::new(Mutex::new(init_inner));

        panel.on_erase_background(|_| {});

        // Custom anti-aliased paint handler
        {
            let panel_paint = panel;
            let state_paint = Arc::clone(&inner);
            panel.on_paint(move |_| {
                let dc = AutoBufferedPaintDC::new(&panel_paint);
                if let Ok(st) = state_paint.lock() {
                    dc.set_background(st.panel_bg);
                    dc.clear();
                    let bmp = if st.pressed {
                        st.bmp_pressed.as_ref().or(st.bmp_hover.as_ref())
                    } else if st.hovered {
                        st.bmp_hover.as_ref().or(st.bmp_normal.as_ref())
                    } else {
                        st.bmp_normal.as_ref()
                    };
                    if let Some(b) = bmp {
                        dc.draw_bitmap(b, 0, 0, false);
                    }
                }
            });
        }

        // Hover & press visual feedback
        {
            let p = panel;
            let st = Arc::clone(&inner);
            panel.on_mouse_enter(move |evt| {
                if let Ok(mut s) = st.lock() {
                    s.hovered = true;
                }
                p.refresh(false, None);
                evt.skip(true);
            });
        }
        {
            let p = panel;
            let st = Arc::clone(&inner);
            panel.on_mouse_leave(move |evt| {
                if let Ok(mut s) = st.lock() {
                    s.hovered = false;
                    s.pressed = false;
                }
                p.refresh(false, None);
                evt.skip(true);
            });
        }

        Self { panel, inner }
    }

    pub fn widget(&self) -> &Panel {
        &self.panel
    }

    pub fn set_icon(&self, icon: IconName, theme: &Theme) {
        if let Ok(mut st) = self.inner.lock() {
            if st.icon != icon {
                st.icon = icon;
                st.rebuild_bitmaps(theme);
            }
        }
        self.panel.refresh(false, None);
    }

    pub fn set_theme(&self, theme: &Theme) {
        if let Ok(mut st) = self.inner.lock() {
            st.rebuild_bitmaps(theme);
        }
        self.panel.refresh(false, None);
    }

    pub fn on_click<F>(&self, mut callback: F)
    where
        F: FnMut() + 'static,
    {
        let p_down = self.panel;
        let st_down = Arc::clone(&self.inner);
        self.panel.on_mouse_left_down(move |_| {
            if let Ok(mut s) = st_down.lock() {
                s.pressed = true;
            }
            p_down.refresh(false, None);
        });

        let p_up = self.panel;
        let st_up = Arc::clone(&self.inner);
        self.panel.on_mouse_left_up(move |_| {
            let was_pressed = if let Ok(mut s) = st_up.lock() {
                let p = s.pressed;
                s.pressed = false;
                p
            } else {
                false
            };
            p_up.refresh(false, None);
            if was_pressed {
                callback();
            }
        });
    }
}

// ============================================================================
// 4. Frameless Static Vector Icon (`StaticIcon` for Volume & Search Indicators)
// ============================================================================

struct StaticIconInner {
    icon: IconName,
    size: i32,
    panel_bg: Colour,
    bmp: Option<Bitmap>,
}

impl StaticIconInner {
    fn rebuild(&mut self, theme: &Theme) {
        self.panel_bg = theme.panel_bg;
        let sz = self.size;
        let bg_hex = hex_color(theme.panel_bg);
        let fg_hex = hex_color(theme.accent);
        let raw = self.icon.load_svg();
        let inner = extract_svg_inner(&raw).replace("#FFFFFF", &fg_hex);
        let scale = sz as f32 / 24.0;

        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {sz} {sz}" width="{sz}" height="{sz}">
  <rect x="0" y="0" width="{sz}" height="{sz}" fill="{bg_hex}"/>
  <g fill="none" transform="scale({scale:.4})">{inner}</g>
</svg>"#
        );

        self.bmp = BitmapBundle::from_svg_text(&svg, Size::new(sz, sz))
            .and_then(|b| b.get_bitmap(Size::new(sz, sz)));
    }
}

#[derive(Clone)]
pub struct StaticIcon {
    panel: Panel,
    inner: Arc<Mutex<StaticIconInner>>,
}

impl StaticIcon {
    pub fn new(parent: &Panel, icon: IconName, size: i32, theme: &Theme) -> Self {
        let panel = Panel::builder(parent)
            .with_size(Size::new(size, size))
            .build();
        panel.set_min_size(Size::new(size, size));
        panel.set_background_style(BackgroundStyle::Paint);
        panel.on_erase_background(|_| {});

        let mut st = StaticIconInner {
            icon,
            size,
            panel_bg: theme.panel_bg,
            bmp: None,
        };
        st.rebuild(theme);
        let inner = Arc::new(Mutex::new(st));

        let p_paint = panel;
        let st_paint = Arc::clone(&inner);
        panel.on_paint(move |_| {
            let dc = AutoBufferedPaintDC::new(&p_paint);
            if let Ok(s) = st_paint.lock() {
                dc.set_background(s.panel_bg);
                dc.clear();
                if let Some(ref b) = s.bmp {
                    dc.draw_bitmap(b, 0, 0, false);
                }
            }
        });

        Self { panel, inner }
    }

    pub fn widget(&self) -> &Panel {
        &self.panel
    }

    pub fn set_theme(&self, theme: &Theme) {
        if let Ok(mut s) = self.inner.lock() {
            s.rebuild(theme);
        }
        self.panel.refresh(false, None);
    }
}
