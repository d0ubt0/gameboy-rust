/// Game Boy Audio Processing Unit (APU)
///
/// The APU generates sound through four channels:
///   - Channel 1: Square wave with frequency sweep + volume envelope
///   - Channel 2: Square wave with volume envelope (no sweep)
///   - Channel 3: Custom waveform from 16-byte wave RAM (32 4-bit samples)
///   - Channel 4: Noise (pseudo-random via LFSR) with volume envelope
///
/// A **Frame Sequencer** (clocked at 512 Hz) drives the length counters,
/// volume envelopes, and frequency sweep at sub-rates:
///   Step 0: Length
///   Step 1: —
///   Step 2: Length, Sweep
///   Step 3: —
///   Step 4: Length
///   Step 5: —
///   Step 6: Length, Sweep
///   Step 7: Volume Envelope
///
/// Register map: 0xFF10 – 0xFF3F
///
/// The APU runs off the system clock (4,194,304 Hz / T-cycle).

pub mod channel1;
pub mod channel2;
pub mod channel3;
pub mod channel4;

use channel1::Channel1;
use channel2::Channel2;
use channel3::Channel3;
use channel4::Channel4;

/// Sample rate we target for audio output
pub const SAMPLE_RATE: u32 = 44100;

/// CPU clock frequency
const CPU_CLOCK_HZ: u32 = 4_194_304;

/// Number of T-cycles between frame sequencer steps (512 Hz = CPU / 8192)
const FRAME_SEQUENCER_PERIOD: u32 = 8192;

/// How many samples we buffer before they can be consumed by the audio backend.
/// ~1/60 second at 44100 Hz ≈ 735 samples, but we use a larger ring so the
/// callback thread never starves.
pub const AUDIO_BUFFER_SIZE: usize = 4096;

pub struct Apu {
    /// Is the APU powered on? (NR52 bit 7)
    pub power: bool,

    // ── Channels ────────────────────────────────────────────────────
    pub ch1: Channel1,
    pub ch2: Channel2,
    pub ch3: Channel3,
    pub ch4: Channel4,

    // ── Mixer / master volume ──────────────────────────────────────
    /// NR50 – Master volume & VIN enable
    pub nr50: u8,
    /// NR51 – Sound panning (which channels go to which output)
    pub nr51: u8,

    // ── Frame Sequencer ────────────────────────────────────────────
    frame_seq_counter: u32,
    frame_seq_step: u8,

    // ── Sample generation ──────────────────────────────────────────
    /// Down-sampler: counts T-cycles to decide when to push a sample
    sample_counter: f64,
    /// How many T-cycles per output sample
    sample_period: f64,

    /// Ring buffer of **stereo** samples (left, right interleaved, f32 in -1..1)
    pub sample_buffer: Vec<f32>,
    /// Write position in the ring buffer
    pub sample_write_pos: usize,
    /// Read position (consumed by audio callback)
    pub sample_read_pos: usize,
}

impl Apu {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            power: true,

            ch1: Channel1::new(),
            ch2: Channel2::new(),
            ch3: Channel3::new(),
            ch4: Channel4::new(),

            nr50: 0x77,
            nr51: 0xF3,

            frame_seq_counter: 0,
            frame_seq_step: 0,

            sample_counter: 0.0,
            sample_period: CPU_CLOCK_HZ as f64 / sample_rate as f64,

