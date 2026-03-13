/// Channel 1 — Square wave with frequency sweep + volume envelope
///
/// Registers:
///   NR10 (0xFF10) — Sweep: period, direction, shift
///   NR11 (0xFF11) — Duty cycle + length load
///   NR12 (0xFF12) — Volume envelope
///   NR13 (0xFF13) — Frequency low 8 bits
///   NR14 (0xFF14) — Trigger, length enable, frequency high 3 bits

/// Duty cycle waveforms (8 steps each)
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
    [1, 0, 0, 0, 0, 0, 0, 1], // 25%
    [1, 0, 0, 0, 0, 1, 1, 1], // 50%
    [0, 1, 1, 1, 1, 1, 1, 0], // 75%
];

pub struct Channel1 {
    pub enabled: bool,

    // ── Sweep ──────────────────────────────────────────────────────
    sweep_period: u8,    // NR10 bits 6-4: sweep time (0 = disabled)
    sweep_negate: bool,  // NR10 bit 3:    0 = addition, 1 = subtraction
    sweep_shift: u8,     // NR10 bits 2-0: shift amount
    sweep_timer: u8,
    sweep_enabled: bool,
    sweep_shadow: u16,   // shadow copy of frequency for calculations
    sweep_negate_used: bool, // obscure: if negate was used then un-negated, disable channel

    // ── Duty / Length ──────────────────────────────────────────────
    duty: u8,            // NR11 bits 7-6
    length_counter: u16, // 6-bit load → counts up to 64
    length_enabled: bool, // NR14 bit 6

    // ── Volume Envelope ────────────────────────────────────────────
    envelope_initial: u8,   // NR12 bits 7-4
    envelope_direction: bool, // NR12 bit 3 (true = increase)
    envelope_period: u8,    // NR12 bits 2-0
    envelope_timer: u8,
    volume: u8,

    // ── Frequency timer ────────────────────────────────────────────
    frequency: u16,      // 11-bit frequency value (NR13 + NR14 low 3 bits)
    timer: u32,          // counts down, reloads from (2048 - frequency) * 4
    duty_pos: u8,        // current position in the 8-step duty waveform

    // ── DAC ────────────────────────────────────────────────────────
    dac_enabled: bool,
}

impl Channel1 {
    pub fn new() -> Self {
        Self {
            enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_timer: 0,
            sweep_enabled: false,
            sweep_shadow: 0,
            sweep_negate_used: false,
            duty: 0,
            length_counter: 0,
            length_enabled: false,
            envelope_initial: 0,
            envelope_direction: false,
            envelope_period: 0,
            envelope_timer: 0,
            volume: 0,
            frequency: 0,
            timer: 0,
            duty_pos: 0,
            dac_enabled: false,
        }
    }

    // ── Tick (called every T-cycle) ────────────────────────────────
    pub fn tick(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer == 0 {
            self.timer = ((2048 - self.frequency as u32) * 4).max(1);
            self.duty_pos = (self.duty_pos + 1) & 7;
        }
    }

