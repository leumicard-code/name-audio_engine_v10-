// ============================================================================
// main.rs — Audio Engine V10 (wxWidgets / GTK3 Edition)
// ============================================================================
//
// Top-to-Bottom Architecture:
//   1. MenuBar (File, Playback, View + Theme & Skins Submenu, Help)
//   2. Audio Controller & Now Playing (Title, Artist, Seek, Circular SVG Icons)
//   3. Live 64-Band Spectrum Visualizer (Log-scaled RTA + Peak Hold + PaintDC)
//   4. Playlist & Media Library (Live Search Filter, SVG Toolbar Icons, ListBox)
//   5. Equalizer & DSP Studio Window (Single-instance 10-Band EQ + Presets)
//   6. Theme Applier, Persistent State Sync & Flat Event Wiring
// ============================================================================

#[allow(dead_code)]
mod audio;
mod config;
mod fonts;
mod icon;
mod theme;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use walkdir::WalkDir;
use wxdragon::appearance::{get_app, AppAppearance, Appearance};
use wxdragon::prelude::*;
use wxdragon::timer::Timer;

use audio::{AudioEngine, EngineEvent, ALL_PRESETS, EQ_FREQUENCIES};
use config::{SavedTrack, UserConfig};
use icon::{build_app_icon, IconButton, IconName, IconRole, StaticIcon};
use theme::{Theme, ThemeMode};

// ============================================================================
// Menu Item IDs
// ============================================================================

const ID_FILE_ADD_FILES: i32 = 1001;
const ID_FILE_ADD_FOLDER: i32 = 1002;
const ID_FILE_CLEAR: i32 = 1003;
const ID_FILE_EXIT: i32 = 1004;

const ID_PLAY_TOGGLE: i32 = 2001;
const ID_PLAY_NEXT: i32 = 2002;
const ID_PLAY_PREV: i32 = 2003;
const ID_PLAY_STOP: i32 = 2004;

const ID_VIEW_EQ: i32 = 1010;
const ID_HELP_ABOUT: i32 = 1020;

const ID_THEME_ELEGANCE: i32 = 1030;
const ID_THEME_CYBERPUNK: i32 = 1031;
const ID_THEME_VINTAGE: i32 = 1032;
const ID_THEME_EMERALD: i32 = 1033;
const ID_THEME_FROST: i32 = 1034;

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus"];
const SPECTRUM_BARS: usize = 64;

// ============================================================================
// Shared Application State
// ============================================================================

#[derive(Clone, Debug)]
struct PlaylistItem {
    path: PathBuf,
    title: String,
    artist: String,
}

struct AppState {
    playlist: Vec<PlaylistItem>,
    /// Indices into `playlist` currently visible in the filtered ListBox
    visible_indices: Vec<usize>,
    filter_query: String,
    current_idx: Option<usize>,
    is_playing: bool,
}

#[derive(Clone)]
struct SpectrumState {
    bars: Vec<f32>,
    peaks: Vec<f32>,
}

impl SpectrumState {
    fn new(count: usize) -> Self {
        Self {
            bars: vec![0.0; count],
            peaks: vec![0.0; count],
        }
    }

    /// Maps linear FFT bins onto `SPECTRUM_BARS` musical bands with smooth ballistics.
    fn update_from_fft(&mut self, fft_bins: &[f32]) {
        if fft_bins.is_empty() {
            return;
        }
        let usable_bins = (fft_bins.len() * 3 / 4).max(1);
        let count = self.bars.len();

        for i in 0..count {
            let t0 = (i as f32 / count as f32).powf(1.65);
            let t1 = ((i + 1) as f32 / count as f32).powf(1.65);

            let b0 = ((t0 * usable_bins as f32) as usize).min(usable_bins - 1);
            let b1 = ((t1 * usable_bins as f32).ceil() as usize).clamp(b0 + 1, usable_bins);

            let mut peak_bin = 0.0f32;
            for &v in &fft_bins[b0..b1] {
                peak_bin = peak_bin.max(v);
            }

            let presence = 1.0 + 0.35 * (i as f32 / count as f32);
            let target = (peak_bin * presence).clamp(0.0, 1.0);

            if target > self.bars[i] {
                self.bars[i] = self.bars[i] * 0.25 + target * 0.75;
            } else {
                self.bars[i] = (self.bars[i] * 0.80).max(0.0);
            }

            if self.bars[i] >= self.peaks[i] {
                self.peaks[i] = self.bars[i];
            } else {
                self.peaks[i] = (self.peaks[i] - 0.018).max(self.bars[i]);
            }
        }
    }

    /// Smoothly decays bars and peaks toward zero when paused or stopped.
    fn decay(&mut self) -> bool {
        let mut active = false;
        for i in 0..self.bars.len() {
            if self.bars[i] > 0.004 || self.peaks[i] > 0.004 {
                active = true;
                self.bars[i] *= 0.82;
                self.peaks[i] = (self.peaks[i] - 0.022).max(self.bars[i]);
            } else {
                self.bars[i] = 0.0;
                self.peaks[i] = 0.0;
            }
        }
        active
    }
}

// ============================================================================
// 1. Menu Bar Builder
// ============================================================================

fn build_menu_bar(initial_theme: ThemeMode) -> MenuBar {
    let file_menu = Menu::builder()
        .append_item(ID_FILE_ADD_FILES, "&Add Audio Files...\tCtrl+O", "Add audio files to playlist")
        .append_item(ID_FILE_ADD_FOLDER, "Add &Folder...", "Recursively add audio files from a folder")
        .append_separator()
        .append_item(ID_FILE_CLEAR, "&Clear Playlist\tCtrl+W", "Clear entire playlist")
        .append_separator()
        .append_item(ID_FILE_EXIT, "E&xit\tCtrl+Q", "Exit Audio Engine V10")
        .build();

    let playback_menu = Menu::builder()
        .append_item(ID_PLAY_TOGGLE, "&Play / Pause\tCtrl+P", "Toggle playback")
        .append_item(ID_PLAY_NEXT, "&Next Track\tCtrl+Right", "Skip to next track")
        .append_item(ID_PLAY_PREV, "Pre&vious Track\tCtrl+Left", "Back to previous track")
        .append_separator()
        .append_item(ID_PLAY_STOP, "&Stop\tCtrl+S", "Stop playback")
        .build();

    let theme_menu = Menu::builder()
        .append_radio_item(ID_THEME_ELEGANCE, "Dark Elegance", "Deep midnight blue studio theme")
        .append_radio_item(ID_THEME_CYBERPUNK, "Cyberpunk Neon", "Obsidian dark with neon cyan & magenta")
        .append_radio_item(ID_THEME_VINTAGE, "Warm Vintage Studio", "Analog walnut & amber gold")
        .append_radio_item(ID_THEME_EMERALD, "Matrix Emerald", "Deep carbon & emerald glow")
        .append_radio_item(ID_THEME_FROST, "Frost Light", "Clean bright daylight theme")
        .build();

    let checked_id = match initial_theme {
        ThemeMode::DarkElegance => ID_THEME_ELEGANCE,
        ThemeMode::CyberpunkNeon => ID_THEME_CYBERPUNK,
        ThemeMode::WarmVintage => ID_THEME_VINTAGE,
        ThemeMode::MatrixEmerald => ID_THEME_EMERALD,
        ThemeMode::FrostLight => ID_THEME_FROST,
    };
    theme_menu.check_item(checked_id, true);

    let view_menu = Menu::builder()
        .append_item(ID_VIEW_EQ, "&Equalizer && DSP Studio...\tCtrl+E", "Open 10-Band EQ Window")
        .append_separator()
        .build();
    view_menu.append_submenu(theme_menu, "&Theme && Skins", "Switch application skin");

    let help_menu = Menu::builder()
        .append_item(ID_HELP_ABOUT, "&About Audio Engine V10", "Show application credits")
        .build();

    MenuBar::builder()
        .append(file_menu, "&File")
        .append(playback_menu, "&Playback")
        .append(view_menu, "&View")
        .append(help_menu, "&Help")
        .build()
}