            sample_buffer: vec![0.0; AUDIO_BUFFER_SIZE * 2], // stereo
            sample_write_pos: 0,
            sample_read_pos: 0,
        }
    }

    // ════════════════════════════════════════════════════════════════
    //  Advance the APU by `t_cycles` T-cycles
    // ════════════════════════════════════════════════════════════════
    pub fn step(&mut self, t_cycles: u32) {
        if !self.power {
            return;
        }

        for _ in 0..t_cycles {
            self.tick();
        }
    }

    /// One T-cycle tick
    fn tick(&mut self) {
        // ── Frame Sequencer ────────────────────────────────────────
        self.frame_seq_counter += 1;
        if self.frame_seq_counter >= FRAME_SEQUENCER_PERIOD {
            self.frame_seq_counter = 0;
            self.clock_frame_sequencer();
        }

        // ── Channel frequency timers ──────────────────────────────
        self.ch1.tick();
        self.ch2.tick();
        self.ch3.tick();
        self.ch4.tick();

        // ── Down-sample to output ─────────────────────────────────
        self.sample_counter += 1.0;
        if self.sample_counter >= self.sample_period {
            self.sample_counter -= self.sample_period;
            self.generate_sample();
        }
    }

    /// Clock the frame sequencer — drives length, sweep, envelope
    fn clock_frame_sequencer(&mut self) {
        match self.frame_seq_step {
            0 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
            }
            1 => {}
            2 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
                self.ch1.clock_sweep();
            }
            3 => {}
            4 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
            }
            5 => {}
            6 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
                self.ch1.clock_sweep();
            }
            7 => {
                self.ch1.clock_envelope();
                self.ch2.clock_envelope();
                self.ch4.clock_envelope();
            }
            _ => unreachable!(),
        }
        self.frame_seq_step = (self.frame_seq_step + 1) & 7;
    }

    /// Mix the four channels and push one stereo sample into the ring buffer
    fn generate_sample(&mut self) {
        let ch1_out = self.ch1.output() as f32;
        let ch2_out = self.ch2.output() as f32;
        let ch3_out = self.ch3.output() as f32;
        let ch4_out = self.ch4.output() as f32;

        let nr51 = self.nr51;

        // Left channel mix
        let mut left: f32 = 0.0;
        if nr51 & 0x10 != 0 { left += ch1_out; }
        if nr51 & 0x20 != 0 { left += ch2_out; }
        if nr51 & 0x40 != 0 { left += ch3_out; }
        if nr51 & 0x80 != 0 { left += ch4_out; }

        // Right channel mix
        let mut right: f32 = 0.0;
        if nr51 & 0x01 != 0 { right += ch1_out; }
        if nr51 & 0x02 != 0 { right += ch2_out; }
        if nr51 & 0x04 != 0 { right += ch3_out; }
        if nr51 & 0x08 != 0 { right += ch4_out; }

        // Master volume (0-7) from NR50
        let left_vol = ((self.nr50 >> 4) & 0x07) as f32 + 1.0;
        let right_vol = (self.nr50 & 0x07) as f32 + 1.0;

        left *= left_vol;
        right *= right_vol;

        // Normalise to -1..1 range
        // Max per side = 4 channels * 15 amplitude * 8 volume = 480
        const NORMALISE: f32 = 1.0 / 480.0;
        left *= NORMALISE;
        right *= NORMALISE;

        // Write into ring buffer
        let buf_len = self.sample_buffer.len();
        self.sample_buffer[self.sample_write_pos % buf_len] = left;
        self.sample_buffer[(self.sample_write_pos + 1) % buf_len] = right;
        self.sample_write_pos = (self.sample_write_pos + 2) % buf_len;
    }

    /// Returns the number of stereo **sample pairs** available for reading.
    pub fn samples_available(&self) -> usize {
        let buf_len = self.sample_buffer.len();
        ((self.sample_write_pos + buf_len - self.sample_read_pos) % buf_len) / 2
    }

    /// Read up to `max` stereo sample pairs into `out` (interleaved L,R,L,R,…).
    /// Returns the number of **sample pairs** actually read.
    pub fn read_samples(&mut self, out: &mut [f32], max_pairs: usize) -> usize {
        let available = self.samples_available();
        let pairs = available.min(max_pairs).min(out.len() / 2);
        let buf_len = self.sample_buffer.len();

        for i in 0..pairs {
            out[i * 2] = self.sample_buffer[self.sample_read_pos];
            out[i * 2 + 1] = self.sample_buffer[(self.sample_read_pos + 1) % buf_len];
            self.sample_read_pos = (self.sample_read_pos + 2) % buf_len;
        }
        pairs
    }

    // ════════════════════════════════════════════════════════════════
    //  Register reads / writes  (0xFF10 – 0xFF3F)
    // ════════════════════════════════════════════════════════════════

    pub fn read_register(&self, address: u16) -> u8 {
        if !self.power && address != 0xFF26 && !(0xFF30..=0xFF3F).contains(&address) {
            return 0xFF;
        }

        match address {
            // ── Channel 1 ──────────────────────────────────────────
            0xFF10 => self.ch1.read_nr10(),
            0xFF11 => self.ch1.read_nr11(),
            0xFF12 => self.ch1.read_nr12(),
            0xFF13 => self.ch1.read_nr13(),
            0xFF14 => self.ch1.read_nr14(),

            // ── Channel 2 ──────────────────────────────────────────
            0xFF15 => 0xFF, // NR20 doesn't exist
            0xFF16 => self.ch2.read_nr21(),
            0xFF17 => self.ch2.read_nr22(),
            0xFF18 => self.ch2.read_nr23(),
            0xFF19 => self.ch2.read_nr24(),

            // ── Channel 3 ──────────────────────────────────────────
            0xFF1A => self.ch3.read_nr30(),
            0xFF1B => self.ch3.read_nr31(),
            0xFF1C => self.ch3.read_nr32(),
            0xFF1D => self.ch3.read_nr33(),
            0xFF1E => self.ch3.read_nr34(),

            // ── Channel 4 ──────────────────────────────────────────
            0xFF1F => 0xFF, // NR40 doesn't exist
            0xFF20 => self.ch4.read_nr41(),
            0xFF21 => self.ch4.read_nr42(),
            0xFF22 => self.ch4.read_nr43(),
            0xFF23 => self.ch4.read_nr44(),

            // ── Master control ─────────────────────────────────────
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => self.read_nr52(),

            // 0xFF27 – 0xFF2F unused
            0xFF27..=0xFF2F => 0xFF,

            // ── Wave RAM ───────────────────────────────────────────
            0xFF30..=0xFF3F => self.ch3.read_wave_ram(address),

            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        // Wave RAM is always accessible
        if (0xFF30..=0xFF3F).contains(&address) {
            self.ch3.write_wave_ram(address, value);
            return;
        }

        // NR52 power control is always accessible
        if address == 0xFF26 {
            self.write_nr52(value);
            return;
        }

        // All other registers are read-only when powered off
        if !self.power {
            return;
        }

        match address {
            // ── Channel 1 ──────────────────────────────────────────
            0xFF10 => self.ch1.write_nr10(value),
            0xFF11 => self.ch1.write_nr11(value),
            0xFF12 => self.ch1.write_nr12(value),
            0xFF13 => self.ch1.write_nr13(value),
            0xFF14 => self.ch1.write_nr14(value),

            // ── Channel 2 ──────────────────────────────────────────
            0xFF15 => {} // NR20 doesn't exist
            0xFF16 => self.ch2.write_nr21(value),
            0xFF17 => self.ch2.write_nr22(value),
            0xFF18 => self.ch2.write_nr23(value),
            0xFF19 => self.ch2.write_nr24(value),

            // ── Channel 3 ──────────────────────────────────────────
            0xFF1A => self.ch3.write_nr30(value),
            0xFF1B => self.ch3.write_nr31(value),
            0xFF1C => self.ch3.write_nr32(value),
            0xFF1D => self.ch3.write_nr33(value),
            0xFF1E => self.ch3.write_nr34(value),

            // ── Channel 4 ──────────────────────────────────────────
            0xFF1F => {} // NR40 doesn't exist
            0xFF20 => self.ch4.write_nr41(value),
            0xFF21 => self.ch4.write_nr42(value),
            0xFF22 => self.ch4.write_nr43(value),
            0xFF23 => self.ch4.write_nr44(value),

            // ── Master control ─────────────────────────────────────
            0xFF24 => self.nr50 = value,
            0xFF25 => self.nr51 = value,

            // 0xFF27 – 0xFF2F unused
            _ => {}
        }
    }

    /// NR52 read: bit 7 = power, bits 3-0 = channel active status
    fn read_nr52(&self) -> u8 {
        let mut val: u8 = 0x70; // bits 6-4 always 1
        if self.power { val |= 0x80; }
        if self.ch1.enabled { val |= 0x01; }
        if self.ch2.enabled { val |= 0x02; }
        if self.ch3.enabled { val |= 0x04; }
        if self.ch4.enabled { val |= 0x08; }
        val
    }

    /// NR52 write: only bit 7 matters (power on/off)
    fn write_nr52(&mut self, value: u8) {
        let was_on = self.power;
        self.power = value & 0x80 != 0;

        if was_on && !self.power {
            // Turning off: clear all registers
            self.ch1 = Channel1::new();
            self.ch2 = Channel2::new();
            // Channel 3 wave RAM is preserved on power off
            let wave_ram = self.ch3.wave_ram;
            self.ch3 = Channel3::new();
            self.ch3.wave_ram = wave_ram;
            self.ch4 = Channel4::new();
            self.nr50 = 0;
            self.nr51 = 0;
            self.frame_seq_step = 0;
        }
    }
}
