pub struct Registers {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: u8, // Flags: Z, N, H, C, 0, 0, 0, 0
    pub h: u8,
    pub l: u8,
    pub pc: u16, // Program Counter
    pub sp: u16, // Stack Pointer
}

impl Registers {
    pub fn new() -> Self {
        // Initial values after boot ROM or for DMG
        Self {
            a: 0x01,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            f: 0xB0,
            h: 0x01,
            l: 0x4D,
            pc: 0x0100, // Starts at entry point after boot
            sp: 0xFFFE,
        }
    }

    // 16-bit get/set helpers
    pub fn af(&self) -> u16 { ((self.a as u16) << 8) | (self.f as u16) }
    pub fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = (value & 0xF0) as u8; // Lower 4 bits of F are always 0
    }

    pub fn bc(&self) -> u16 { ((self.b as u16) << 8) | (self.c as u16) }
    pub fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }

    pub fn de(&self) -> u16 { ((self.d as u16) << 8) | (self.e as u16) }
    pub fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = value as u8;
    }

    pub fn hl(&self) -> u16 { ((self.h as u16) << 8) | (self.l as u16) }
    pub fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = value as u8;
    }

    // Flag helpers
    pub fn z(&self) -> bool { (self.f & 0x80) != 0 }
    pub fn n(&self) -> bool { (self.f & 0x40) != 0 }
    pub fn h_flag(&self) -> bool { (self.f & 0x20) != 0 }
    pub fn c(&self) -> bool { (self.f & 0x10) != 0 }

    pub fn set_z(&mut self, value: bool) { if value { self.f |= 0x80 } else { self.f &= !0x80 } }
    pub fn set_n(&mut self, value: bool) { if value { self.f |= 0x40 } else { self.f &= !0x40 } }
    pub fn set_h(&mut self, value: bool) { if value { self.f |= 0x20 } else { self.f &= !0x20 } }
    pub fn set_c(&mut self, value: bool) { if value { self.f |= 0x10 } else { self.f &= !0x10 } }
}

#[derive(Debug, Clone, Copy)]
pub enum Flag {
    Zero = 0x80,
    Subtract = 0x40,
    HalfCarry = 0x20,
    Carry = 0x10,
}
