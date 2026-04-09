/// Memory Bank Controller (MBC) trait and implementations
///
/// The Game Boy cartridge header at 0x0147 specifies the MBC type:
///   0x00: ROM Only (NoMbc)
///   0x01: MBC1
///   0x02: MBC1+RAM
///   0x03: MBC1+RAM+BATTERY
///   ... (more types exist)

pub trait Mbc: Send + Sync {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);
}

// ============================================================================
// NoMBC — ROM Only (up to 32KB)
// ============================================================================

pub struct NoMbc {
    rom: Vec<u8>,
    ram: Vec<u8>,
}

impl NoMbc {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
        }
    }
}

impl Mbc for NoMbc {
    fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x7FFF => {
                self.rom.get(address as usize).cloned().unwrap_or(0xFF)
            }
            0xA000..=0xBFFF => {
                let offset = (address - 0xA000) as usize;
                self.ram.get(offset).cloned().unwrap_or(0xFF)
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        match address {
            0xA000..=0xBFFF => {
                let offset = (address - 0xA000) as usize;
                if offset < self.ram.len() {
                    self.ram[offset] = value;
                }
            }
            _ => {} // ROM writes are ignored
        }
    }
}

// ============================================================================
// MBC1 — Most common MBC (up to 2MB ROM / 32KB RAM)
// ============================================================================

pub struct Mbc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    ram_enabled: bool,
    rom_bank: u8,       // 5-bit ROM bank number (1-31 for the switchable bank)
    ram_bank: u8,       // 2-bit RAM bank number (0-3)
    banking_mode: bool, // false = ROM banking mode, true = RAM banking mode
}

impl Mbc1 {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            banking_mode: false,
        }
    }

    /// Get the effective ROM bank number for the switchable area (0x4000-0x7FFF)
    fn effective_rom_bank(&self) -> usize {
        let mut bank = self.rom_bank as usize;
        if !self.banking_mode {
            // In ROM banking mode, upper 2 bits from ram_bank apply to ROM
            bank |= (self.ram_bank as usize) << 5;
        }
        // MBC1 quirk: banks 0x00, 0x20, 0x40, 0x60 are never accessible here
        // (they map to 0x01, 0x21, 0x41, 0x61 respectively)
        // This is already handled because rom_bank minimum is 1
        bank
    }

    /// Get the bank for the fixed area (0x0000-0x3FFF)
    fn bank_zero(&self) -> usize {
        if self.banking_mode {
            // In RAM banking mode, bank 0 area uses upper bits
            (self.ram_bank as usize) << 5
        } else {
            0
        }
    }
}

impl Mbc for Mbc1 {
    fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => {
                let bank = self.bank_zero();
                let offset = bank * 0x4000 + address as usize;
                self.rom.get(offset).cloned().unwrap_or(0xFF)
            }
            0x4000..=0x7FFF => {
                let bank = self.effective_rom_bank();
                let offset = bank * 0x4000 + (address as usize - 0x4000);
                self.rom.get(offset).cloned().unwrap_or(0xFF)
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled || self.ram.is_empty() {
                    return 0xFF;
                }
                let bank = if self.banking_mode { self.ram_bank as usize } else { 0 };
                let offset = bank * 0x2000 + (address as usize - 0xA000);
                self.ram.get(offset).cloned().unwrap_or(0xFF)
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                // RAM Enable: writing 0x0A to lower nibble enables RAM
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                // ROM Bank Number (lower 5 bits)
                let mut bank = value & 0x1F;
                if bank == 0 {
                    bank = 1; // Bank 0 is not selectable, maps to 1
                }
                self.rom_bank = bank;
            }
            0x4000..=0x5FFF => {
                // RAM Bank Number / Upper ROM Bank bits (2 bits)
                self.ram_bank = value & 0x03;
            }
            0x6000..=0x7FFF => {
                // Banking Mode Select
                self.banking_mode = (value & 0x01) != 0;
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled || self.ram.is_empty() {
                    return;
                }
                let bank = if self.banking_mode { self.ram_bank as usize } else { 0 };
                let offset = bank * 0x2000 + (address as usize - 0xA000);
                if offset < self.ram.len() {
                    self.ram[offset] = value;
                }
            }
            _ => {}
        }
    }
}

