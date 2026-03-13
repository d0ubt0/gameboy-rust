/// Channel 3 — Custom waveform (Wave channel)
///
/// Plays back 32 × 4-bit samples stored in Wave RAM (0xFF30 – 0xFF3F).
///
/// Registers:
///   NR30 (0xFF1A) — DAC enable (bit 7)
///   NR31 (0xFF1B) — Length load (8 bits → counts up to 256)
///   NR32 (0xFF1C) — Volume code (bits 6-5): 0=mute, 1=100%, 2=50%, 3=25%
///   NR33 (0xFF1D) — Frequency low 8 bits
///   NR34 (0xFF1E) — Trigger, length enable, frequency high 3 bits

pub struct Channel3 {
    pub enabled: bool,

    // ── DAC ────────────────────────────────────────────────────────
    dac_enabled: bool,

    // ── Length ──────────────────────────────────────────────────────
    length_counter: u16,
    length_enabled: bool,

    // ── Volume ─────────────────────────────────────────────────────
    volume_code: u8, // 0-3

    // ── Frequency / timer ──────────────────────────────────────────
    frequency: u16,
    timer: u32,

    // ── Wave position ──────────────────────────────────────────────
    /// Current position in the 32-sample waveform (0–31)
    wave_pos: u8,
    /// Current sample value (4-bit, 0–15)
    sample_buffer: u8,

    // ── Wave RAM ───────────────────────────────────────────────────
    /// 16 bytes = 32 × 4-bit samples. Each byte holds two samples:
    /// upper nibble first, lower nibble second.
    pub wave_ram: [u8; 16],
}

impl Channel3 {
    pub fn new() -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            length_counter: 0,
            length_enabled: false,
            volume_code: 0,
            frequency: 0,
            timer: 0,
            wave_pos: 0,
            sample_buffer: 0,
            wave_ram: [0; 16],
        }
    }

    pub fn tick(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer == 0 {
            // Wave channel period = (2048 - frequency) * 2
            self.timer = ((2048 - self.frequency as u32) * 2).max(1);
            self.wave_pos = (self.wave_pos + 1) & 31;

            // Read the current sample from wave RAM
            let byte_index = (self.wave_pos / 2) as usize;
            self.sample_buffer = if self.wave_pos & 1 == 0 {
                (self.wave_ram[byte_index] >> 4) & 0x0F // upper nibble
            } else {
                self.wave_ram[byte_index] & 0x0F // lower nibble
            };
        }
    }

    pub fn output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }

        let shifted = match self.volume_code {
            0 => 4, // mute (shift right by 4 = 0 for any 4-bit value)
            1 => 0, // 100%
            2 => 1, // 50%
            3 => 2, // 25%
            _ => 4,
        };

        self.sample_buffer >> shifted
    }

    // ── Frame-sequencer clocks ─────────────────────────────────────

    pub fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    // ── Register access ────────────────────────────────────────────

    // NR30 – 0xFF1A
    pub fn read_nr30(&self) -> u8 {
        0x7F | if self.dac_enabled { 0x80 } else { 0 }
    }

    pub fn write_nr30(&mut self, value: u8) {
        self.dac_enabled = value & 0x80 != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    // NR31 – 0xFF1B
    pub fn read_nr31(&self) -> u8 {
        0xFF // write-only
    }

    pub fn write_nr31(&mut self, value: u8) {
        self.length_counter = 256 - value as u16;
    }

    // NR32 – 0xFF1C
    pub fn read_nr32(&self) -> u8 {
        0x9F | ((self.volume_code & 0x03) << 5)
    }

    pub fn write_nr32(&mut self, value: u8) {
        self.volume_code = (value >> 5) & 0x03;
    }

    // NR33 – 0xFF1D
    pub fn read_nr33(&self) -> u8 {
        0xFF // write-only
    }

    pub fn write_nr33(&mut self, value: u8) {
        self.frequency = (self.frequency & 0x700) | value as u16;
    }

    // NR34 – 0xFF1E
    pub fn read_nr34(&self) -> u8 {
        0xBF | if self.length_enabled { 0x40 } else { 0 }
    }

    pub fn write_nr34(&mut self, value: u8) {
        self.length_enabled = value & 0x40 != 0;
        self.frequency = (self.frequency & 0x0FF) | (((value & 0x07) as u16) << 8);

        if value & 0x80 != 0 {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;

        if self.length_counter == 0 {
            self.length_counter = 256;
        }

        self.timer = ((2048 - self.frequency as u32) * 2).max(1);
        self.wave_pos = 0;

        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    // ── Wave RAM access ────────────────────────────────────────────

    pub fn read_wave_ram(&self, address: u16) -> u8 {
        // While the channel is active, the CPU can only read the byte the
        // wave channel is currently reading. We simplify this to always
        // allow access (accurate enough for most games).
        let index = (address - 0xFF30) as usize;
        self.wave_ram[index]
    }

    pub fn write_wave_ram(&mut self, address: u16, value: u8) {
        let index = (address - 0xFF30) as usize;
        self.wave_ram[index] = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_ram_access() {
        let mut ch = Channel3::new();
        ch.write_wave_ram(0xFF30, 0xAB);
        assert_eq!(ch.read_wave_ram(0xFF30), 0xAB);
    }

    #[test]
    fn test_volume_codes() {
        let mut ch = Channel3::new();
        ch.enabled = true;
        ch.dac_enabled = true;
        ch.sample_buffer = 0x0F; // max 4-bit value

        ch.volume_code = 1; // 100%
        assert_eq!(ch.output(), 0x0F);

        ch.volume_code = 2; // 50%
        assert_eq!(ch.output(), 0x07);

        ch.volume_code = 3; // 25%
        assert_eq!(ch.output(), 0x03);

        ch.volume_code = 0; // mute
        assert_eq!(ch.output(), 0x00);
    }

    #[test]
    fn test_channel3_trigger() {
        let mut ch = Channel3::new();
        ch.dac_enabled = true;
        ch.frequency = 1500;
        ch.length_counter = 0;

        ch.trigger();

        assert!(ch.enabled);
        assert_eq!(ch.length_counter, 256);
        assert_eq!(ch.wave_pos, 0);
    }

    #[test]
    fn test_channel3_length_disables() {
        let mut ch = Channel3::new();
        ch.enabled = true;
        ch.length_enabled = true;
        ch.length_counter = 1;

        ch.clock_length();
        assert!(!ch.enabled);
    }
}
