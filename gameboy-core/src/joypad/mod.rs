/// Game Boy Joypad (P1/JOYP register at 0xFF00)
///
/// The Game Boy has 8 buttons arranged in two groups:
/// - Direction buttons: Right, Left, Up, Down
/// - Action buttons:    A, B, Select, Start
///
/// Register P1 (0xFF00) layout:
///   Bit 7-6: Not used (always 1)
///   Bit 5:   P15 — Select Action buttons    (0 = selected)
///   Bit 4:   P14 — Select Direction buttons  (0 = selected)
///   Bit 3:   P13 — Down  or Start   (0 = pressed, read-only)
///   Bit 2:   P12 — Up    or Select  (0 = pressed, read-only)
///   Bit 1:   P11 — Left  or B       (0 = pressed, read-only)
///   Bit 0:   P10 — Right or A       (0 = pressed, read-only)
///
/// A joypad interrupt (INT $60) is requested when any of the lower 4 bits
/// transition from high to low (i.e., a button is newly pressed while its
/// group is selected).

/// Represents the 8 physical buttons of the Game Boy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

pub struct Joypad {
    /// Current state of the direction buttons (active LOW: 0 = pressed)
    /// Bit 0 = Right, Bit 1 = Left, Bit 2 = Up, Bit 3 = Down
    direction_state: u8,

    /// Current state of the action buttons (active LOW: 0 = pressed)
    /// Bit 0 = A, Bit 1 = B, Bit 2 = Select, Bit 3 = Start
    action_state: u8,

    /// Which button group is selected (written by CPU via bits 4-5)
    /// Bit 4 = select direction (0 = selected)
    /// Bit 5 = select action   (0 = selected)
    select: u8,

    /// Pending joypad interrupt flag (bit 4 of IF)
    interrupt_pending: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            direction_state: 0x0F, // All buttons released (bits high)
            action_state: 0x0F,    // All buttons released (bits high)
            select: 0x30,          // Neither group selected
            interrupt_pending: false,
        }
    }

    /// Press a button. May trigger a joypad interrupt.
    pub fn press(&mut self, button: Button) {
        match button {
            Button::Right  => self.direction_state &= !0x01,
            Button::Left   => self.direction_state &= !0x02,
            Button::Up     => self.direction_state &= !0x04,
            Button::Down   => self.direction_state &= !0x08,
            Button::A      => self.action_state &= !0x01,
            Button::B      => self.action_state &= !0x02,
            Button::Select => self.action_state &= !0x04,
            Button::Start  => self.action_state &= !0x08,
        }

        // Check if this button press triggers an interrupt.
        // An interrupt fires when a selected input line goes from high to low.
        let current_output = self.read_output_lines();
        if current_output & 0x0F != 0x0F {
            // At least one selected button is pressed
            self.interrupt_pending = true;
        }
    }

    /// Release a button.
    pub fn release(&mut self, button: Button) {
        match button {
            Button::Right  => self.direction_state |= 0x01,
            Button::Left   => self.direction_state |= 0x02,
            Button::Up     => self.direction_state |= 0x04,
            Button::Down   => self.direction_state |= 0x08,
            Button::A      => self.action_state |= 0x01,
            Button::B      => self.action_state |= 0x02,
            Button::Select => self.action_state |= 0x04,
            Button::Start  => self.action_state |= 0x08,
        }
    }

    /// Read the P1/JOYP register (0xFF00).
    ///
    /// The lower nibble reflects which buttons are currently pressed in the
    /// selected group(s). Upper bits 7-6 are always 1.
    pub fn read(&self) -> u8 {
        0xC0 | self.select | self.read_output_lines()
    }

    /// Write to the P1/JOYP register (0xFF00).
    ///
    /// Only bits 4-5 (group select) are writable. The lower nibble is read-only.
    pub fn write(&mut self, value: u8) {
        // Store previous output state for interrupt edge detection
        let prev_output = self.read_output_lines();

        // Only bits 4-5 (select) are writable
        self.select = value & 0x30;

        // Check for high-to-low transition on output lines (interrupt trigger)
        let new_output = self.read_output_lines();
        // If any bit went from 1 -> 0 (button appears pressed after selecting new group)
        if (prev_output & !new_output) & 0x0F != 0 {
            self.interrupt_pending = true;
        }
    }

    /// Return the lower nibble based on the currently selected button group(s).
    fn read_output_lines(&self) -> u8 {
        let mut result = 0x0F; // Default: all high (no buttons)

        // Bit 4 = 0 means direction group selected
        if self.select & 0x10 == 0 {
            result &= self.direction_state;
        }

        // Bit 5 = 0 means action group selected
        if self.select & 0x20 == 0 {
            result &= self.action_state;
        }

        result
    }

    /// Take the pending interrupt flag (returns true once, then resets).
    pub fn take_interrupts(&mut self) -> u8 {
        if self.interrupt_pending {
            self.interrupt_pending = false;
            0x10 // Bit 4 = Joypad interrupt
        } else {
            0x00
        }
    }
}
