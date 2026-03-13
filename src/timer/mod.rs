/// Game Boy Timer implementation
///
/// The Game Boy has a 16-bit internal counter (system clock) that runs at
/// the CPU frequency (4,194,304 Hz). The upper byte of this counter is
/// exposed as the DIV register (0xFF04). Writing any value to DIV resets
/// the entire 16-bit counter to 0.
///
/// TIMA (0xFF05) is a configurable timer that increments at a rate selected
/// by TAC bits 0-1. When TIMA overflows (0xFF -> 0x00), it is reloaded with
/// the value in TMA (0xFF06) and a Timer interrupt (bit 2 of IF) is requested.
///
/// TAC (0xFF07):
///   Bit 2   - Timer Enable (0=Disabled, 1=Enabled)
///   Bits 1-0 - Clock Select:
///     00: CPU Clock / 1024 (4096 Hz)    -> bit 9 of system counter
///     01: CPU Clock / 16   (262144 Hz)  -> bit 3 of system counter
///     10: CPU Clock / 64   (65536 Hz)   -> bit 5 of system counter
///     11: CPU Clock / 256  (16384 Hz)   -> bit 7 of system counter
///
/// The TIMA increment is triggered by a **falling edge** of the selected
/// bit ANDed with the timer-enable bit. This means that writing to DIV
/// (which zeroes the counter) or disabling the timer in TAC can cause a
/// "spurious" TIMA increment if the selected bit was previously 1.

/// Interrupt flag bit for the Timer interrupt
pub const TIMER_INTERRUPT: u8 = 0x04; // bit 2

pub struct Timer {
    /// 16-bit internal system counter. DIV register = upper byte (bits 8-15).
    sys_counter: u16,

    /// TIMA – Timer Counter (0xFF05)
    pub tima: u8,

    /// TMA – Timer Modulo (0xFF06)
    pub tma: u8,

    /// TAC – Timer Control (0xFF07)
    pub tac: u8,

    /// Previous value of (selected_bit AND timer_enable).
    /// Used for falling-edge detection.
    prev_and_result: bool,

    /// Pending interrupt flag to be collected by the emulator loop.
    pub interrupt_request: u8,

    /// Number of T-cycles remaining to delay the TMA reload after overflow.
    /// On real hardware there is a 1 M-cycle (4 T-cycle) delay before TMA
    /// is loaded into TIMA, but many emulators simplify this. We implement
    /// the delay for accuracy.
    overflow_countdown: u8,

    /// Whether TIMA overflowed and we are in the reload delay period.
    overflow_pending: bool,
}

impl Timer {
    pub fn new() -> Self {
        // After the boot ROM, the system counter is at 0xABCC (DIV=0xAB).
        Self {
            sys_counter: 0xABCC,
            tima: 0x00,
            tma: 0x00,
            tac: 0xF8, // upper bits read as 1
            prev_and_result: false,
            interrupt_request: 0,
            overflow_countdown: 0,
            overflow_pending: false,
        }
    }

    // ── Register access ──────────────────────────────────────────────

    /// Read the DIV register (0xFF04): upper byte of the system counter.
    pub fn read_div(&self) -> u8 {
        (self.sys_counter >> 8) as u8
    }

    /// Write to DIV (0xFF04): resets the entire 16-bit system counter.
    pub fn write_div(&mut self) {
        // Before resetting, check for falling edge
        let old_bit = self.selected_bit_value();
        self.sys_counter = 0;
        let new_bit = self.selected_bit_value();
        self.detect_falling_edge(old_bit, new_bit);
    }

    /// Read TIMA (0xFF05)
    pub fn read_tima(&self) -> u8 {
        self.tima
    }

    /// Write to TIMA (0xFF05)
    pub fn write_tima(&mut self, value: u8) {
        // If we write to TIMA during the reload delay, cancel the overflow
        if self.overflow_pending {
            self.overflow_pending = false;
            self.overflow_countdown = 0;
        }
        self.tima = value;
    }

    /// Read TMA (0xFF06)
    pub fn read_tma(&self) -> u8 {
        self.tma
    }

    /// Write to TMA (0xFF06)
    pub fn write_tma(&mut self, value: u8) {
        self.tma = value;
        // If we write to TMA during the exact cycle that the reload happens,
        // the new TMA value is what gets loaded. This is handled naturally
        // because we set tima = tma after writing tma.
    }

    /// Read TAC (0xFF07)
    pub fn read_tac(&self) -> u8 {
        self.tac | 0xF8 // unused upper bits read as 1
    }

    /// Write to TAC (0xFF07)
    pub fn write_tac(&mut self, value: u8) {
        let old_bit = self.selected_bit_value();
        self.tac = value & 0x07; // only lower 3 bits matter
        let new_bit = self.selected_bit_value();
        self.detect_falling_edge(old_bit, new_bit);
    }

    // ── Step / tick ──────────────────────────────────────────────────

    /// Advance the timer by `t_cycles` T-cycles (each CPU step returns T-cycles).
    /// This should be called once per CPU instruction with the number of T-cycles
    /// consumed by that instruction.
    pub fn step(&mut self, t_cycles: u32) {
        for _ in 0..t_cycles {
            self.tick();
        }
    }