// ============================================================================
// 2. Controller & Now Playing Section Builder (Circular Studio SVG Controls)
// ============================================================================

#[derive(Clone)]
struct ControllerSection {
    sizer: StaticBoxSizer,
    lbl_title: StaticText,
    lbl_artist: StaticText,
    lbl_time: StaticText,
    ico_vol: StaticIcon,
    lbl_vol: StaticText,
    btn_prev: IconButton,
    btn_play: IconButton,
    btn_next: IconButton,
    btn_stop: IconButton,
    btn_eq: IconButton,
    seek_slider: Slider,
    volume_slider: Slider,
}

fn build_controller_section(panel: &Panel, initial_vol_pct: i32, theme: &Theme) -> ControllerSection {
    let sizer = StaticBoxSizerBuilder::new_with_label(
        Orientation::Vertical,
        panel,
        " Audio Controller ",
    )
    .build();
    if let Some(sb) = sizer.get_static_box() {
        if let Some(f) = fonts::ui_bold(10) {
            sb.set_font(&f);
        }
    }

    let lbl_title = StaticText::builder(panel)
        .with_label("No Track Loaded")
        .build();

    if let Some(font) = fonts::ui_bold(15) {
        lbl_title.set_font(&font);
    }

    let lbl_artist = StaticText::builder(panel)
        .with_label("Add audio files or a music folder to start listening")
        .build();
    if let Some(font) = fonts::ui_semibold(10) {
        lbl_artist.set_font(&font);
    }

    sizer.add(&lbl_title, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&lbl_artist, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 6);

    // Seek Bar Row
    let seek_row = BoxSizer::builder(Orientation::Horizontal).build();
    let lbl_time = StaticText::builder(panel)
        .with_label("0:00 / 0:00")
        .with_size(Size::new(92, -1))
        .build();

    let seek_slider = Slider::builder(panel)
        .with_value(0)
        .with_min_value(0)
        .with_max_value(1000)
        .build();

    seek_row.add(&lbl_time, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 8);
    seek_row.add(&seek_slider, 1, SizerFlag::AlignCenterVertical, 0);
    sizer.add_sizer(&seek_row, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top | SizerFlag::Expand, 6);

    // Circular Studio Transport Controls & Volume Row
    let btn_row = BoxSizer::builder(Orientation::Horizontal).build();

    let btn_prev = IconButton::new(
        panel,
        IconName::Prev,
        IconRole::Standard,
        "Previous Track (Ctrl+Left)",
        theme,
    );
    let btn_play = IconButton::new(
        panel,
        IconName::Play,
        IconRole::Hero,
        "Play / Pause (Ctrl+P)",
        theme,
    );
    let btn_next = IconButton::new(
        panel,
        IconName::Next,
        IconRole::Standard,
        "Next Track (Ctrl+Right)",
        theme,
    );
    let btn_stop = IconButton::new(
        panel,
        IconName::Stop,
        IconRole::Standard,
        "Stop Playback (Ctrl+S)",
        theme,
    );
    let btn_eq = IconButton::new(
        panel,
        IconName::Eq,
        IconRole::Accent,
        "Equalizer & DSP Studio (Ctrl+E)",
        theme,
    );

    btn_row.add(btn_prev.widget(), 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 8);
    btn_row.add(btn_play.widget(), 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 8);
    btn_row.add(btn_next.widget(), 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 8);
    btn_row.add(btn_stop.widget(), 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 18);
    btn_row.add(btn_eq.widget(), 0, SizerFlag::AlignCenterVertical, 0);

    btn_row.add_stretch_spacer(1);

    let ico_vol = StaticIcon::new(panel, IconName::Volume, 20, theme);
    let lbl_vol = StaticText::builder(panel)
        .with_label(&format!("{}%", initial_vol_pct))
        .with_size(Size::new(42, -1))
        .build();
    let volume_slider = Slider::builder(panel)
        .with_value(initial_vol_pct)
        .with_min_value(0)
        .with_max_value(100)
        .with_size(Size::new(145, -1))
        .build();

    btn_row.add(ico_vol.widget(), 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 6);
    btn_row.add(&lbl_vol, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 4);
    btn_row.add(&volume_slider, 0, SizerFlag::AlignCenterVertical, 0);

    sizer.add_sizer(&btn_row, 0, SizerFlag::All | SizerFlag::Expand, 6);

    ControllerSection {
        sizer,
        lbl_title,
        lbl_artist,
        lbl_time,
        ico_vol,
        lbl_vol,
        btn_prev,
        btn_play,
        btn_next,
        btn_stop,
        btn_eq,
        seek_slider,
        volume_slider,
    }
}

// ============================================================================
// 3. Live 64-Band Spectrum Visualizer Builder (Custom Hardware-Buffered Canvas)
// ============================================================================

#[derive(Clone)]
struct SpectrumSection {
    sizer: StaticBoxSizer,
    canvas: Panel,
    lbl_mode: StaticText,
}