// ============================================================================
// MBC3 — Supports up to 2MB ROM and 32KB RAM + Timer (RTC)
// ============================================================================

pub struct Mbc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    ram_enabled: bool,
    rom_bank: u8,
    ram_bank: u8,
    // Real Time Clock (RTC) registers (simplified: just placeholders)
    rtc_registers: [u8; 5],
}

impl Mbc3 {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        Self {
            rom,
            ram: vec![0; ram_size],
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            rtc_registers: [0; 5],
        }
    }
}

impl Mbc for Mbc3 {
    fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => {
                // Fixed ROM Bank 0
                self.rom.get(address as usize).cloned().unwrap_or(0xFF)
            }
            0x4000..=0x7FFF => {
                // Switchable ROM Bank 01-7F
                let offset = (self.rom_bank as usize) * 0x4000 + (address as usize - 0x4000);
                self.rom.get(offset).cloned().unwrap_or(0xFF)
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return 0xFF;
                }
                match self.ram_bank {
                    0x00..=0x03 => {
                        // RAM Bank 00-03
                        let offset = (self.ram_bank as usize) * 0x2000 + (address as usize - 0xA000);
                        self.ram.get(offset).cloned().unwrap_or(0xFF)
                    }
                    0x08..=0x0C => {
                        // RTC Register
                        self.rtc_registers[(self.ram_bank - 0x08) as usize]
                    }
                    _ => 0xFF,
                }
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                // RAM and Timer Enable
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                // ROM Bank Number (7 bits: 0-127)
                let mut bank = value & 0x7F;
                if bank == 0 {
                    bank = 1;
                }
                self.rom_bank = bank;
            }
            0x4000..=0x5FFF => {
                // RAM Bank Number or RTC Register Select
                self.ram_bank = value;
            }
            0x6000..=0x7FFF => {
                // Latch Clock Data (writing 0 then 1 latches)
                // Simplified: do nothing for now
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return;
                }
                match self.ram_bank {
                    0x00..=0x03 => {
                        let offset = (self.ram_bank as usize) * 0x2000 + (address as usize - 0xA000);
                        if offset < self.ram.len() {
                            self.ram[offset] = value;
                        }
                    }
                    0x08..=0x0C => {
                        self.rtc_registers[(self.ram_bank - 0x08) as usize] = value;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_mbc_read() {
        let rom = vec![0x42; 0x8000];
        let mbc = NoMbc::new(rom, 0);
        assert_eq!(mbc.read(0x0000), 0x42);
        assert_eq!(mbc.read(0x7FFF), 0x42);
        assert_eq!(mbc.read(0xA000), 0xFF); // No RAM
    }

    #[test]
    fn test_mbc1_rom_banking() {
        // Create a 64KB ROM (4 banks)
        let mut rom = vec![0; 0x10000];
        // Write marker to bank 2 at offset 0x8000 (bank 2 * 0x4000)
        rom[0x8000] = 0xAB;

        let mut mbc = Mbc1::new(rom, 0);

        // Select bank 2
        mbc.write(0x2000, 2);
        assert_eq!(mbc.read(0x4000), 0xAB);
    }

    #[test]
    fn test_mbc1_ram() {
        let rom = vec![0; 0x8000];
        let mut mbc = Mbc1::new(rom, 0x2000);

        // RAM should be disabled by default
        mbc.write(0xA000, 0x42);
        assert_eq!(mbc.read(0xA000), 0xFF);

        // Enable RAM
        mbc.write(0x0000, 0x0A);
        mbc.write(0xA000, 0x42);
        assert_eq!(mbc.read(0xA000), 0x42);

        // Disable RAM
        mbc.write(0x0000, 0x00);
        assert_eq!(mbc.read(0xA000), 0xFF);
    }

    #[test]
    fn test_mbc1_bank_zero_never_selected() {
        let rom = vec![0; 0x8000];
        let mut mbc = Mbc1::new(rom, 0);

        // Selecting bank 0 should map to bank 1
        mbc.write(0x2000, 0);
        assert_eq!(mbc.rom_bank, 1);
    }
}
