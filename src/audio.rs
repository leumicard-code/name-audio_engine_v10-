// ============================================================================
// audio.rs — GStreamer 10-Band DSP & Real-Time FFT Audio Engine
// ============================================================================

use anyhow::{anyhow, Context, Result};
use gstreamer as gst;
use gstreamer::prelude::*;

/// Official center frequencies (in Hz) of GStreamer's `equalizer-10bands` plugin.
pub const EQ_FREQUENCIES: [u32; 10] = [29, 59, 119, 237, 474, 947, 1889, 3770, 7523, 15011];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Preset {
    Flat,
    WarmVintage,
    ModernClean,
    BassBoosted,
    VocalEnhance,
    ClubMaster,
}

pub const ALL_PRESETS: [Preset; 6] = [
    Preset::Flat,
    Preset::WarmVintage,
    Preset::ModernClean,
    Preset::BassBoosted,
    Preset::VocalEnhance,
    Preset::ClubMaster,
];

impl std::fmt::Display for Preset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Preset::Flat => write!(f, "Flat (Normal)"),
            Preset::WarmVintage => write!(f, "Warm Vintage"),
            Preset::ModernClean => write!(f, "Modern Clean"),
            Preset::BassBoosted => write!(f, "Bass Boosted"),
            Preset::VocalEnhance => write!(f, "Vocal Enhancer"),
            Preset::ClubMaster => write!(f, "Club Master"),
        }
    }
}

impl Preset {
    /// Returns the exact 10-band EQ curve (in dB) for each studio preset.
    pub fn gains(&self) -> [f64; 10] {
        match self {
            Preset::Flat => [0.0; 10],
            Preset::WarmVintage => [2.0, 2.5, 1.5, 0.5, 0.0, 0.0, 0.0, -0.5, -1.0, -1.5],
            Preset::ModernClean => [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.5, 1.0],
            Preset::BassBoosted => [6.8, 5.2, 2.8, 1.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
            Preset::VocalEnhance => [0.0, 0.0, 0.0, -1.0, 1.0, 2.5, 2.0, 1.5, 0.0, 0.0],
            Preset::ClubMaster => [7.65, 5.85, 3.15, 2.0, 1.0, 0.0, 0.0, 2.0, 2.5, 0.0],
        }
    }
}

/// Metadata automatically extracted from the currently playing audio stream.
#[derive(Debug, Clone, Default)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

/// Real-time events emitted by the DSP engine to the GUI loop.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Normalized FFT magnitude bins in `0.0..=1.0`.
    Spectrum(Vec<f32>),
    /// Current track reached end-of-stream so the player can advance.
    EndOfStream,
    /// Decoded ID3 / Vorbis / FLAC tags from the active stream.
    MetadataUpdated(TrackMetadata),
    /// Decoding or playback error details.
    Error(String),
}

pub struct AudioEngine {
    pipeline: gst::Pipeline,
    src: gst::Element,
    pre_gain: gst::Element,
    eq: gst::Element,
    balance: gst::Element,
    volume: gst::Element,
    eq_bands: [f64; 10],
    auto_headroom: bool,
    spectrum_threshold_db: f32,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        gst::init().context("Failed to initialize GStreamer")?;

        let pipeline = gst::Pipeline::new();

        // 1. File source & universal stream decoder
        let src = gst::ElementFactory::make("filesrc")
            .name("filesrc0")
            .build()
            .context("Missing 'filesrc' plugin")?;

        let decodebin = gst::ElementFactory::make("decodebin")
            .name("decoder")
            .build()
            .context("Missing 'decodebin' plugin")?;

        // 2. Sample format conversion & resampling
        let convert = gst::ElementFactory::make("audioconvert")
            .name("convert")
            .build()
            .context("Missing 'audioconvert' plugin")?;

        let resample = gst::ElementFactory::make("audioresample")
            .name("resample")
            .build()
            .context("Missing 'audioresample' plugin")?;

        // 3. Smart Pre-Gain (automatic anti-clipping headroom before EQ boost)
        let pre_gain = gst::ElementFactory::make("volume")
            .name("pre_gain")
            .property("volume", 1.0f64)
            .build()
            .context("Missing 'volume' plugin")?;

        // 4. 10-Band Graphic Studio Equalizer
        let eq = gst::ElementFactory::make("equalizer-10bands")
            .name("eq")
            .build()
            .context("Missing 'equalizer-10bands' plugin (gst-plugins-good)")?;

        // 5. Optional ReplayGain limiter for peak protection (if installed)
        let limiter = gst::ElementFactory::make("rglimiter")
            .name("limiter")
            .build()
            .ok();

        // 6. Stereo Panorama / Balance control
        let balance = gst::ElementFactory::make("audiopanorama")
            .name("balance")
            .property("panorama", 0.0f32)
            .build()
            .context("Missing 'audiopanorama' plugin")?;