fn build_spectrum_section(
    panel: &Panel,
    spectrum_state: Arc<Mutex<SpectrumState>>,
    active_theme: Arc<Mutex<Theme>>,
) -> SpectrumSection {
    let sizer = StaticBoxSizerBuilder::new_with_label(
        Orientation::Vertical,
        panel,
        " Live Spectrum Visualizer ",
    )
    .build();
    if let Some(sb) = sizer.get_static_box() {
        if let Some(f) = fonts::ui_bold(10) {
            sb.set_font(&f);
        }
    }

    let canvas = Panel::builder(panel)
        .with_size(Size::new(-1, 92))
        .build();
    canvas.set_min_size(Size::new(320, 92));
    canvas.set_background_style(BackgroundStyle::Paint);

    canvas.on_erase_background(|_| {});

    let canvas_resize = canvas;
    canvas.on_size(move |evt| {
        canvas_resize.refresh(false, None);
        evt.skip(true);
    });

    let canvas_paint = canvas;
    canvas.on_paint(move |_| {
        let dc = AutoBufferedPaintDC::new(&canvas_paint);
        let size = canvas_paint.get_client_size();
        let w = size.width.max(1);
        let h = size.height.max(1);

        let theme = active_theme.lock().unwrap().clone();
        let snap = spectrum_state.lock().unwrap().clone();

        dc.set_background(theme.card_bg);
        dc.clear();
        dc.set_pen(Colour::new(0, 0, 0, 0), 0, PenStyle::Transparent);

        let count = snap.bars.len().max(1);
        let pad_x = 10;
        let pad_y = 8;
        let inner_w = (w - pad_x * 2).max(count as i32);
        let inner_h = (h - pad_y * 2).max(10);
        let gap = 2;
        let step = inner_w as f32 / count as f32;

        for i in 0..count {
            let x0 = pad_x + (i as f32 * step).round() as i32;
            let x1 = pad_x + ((i + 1) as f32 * step).round() as i32;
            let bar_w = (x1 - x0 - gap).max(2);

            dc.set_brush(theme.spec_slot, BrushStyle::Solid);
            dc.draw_rounded_rectangle(x0, pad_y, bar_w, inner_h, 2.0);

            let val = snap.bars[i].clamp(0.0, 1.0);
            let bar_h = ((val * inner_h as f32).round() as i32).max(3);
            let y = pad_y + inner_h - bar_h;

            dc.set_brush(theme.bar_color(i, count), BrushStyle::Solid);
            dc.draw_rounded_rectangle(x0, y, bar_w, bar_h, 2.0);

            let peak = snap.peaks[i].clamp(0.0, 1.0);
            if peak > 0.06 {
                let peak_y = (pad_y + inner_h - (peak * inner_h as f32).round() as i32 - 2)
                    .clamp(pad_y, pad_y + inner_h - 2);
                dc.set_brush(theme.text_main, BrushStyle::Solid);
                dc.draw_rectangle(x0, peak_y, bar_w, 2);
            }
        }
    });

    let lbl_mode = StaticText::builder(panel)
        .with_label("64-Band Logarithmic RTA • Peak Hold • Smart Anti-Clipping Headroom")
        .build();

    sizer.add(&canvas, 0, SizerFlag::All | SizerFlag::Expand, 6);
    sizer.add(&lbl_mode, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 6);

    SpectrumSection {
        sizer,
        canvas,
        lbl_mode,
    }
}

// ============================================================================
// 4. Playlist & Media Library Section Builder (Vector Toolbar Icons)
// ============================================================================

#[derive(Clone)]
struct PlaylistSection {
    sizer: StaticBoxSizer,
    ico_search: StaticIcon,
    lbl_search: StaticText,
    txt_filter: TextCtrl,
    btn_add_files: IconButton,
    btn_add_folder: IconButton,
    btn_clear: IconButton,
    list_box: ListBox,
}

fn build_playlist_section(panel: &Panel, theme: &Theme) -> PlaylistSection {
    let sizer = StaticBoxSizerBuilder::new_with_label(
        Orientation::Vertical,
        panel,
        " Playlist && Media Library ",
    )
    .build();
    if let Some(sb) = sizer.get_static_box() {
        if let Some(f) = fonts::ui_bold(10) {
            sb.set_font(&f);
        }
    }

    let tool_row = BoxSizer::builder(Orientation::Horizontal).build();

    let ico_search = StaticIcon::new(panel, IconName::Search, 18, theme);
    let lbl_search = StaticText::builder(panel)
        .with_label("Filter:")
        .build();

    let txt_filter = TextCtrl::builder(panel)
        .with_value("")
        .build();

    let btn_add_files = IconButton::new(
        panel,
        IconName::AddFile,
        IconRole::Compact,
        "Add Audio Files (Ctrl+O)",
        theme,
    );
    let btn_add_folder = IconButton::new(
        panel,
        IconName::Folder,
        IconRole::Compact,
        "Add Music Folder",
        theme,
    );
    let btn_clear = IconButton::new(
        panel,
        IconName::Trash,
        IconRole::Compact,
        "Clear Playlist (Ctrl+W)",
        theme,
    );

    tool_row.add(ico_search.widget(), 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 5);
    tool_row.add(&lbl_search, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 6);
    tool_row.add(&txt_filter, 1, SizerFlag::AlignCenterVertical | SizerFlag::Right, 10);
    tool_row.add(btn_add_files.widget(), 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 6);
    tool_row.add(btn_add_folder.widget(), 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 6);
    tool_row.add(btn_clear.widget(), 0, SizerFlag::AlignCenterVertical, 0);

    sizer.add_sizer(&tool_row, 0, SizerFlag::All | SizerFlag::Expand, 6);

    let list_box = ListBox::builder(panel)
        .with_style(ListBoxStyle::Default)
        .build();

    if let Some(font) = fonts::ui_semibold(10) {
        list_box.set_font(&font);
    }

    sizer.add(&list_box, 1, SizerFlag::All | SizerFlag::Expand, 6);

    PlaylistSection {
        sizer,
        ico_search,
        lbl_search,
        txt_filter,
        btn_add_files,
        btn_add_folder,
        btn_clear,
        list_box,
    }
}

// ============================================================================
// 5. Equalizer & DSP Studio Window (Single-Instance + Skin-Aware)
// ============================================================================

