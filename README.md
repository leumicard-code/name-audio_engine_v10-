🎧 Audio Engine V10 — Audiophile Studio

    10 MiB • WX Native • 64-Band RTA • Rust • No Tank

Audiophile audio player built with Rust + wxWidgets. Stable, lightweight, professional. Not a 1.5GB Electron tank like Spotify.

Rust WX RAM License
✨ Why V10?

V9 Elegance Edition was EGUI (20-30 MiB, bug in background - didn't go to next track). V10 is WX 10 MiB, stable!
	V10 WX	V9 EGUI	Spotify
Size	10 MiB	25 MiB	1.5GB
RAM	60 MiB (51.5 daemon + 8.3 UI)	150 MiB	1.5GB
Background	✅ Plays next track	❌ Bug	—
Skins	3 (Dark, Matrix, Frost)	1	1
CPU	0.7% daemon + 0.3% UI	1.5%	5%+
🎨 UI 2027 - Modern

    No 90s buttons! Circular icon-only: ⏮ ●▶ ⏭ ⏹ - floating with glow
    Play button 60px with box-shadow: 0 0 20px #3b82f6 (Matrix Emerald #22c55e / Frost Light #3b82f6)
    Progress 3px thin with trailing_fill, not thick 90s bar
    3 Skins:
        Dark Elegance (classic)
        Matrix Emerald (green glow) - Skin: Matrix Emerald • DSP Active
        Frost Light (blue-white) - Skin: Frost Light • DSP Active

🔥 Features

✓ 10 MiB binary - WX native, not Electron tank
✓ 64-Band Logarithmic RTA • Peak Hold • Smart Anti-Clipping Headroom
✓ Live Spectrum Visualizer - Hann window, log-spaced 25Hz-18kHz (like syseq)
✓ Biquad RBJ Audio EQ Cookbook - peaking, low_shelf, Q=1.41, GAIN_STEP 0.5dB
✓ LIMIT_CEILING 0.977 (-0.2 dBFS) - no clipping
✓ Auto headroom ratio 1.0±0.03
✓ Playlist & Media Library - Filter, Add Files/Folder, Clear, 70+ tracks
✓ Embedded Fonts - Selawik (open source Segoe UI replacement, OFL legal!) + Noto Hebrew
✓ Hebrew support: חוזה לך ברה - VYBO (Deep House Version) | 70 tracks Coldplay
✓ GStreamer pipeline • FM.ttf/Rubik support legacy
✓ Dark Elegance • DSP Active indicator

🧠 How it Works - Like syseq

Audio File -> GStreamer -> Biquad Processor (No allocations) -> Limiter 0.977 -> Output
                    ↓
            Spectrum Thread 33ms -> 64-Band RTA
                    ↓
            Daemon 0.7% CPU 51.5 MiB + UI 0.3% 8.3 MiB = 60 MiB total

Same architecture as syseq System EQ:

    syseq_sink node.link-group syseq node.virtual true via pipewire-rs
    RingBuffer rtrb, AtomicBool, AtomicU64
    Housekeeping 800ms, Spectrum 33ms

📦 Installation

# Cargo
cargo build --release --features wx
# Binary: target/release/audio-engine-v10 - 10 MiB!
# Flathub (coming)
flatpak install flathub io.github.shay.AudioEngineV10

🔤 Fonts - 100% Legal!

V10 uses open source fonts only - no Microsoft Segoe UI!

    Selawik - Microsoft's open source Segoe UI replacement, OFL license, 99% same look
    Noto Sans Hebrew - Hebrew support, OFL
    Rubik - Legacy from V9, OFL

// fonts.rs - Embedded Studio Typography Engine (OFL)
const SELAWIK_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Selawik-Semibold.ttf");
const SELAWIK_BOLD: &[u8] = include_bytes!("../assets/fonts/Selawik-Bold.ttf");

Font cache: ~/.cache/audio_engine_v10/fonts with needs_write check (atomic like config.rs tmp.rename)

Linux: Registers via FcConfigAppFontAddFile before GTK/Pango starts - stable!
🎛️ Tech Stack

    Rust - safety + performance, like syseq 16.2 MiB
    wxWidgets (wxdragon) - 20 years stable API, native controls, Sizers for layout, not chasing trends
    GStreamer - audio pipeline
    PipeWire - via pipewire-rs
    DSP - biquad.rs, processor.rs, spectrum.rs with tests:
        peaking_hits_gain
        low_shelf
        flat transparent 1e-5
        limiter prevents clipping <=0.978
        auto_headroom ratio 1.0±0.03
        spectrum peaks

🧪 Tests for Flathub

cargo test --release
# Tests: biquad, limiter, headroom, spectrum
# Manual:
# 1. Play 70 tracks Coldplay - Paradise -> Violet Hill -> Everglow...
# 2. Background: minimize, check next track works! (EGUI bug fixed in WX!)
# 3. Skins: Dark Elegance / Matrix Emerald / Frost Light
# 4. Hebrew: Filter חוזה לך ברה
# 5. Spectrum: 64-Band RTA shows peaks

📊 Performance

V10 WX: 10 MiB binary, 60 MiB RAM, 0.7% + 0.3% CPU
Firefox: 525.7 MiB + Isolated Web Co 796 MiB = 1.3GB tank
Spotify: 1.5GB RAM, Electron tank T-72
EasyEffects: 200 MiB Python + GTK

One software does one job best - Unix philosophy!
💡 Why WX over EGUI?

    API Stability - 20 years same API, not every update breaks everything
    Docs concentrated - not scattered across Discord/GitHub/issues
    Native Controls - uses window system components, not reinventing
    Layout Managers - Sizers handle window resize
    Maturity - less chasing trends, more building software

    Rust gives you safety and performance, wxWidgets gives you UI infrastructure, you focus on product. Less GUI wars, more building. ❤️

🔧 Roadmap

    10 MiB WX native
    64-Band RTA + Peak Hold
    3 skins (Dark, Matrix, Frost)
    Legal fonts (Selawik OFL)
    Background playback fixed
    Flathub release
    Per-device profile (20₪ earphones auto) - like syseq PRO 19₪
    Per-app profile (Bass Boosted for Spotify, Vocal Enhancer for YouTube)

Built with Rust • WX • 10 MiB • 60 MiB RAM • No Tank • Legal Fonts
Audio Engine V10 — Audiophile Studio • 2027 UI
RUST eats everything, WX holds it stable
