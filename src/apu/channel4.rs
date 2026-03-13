/// Channel 4 — Noise (LFSR-based pseudo-random)
///
/// Generates noise using a 15-bit or 7-bit linear feedback shift register.
///
/// Registers:
///   NR41 (0xFF20) — Length load (6 bits, max 64)
///   NR42 (0xFF21) — Volume envelope
///   NR43 (0xFF22) — Clock shift, width mode, divisor code
///   NR44 (0xFF23) — Trigger, length enable

/// Divisor lookup for NR43 bits 2-0
const DIVISOR_TABLE: [u32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

pub struct Channel4 {
    pub enabled: bool,

    // ── Length ──────────────────────────────────────────────────────
    length_counter: u16,
    length_enabled: bool,

    // ── Volume Envelope ────────────────────────────────────────────
    envelope_initial: u8,
    envelope_direction: bool,
    envelope_period: u8,
    envelope_timer: u8,
    volume: u8,

    // ── Noise generator ────────────────────────────────────────────
    clock_shift: u8,     // NR43 bits 7-4
    width_mode: bool,    // NR43 bit 3: false = 15-bit, true = 7-bit
    divisor_code: u8,    // NR43 bits 2-0

    /// 15-bit LFSR (linear feedback shift register)
    lfsr: u16,
    timer: u32,

    // ── DAC ────────────────────────────────────────────────────────
    dac_enabled: bool,
}

impl Channel4 {
    pub fn new() -> Self {
        Self {
            enabled: false,
            length_counter: 0,
            length_enabled: false,
            envelope_initial: 0,
            envelope_direction: false,
            envelope_period: 0,
            envelope_timer: 0,
            volume: 0,
            clock_shift: 0,
            width_mode: false,
            divisor_code: 0,
            lfsr: 0x7FFF, // all bits set
            timer: 0,
            dac_enabled: false,
        }
    }

    pub fn tick(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer == 0 {
            self.timer = self.calc_period().max(1);

            // XOR bits 0 and 1 of LFSR
            let xor_result = (self.lfsr & 0x01) ^ ((self.lfsr >> 1) & 0x01);

            // Shift right
            self.lfsr >>= 1;

            // Set bit 14 to xor_result
            self.lfsr |= xor_result << 14;

            // If 7-bit mode, also set bit 6
            if self.width_mode {
                self.lfsr &= !(1 << 6);
                self.lfsr |= xor_result << 6;
            }
        }
    }

    pub fn output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }
        // Output is inverted bit 0 of LFSR
        let bit = (!self.lfsr & 0x01) as u8;
        bit * self.volume
    }

    fn calc_period(&self) -> u32 {
        DIVISOR_TABLE[self.divisor_code as usize] << self.clock_shift
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

    pub fn clock_envelope(&mut self) {
        if self.envelope_period == 0 {
            return;
        }
        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }
        if self.envelope_timer == 0 {
            self.envelope_timer = if self.envelope_period != 0 { self.envelope_period } else { 8 };

            if self.envelope_direction && self.volume < 15 {
                self.volume += 1;
            } else if !self.envelope_direction && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    // ── Register access ────────────────────────────────────────────

    // NR41 – 0xFF20
    pub fn read_nr41(&self) -> u8 {
        0xFF // write-only
    }

    pub fn write_nr41(&mut self, value: u8) {
        self.length_counter = 64 - (value & 0x3F) as u16;
    }

    // NR42 – 0xFF21
    pub fn read_nr42(&self) -> u8 {
        (self.envelope_initial << 4)
            | if self.envelope_direction { 0x08 } else { 0 }
            | self.envelope_period
    }

    pub fn write_nr42(&mut self, value: u8) {
        self.envelope_initial = (value >> 4) & 0x0F;
        self.envelope_direction = value & 0x08 != 0;
        self.envelope_period = value & 0x07;

        self.dac_enabled = value & 0xF8 != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    // NR43 – 0xFF22
    pub fn read_nr43(&self) -> u8 {
        (self.clock_shift << 4)
            | if self.width_mode { 0x08 } else { 0 }
            | self.divisor_code
    }

    pub fn write_nr43(&mut self, value: u8) {
        self.clock_shift = (value >> 4) & 0x0F;
        self.width_mode = value & 0x08 != 0;
        self.divisor_code = value & 0x07;
    }

    // NR44 – 0xFF23
    pub fn read_nr44(&self) -> u8 {
        0xBF | if self.length_enabled { 0x40 } else { 0 }
    }

    pub fn write_nr44(&mut self, value: u8) {
        self.length_enabled = value & 0x40 != 0;

        if value & 0x80 != 0 {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;

        if self.length_counter == 0 {
            self.length_counter = 64;
        }

        self.timer = self.calc_period().max(1);

        // Reset LFSR to all 1s
        self.lfsr = 0x7FFF;

        // Reload envelope
        self.volume = self.envelope_initial;
        self.envelope_timer = if self.envelope_period != 0 { self.envelope_period } else { 8 };

        if !self.dac_enabled {
            self.enabled = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lfsr_initial_state() {
        let ch = Channel4::new();
        assert_eq!(ch.lfsr, 0x7FFF);
    }

    #[test]
    fn test_noise_output() {
        let mut ch = Channel4::new();
        ch.enabled = true;
        ch.dac_enabled = true;
        ch.volume = 10;
        ch.lfsr = 0x7FFE; // bit 0 = 0 → inverted = 1

        assert_eq!(ch.output(), 10);

        ch.lfsr = 0x7FFF; // bit 0 = 1 → inverted = 0
        assert_eq!(ch.output(), 0);
    }

    #[test]
    fn test_trigger_resets_lfsr() {
        let mut ch = Channel4::new();
        ch.dac_enabled = true;
        ch.lfsr = 0x0000;
        ch.envelope_initial = 7;
        ch.length_counter = 0;

        ch.trigger();

        assert!(ch.enabled);
        assert_eq!(ch.lfsr, 0x7FFF);
        assert_eq!(ch.volume, 7);
        assert_eq!(ch.length_counter, 64);
    }

    #[test]
    fn test_width_mode_7bit() {
        let mut ch = Channel4::new();
        ch.enabled = true;
        ch.dac_enabled = true;
        ch.width_mode = true;
        ch.lfsr = 0x7FFF;
        ch.timer = 1;
        ch.clock_shift = 0;
        ch.divisor_code = 0;

        // Tick to advance LFSR
        ch.tick();

        // After one tick: XOR of bits 0,1 of 0x7FFF = 1^1 = 0
        // Shift right: 0x3FFF  (bit 14 = 0, bit 6 also set to 0)
        // In 7-bit mode, bit 6 is also set to xor_result = 0
        // So bit 6 should be 0
        assert_eq!(ch.lfsr & (1 << 6), 0);
    }

    #[test]
    fn test_divisor_period() {
        let mut ch = Channel4::new();
        ch.divisor_code = 0;
        ch.clock_shift = 0;
        assert_eq!(ch.calc_period(), 8);

        ch.divisor_code = 1;
        ch.clock_shift = 2;
        assert_eq!(ch.calc_period(), 16 << 2);
    }
}