fn open_equalizer_dialog(
    eq_window_slot: &Arc<Mutex<Option<Frame>>>,
    engine: Arc<Mutex<AudioEngine>>,
    user_cfg: Arc<Mutex<UserConfig>>,
    theme: Theme,
) {
    // Raise existing EQ Studio window if already open
    if let Ok(mut guard) = eq_window_slot.lock() {
        if let Some(existing) = *guard {
            if existing.is_valid() {
                existing.show(true);
                existing.raise();
                return;
            }
        }
        *guard = None;
    }

    let dialog = Frame::builder()
        .with_title("Equalizer & DSP Studio — Audio Engine V10")
        .with_size(Size::new(740, 500))
        .build();

    if let Ok(mut guard) = eq_window_slot.lock() {
        *guard = Some(dialog);
    }

    if let Some(icon) = build_app_icon() {
        dialog.set_icon(&icon);
    }

    let panel = Panel::builder(&dialog).build();
    panel.set_background_color(theme.panel_bg);
    if let Some(base_font) = fonts::ui_semibold(10) {
        dialog.set_font(&base_font);
        panel.set_font(&base_font);
    }

    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    let title = StaticText::builder(&panel)
        .with_label("10-Band Graphic Studio Equalizer (-12 dB to +12 dB)")
        .build();
    title.set_foreground_color(theme.text_main);
    if let Some(font) = fonts::ui_bold(12) {
        title.set_font(&font);
    }
    sizer.add(&title, 0, SizerFlag::All | SizerFlag::AlignCenterHorizontal, 10);

    let current_gains = engine
        .lock()
        .map(|e| *e.get_eq_bands())
        .unwrap_or([0.0; 10]);

    let bands_row = BoxSizer::builder(Orientation::Horizontal).build();
    let mut band_sliders: Vec<Slider> = Vec::with_capacity(10);
    let mut band_val_labels: Vec<StaticText> = Vec::with_capacity(10);

    for (band_idx, &freq) in EQ_FREQUENCIES.iter().enumerate() {
        let col = BoxSizer::builder(Orientation::Vertical).build();

        let init_db = current_gains[band_idx].round() as i32;
        let lbl_val = StaticText::builder(&panel)
            .with_label(&format!("{:+}", init_db))
            .build();
        lbl_val.set_foreground_color(theme.accent);

        let slider = Slider::builder(&panel)
            .with_style(SliderStyle::Vertical)
            .with_value(-init_db)
            .with_min_value(-12)
            .with_max_value(12)
            .build();

        let freq_text = if freq >= 1000 {
            format!("{:.1}k", freq as f32 / 1000.0)
        } else {
            format!("{}Hz", freq)
        };
        let lbl_freq = StaticText::builder(&panel).with_label(&freq_text).build();
        lbl_freq.set_foreground_color(theme.text_muted);

        let engine_clone = Arc::clone(&engine);
        let cfg_clone = Arc::clone(&user_cfg);
        let slider_copy = slider;
        let lbl_val_copy = lbl_val;
        slider.on_slider(move |_| {
            let gain_db = -(slider_copy.get_value() as f64);
            lbl_val_copy.set_label(&format!("{:+.0}", gain_db));
            if let Ok(mut eng) = engine_clone.lock() {
                eng.set_eq_band(band_idx, gain_db);
                if let Ok(mut cfg) = cfg_clone.lock() {
                    cfg.eq_bands = *eng.get_eq_bands();
                }
            }
        });

        col.add(&lbl_val, 0, SizerFlag::AlignCenterHorizontal | SizerFlag::Bottom, 4);
        col.add(&slider, 1, SizerFlag::Expand, 2);
        col.add(&lbl_freq, 0, SizerFlag::AlignCenterHorizontal | SizerFlag::Top, 4);

        bands_row.add_sizer(&col, 1, SizerFlag::All | SizerFlag::Expand, 4);
        band_sliders.push(slider);
        band_val_labels.push(lbl_val);
    }

    let preset_box = StaticBoxSizerBuilder::new_with_label(
        Orientation::Horizontal,
        &panel,
        " Studio Presets ",
    )
    .build();
    if let Some(sb) = preset_box.get_static_box() {
        sb.set_foreground_color(theme.accent);
        if let Some(f) = fonts::ui_bold(10) {
            sb.set_font(&f);
        }
    }

    for p in ALL_PRESETS {
        let btn = Button::builder(&panel).with_label(&format!("{}", p)).build();
        btn.set_background_color(theme.btn_bg);
        btn.set_foreground_color(theme.text_main);

        let engine_clone = Arc::clone(&engine);
        let cfg_clone = Arc::clone(&user_cfg);
        let sliders_ref = band_sliders.clone();
        let labels_ref = band_val_labels.clone();
        btn.on_click(move |_| {
            let gains = p.gains();
            if let Ok(mut eng) = engine_clone.lock() {
                eng.set_preset(p);
            }
            if let Ok(mut cfg) = cfg_clone.lock() {
                cfg.eq_bands = gains;
                cfg.save();
            }
            for i in 0..10 {
                let db = gains[i].round() as i32;
                sliders_ref[i].set_value(-db);
                labels_ref[i].set_label(&format!("{:+}", db));
            }
        });
        preset_box.add(&btn, 1, SizerFlag::All, 4);
    }

    sizer.add_sizer(&preset_box, 0, SizerFlag::All | SizerFlag::Expand, 8);
    sizer.add_sizer(&bands_row, 1, SizerFlag::All | SizerFlag::Expand, 10);

    let btn_close = Button::builder(&panel).with_label("Close Studio").build();
    btn_close.set_background_color(theme.accent);
    btn_close.set_foreground_color(theme.accent_fg);
    let dialog_copy = dialog;
    btn_close.on_click(move |_| {
        dialog_copy.close(true);
    });
    sizer.add(&btn_close, 0, SizerFlag::All | SizerFlag::AlignCenterHorizontal, 8);

    let cfg_close = Arc::clone(&user_cfg);
    let eq_slot_close = Arc::clone(eq_window_slot);
    dialog.on_close(move |evt| {
        if let Ok(cfg) = cfg_close.lock() {
            cfg.save();
        }
        if let Ok(mut guard) = eq_slot_close.lock() {
            *guard = None;
        }
        evt.skip(true);
    });

    panel.set_sizer(sizer, true);
    dialog.centre();
    dialog.show(true);
}

// ============================================================================
// 6. Theme Applier, Persistence Sync & Playlist Helpers
// ============================================================================

fn apply_theme_to_ui(
    theme: &Theme,
    panel: &Panel,
    ctrl: &ControllerSection,
    spec: &SpectrumSection,
    pl: &PlaylistSection,
    status_bar: &StatusBar,
) {
    panel.set_background_color(theme.panel_bg);

    if let Some(sb) = ctrl.sizer.get_static_box() {
        sb.set_foreground_color(theme.accent);
    }
    if let Some(sb) = spec.sizer.get_static_box() {
        sb.set_foreground_color(theme.accent);
    }
    if let Some(sb) = pl.sizer.get_static_box() {
        sb.set_foreground_color(theme.accent);
    }

    ctrl.lbl_title.set_foreground_color(theme.text_main);
    ctrl.lbl_artist.set_foreground_color(theme.accent);
    ctrl.lbl_time.set_foreground_color(theme.text_muted);
    ctrl.lbl_vol.set_foreground_color(theme.text_main);

    // Re-tint all custom vector SVG icon controls to match the active skin
    ctrl.btn_prev.set_theme(theme);
    ctrl.btn_play.set_theme(theme);
    ctrl.btn_next.set_theme(theme);
    ctrl.btn_stop.set_theme(theme);
    ctrl.btn_eq.set_theme(theme);
    ctrl.ico_vol.set_theme(theme);

    spec.canvas.set_background_color(theme.card_bg);
    spec.lbl_mode.set_foreground_color(theme.text_muted);

    pl.ico_search.set_theme(theme);
    pl.lbl_search.set_foreground_color(theme.text_main);
    pl.txt_filter.set_background_color(theme.card_bg);
    pl.txt_filter.set_foreground_color(theme.text_main);
    pl.btn_add_files.set_theme(theme);
    pl.btn_add_folder.set_theme(theme);
    pl.btn_clear.set_theme(theme);

    pl.list_box.set_background_color(theme.card_bg);
    pl.list_box.set_foreground_color(theme.text_main);

    status_bar.set_status_text(&format!("{} • DSP Active", theme.name), 2);

    spec.canvas.refresh(false, None);
    panel.refresh(true, None);
}

