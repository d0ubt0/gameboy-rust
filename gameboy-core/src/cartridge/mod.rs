pub mod mbc;

use crate::mmu::Memory;
use mbc::{Mbc, Mbc1, Mbc3, NoMbc};
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// ROM Header offsets
const TITLE_START: usize = 0x0134;
const TITLE_END: usize = 0x0143;
const CARTRIDGE_TYPE: usize = 0x0147;
const ROM_SIZE: usize = 0x0148;
const RAM_SIZE: usize = 0x0149;
const HEADER_CHECKSUM: usize = 0x014D;

/// ROM header information parsed from the cartridge
#[derive(Debug, Clone)]
pub struct CartridgeHeader {
    pub title: String,
    pub cartridge_type: u8,
    pub rom_size: usize,
    pub ram_size: usize,
    pub header_checksum: u8,
    pub checksum_valid: bool,
}

pub struct Cartridge {
    pub mbc: Box<dyn Mbc>,
    pub header: CartridgeHeader,
}

impl Cartridge {
    /// Load a ROM from a file path (not available on WASM)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let rom_data = fs::read(path.as_ref())
            .map_err(|e| format!("Failed to read ROM file '{}': {}", path.as_ref().display(), e))?;

        Self::from_bytes(rom_data)
    }

    /// Create a cartridge from raw ROM bytes
    pub fn from_bytes(rom: Vec<u8>) -> Result<Self, String> {
        if rom.len() < 0x0150 {
            return Err(format!(
                "ROM too small: {} bytes (minimum 336 bytes for valid header)",
                rom.len()
            ));
        }

        let header = Self::parse_header(&rom)?;

        log::info!("Cartridge loaded:");
        log::info!("  Title: {}", header.title);
        log::info!("  Type: 0x{:02X} ({})", header.cartridge_type, Self::cartridge_type_name(header.cartridge_type));
        log::info!("  ROM: {} KB ({} banks)", header.rom_size / 1024, header.rom_size / 0x4000);
        log::info!("  RAM: {} KB", header.ram_size / 1024);
        log::info!("  Header checksum: {} (0x{:02X})", if header.checksum_valid { "VALID" } else { "INVALID" }, header.header_checksum);

        let mbc: Box<dyn Mbc> = match header.cartridge_type {
            0x00 => {
                // ROM Only
                Box::new(NoMbc::new(rom, header.ram_size))
            }
            0x01 | 0x02 | 0x03 => {
                // MBC1, MBC1+RAM, MBC1+RAM+BATTERY
                Box::new(Mbc1::new(rom, header.ram_size))
            }
            0x0F..=0x13 => {
                // MBC3, MBC3+RAM, etc.
                Box::new(Mbc3::new(rom, header.ram_size))
            }
            other => {
                log::warn!("Unsupported cartridge type: 0x{:02X}, falling back to NoMbc", other);
                Box::new(NoMbc::new(rom, header.ram_size))
            }
        };

        Ok(Self { mbc, header })
    }

    /// Create a minimal cartridge for testing (no header validation)
    pub fn new(rom: Vec<u8>) -> Self {
        let ram_size = 0;
        let header = CartridgeHeader {
            title: String::from("TEST ROM"),
            cartridge_type: 0x00,
            rom_size: rom.len(),
            ram_size,
            header_checksum: 0,
            checksum_valid: false,
        };
        Self {
            mbc: Box::new(NoMbc::new(rom, ram_size)),
            header,
        }
    }

    /// Parse the cartridge header
    fn parse_header(rom: &[u8]) -> Result<CartridgeHeader, String> {
        // Parse title (may contain null terminators)
        let title_bytes = &rom[TITLE_START..=TITLE_END];
        let title = title_bytes
            .iter()
            .take_while(|&&b| b != 0)
            .filter(|&&b| b.is_ascii_graphic() || b == b' ')
            .map(|&b| b as char)
            .collect::<String>();

        let cartridge_type = rom[CARTRIDGE_TYPE];

        // ROM size: 32KB << rom_size_code
        let rom_size_code = rom[ROM_SIZE];
        let rom_size = match rom_size_code {
            0x00..=0x08 => 0x8000 << rom_size_code,
            _ => return Err(format!("Unknown ROM size code: 0x{:02X}", rom_size_code)),
        };

        // RAM size
        let ram_size_code = rom[RAM_SIZE];
        let ram_size = match ram_size_code {
            0x00 => 0,
            0x01 => 0x800,   // 2KB (listed but unused in practice)
            0x02 => 0x2000,  // 8KB
            0x03 => 0x8000,  // 32KB (4 banks of 8KB)
            0x04 => 0x20000, // 128KB (16 banks of 8KB)
            0x05 => 0x10000, // 64KB (8 banks of 8KB)
            _ => {
                log::warn!("Unknown RAM size code: 0x{:02X}, defaulting to 0", ram_size_code);
                0
            }
        };

        // Header checksum
        let header_checksum = rom[HEADER_CHECKSUM];
        let mut checksum: u8 = 0;
        for addr in 0x0134..=0x014C {
            checksum = checksum.wrapping_sub(rom[addr]).wrapping_sub(1);
        }
        let checksum_valid = checksum == header_checksum;

        Ok(CartridgeHeader {
            title,
            cartridge_type,
            rom_size,
            ram_size,
            header_checksum,
            checksum_valid,
        })
    }

    /// Return a human-readable name for the cartridge type
    fn cartridge_type_name(code: u8) -> &'static str {
        match code {
            0x00 => "ROM Only",
            0x01 => "MBC1",
            0x02 => "MBC1+RAM",
            0x03 => "MBC1+RAM+BATTERY",
            0x05 => "MBC2",
            0x06 => "MBC2+BATTERY",
            0x08 => "ROM+RAM",
            0x09 => "ROM+RAM+BATTERY",
            0x0F => "MBC3+TIMER+BATTERY",
            0x10 => "MBC3+TIMER+RAM+BATTERY",
            0x11 => "MBC3",
            0x12 => "MBC3+RAM",
            0x13 => "MBC3+RAM+BATTERY",
            0x19 => "MBC5",
            0x1A => "MBC5+RAM",
            0x1B => "MBC5+RAM+BATTERY",
            _ => "Unknown",
        }
    }
}

impl Memory for Cartridge {
    fn read(&self, address: u16) -> u8 {
        self.mbc.read(address)
    }

    fn write(&mut self, address: u16, value: u8) {
        self.mbc.write(address, value);
    }
}
