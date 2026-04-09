/// Channel 2 — Square wave with volume envelope (no sweep)
///
/// Registers:
///   NR21 (0xFF16) — Duty cycle + length load
///   NR22 (0xFF17) — Volume envelope
///   NR23 (0xFF18) — Frequency low 8 bits
///   NR24 (0xFF19) — Trigger, length enable, frequency high 3 bits

/// Duty cycle waveforms (same as Channel 1)
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
    [1, 0, 0, 0, 0, 0, 0, 1], // 25%
    [1, 0, 0, 0, 0, 1, 1, 1], // 50%
    [0, 1, 1, 1, 1, 1, 1, 0], // 75%
];

pub struct Channel2 {
    pub enabled: bool,

    // ── Duty / Length ──────────────────────────────────────────────
    duty: u8,
    length_counter: u16,
    length_enabled: bool,

    // ── Volume Envelope ────────────────────────────────────────────
    envelope_initial: u8,
    envelope_direction: bool,
    envelope_period: u8,
    envelope_timer: u8,
    volume: u8,

    // ── Frequency timer ────────────────────────────────────────────
    frequency: u16,
    timer: u32,
    duty_pos: u8,

    // ── DAC ────────────────────────────────────────────────────────
    dac_enabled: bool,
}

impl Channel2 {
    pub fn new() -> Self {
        Self {
            enabled: false,
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

    pub fn tick(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer == 0 {
            self.timer = ((2048 - self.frequency as u32) * 4).max(1);
            self.duty_pos = (self.duty_pos + 1) & 7;
        }
    }

    pub fn output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }
        DUTY_TABLE[self.duty as usize][self.duty_pos as usize] * self.volume
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

    // NR21 – 0xFF16
    pub fn read_nr21(&self) -> u8 {
        (self.duty << 6) | 0x3F
    }

    pub fn write_nr21(&mut self, value: u8) {
        self.duty = (value >> 6) & 0x03;
        self.length_counter = 64 - (value & 0x3F) as u16;
    }

    // NR22 – 0xFF17
    pub fn read_nr22(&self) -> u8 {
        (self.envelope_initial << 4)
            | if self.envelope_direction { 0x08 } else { 0 }
            | self.envelope_period
    }

    pub fn write_nr22(&mut self, value: u8) {
        self.envelope_initial = (value >> 4) & 0x0F;
        self.envelope_direction = value & 0x08 != 0;
        self.envelope_period = value & 0x07;

        self.dac_enabled = value & 0xF8 != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    // NR23 – 0xFF18
    pub fn read_nr23(&self) -> u8 {
        0xFF // write-only
    }

    pub fn write_nr23(&mut self, value: u8) {
        self.frequency = (self.frequency & 0x700) | value as u16;
    }

    // NR24 – 0xFF19
    pub fn read_nr24(&self) -> u8 {
        0xBF | if self.length_enabled { 0x40 } else { 0 }
    }

    pub fn write_nr24(&mut self, value: u8) {
        self.length_enabled = value & 0x40 != 0;
        self.frequency = (self.frequency & 0x0FF) | (((value & 0x07) as u16) << 8);

        if value & 0x80 != 0 {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;

        if self.length_counter == 0 {
            self.length_counter = 64;
        }

        self.timer = ((2048 - self.frequency as u32) * 4).max(1);

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
    fn test_channel2_output() {
        let mut ch = Channel2::new();
        ch.enabled = true;
        ch.dac_enabled = true;
        ch.volume = 10;
        ch.duty = 1; // 25%
        ch.duty_pos = 0;

        assert_eq!(ch.output(), DUTY_TABLE[1][0] * 10);
    }

    #[test]
    fn test_channel2_trigger() {
        let mut ch = Channel2::new();
        ch.dac_enabled = true;
        ch.envelope_initial = 8;
        ch.frequency = 1000;
        ch.length_counter = 0;

        ch.trigger();

        assert!(ch.enabled);
        assert_eq!(ch.volume, 8);
        assert_eq!(ch.length_counter, 64);
    }

    #[test]
    fn test_channel2_length_disable() {
        let mut ch = Channel2::new();
        ch.enabled = true;
        ch.length_enabled = true;
        ch.length_counter = 1;

        ch.clock_length();
        assert!(!ch.enabled);
    }
}