        // 7. Real-Time Spectrum Analyzer (placed *before* master volume so visual bars stay independent of speaker volume)
        let spectrum_threshold_db = -60.0f32;
        let spectrum = gst::ElementFactory::make("spectrum")
            .name("spectrum")
            .property("bands", 128u32)
            .property("threshold", spectrum_threshold_db as i32)
            .property("interval", 33_333_333u64) // ~30 FPS updates
            .property("post-messages", true)
            .property("message-magnitude", true)
            .property("message-phase", false)
            .build()
            .context("Missing 'spectrum' plugin")?;

        // 8. Master Volume & Mute
        let volume = gst::ElementFactory::make("volume")
            .name("master_volume")
            .property("volume", 0.8f64)
            .property("mute", false)
            .build()
            .context("Missing 'volume' plugin")?;

        // 9. Output conversion & hardware audio sink
        let convert_out = gst::ElementFactory::make("audioconvert")
            .name("convert_out")
            .build()?;

        let sink = gst::ElementFactory::make("autoaudiosink")
            .name("audio_sink")
            .build()
            .context("Missing 'autoaudiosink' plugin")?;

        // Assemble GStreamer pipeline
        pipeline.add_many([
            &src,
            &decodebin,
            &convert,
            &resample,
            &pre_gain,
            &eq,
            &balance,
            &spectrum,
            &volume,
            &convert_out,
            &sink,
        ])?;

        src.link(&decodebin)?;

        // Link DSP chain (including optional peak limiter when available)
        if let Some(ref lim) = limiter {
            pipeline.add(lim)?;
            gst::Element::link_many([
                &convert,
                &resample,
                &pre_gain,
                &eq,
                lim,
                &balance,
                &spectrum,
                &volume,
                &convert_out,
                &sink,
            ])?;
        } else {
            gst::Element::link_many([
                &convert,
                &resample,
                &pre_gain,
                &eq,
                &balance,
                &spectrum,
                &volume,
                &convert_out,
                &sink,
            ])?;
        }

        // Dynamically link `decodebin` pad once an audio stream is detected
        let convert_weak = convert.downgrade();
        decodebin.connect_pad_added(move |_, src_pad| {
            let Some(convert) = convert_weak.upgrade() else {
                return;
            };

            let caps = src_pad
                .current_caps()
                .unwrap_or_else(|| src_pad.query_caps(None));

            let is_audio = caps
                .structure(0)
                .map(|s| s.name().starts_with("audio/"))
                .unwrap_or(false);

            if !is_audio {
                return;
            }

            if let Some(sink_pad) = convert.static_pad("sink") {
                if !sink_pad.is_linked() {
                    let _ = src_pad.link(&sink_pad);
                }
            }
        });