fn truncate_display_title(title: &str) -> String {
    const MAX_CHARS: usize = 68;
    let count = title.chars().count();
    if count <= MAX_CHARS {
        title.to_string()
    } else {
        let mut s: String = title.chars().take(MAX_CHARS - 1).collect();
        s.push('…');
        s
    }
}

fn sync_playlist_to_config(st: &AppState, user_cfg: &Arc<Mutex<UserConfig>>) {
    if let Ok(mut cfg) = user_cfg.lock() {
        cfg.playlist = st
            .playlist
            .iter()
            .map(|item| SavedTrack {
                path: item.path.clone(),
                title: item.title.clone(),
                artist: item.artist.clone(),
            })
            .collect();
        cfg.current_idx = st.current_idx;
        cfg.save();
    }
}

fn rebuild_playlist_view(state: &mut AppState, list_box: &ListBox) {
    list_box.clear();
    state.visible_indices.clear();

    let query = state.filter_query.trim().to_lowercase();
    for (idx, item) in state.playlist.iter().enumerate() {
        if !query.is_empty()
            && !item.title.to_lowercase().contains(&query)
            && !item.artist.to_lowercase().contains(&query)
        {
            continue;
        }
        state.visible_indices.push(idx);
        let prefix = if state.current_idx == Some(idx) { "▶ " } else { "  " };
        let line = if item.artist.is_empty() {
            format!("{}{:02}. {}", prefix, idx + 1, item.title)
        } else {
            format!("{}{:02}. {} — {}", prefix, idx + 1, item.title, item.artist)
        };
        list_box.append(&line);
    }

    if let Some(cur) = state.current_idx {
        if let Some(pos) = state.visible_indices.iter().position(|&i| i == cur) {
            list_box.set_selection(pos as u32, true);
        }
    }
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn add_paths_to_state(
    paths: Vec<PathBuf>,
    state: &Arc<Mutex<AppState>>,
    user_cfg: &Arc<Mutex<UserConfig>>,
    list_box: &ListBox,
    status_bar: &StatusBar,
) -> Option<usize> {
    let mut first_added_idx = None;
    if let Ok(mut st) = state.lock() {
        for path in paths {
            if !is_audio_file(&path) {
                continue;
            }
            if first_added_idx.is_none() {
                first_added_idx = Some(st.playlist.len());
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Track")
                .trim()
                .to_string();
            st.playlist.push(PlaylistItem {
                path,
                title: stem,
                artist: String::new(),
            });
        }
        rebuild_playlist_view(&mut st, list_box);
        status_bar.set_status_text(&format!("{} tracks", st.playlist.len()), 1);
        sync_playlist_to_config(&st, user_cfg);
    }
    first_added_idx
}

fn collect_cli_audio_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for arg in std::env::args().skip(1) {
        let p = PathBuf::from(arg);
        if p.is_file() && is_audio_file(&p) {
            out.push(p);
        } else if p.is_dir() {
            let mut dir_files: Vec<PathBuf> = WalkDir::new(p)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file() && is_audio_file(e.path()))
                .map(|e| e.into_path())
                .collect();
            dir_files.sort();
            out.extend(dir_files);
        }
    }
    out
}

fn format_time(sec: u64) -> String {
    format!("{}:{:02}", sec / 60, sec % 60)
}

// ============================================================================
// Main Application Entry Point
// ============================================================================

