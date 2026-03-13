use super::Memory;
use crate::cartridge::Cartridge;
use crate::joypad::Joypad;
use crate::ppu::Ppu;
use crate::apu::Apu;
use crate::timer::Timer;

pub struct Bus {
    pub cartridge: Option<Cartridge>,
    pub ppu: Ppu,
    pub apu: Apu,
    pub timer: Timer,
    pub wram: [u8; 0x2000],      // 8KB Work RAM (0xC000 - 0xDFFF)
    pub hram: [u8; 0x7F],        // 127B High RAM (0xFF80 - 0xFFFE)

    // Interrupt registers
    pub interrupt_enable: u8,     // 0xFFFF - IE (Interrupt Enable)
    pub interrupt_flags: u8,      // 0xFF0F - IF (Interrupt Flags)

    // I/O registers that don't belong to a specific subsystem yet
    pub serial_data: u8,          // 0xFF01 - SB
    pub serial_control: u8,       // 0xFF02 - SC

    // Joypad
    pub joypad: Joypad,

    // DMA state
    pub dma_active: bool,
    pub dma_source: u16,
    pub dma_offset: u8,
}

impl Bus {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            cartridge: None,
            ppu: Ppu::new(),
            apu: Apu::new(sample_rate),
            timer: Timer::new(),
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            interrupt_enable: 0x00,
            interrupt_flags: 0xE1,   // Post-boot value
            serial_data: 0x00,
            serial_control: 0x7E,
            joypad: Joypad::new(),
            dma_active: false,
            dma_source: 0,
            dma_offset: 0,
        }
    }

    /// Start an OAM DMA transfer
    fn start_dma(&mut self, value: u8) {
        self.dma_source = (value as u16) << 8;
        self.dma_active = true;
        self.dma_offset = 0;
    }

    /// Tick the DMA transfer (called each machine cycle)
    pub fn tick_dma(&mut self) {
        if !self.dma_active {
            return;
        }

        if self.dma_offset < 0xA0 {
            let source_addr = self.dma_source + self.dma_offset as u16;
            let byte = self.dma_read(source_addr);
            self.ppu.oam[self.dma_offset as usize] = byte;
            self.dma_offset += 1;
        }

        if self.dma_offset >= 0xA0 {
            self.dma_active = false;
        }
    }

    /// Read function used during DMA (bypasses OAM)
    fn dma_read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x7FFF => {
                if let Some(cart) = &self.cartridge {
                    cart.read(address)
                } else {
                    0xFF
                }
            }
            0x8000..=0x9FFF => {
                self.ppu.vram[(address - 0x8000) as usize]
            }
            0xC000..=0xDFFF => {
                self.wram[(address - 0xC000) as usize]
            }
            _ => 0xFF,
        }
    }
}

impl Memory for Bus {
    fn read(&self, address: u16) -> u8 {
        match address {
            // ROM + External RAM
            0x0000..=0x7FFF => {
                if let Some(cart) = &self.cartridge {
                    cart.read(address)
                } else {
                    0xFF
                }
            }
            // VRAM
            0x8000..=0x9FFF => {
                self.ppu.vram[(address - 0x8000) as usize]
            }
            // External RAM
            0xA000..=0xBFFF => {
                if let Some(cart) = &self.cartridge {
                    cart.read(address)
                } else {
                    0xFF
                }
            }
            // WRAM
            0xC000..=0xDFFF => {
                self.wram[(address - 0xC000) as usize]
            }
            // Echo RAM
            0xE000..=0xFDFF => {
                self.wram[(address - 0xE000) as usize]
            }
            // OAM
            0xFE00..=0xFE9F => {
                self.ppu.oam[(address - 0xFE00) as usize]
            }
            // Not Usable
            0xFEA0..=0xFEFF => {
                0x00
            }
            // I/O Registers
            0xFF00..=0xFF7F => {
                self.read_io(address)
            }
            // HRAM
            0xFF80..=0xFFFE => {
                self.hram[(address - 0xFF80) as usize]
            }
            // Interrupt Enable
            0xFFFF => {
                self.interrupt_enable
            }
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        match address {
            // ROM / MBC registers
            0x0000..=0x7FFF => {
                if let Some(cart) = &mut self.cartridge {
                    cart.write(address, value);
                }
            }
            // VRAM
            0x8000..=0x9FFF => {
                self.ppu.vram[(address - 0x8000) as usize] = value;
            }
            // External RAM
            0xA000..=0xBFFF => {
                if let Some(cart) = &mut self.cartridge {
                    cart.write(address, value);
                }
            }
            // WRAM
            0xC000..=0xDFFF => {
                self.wram[(address - 0xC000) as usize] = value;
            }
            // Echo RAM
            0xE000..=0xFDFF => {
                self.wram[(address - 0xE000) as usize] = value;
            }
            // OAM
            0xFE00..=0xFE9F => {
                self.ppu.oam[(address - 0xFE00) as usize] = value;
            }
            // Not Usable
            0xFEA0..=0xFEFF => {}
            // I/O Registers
            0xFF00..=0xFF7F => {
                self.write_io(address, value);
            }
            // HRAM
            0xFF80..=0xFFFE => {
                self.hram[(address - 0xFF80) as usize] = value;
            }
            // Interrupt Enable
            0xFFFF => {
                self.interrupt_enable = value;
            }
        }
    }
}

impl Bus {
    /// Read an I/O register
    fn read_io(&self, address: u16) -> u8 {
        match address {
            // Joypad
            0xFF00 => self.joypad.read(),

            // Serial
            0xFF01 => self.serial_data,
            0xFF02 => self.serial_control,

            // Timer
            0xFF04 => self.timer.read_div(),
            0xFF05 => self.timer.read_tima(),
            0xFF06 => self.timer.read_tma(),
            0xFF07 => self.timer.read_tac(),

            // Interrupt Flags
            0xFF0F => self.interrupt_flags | 0xE0, // Upper 3 bits always 1

            // Sound (APU registers + wave RAM)
            0xFF10..=0xFF3F => self.apu.read_register(address),

            // DMA register (write-only, reads return last written value)
            0xFF46 => (self.dma_source >> 8) as u8,

            // PPU registers
            0xFF40..=0xFF4B => self.ppu.read_register(address),

            // Unused I/O
            _ => 0xFF,
        }
    }

    /// Write an I/O register
    fn write_io(&mut self, address: u16, value: u8) {
        match address {
            // Joypad
            0xFF00 => {
                self.joypad.write(value);
            }

            // Serial
            0xFF01 => self.serial_data = value,
            0xFF02 => {
                self.serial_control = value;
                // If transfer is started (bit 7), print the byte for Blargg test ROMs
                if value & 0x80 != 0 {
                    // Blargg serial output — useful for test ROMs
                    //print!("{}", self.serial_data as char);
                }
            }

            // Timer
            0xFF04 => self.timer.write_div(),
            0xFF05 => self.timer.write_tima(value),
            0xFF06 => self.timer.write_tma(value),
            0xFF07 => self.timer.write_tac(value),

            // Interrupt Flags
            0xFF0F => self.interrupt_flags = value & 0x1F,

            // Sound (APU registers + wave RAM)
            0xFF10..=0xFF3F => self.apu.write_register(address, value),

            // PPU registers
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B => {
                self.ppu.write_register(address, value);
            }

            // DMA
            0xFF46 => {
                self.start_dma(value);
            }

            // Unused I/O
            _ => {}
        }
    }
}