    /// Advance the timer by exactly 1 T-cycle.
    fn tick(&mut self) {
        // Handle overflow delay
        if self.overflow_pending {
            self.overflow_countdown -= 1;
            if self.overflow_countdown == 0 {
                self.overflow_pending = false;
                self.tima = self.tma;
                self.interrupt_request |= TIMER_INTERRUPT;
            }
        }

        let old_bit = self.selected_bit_value();

        // Increment the system counter
        self.sys_counter = self.sys_counter.wrapping_add(1);

        let new_bit = self.selected_bit_value();
        self.detect_falling_edge(old_bit, new_bit);
    }

    // ── Internal helpers ─────────────────────────────────────────────

    /// Returns the current value of: (timer_enabled AND selected_counter_bit).
    fn selected_bit_value(&self) -> bool {
        let timer_enabled = self.tac & 0x04 != 0;
        if !timer_enabled {
            return false;
        }

        let bit_position = match self.tac & 0x03 {
            0b00 => 9,  // CPU / 1024
            0b01 => 3,  // CPU / 16
            0b10 => 5,  // CPU / 64
            0b11 => 7,  // CPU / 256
            _ => unreachable!(),
        };

        (self.sys_counter >> bit_position) & 1 == 1
    }

    /// Detect a falling edge on the AND result and increment TIMA if detected.
    fn detect_falling_edge(&mut self, old: bool, new: bool) {
        // Falling edge: was 1, now 0
        if self.prev_and_result && !new {
            self.increment_tima();
        }
        self.prev_and_result = new;
        let _ = old; // old is used implicitly through prev_and_result tracking
    }

    /// Increment TIMA. If it overflows, start the 4-cycle reload delay.
    fn increment_tima(&mut self) {
        let (new_tima, overflow) = self.tima.overflowing_add(1);
        self.tima = new_tima;

        if overflow {
            // TIMA overflows to 0x00 and will be reloaded with TMA after 4 T-cycles
            self.overflow_pending = true;
            self.overflow_countdown = 4;
        }
    }

    /// Take and clear any pending interrupt requests.
    pub fn take_interrupts(&mut self) -> u8 {
        let irq = self.interrupt_request;
        self.interrupt_request = 0;
        irq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_div_increments() {
        let mut timer = Timer::new();
        timer.sys_counter = 0; // start from zero for predictable testing

        // DIV is upper byte of sys_counter. It takes 256 T-cycles to increment DIV by 1.
        for _ in 0..256 {
            timer.tick();
        }
        assert_eq!(timer.read_div(), 1);
    }

    #[test]
    fn test_div_reset_on_write() {
        let mut timer = Timer::new();
        timer.sys_counter = 0x1234;
        assert_eq!(timer.read_div(), 0x12);
        timer.write_div();
        assert_eq!(timer.read_div(), 0x00);
        assert_eq!(timer.sys_counter, 0);
    }

    #[test]
    fn test_tima_increments_at_correct_rate() {
        let mut timer = Timer::new();
        timer.sys_counter = 0;
        timer.tima = 0;
        timer.tma = 0;
        timer.tac = 0x05; // enabled, clock select 01 -> every 16 T-cycles
        timer.prev_and_result = false;

        // After 16 T-cycles, TIMA should increment once
        // Bit 3 toggles every 8 cycles. Falling edge at cycle 16.
        for _ in 0..16 {
            timer.tick();
        }
        assert_eq!(timer.tima, 1, "TIMA should be 1 after 16 T-cycles with clock/16");
    }

    #[test]
    fn test_tima_overflow_fires_interrupt() {
        let mut timer = Timer::new();
        timer.sys_counter = 0;
        timer.tima = 0xFF;
        timer.tma = 0x42;
        timer.tac = 0x05; // enabled, clock/16
        timer.prev_and_result = false;

        // Tick until TIMA overflows (next falling edge of bit 3)
        for _ in 0..16 {
            timer.tick();
        }

        // TIMA should have overflowed, starting the 4-cycle reload delay
        assert!(timer.overflow_pending, "Overflow should be pending");

        // After 4 more T-cycles, TMA should be loaded and interrupt fired
        for _ in 0..4 {
            timer.tick();
        }

        assert_eq!(timer.tima, 0x42, "TIMA should be reloaded with TMA (0x42)");
        assert_ne!(timer.interrupt_request & TIMER_INTERRUPT, 0, "Timer interrupt should be requested");
    }

    #[test]
    fn test_tac_upper_bits_read_as_ones() {
        let timer = Timer::new();
        assert_eq!(timer.read_tac() & 0xF8, 0xF8);
    }

    #[test]
    fn test_timer_disabled_no_tima_increment() {
        let mut timer = Timer::new();
        timer.sys_counter = 0;
        timer.tima = 0;
        timer.tac = 0x01; // disabled (bit 2 = 0), clock/16
        timer.prev_and_result = false;

        for _ in 0..256 {
            timer.tick();
        }
        assert_eq!(timer.tima, 0, "TIMA should not increment when timer is disabled");
    }
}