    /// Get current output amplitude (0–15), or 0 if channel off / DAC off
    pub fn output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }
        let sample = DUTY_TABLE[self.duty as usize][self.duty_pos as usize];
        sample * self.volume
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

    pub fn clock_sweep(&mut self) {
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period != 0 { self.sweep_period } else { 8 };

            if self.sweep_enabled && self.sweep_period != 0 {
                let new_freq = self.sweep_calculation();
                if new_freq <= 2047 && self.sweep_shift != 0 {
                    self.sweep_shadow = new_freq;
                    self.frequency = new_freq;
                    // Overflow check again with new frequency
                    let _ = self.sweep_calculation();
                }
            }
        }
    }

    /// Perform sweep frequency calculation; disables channel on overflow
    fn sweep_calculation(&mut self) -> u16 {
        let shifted = self.sweep_shadow >> self.sweep_shift;
        let new_freq = if self.sweep_negate {
            self.sweep_negate_used = true;
            self.sweep_shadow.wrapping_sub(shifted)
        } else {
            self.sweep_shadow.wrapping_add(shifted)
        };

        if new_freq > 2047 {
            self.enabled = false;
        }
        new_freq
    }

    // ── Register access ────────────────────────────────────────────

    // NR10 – 0xFF10
    pub fn read_nr10(&self) -> u8 {
        0x80 // bit 7 always 1
            | (self.sweep_period << 4)
            | if self.sweep_negate { 0x08 } else { 0 }
            | self.sweep_shift
    }

    pub fn write_nr10(&mut self, value: u8) {
        let new_negate = value & 0x08 != 0;
        self.sweep_period = (value >> 4) & 0x07;
        self.sweep_shift = value & 0x07;

        // Obscure behaviour: if negate was used in a calculation and then
        // negate is turned OFF, the channel is immediately disabled.
        if self.sweep_negate_used && !new_negate {
            self.enabled = false;
        }
        self.sweep_negate = new_negate;
    }

    // NR11 – 0xFF11
    pub fn read_nr11(&self) -> u8 {
        (self.duty << 6) | 0x3F // lower 6 bits always read as 1
    }

    pub fn write_nr11(&mut self, value: u8) {
        self.duty = (value >> 6) & 0x03;
        self.length_counter = 64 - (value & 0x3F) as u16;
    }

    // NR12 – 0xFF12
    pub fn read_nr12(&self) -> u8 {
        (self.envelope_initial << 4)
            | if self.envelope_direction { 0x08 } else { 0 }
            | self.envelope_period
    }

    pub fn write_nr12(&mut self, value: u8) {
        self.envelope_initial = (value >> 4) & 0x0F;
        self.envelope_direction = value & 0x08 != 0;
        self.envelope_period = value & 0x07;

        // DAC is enabled when upper 5 bits of NR12 are not all zero
        self.dac_enabled = value & 0xF8 != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    // NR13 – 0xFF13
    pub fn read_nr13(&self) -> u8 {
        0xFF // write-only
    }

    pub fn write_nr13(&mut self, value: u8) {
        self.frequency = (self.frequency & 0x700) | value as u16;
    }

    // NR14 – 0xFF14
    pub fn read_nr14(&self) -> u8 {
        0xBF | if self.length_enabled { 0x40 } else { 0 }
    }

    pub fn write_nr14(&mut self, value: u8) {
        self.length_enabled = value & 0x40 != 0;
        self.frequency = (self.frequency & 0x0FF) | (((value & 0x07) as u16) << 8);

        // Trigger
        if value & 0x80 != 0 {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;

        if self.length_counter == 0 {
            self.length_counter = 64;
        }

        // Reload frequency timer
        self.timer = ((2048 - self.frequency as u32) * 4).max(1);

        // Reload envelope
        self.volume = self.envelope_initial;
        self.envelope_timer = if self.envelope_period != 0 { self.envelope_period } else { 8 };

        // Reload sweep
        self.sweep_shadow = self.frequency;
        self.sweep_timer = if self.sweep_period != 0 { self.sweep_period } else { 8 };
        self.sweep_enabled = self.sweep_period != 0 || self.sweep_shift != 0;
        self.sweep_negate_used = false;

        if self.sweep_shift != 0 {
            let _ = self.sweep_calculation();
        }

        if !self.dac_enabled {
            self.enabled = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duty_waveform_output() {
        let mut ch = Channel1::new();
        ch.enabled = true;
        ch.dac_enabled = true;
        ch.volume = 15;
        ch.duty = 2; // 50%

        // Step through all 8 positions and verify output
        let expected = &DUTY_TABLE[2];
        for i in 0..8u8 {
            ch.duty_pos = i;
            assert_eq!(ch.output(), expected[i as usize] * 15);
        }
    }

    #[test]
    fn test_length_counter_disables() {
        let mut ch = Channel1::new();
        ch.enabled = true;
        ch.length_enabled = true;
        ch.length_counter = 1;

        ch.clock_length();
        assert!(!ch.enabled, "Channel should be disabled when length reaches 0");
    }

    #[test]
    fn test_dac_off_disables_channel() {
        let mut ch = Channel1::new();
        ch.enabled = true;
        ch.dac_enabled = true;

        ch.write_nr12(0x00); // DAC off (upper 5 bits = 0)
        assert!(!ch.dac_enabled);
        assert!(!ch.enabled);
    }

    #[test]
    fn test_sweep_overflow_disables() {
        let mut ch = Channel1::new();
        ch.enabled = true;
        ch.dac_enabled = true;
        ch.sweep_enabled = true;
        ch.sweep_shadow = 2000;
        ch.sweep_shift = 1;
        ch.sweep_negate = false;
        ch.sweep_period = 1;
        ch.sweep_timer = 1;

        // 2000 + (2000 >> 1) = 3000 > 2047 → should disable
        ch.clock_sweep();
        assert!(!ch.enabled, "Channel should be disabled on sweep overflow");
    }

    #[test]
    fn test_envelope_increases_volume() {
        let mut ch = Channel1::new();
        ch.envelope_period = 1;
        ch.envelope_timer = 1;
        ch.envelope_direction = true; // increase
        ch.volume = 5;

        ch.clock_envelope();
        assert_eq!(ch.volume, 6);
    }

    #[test]
    fn test_envelope_decreases_volume() {
        let mut ch = Channel1::new();
        ch.envelope_period = 1;
        ch.envelope_timer = 1;
        ch.envelope_direction = false; // decrease
        ch.volume = 5;

        ch.clock_envelope();
        assert_eq!(ch.volume, 4);
    }

    #[test]
    fn test_trigger_reloads_state() {
        let mut ch = Channel1::new();
        ch.dac_enabled = true;
        ch.envelope_initial = 10;
        ch.envelope_period = 3;
        ch.frequency = 500;
        ch.sweep_period = 2;
        ch.sweep_shift = 1;
        ch.length_counter = 0;

        ch.trigger();

        assert!(ch.enabled);
        assert_eq!(ch.volume, 10);
        assert_eq!(ch.length_counter, 64);
        assert_eq!(ch.sweep_shadow, 500);
        assert!(ch.sweep_enabled);
    }
}