        Ok(Self {
            pipeline,
            src,
            pre_gain,
            eq,
            balance,
            volume,
            eq_bands: [0.0; 10],
            auto_headroom: true,
            spectrum_threshold_db,
        })
    }

    // ================================================================
    // Equalizer & Smart Headroom Management
    // ================================================================

    /// Enables or disables automatic pre-gain attenuation to prevent digital clipping.
    pub fn set_auto_headroom(&mut self, enabled: bool) {
        self.auto_headroom = enabled;
        self.update_headroom();
    }

    /// Recomputes pre-gain attenuation from the highest boosted EQ band.
    fn update_headroom(&self) {
        if !self.auto_headroom {
            self.pre_gain.set_property("volume", 1.0f64);
            return;
        }

        let max_boost_db = self.eq_bands.iter().copied().fold(0.0_f64, f64::max);

        let pre_gain_linear = if max_boost_db > 0.0 {
            10_f64.powf(-max_boost_db / 20.0)
        } else {
            1.0
        };

        self.pre_gain.set_property("volume", pre_gain_linear);
    }

    pub fn set_eq_band(&mut self, band_index: usize, gain_db: f64) {
        if band_index < 10 {
            let clamped = gain_db.clamp(-12.0, 12.0);
            self.eq_bands[band_index] = clamped;
            let prop_name = format!("band{band_index}");
            self.eq.set_property(&prop_name, clamped);
            self.update_headroom();
        }
    }

    pub fn set_all_eq_bands(&mut self, gains: [f64; 10]) {
        for (i, &gain) in gains.iter().enumerate() {
            let clamped = gain.clamp(-12.0, 12.0);
            self.eq_bands[i] = clamped;
            let prop_name = format!("band{i}");
            self.eq.set_property(&prop_name, clamped);
        }
        self.update_headroom();
    }

    pub fn get_eq_bands(&self) -> &[f64; 10] {
        &self.eq_bands
    }

    pub fn reset_eq(&mut self) {
        self.set_all_eq_bands([0.0; 10]);
    }

    pub fn set_bass_boost(&mut self, amount_db: f64) {
        let amount = amount_db.clamp(0.0, 12.0);
        self.set_eq_band(0, amount * 0.85);
        self.set_eq_band(1, amount * 0.65);
        self.set_eq_band(2, amount * 0.35);
    }

    pub fn set_preset(&mut self, preset: Preset) {
        self.set_all_eq_bands(preset.gains());
    }

    // ================================================================
    // Master Controls (Volume, Mute, Balance)
    // ================================================================

    pub fn set_volume(&self, volume: f64) {
        self.volume.set_property("volume", volume.clamp(0.0, 1.5));
    }

    pub fn get_volume(&self) -> f64 {
        self.volume.property::<f64>("volume")
    }

    pub fn set_mute(&self, muted: bool) {
        self.volume.set_property("mute", muted);
    }

    pub fn is_muted(&self) -> bool {
        self.volume.property::<bool>("mute")
    }

    /// Sets stereo balance (`-1.0` = full left, `0.0` = center, `1.0` = full right).
    pub fn set_balance(&self, pan: f32) {
        self.balance.set_property("panorama", pan.clamp(-1.0, 1.0));
    }

    // ================================================================
    // Transport & Playback Control
    // ================================================================

    pub fn load_file(&self, filepath: &str) -> Result<()> {
        // Resetting to Null guarantees a clean `decodebin` pad state across formats
        self.pipeline
            .set_state(gst::State::Null)
            .context("Failed to reset pipeline state to Null")?;

        self.src.set_property("location", filepath);
        self.play()
    }

    pub fn play(&self) -> Result<()> {
        self.pipeline
            .set_state(gst::State::Playing)
            .map(|_| ())
            .map_err(|err| anyhow!("Failed to start playback: {err:?}"))
    }

    pub fn pause(&self) -> Result<()> {
        self.pipeline
            .set_state(gst::State::Paused)
            .map(|_| ())
            .map_err(|err| anyhow!("Failed to pause playback: {err:?}"))
    }

    pub fn stop(&self) -> Result<()> {
        self.pipeline
            .set_state(gst::State::Null)
            .map(|_| ())
            .map_err(|err| anyhow!("Failed to stop playback: {err:?}"))
    }

    pub fn is_playing(&self) -> bool {
        let (_, current, _) = self.pipeline.state(gst::ClockTime::ZERO);
        current == gst::State::Playing
    }

    /// Returns current playback position in fractional seconds.
    pub fn get_position(&self) -> Option<f64> {
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map(|t| t.nseconds() as f64 / 1_000_000_000.0)
    }

    /// Returns total stream duration in fractional seconds.
    pub fn get_duration(&self) -> Option<f64> {
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|t| t.nseconds() as f64 / 1_000_000_000.0)
    }

    /// Performs an accurate flush seek to `position_sec` seconds.
    pub fn seek_position(&self, position_sec: f64) -> Result<()> {
        if !position_sec.is_finite() || position_sec < 0.0 {
            return Err(anyhow!("Invalid seek position: {position_sec}"));
        }
        let position = gst::ClockTime::from_nseconds((position_sec * 1_000_000_000.0) as u64);
        self.pipeline
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, position)
            .map_err(|err| anyhow!("Seek failed: {err:?}"))
    }

    // ================================================================
    // Event Polling for GUI Integration
    // ================================================================

    /// Drains pending messages from the GStreamer bus (~30 FPS from GUI timer).
    pub fn poll_events(&self) -> Vec<EngineEvent> {
        let mut events = Vec::new();
        let Some(bus) = self.pipeline.bus() else {
            return events;
        };

        let mut latest_spectrum: Option<Vec<f32>> = None;
        let mut metadata = TrackMetadata::default();
        let mut metadata_changed = false;

        while let Some(msg) = bus.pop() {
            match msg.view() {
                gst::MessageView::Eos(..) => {
                    events.push(EngineEvent::EndOfStream);
                }
                gst::MessageView::Error(err) => {
                    let debug_info = err.debug().unwrap_or_default();
                    events.push(EngineEvent::Error(format!(
                        "{} ({})",
                        err.error(),
                        debug_info
                    )));
                }
                gst::MessageView::Tag(tag_msg) => {
                    let tags = tag_msg.tags();
                    if let Some(title) = tags.get::<gst::tags::Title>() {
                        metadata.title = Some(title.get().to_string());
                        metadata_changed = true;
                    }
                    if let Some(artist) = tags.get::<gst::tags::Artist>() {
                        metadata.artist = Some(artist.get().to_string());
                        metadata_changed = true;
                    }
                    if let Some(album) = tags.get::<gst::tags::Album>() {
                        metadata.album = Some(album.get().to_string());
                        metadata_changed = true;
                    }
                }
                gst::MessageView::Element(elem) => {
                    if let Some(structure) = elem.structure() {
                        if structure.name() == "spectrum" {
                            if let Ok(mags) = structure.get::<gst::List>("magnitude") {
                                let threshold = self.spectrum_threshold_db;
                                let freqs: Vec<f32> = mags
                                    .as_slice()
                                    .iter()
                                    .filter_map(|val| val.get::<f32>().ok())
                                    .map(|db| ((db - threshold) / -threshold).clamp(0.0, 1.0))
                                    .collect();
                                latest_spectrum = Some(freqs);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if metadata_changed {
            events.push(EngineEvent::MetadataUpdated(metadata));
        }

        if let Some(spec) = latest_spectrum {
            events.push(EngineEvent::Spectrum(spec));
        }

        events
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}
