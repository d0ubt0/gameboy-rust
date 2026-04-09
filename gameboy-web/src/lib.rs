use wasm_bindgen::prelude::*;

use gameboy_core::emulator::GameBoy;
use gameboy_core::joypad::Button;
use gameboy_core::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// Web Audio API sample rate (standard)
const WEB_AUDIO_SAMPLE_RATE: u32 = 44100;

/// Wrapper around the Game Boy emulator for the web frontend.
///
/// Exposes the emulator through wasm-bindgen so JavaScript can:
///   - Load ROMs (as byte arrays)
///   - Run one frame of emulation
///   - Read the RGBA frame-buffer directly from WASM linear memory
///   - Read audio samples directly from WASM linear memory
///   - Send key-down/key-up events
#[wasm_bindgen]
pub struct GameBoyWeb {
    gameboy: GameBoy,
    rom_loaded: bool,
    /// Temporary buffer to hold audio samples extracted from the APU ring buffer.
    /// This lives here so JS can read it from stable memory.
    audio_staging: Vec<f32>,
}

#[wasm_bindgen]
impl GameBoyWeb {
    /// Create a new emulator instance.
    /// `sample_rate` is the Web Audio API's sample rate (usually 44100 or 48000).
    #[wasm_bindgen(constructor)]
    pub fn new(sample_rate: Option<u32>) -> Self {
        // Initialise wasm-logger so log::info!(), etc. go to the browser console
        wasm_logger::init(wasm_logger::Config::default());

        let rate = sample_rate.unwrap_or(WEB_AUDIO_SAMPLE_RATE);
        log::info!("GameBoyWeb: creating emulator with sample rate {}", rate);

        Self {
            gameboy: GameBoy::new(rate),
            rom_loaded: false,
            audio_staging: Vec::with_capacity(4096),
        }
    }

    // ── ROM loading ─────────────────────────────────────────────────────

    /// Load a ROM from a `Uint8Array`.  Returns the cartridge title on
    /// success or throws on invalid ROM data.
    pub fn load_rom(&mut self, rom_data: &[u8]) -> Result<String, JsValue> {
        let rom = rom_data.to_vec();
        self.gameboy
            .load_rom_bytes(rom)
            .map_err(|e| JsValue::from_str(&e))?;

        self.rom_loaded = true;
        let title = self
            .gameboy
            .bus
            .cartridge
            .as_ref()
            .map(|c| c.header.title.clone())
            .unwrap_or_default();

        log::info!("ROM loaded: {}", title);
        Ok(title)
    }

    /// Whether a ROM has been loaded.
    pub fn is_rom_loaded(&self) -> bool {
        self.rom_loaded
    }

    // ── Emulation ───────────────────────────────────────────────────────

    /// Advance the emulator by exactly one frame (~70 224 CPU cycles).
    pub fn run_frame(&mut self) {
        if self.rom_loaded {
            self.gameboy.run_frame();
        }
    }

    // ── Video ───────────────────────────────────────────────────────────

    /// Pointer to the RGBA frame buffer inside WASM linear memory.
    /// JS can build a `Uint8ClampedArray` view over this for `putImageData`.
    pub fn frame_buffer_ptr(&self) -> *const u8 {
        self.gameboy.frame_buffer().as_ptr()
    }

    /// Length of the frame buffer in bytes (160 × 144 × 4 = 92 160).
    pub fn frame_buffer_len(&self) -> usize {
        self.gameboy.frame_buffer().len()
    }

    pub fn screen_width(&self) -> u32 {
        SCREEN_WIDTH as u32
    }

    pub fn screen_height(&self) -> u32 {
        SCREEN_HEIGHT as u32
    }

    // ── Audio ───────────────────────────────────────────────────────────

    /// Number of stereo sample *pairs* the APU has ready for reading.
    pub fn audio_samples_available(&self) -> usize {
        self.gameboy.bus.apu.samples_available()
    }

    /// Copy all available audio samples into the staging buffer and return
    /// its pointer so JS can create a `Float32Array` view.
    /// Returns the number of **floats** written (pairs × 2).
    pub fn drain_audio_samples(&mut self) -> usize {
        let available = self.gameboy.bus.apu.samples_available();
        if available == 0 {
            return 0;
        }

        // Ensure staging buffer is big enough
        let float_count = available * 2;
        self.audio_staging.resize(float_count, 0.0);

        let pairs_read = self.gameboy.bus.apu.read_samples(&mut self.audio_staging, available);
        pairs_read * 2
    }

    /// Pointer to the audio staging buffer in WASM linear memory.
    pub fn audio_staging_ptr(&self) -> *const f32 {
        self.audio_staging.as_ptr()
    }

    // ── Input ───────────────────────────────────────────────────────────

    /// Press a Game Boy button.  `btn` must be 0–7 (see `map_button`).
    pub fn key_down(&mut self, btn: u8) {
        if let Some(button) = map_button(btn) {
            self.gameboy.bus.joypad.press(button);
        }
    }

    /// Release a Game Boy button.
    pub fn key_up(&mut self, btn: u8) {
        if let Some(button) = map_button(btn) {
            self.gameboy.bus.joypad.release(button);
        }
    }
}

/// Map a numeric button ID (chosen by convention with `main.js`) to the
/// core `Button` enum.
///
/// | ID | Button |
/// |----|--------|
/// | 0  | Right  |
/// | 1  | Left   |
/// | 2  | Up     |
/// | 3  | Down   |
/// | 4  | B      |
/// | 5  | A      |
/// | 6  | Start  |
/// | 7  | Select |
fn map_button(id: u8) -> Option<Button> {
    match id {
        0 => Some(Button::Right),
        1 => Some(Button::Left),
        2 => Some(Button::Up),
        3 => Some(Button::Down),
        4 => Some(Button::B),
        5 => Some(Button::A),
        6 => Some(Button::Start),
        7 => Some(Button::Select),
        _ => None,
    }
}