fn main() {
    fonts::init_embedded_fonts();
    SystemOptions::set_option_by_int("msw.no-manifest-check", 1);

    let _ = wxdragon::main(|_| {
        let loaded_cfg = UserConfig::load();
        let initial_mode = loaded_cfg.theme;
        let initial_vol = loaded_cfg.volume_pct;
        let initial_eq = loaded_cfg.eq_bands;
        let initial_theme = Theme::get(initial_mode);

        if let Some(app) = get_app() {
            let _ = app.set_appearance(if initial_theme.is_dark() {
                Appearance::Dark
            } else {
                Appearance::Light
            });
        }

        let user_cfg = Arc::new(Mutex::new(loaded_cfg.clone()));
        let active_theme = Arc::new(Mutex::new(initial_theme.clone()));
        let spectrum_state = Arc::new(Mutex::new(SpectrumState::new(SPECTRUM_BARS)));
        let is_seeking = Arc::new(AtomicBool::new(false));
        let eq_window_slot: Arc<Mutex<Option<Frame>>> = Arc::new(Mutex::new(None));

        let frame = Frame::builder()
            .with_title("Audio Engine V10 — Audiophile Studio")
            .with_size(Size::new(920, 750))
            .build();

        if let Some(icon) = build_app_icon() {
            frame.set_icon(&icon);
        }

        let menu_bar = build_menu_bar(initial_mode);
        frame.set_menu_bar(menu_bar);

        let status_bar = frame.create_status_bar(3, 0, 99, "status_bar");
        status_bar.set_status_text("System Ready", 0);
        status_bar.set_status_text("0 tracks", 1);

        let panel = Panel::builder(&frame).build();
        if let Some(base_font) = fonts::ui_semibold(10) {
            frame.set_font(&base_font);
            panel.set_font(&base_font);
            status_bar.set_font(&base_font);
        }
        let main_sizer = BoxSizer::builder(Orientation::Vertical).build();

        let controller_ui = build_controller_section(&panel, initial_vol, &initial_theme);
        let spectrum_ui = build_spectrum_section(
            &panel,
            Arc::clone(&spectrum_state),
            Arc::clone(&active_theme),
        );
        let playlist_ui = build_playlist_section(&panel, &initial_theme);

        main_sizer.add_sizer(&controller_ui.sizer, 0, SizerFlag::All | SizerFlag::Expand, 8);
        main_sizer.add_sizer(&spectrum_ui.sizer, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Expand, 8);
        main_sizer.add_sizer(&playlist_ui.sizer, 1, SizerFlag::All | SizerFlag::Expand, 8);

        panel.set_sizer(main_sizer, true);

        {
            let t = active_theme.lock().unwrap().clone();
            apply_theme_to_ui(&t, &panel, &controller_ui, &spectrum_ui, &playlist_ui, &status_bar);
        }
        panel.layout();

        let mut raw_engine = AudioEngine::new().expect("Failed to initialize GStreamer AudioEngine");
        raw_engine.set_volume(initial_vol as f64 / 100.0);
        raw_engine.set_all_eq_bands(initial_eq);
        let engine = Arc::new(Mutex::new(raw_engine));

        let restored_playlist: Vec<PlaylistItem> = loaded_cfg
            .playlist
            .into_iter()
            .map(|t| PlaylistItem {
                path: t.path,
                title: t.title,
                artist: t.artist,
            })
            .collect();

        let app_state = Arc::new(Mutex::new(AppState {
            playlist: restored_playlist,
            visible_indices: Vec::new(),
            filter_query: String::new(),
            current_idx: loaded_cfg.current_idx,
            is_playing: false,
        }));

        {
            let mut st = app_state.lock().unwrap();
            rebuild_playlist_view(&mut st, &playlist_ui.list_box);
            status_bar.set_status_text(&format!("{} tracks", st.playlist.len()), 1);
            if let Some(idx) = st.current_idx {
                if let Some(item) = st.playlist.get(idx) {
                    controller_ui.lbl_title.set_label(&truncate_display_title(&item.title));
                    if !item.artist.is_empty() {
                        controller_ui.lbl_artist.set_label(&item.artist);
                    } else {
                        controller_ui.lbl_artist.set_label("Ready to play saved track");
                    }
                    status_bar.set_status_text(&format!("Ready: {}", item.title), 0);
                }
            }
        }

        // Shared Actions
        let play_track = {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&app_state);
            let cfg = Arc::clone(&user_cfg);
            let active_theme = Arc::clone(&active_theme);
            let lbl_title = controller_ui.lbl_title;
            let lbl_artist = controller_ui.lbl_artist;
            let btn_play = controller_ui.btn_play.clone();
            let list_box = playlist_ui.list_box;
            let status_bar = status_bar;

            move |idx: usize| {
                let (path_str, title, artist) = {
                    let mut st = state.lock().unwrap();
                    if idx >= st.playlist.len() {
                        return;
                    }
                    st.current_idx = Some(idx);
                    st.is_playing = true;
                    let item = st.playlist[idx].clone();
                    rebuild_playlist_view(&mut st, &list_box);
                    sync_playlist_to_config(&st, &cfg);
                    (item.path.to_string_lossy().to_string(), item.title, item.artist)
                };

                if let Ok(eng) = engine.lock() {
                    let _ = eng.load_file(&path_str);
                    let _ = eng.play();
                }

                lbl_title.set_label(&truncate_display_title(&title));
                if artist.is_empty() {
                    lbl_artist.set_label("Playing Local Studio Stream");
                } else {
                    lbl_artist.set_label(&artist);
                }
                let t = active_theme.lock().unwrap().clone();
                btn_play.set_icon(IconName::Pause, &t);
                status_bar.set_status_text(&format!("Playing: {}", title), 0);
            }
        };

        let cli_paths = collect_cli_audio_paths();
        if !cli_paths.is_empty() {
            if let Some(start_idx) = add_paths_to_state(
                cli_paths,
                &app_state,
                &user_cfg,
                &playlist_ui.list_box,
                &status_bar,
            ) {
                play_track(start_idx);
            }
        }

        let pick_files_action = {
            let state = Arc::clone(&app_state);
            let cfg = Arc::clone(&user_cfg);
            let list_box = playlist_ui.list_box;
            let status_bar = status_bar;
            move || {
                if let Some(files) = rfd::FileDialog::new()
                    .add_filter("Audio Files", AUDIO_EXTENSIONS)
                    .pick_files()
                {
                    add_paths_to_state(files, &state, &cfg, &list_box, &status_bar);
                }
            }
        };

        let pick_folder_action = {
            let state = Arc::clone(&app_state);
            let cfg = Arc::clone(&user_cfg);
            let list_box = playlist_ui.list_box;
            let status_bar = status_bar;
            move || {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let mut files: Vec<PathBuf> = WalkDir::new(folder)
                        .into_iter()
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().is_file() && is_audio_file(e.path()))
                        .map(|e| e.into_path())
                        .collect();
                    files.sort();
                    add_paths_to_state(files, &state, &cfg, &list_box, &status_bar);
                }
            }
        };

        let clear_playlist_action = {
            let state = Arc::clone(&app_state);
            let cfg = Arc::clone(&user_cfg);
            let engine = Arc::clone(&engine);
            let active_theme = Arc::clone(&active_theme);
            let list_box = playlist_ui.list_box;
            let status_bar = status_bar;
            let lbl_title = controller_ui.lbl_title;
            let lbl_artist = controller_ui.lbl_artist;
            let lbl_time = controller_ui.lbl_time;
            let seek_slider = controller_ui.seek_slider;
            let btn_play = controller_ui.btn_play.clone();
            move || {
                if let Ok(mut st) = state.lock() {
                    st.playlist.clear();
                    st.visible_indices.clear();
                    st.current_idx = None;
                    st.is_playing = false;
                    list_box.clear();
                    status_bar.set_status_text("Playlist Cleared", 0);
                    status_bar.set_status_text("0 tracks", 1);
                    lbl_title.set_label("No Track Loaded");
                    lbl_artist.set_label("Add audio files or a music folder to start listening");
                    lbl_time.set_label("0:00 / 0:00");
                    seek_slider.set_value(0);
                    let t = active_theme.lock().unwrap().clone();
                    btn_play.set_icon(IconName::Play, &t);
                    sync_playlist_to_config(&st, &cfg);
                }
                if let Ok(eng) = engine.lock() {
                    let _ = eng.stop();
                }
            }
        };

        let toggle_play_action = {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&app_state);
            let active_theme = Arc::clone(&active_theme);
            let btn_play = controller_ui.btn_play.clone();
            let play_selected = play_track.clone();
            move || {
                let mut st = state.lock().unwrap();
                let t = active_theme.lock().unwrap().clone();
                if let Some(cur_idx) = st.current_idx {
                    if st.is_playing {
                        if let Ok(eng) = engine.lock() {
                            let _ = eng.pause();
                        }
                        st.is_playing = false;
                        btn_play.set_icon(IconName::Play, &t);
                    } else {
                        let resumed = engine
                            .lock()
                            .ok()
                            .map(|eng| {
                                if eng.get_duration().is_some() {
                                    let _ = eng.play();
                                    true
                                } else {
                                    false
                                }
                            })
                            .unwrap_or(false);

                        if resumed {
                            st.is_playing = true;
                            btn_play.set_icon(IconName::Pause, &t);
                        } else {
                            drop(st);
                            play_selected(cur_idx);
                        }
                    }
                } else if !st.playlist.is_empty() {
                    drop(st);
                    play_selected(0);
                }
            }
        };

        // ====================================================================
        // Event Wiring (Circular SVG Icon Buttons, Sliders, Search, MenuBar)
        // ====================================================================

        // Open Equalizer Icon Button
        {
            let eq_slot = Arc::clone(&eq_window_slot);
            let engine = Arc::clone(&engine);
            let cfg = Arc::clone(&user_cfg);
            let active_theme = Arc::clone(&active_theme);
            controller_ui.btn_eq.on_click(move || {
                let t = active_theme.lock().unwrap().clone();
                open_equalizer_dialog(&eq_slot, Arc::clone(&engine), Arc::clone(&cfg), t);
            });
        }

        // Playlist Toolbar Icon Buttons
        {
            let pick = pick_files_action.clone();
            playlist_ui.btn_add_files.on_click(move || pick());
        }
        {
            let pick_dir = pick_folder_action.clone();
            playlist_ui.btn_add_folder.on_click(move || pick_dir());
        }
        {
            let clear = clear_playlist_action.clone();
            playlist_ui.btn_clear.on_click(move || clear());
        }

        // Live Playlist Filter Input
        {
            let state = Arc::clone(&app_state);
            let txt_filter = playlist_ui.txt_filter;
            let list_box = playlist_ui.list_box;
            playlist_ui.txt_filter.on_text_updated(move |_| {
                if let Ok(mut st) = state.lock() {
                    st.filter_query = txt_filter.get_value();
                    rebuild_playlist_view(&mut st, &list_box);
                }
            });
        }

        // Double Click Track in Filtered ListBox
        {
            let state = Arc::clone(&app_state);
            let play = play_track.clone();
            playlist_ui.list_box.on_item_double_clicked(move |evt| {
                if let Some(sel) = evt.get_selection() {
                    if sel >= 0 {
                        let real_idx = state
                            .lock()
                            .ok()
                            .and_then(|st| st.visible_indices.get(sel as usize).copied());
                        if let Some(idx) = real_idx {
                            play(idx);
                        }
                    }
                }
            });
        }

        // Circular Transport Icon Buttons
        {
            let toggle = toggle_play_action.clone();
            controller_ui.btn_play.on_click(move || toggle());
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&app_state);
            let active_theme = Arc::clone(&active_theme);
            let btn_play = controller_ui.btn_play.clone();
            let seek_slider = controller_ui.seek_slider;
            let lbl_time = controller_ui.lbl_time;
            controller_ui.btn_stop.on_click(move || {
                if let Ok(mut st) = state.lock() {
                    st.is_playing = false;
                }
                let t = active_theme.lock().unwrap().clone();
                btn_play.set_icon(IconName::Play, &t);
                seek_slider.set_value(0);
                lbl_time.set_label("0:00 / 0:00");
                if let Ok(eng) = engine.lock() {
                    let _ = eng.stop();
                }
            });
        }
        {
            let state = Arc::clone(&app_state);
            let play = play_track.clone();
            controller_ui.btn_next.on_click(move || {
                let next_idx = {
                    let st = state.lock().unwrap();
                    if st.playlist.is_empty() {
                        None
                    } else {
                        st.current_idx.map(|i| (i + 1) % st.playlist.len())
                    }
                };
                if let Some(n) = next_idx {
                    play(n);
                }
            });
        }
        {
            let state = Arc::clone(&app_state);
            let play = play_track.clone();
            controller_ui.btn_prev.on_click(move || {
                let prev_idx = {
                    let st = state.lock().unwrap();
                    if st.playlist.is_empty() {
                        None
                    } else {
                        st.current_idx.map(|i| if i > 0 { i - 1 } else { st.playlist.len() - 1 })
                    }
                };
                if let Some(p) = prev_idx {
                    play(p);
                }
            });
        }

        // Volume & Seek Sliders
        {
            let engine = Arc::clone(&engine);
            let cfg = Arc::clone(&user_cfg);
            let vol_slider = controller_ui.volume_slider;
            let lbl_vol = controller_ui.lbl_vol;
            controller_ui.volume_slider.on_slider(move |_| {
                let pct = vol_slider.get_value();
                lbl_vol.set_label(&format!("{}%", pct));
                if let Ok(eng) = engine.lock() {
                    eng.set_volume(pct as f64 / 100.0);
                }
                if let Ok(mut c) = cfg.lock() {
                    c.volume_pct = pct;
                }
            });
        }
        {
            let seeking_down = Arc::clone(&is_seeking);
            controller_ui.seek_slider.on_mouse_left_down(move |evt| {
                seeking_down.store(true, Ordering::Relaxed);
                evt.skip(true);
            });

            let seeking_up = Arc::clone(&is_seeking);
            controller_ui.seek_slider.on_mouse_left_up(move |evt| {
                seeking_up.store(false, Ordering::Relaxed);
                evt.skip(true);
            });

            let engine = Arc::clone(&engine);
            let seek_slider = controller_ui.seek_slider;
            let lbl_time = controller_ui.lbl_time;
            controller_ui.seek_slider.on_slider(move |_| {
                let val = seek_slider.get_value() as f64;
                if let Ok(eng) = engine.lock() {
                    if let Some(dur) = eng.get_duration() {
                        let target_sec = (val / 1000.0) * dur;
                        lbl_time.set_label(&format!(
                            "{} / {}",
                            format_time(target_sec as u64),
                            format_time(dur as u64)
                        ));
                        let _ = eng.seek_position(target_sec);
                    }
                }
            });
        }

        // MenuBar Event Handler (File, Playback, View, Themes, Help)
        {
            let frame_m = frame;
            let panel_m = panel;
            let eq_slot_m = Arc::clone(&eq_window_slot);
            let engine_m = Arc::clone(&engine);
            let state_m = Arc::clone(&app_state);
            let cfg_m = Arc::clone(&user_cfg);
            let active_theme_m = Arc::clone(&active_theme);
            let status_bar_m = status_bar;
            let pick_files = pick_files_action;
            let pick_folder = pick_folder_action;
            let clear_pl = clear_playlist_action;
            let toggle_play = toggle_play_action;
            let play_m = play_track.clone();
            let ctrl_handles = controller_ui.clone();
            let spec_handles = spectrum_ui.clone();
            let pl_handles = playlist_ui.clone();

            frame.on_menu_selected(move |evt| {
                match evt.get_id() {
                    ID_FILE_ADD_FILES => pick_files(),
                    ID_FILE_ADD_FOLDER => pick_folder(),
                    ID_FILE_CLEAR => clear_pl(),
                    ID_FILE_EXIT => {
                        frame_m.close(true);
                    }
                    ID_PLAY_TOGGLE => toggle_play(),
                    ID_PLAY_NEXT => {
                        let next = state_m
                            .lock()
                            .ok()
                            .and_then(|st| {
                                (!st.playlist.is_empty())
                                    .then(|| st.current_idx.map(|i| (i + 1) % st.playlist.len()))
                                    .flatten()
                            });
                        if let Some(n) = next {
                            play_m(n);
                        }
                    }
                    ID_PLAY_PREV => {
                        let prev = state_m
                            .lock()
                            .ok()
                            .and_then(|st| {
                                (!st.playlist.is_empty())
                                    .then(|| {
                                        st.current_idx.map(|i| {
                                            if i > 0 {
                                                i - 1
                                            } else {
                                                st.playlist.len() - 1
                                            }
                                        })
                                    })
                                    .flatten()
                            });
                        if let Some(p) = prev {
                            play_m(p);
                        }
                    }
                    ID_PLAY_STOP => {
                        if let Ok(mut st) = state_m.lock() {
                            st.is_playing = false;
                        }
                        let t = active_theme_m.lock().unwrap().clone();
                        ctrl_handles.btn_play.set_icon(IconName::Play, &t);
                        ctrl_handles.seek_slider.set_value(0);
                        ctrl_handles.lbl_time.set_label("0:00 / 0:00");
                        if let Ok(eng) = engine_m.lock() {
                            let _ = eng.stop();
                        }
                    }
                    ID_VIEW_EQ => {
                        let t = active_theme_m.lock().unwrap().clone();
                        open_equalizer_dialog(&eq_slot_m, Arc::clone(&engine_m), Arc::clone(&cfg_m), t);
                    }
                    ID_HELP_ABOUT => {
                        let dlg = MessageDialog::builder(
                            &frame_m,
                            "Audio Engine V10 — Audiophile DSP Studio\n\n\
                             Shay Kadosh © 2000-2026\n\
                             System Architect & Lead Developer\n\n\
                             • 10-Band GStreamer Graphic Equalizer\n\
                             • 64-Band Logarithmic RTA Spectrum + Peak Hold\n\
                             • Smart Anti-Clipping Auto-Headroom\n\
                             • Full Playlist, EQ & Skin Persistence\n\
                             • Native wxWidgets / GTK3 Vector Studio UI",
                            "About Audio Engine V10",
                        )
                        .with_style(
                            MessageDialogStyle::OK
                                | MessageDialogStyle::IconInformation
                                | MessageDialogStyle::Centre,
                        )
                        .build();
                        dlg.show_modal();
                    }
                    id @ (ID_THEME_ELEGANCE
                    | ID_THEME_CYBERPUNK
                    | ID_THEME_VINTAGE
                    | ID_THEME_EMERALD
                    | ID_THEME_FROST) => {
                        let mode = match id {
                            ID_THEME_CYBERPUNK => ThemeMode::CyberpunkNeon,
                            ID_THEME_VINTAGE => ThemeMode::WarmVintage,
                            ID_THEME_EMERALD => ThemeMode::MatrixEmerald,
                            ID_THEME_FROST => ThemeMode::FrostLight,
                            _ => ThemeMode::DarkElegance,
                        };
                        let new_theme = Theme::get(mode);
                        if let Some(app) = get_app() {
                            let _ = app.set_appearance(if new_theme.is_dark() {
                                Appearance::Dark
                            } else {
                                Appearance::Light
                            });
                        }
                        *active_theme_m.lock().unwrap() = new_theme.clone();
                        if let Ok(mut c) = cfg_m.lock() {
                            c.theme = mode;
                            c.save();
                        }
                        apply_theme_to_ui(
                            &new_theme,
                            &panel_m,
                            &ctrl_handles,
                            &spec_handles,
                            &pl_handles,
                            &status_bar_m,
                        );
                    }
                    _ => {}
                }
            });
        }

        // ====================================================================
        // High-Precision UI Timer (~30 FPS Spectrum + Metadata + Seek Sync)
        // ====================================================================
        let timer = Timer::new(&frame);
        let engine_timer = Arc::clone(&engine);
        let state_timer = Arc::clone(&app_state);
        let cfg_timer = Arc::clone(&user_cfg);
        let active_theme_timer = Arc::clone(&active_theme);
        let spectrum_timer = Arc::clone(&spectrum_state);
        let seeking_timer = Arc::clone(&is_seeking);
        let canvas_timer = spectrum_ui.canvas;
        let seek_slider = controller_ui.seek_slider;
        let lbl_time = controller_ui.lbl_time;
        let lbl_title = controller_ui.lbl_title;
        let lbl_artist = controller_ui.lbl_artist;
        let btn_play_timer = controller_ui.btn_play.clone();
        let list_box_timer = playlist_ui.list_box;
        let status_bar_timer = status_bar;
        let play_next_eos = play_track;
        let mut last_sec_shown: Option<(u64, u64)> = None;

        timer.on_tick(move |_| {
            let mut next_to_play: Option<usize> = None;
            let mut reached_end = false;
            let mut got_fft = false;

            if let Ok(eng) = engine_timer.lock() {
                for evt in eng.poll_events() {
                    match evt {
                        EngineEvent::Spectrum(fft_bins) => {
                            if let Ok(mut spec) = spectrum_timer.lock() {
                                spec.update_from_fft(&fft_bins);
                            }
                            got_fft = true;
                            canvas_timer.refresh(false, None);
                        }
                        EngineEvent::MetadataUpdated(meta) => {
                            if let Ok(mut st) = state_timer.lock() {
                                if let Some(cur) = st.current_idx {
                                    if let Some(title) = meta.title {
                                        if !title.trim().is_empty() {
                                            st.playlist[cur].title = title.clone();
                                            lbl_title.set_label(&truncate_display_title(&title));
                                            status_bar_timer
                                                .set_status_text(&format!("Playing: {}", title), 0);
                                        }
                                    }
                                    if let Some(artist) = meta.artist {
                                        if !artist.trim().is_empty() {
                                            st.playlist[cur].artist = artist.clone();
                                            lbl_artist.set_label(&artist);
                                        }
                                    }
                                    rebuild_playlist_view(&mut st, &list_box_timer);
                                    sync_playlist_to_config(&st, &cfg_timer);
                                }
                            }
                        }
                        EngineEvent::EndOfStream => {
                            if let Ok(mut st) = state_timer.lock() {
                                next_to_play = st
                                    .current_idx
                                    .and_then(|i| (i + 1 < st.playlist.len()).then_some(i + 1));
                                if next_to_play.is_none() {
                                    st.is_playing = false;
                                    reached_end = true;
                                }
                            }
                        }
                        EngineEvent::Error(err_msg) => {
                            status_bar_timer.set_status_text(&format!("Audio Error: {}", err_msg), 0);
                        }
                    }
                }

                if reached_end {
                    let t = active_theme_timer.lock().unwrap().clone();
                    btn_play_timer.set_icon(IconName::Play, &t);
                }

                if !got_fft {
                    let is_playing = state_timer.lock().map(|s| s.is_playing).unwrap_or(false);
                    if !is_playing {
                        if let Ok(mut spec) = spectrum_timer.lock() {
                            if spec.decay() {
                                canvas_timer.refresh(false, None);
                            }
                        }
                    }
                }

                if !seeking_timer.load(Ordering::Relaxed) {
                    if let (Some(pos), Some(dur)) = (eng.get_position(), eng.get_duration()) {
                        if dur > 0.0 {
                            let pct = ((pos / dur) * 1000.0).round() as i32;
                            if (seek_slider.get_value() - pct).abs() >= 2 {
                                seek_slider.set_value(pct);
                            }
                            let pair = (pos as u64, dur as u64);
                            if last_sec_shown != Some(pair) {
                                last_sec_shown = Some(pair);
                                lbl_time.set_label(&format!(
                                    "{} / {}",
                                    format_time(pair.0),
                                    format_time(pair.1)
                                ));
                            }
                        }
                    }
                }
            }

            if let Some(n) = next_to_play {
                play_next_eos(n);
            }
        });
        timer.start(33, false);

        let cfg_on_close = Arc::clone(&user_cfg);
        frame.on_close(move |evt| {
            if let Ok(cfg) = cfg_on_close.lock() {
                cfg.save();
            }
            timer.stop();
            evt.skip(true);
        });

        frame.centre();
        frame.show(true);
    });
}
