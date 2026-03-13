use crate::cpu::Cpu;
use crate::mmu::Memory;

impl Cpu {
    // ALU Helpers
    fn add(&mut self, a: u8, b: u8) -> u8 {
        let res = a.wrapping_add(b);
        self.registers.set_z(res == 0);
        self.registers.set_n(false);
        self.registers.set_h((a & 0x0F) + (b & 0x0F) > 0x0F);
        self.registers.set_c((a as u16) + (b as u16) > 0xFF);
        res
    }

    fn adc(&mut self, a: u8, b: u8) -> u8 {
        let carry = if self.registers.c() { 1 } else { 0 };
        let res = a.wrapping_add(b).wrapping_add(carry);
        self.registers.set_z(res == 0);
        self.registers.set_n(false);
        self.registers.set_h((a & 0x0F) + (b & 0x0F) + carry > 0x0F);
        self.registers.set_c((a as u16) + (b as u16) + (carry as u16) > 0xFF);
        res
    }

    fn sub(&mut self, a: u8, b: u8) -> u8 {
        let res = a.wrapping_sub(b);
        self.registers.set_z(res == 0);
        self.registers.set_n(true);
        self.registers.set_h((a & 0x0F) < (b & 0x0F));
        self.registers.set_c((a as u16) < (b as u16));
        res
    }

    fn sbc(&mut self, a: u8, b: u8) -> u8 {
        let carry = if self.registers.c() { 1 } else { 0 };
        let res = a.wrapping_sub(b).wrapping_sub(carry);
        self.registers.set_z(res == 0);
        self.registers.set_n(true);
        self.registers.set_h((a & 0x0F) < (b & 0x0F) + carry);
        self.registers.set_c((a as u16) < (b as u16) + (carry as u16));
        res
    }

    fn and(&mut self, a: u8, b: u8) -> u8 {
        let res = a & b;
        self.registers.set_z(res == 0);
        self.registers.set_n(false);
        self.registers.set_h(true);
        self.registers.set_c(false);
        res
    }

    fn or(&mut self, a: u8, b: u8) -> u8 {
        let res = a | b;
        self.registers.set_z(res == 0);
        self.registers.set_n(false);
        self.registers.set_h(false);
        self.registers.set_c(false);
        res
    }

    fn xor(&mut self, a: u8, b: u8) -> u8 {
        let res = a ^ b;
        self.registers.set_z(res == 0);
        self.registers.set_n(false);
        self.registers.set_h(false);
        self.registers.set_c(false);
        res
    }

    fn cp(&mut self, a: u8, b: u8) {
        let _ = self.sub(a, b);
    }

    fn inc(&mut self, a: u8) -> u8 {
        let res = a.wrapping_add(1);
        self.registers.set_z(res == 0);
        self.registers.set_n(false);
        self.registers.set_h((a & 0x0F) + 1 > 0x0F);
        // Carry is not affected
        res
    }

    fn dec(&mut self, a: u8) -> u8 {
        let res = a.wrapping_sub(1);
        self.registers.set_z(res == 0);
        self.registers.set_n(true);
        self.registers.set_h((a & 0x0F) == 0);
        // Carry is not affected
        res
    }

    // Stack Helpers
    fn push<M: Memory>(&mut self, value: u16, memory: &mut M) {
        let bytes = value.to_be_bytes(); // High byte first at SP-1
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        memory.write(self.registers.sp, bytes[0]);
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        memory.write(self.registers.sp, bytes[1]);
    }

    fn pop<M: Memory>(&mut self, memory: &M) -> u16 {
        let lo = memory.read(self.registers.sp);
        self.registers.sp = self.registers.sp.wrapping_add(1);
        let hi = memory.read(self.registers.sp);
        self.registers.sp = self.registers.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }

    // CB Helpers
    fn get_reg_val<M: Memory>(&self, reg_idx: u8, memory: &M) -> u8 {
        match reg_idx {
            0 => self.registers.b,
            1 => self.registers.c,
            2 => self.registers.d,
            3 => self.registers.e,
            4 => self.registers.h,
            5 => self.registers.l,
            6 => memory.read(self.registers.hl()),
            7 => self.registers.a,
            _ => unreachable!(),
        }
    }

    fn set_reg_val<M: Memory>(&mut self, reg_idx: u8, val: u8, memory: &mut M) {
        match reg_idx {
            0 => self.registers.b = val,
            1 => self.registers.c = val,
            2 => self.registers.d = val,
            3 => self.registers.e = val,
            4 => self.registers.h = val,
            5 => self.registers.l = val,
            6 => memory.write(self.registers.hl(), val),
            7 => self.registers.a = val,
            _ => unreachable!(),
        }
    }

    pub fn execute_cb<M: Memory>(&mut self, memory: &mut M) -> u32 {
        let opcode = self.fetch(memory);
        let reg_idx = opcode % 8;
        let mut val = self.get_reg_val(reg_idx, memory);
        let mut cycles = if reg_idx == 6 { 16 } else { 8 };

        match opcode {
            0x00..=0x07 => { // RLC
                let carry = (val & 0x80) >> 7;
                val = (val << 1) | carry;
                self.registers.set_z(val == 0);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(carry == 1);
            }
            0x08..=0x0F => { // RRC
                let carry = val & 0x01;
                val = (val >> 1) | (carry << 7);
                self.registers.set_z(val == 0);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(carry == 1);
            }
            0x10..=0x17 => { // RL
                let old_carry = if self.registers.c() { 1 } else { 0 };
                let new_carry = (val & 0x80) >> 7;
                val = (val << 1) | old_carry;
                self.registers.set_z(val == 0);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(new_carry == 1);
            }
            0x18..=0x1F => { // RR
                let old_carry = if self.registers.c() { 0x80 } else { 0 };
                let new_carry = val & 0x01;
                val = (val >> 1) | old_carry;
                self.registers.set_z(val == 0);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(new_carry == 1);
            }
            0x20..=0x27 => { // SLA
                let carry = (val & 0x80) >> 7;
                val <<= 1;
                self.registers.set_z(val == 0);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(carry == 1);
            }
            0x28..=0x2F => { // SRA
                let carry = val & 0x01;
                val = (val as i8 >> 1) as u8;
                self.registers.set_z(val == 0);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(carry == 1);
            }
            0x30..=0x37 => { // SWAP
                val = ((val & 0x0F) << 4) | ((val & 0xF0) >> 4);
                self.registers.set_z(val == 0);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(false);
            }
            0x38..=0x3F => { // SRL
                let carry = val & 0x01;
                val >>= 1;
                self.registers.set_z(val == 0);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(carry == 1);
            }
            0x40..=0x7F => { // BIT
                let bit = (opcode - 0x40) / 8;
                self.registers.set_z((val & (1 << bit)) == 0);
                self.registers.set_n(false);
                self.registers.set_h(true);
                cycles = if reg_idx == 6 { 12 } else { 8 };
                return cycles; // BIT doesn't write back
            }
            0x80..=0xBF => { // RES
                let bit = (opcode - 0x80) / 8;
                val &= !(1 << bit);
            }
            0xC0..=0xFF => { // SET
                let bit = (opcode - 0xC0) / 8;
                val |= 1 << bit;
            }
        }

        self.set_reg_val(reg_idx, val, memory);
        cycles
    }

    pub fn execute_instruction<M: Memory>(&mut self, opcode: u8, memory: &mut M) -> u32 {
        match opcode {
            0x00 => 4, // NOP
            
            // 8-bit Loads LD r, n
            0x06 => { let v = self.fetch(memory); self.registers.b = v; 8 }
            0x0E => { let v = self.fetch(memory); self.registers.c = v; 8 }
            0x16 => { let v = self.fetch(memory); self.registers.d = v; 8 }
            0x1E => { let v = self.fetch(memory); self.registers.e = v; 8 }
            0x26 => { let v = self.fetch(memory); self.registers.h = v; 8 }
            0x2E => { let v = self.fetch(memory); self.registers.l = v; 8 }
            0x36 => { let v = self.fetch(memory); memory.write(self.registers.hl(), v); 12 } // LD (HL), n
            0x3E => { let v = self.fetch(memory); self.registers.a = v; 8 }

            // 8-bit Loads LD r, r'
            0x40 => { 4 } // LD B, B (Nop-like)
            0x41 => { self.registers.b = self.registers.c; 4 }
            0x42 => { self.registers.b = self.registers.d; 4 }
            0x43 => { self.registers.b = self.registers.e; 4 }
            0x44 => { self.registers.b = self.registers.h; 4 }
            0x45 => { self.registers.b = self.registers.l; 4 }
            0x46 => { self.registers.b = memory.read(self.registers.hl()); 8 }
            0x47 => { self.registers.b = self.registers.a; 4 }

            0x48 => { self.registers.c = self.registers.b; 4 }
            0x49 => { 4 } // LD C, C
            0x4A => { self.registers.c = self.registers.d; 4 }
            0x4B => { self.registers.c = self.registers.e; 4 }
            0x4C => { self.registers.c = self.registers.h; 4 }
            0x4D => { self.registers.c = self.registers.l; 4 }
            0x4E => { self.registers.c = memory.read(self.registers.hl()); 8 }
            0x4F => { self.registers.c = self.registers.a; 4 }

            0x50 => { self.registers.d = self.registers.b; 4 }
            0x51 => { self.registers.d = self.registers.c; 4 }
            0x52 => { 4 } // LD D, D
            0x53 => { self.registers.d = self.registers.e; 4 }
            0x54 => { self.registers.d = self.registers.h; 4 }
            0x55 => { self.registers.d = self.registers.l; 4 }
            0x56 => { self.registers.d = memory.read(self.registers.hl()); 8 }
            0x57 => { self.registers.d = self.registers.a; 4 }

            0x58 => { self.registers.e = self.registers.b; 4 }
            0x59 => { self.registers.e = self.registers.c; 4 }
            0x5A => { self.registers.e = self.registers.d; 4 }
            0x5B => { 4 } // LD E, E
            0x5C => { self.registers.e = self.registers.h; 4 }
            0x5D => { self.registers.e = self.registers.l; 4 }
            0x5E => { self.registers.e = memory.read(self.registers.hl()); 8 }
            0x5F => { self.registers.e = self.registers.a; 4 }

            0x60 => { self.registers.h = self.registers.b; 4 }
            0x61 => { self.registers.h = self.registers.c; 4 }
            0x62 => { self.registers.h = self.registers.d; 4 }
            0x63 => { self.registers.h = self.registers.e; 4 }
            0x64 => { 4 } // LD H, H
            0x65 => { self.registers.h = self.registers.l; 4 }
            0x66 => { self.registers.h = memory.read(self.registers.hl()); 8 }
            0x67 => { self.registers.h = self.registers.a; 4 }

            0x68 => { self.registers.l = self.registers.b; 4 }
            0x69 => { self.registers.l = self.registers.c; 4 }
            0x6A => { self.registers.l = self.registers.d; 4 }
            0x6B => { self.registers.l = self.registers.e; 4 }
            0x6C => { self.registers.l = self.registers.h; 4 }
            0x6D => { 4 } // LD L, L
            0x6E => { self.registers.l = memory.read(self.registers.hl()); 8 }
            0x6F => { self.registers.l = self.registers.a; 4 }

            0x70 => { memory.write(self.registers.hl(), self.registers.b); 8 }
            0x71 => { memory.write(self.registers.hl(), self.registers.c); 8 }
            0x72 => { memory.write(self.registers.hl(), self.registers.d); 8 }
            0x73 => { memory.write(self.registers.hl(), self.registers.e); 8 }
            0x74 => { memory.write(self.registers.hl(), self.registers.h); 8 }
            0x75 => { memory.write(self.registers.hl(), self.registers.l); 8 }
            0x76 => { self.halted = true; 4 } // HALT
            0x77 => { memory.write(self.registers.hl(), self.registers.a); 8 }

            0x78 => { self.registers.a = self.registers.b; 4 }
            0x79 => { self.registers.a = self.registers.c; 4 }
            0x7A => { self.registers.a = self.registers.d; 4 }
            0x7B => { self.registers.a = self.registers.e; 4 }
            0x7C => { self.registers.a = self.registers.h; 4 }
            0x7D => { self.registers.a = self.registers.l; 4 }
            0x7E => { self.registers.a = memory.read(self.registers.hl()); 8 }
            0x7F => { 4 } // LD A, A

            // Arithmetic A, r
            0x80 => { self.registers.a = self.add(self.registers.a, self.registers.b); 4 }
            0x81 => { self.registers.a = self.add(self.registers.a, self.registers.c); 4 }
            0x82 => { self.registers.a = self.add(self.registers.a, self.registers.d); 4 }
            0x83 => { self.registers.a = self.add(self.registers.a, self.registers.e); 4 }
            0x84 => { self.registers.a = self.add(self.registers.a, self.registers.h); 4 }
            0x85 => { self.registers.a = self.add(self.registers.a, self.registers.l); 4 }
            0x86 => { let v = memory.read(self.registers.hl()); self.registers.a = self.add(self.registers.a, v); 8 }
            0x87 => { self.registers.a = self.add(self.registers.a, self.registers.a); 4 }

            0x88 => { self.registers.a = self.adc(self.registers.a, self.registers.b); 4 }
            0x89 => { self.registers.a = self.adc(self.registers.a, self.registers.c); 4 }
            0x8A => { self.registers.a = self.adc(self.registers.a, self.registers.d); 4 }
            0x8B => { self.registers.a = self.adc(self.registers.a, self.registers.e); 4 }
            0x8C => { self.registers.a = self.adc(self.registers.a, self.registers.h); 4 }
            0x8D => { self.registers.a = self.adc(self.registers.a, self.registers.l); 4 }
            0x8E => { let v = memory.read(self.registers.hl()); self.registers.a = self.adc(self.registers.a, v); 8 }
            0x8F => { self.registers.a = self.adc(self.registers.a, self.registers.a); 4 }

            0x90 => { self.registers.a = self.sub(self.registers.a, self.registers.b); 4 }
            0x91 => { self.registers.a = self.sub(self.registers.a, self.registers.c); 4 }
            0x92 => { self.registers.a = self.sub(self.registers.a, self.registers.d); 4 }
            0x93 => { self.registers.a = self.sub(self.registers.a, self.registers.e); 4 }
            0x94 => { self.registers.a = self.sub(self.registers.a, self.registers.h); 4 }
            0x95 => { self.registers.a = self.sub(self.registers.a, self.registers.l); 4 }
            0x96 => { let v = memory.read(self.registers.hl()); self.registers.a = self.sub(self.registers.a, v); 8 }
            0x97 => { self.registers.a = self.sub(self.registers.a, self.registers.a); 4 }

            0x98 => { self.registers.a = self.sbc(self.registers.a, self.registers.b); 4 }
            0x99 => { self.registers.a = self.sbc(self.registers.a, self.registers.c); 4 }
            0x9A => { self.registers.a = self.sbc(self.registers.a, self.registers.d); 4 }
            0x9B => { self.registers.a = self.sbc(self.registers.a, self.registers.e); 4 }
            0x9C => { self.registers.a = self.sbc(self.registers.a, self.registers.h); 4 }
            0x9D => { self.registers.a = self.sbc(self.registers.a, self.registers.l); 4 }
            0x9E => { let v = memory.read(self.registers.hl()); self.registers.a = self.sbc(self.registers.a, v); 8 }
            0x9F => { self.registers.a = self.sbc(self.registers.a, self.registers.a); 4 }

            0xA0 => { self.registers.a = self.and(self.registers.a, self.registers.b); 4 }
            0xA1 => { self.registers.a = self.and(self.registers.a, self.registers.c); 4 }
            0xA2 => { self.registers.a = self.and(self.registers.a, self.registers.d); 4 }
            0xA3 => { self.registers.a = self.and(self.registers.a, self.registers.e); 4 }
            0xA4 => { self.registers.a = self.and(self.registers.a, self.registers.h); 4 }
            0xA5 => { self.registers.a = self.and(self.registers.a, self.registers.l); 4 }
            0xA6 => { let v = memory.read(self.registers.hl()); self.registers.a = self.and(self.registers.a, v); 8 }
            0xA7 => { self.registers.a = self.and(self.registers.a, self.registers.a); 4 }

            0xA8 => { self.registers.a = self.xor(self.registers.a, self.registers.b); 4 }
            0xA9 => { self.registers.a = self.xor(self.registers.a, self.registers.c); 4 }
            0xAA => { self.registers.a = self.xor(self.registers.a, self.registers.d); 4 }
            0xAB => { self.registers.a = self.xor(self.registers.a, self.registers.e); 4 }
            0xAC => { self.registers.a = self.xor(self.registers.a, self.registers.h); 4 }
            0xAD => { self.registers.a = self.xor(self.registers.a, self.registers.l); 4 }
            0xAE => { let v = memory.read(self.registers.hl()); self.registers.a = self.xor(self.registers.a, v); 8 }
            0xAF => { self.registers.a = self.xor(self.registers.a, self.registers.a); 4 }

            0xB0 => { self.registers.a = self.or(self.registers.a, self.registers.b); 4 }
            0xB1 => { self.registers.a = self.or(self.registers.a, self.registers.c); 4 }
            0xB2 => { self.registers.a = self.or(self.registers.a, self.registers.d); 4 }
            0xB3 => { self.registers.a = self.or(self.registers.a, self.registers.e); 4 }
            0xB4 => { self.registers.a = self.or(self.registers.a, self.registers.h); 4 }
            0xB5 => { self.registers.a = self.or(self.registers.a, self.registers.l); 4 }
            0xB6 => { let v = memory.read(self.registers.hl()); self.registers.a = self.or(self.registers.a, v); 8 }
            0xB7 => { self.registers.a = self.or(self.registers.a, self.registers.a); 4 }

            0xB8 => { self.cp(self.registers.a, self.registers.b); 4 }
            0xB9 => { self.cp(self.registers.a, self.registers.c); 4 }
            0xBA => { self.cp(self.registers.a, self.registers.d); 4 }
            0xBB => { self.cp(self.registers.a, self.registers.e); 4 }
            0xBC => { self.cp(self.registers.a, self.registers.h); 4 }
            0xBD => { self.cp(self.registers.a, self.registers.l); 4 }
            0xBE => { let v = memory.read(self.registers.hl()); self.cp(self.registers.a, v); 8 }
            0xBF => { self.cp(self.registers.a, self.registers.a); 4 }

            // Arithmetic A, n
            0xC6 => { let v = self.fetch(memory); self.registers.a = self.add(self.registers.a, v); 8 }
            0xCE => { let v = self.fetch(memory); self.registers.a = self.adc(self.registers.a, v); 8 }
            0xD6 => { let v = self.fetch(memory); self.registers.a = self.sub(self.registers.a, v); 8 }
            0xDE => { let v = self.fetch(memory); self.registers.a = self.sbc(self.registers.a, v); 8 }
            0xE6 => { let v = self.fetch(memory); self.registers.a = self.and(self.registers.a, v); 8 }
            0xEE => { let v = self.fetch(memory); self.registers.a = self.xor(self.registers.a, v); 8 }
            0xF6 => { let v = self.fetch(memory); self.registers.a = self.or(self.registers.a, v); 8 }
            0xFE => { let v = self.fetch(memory); self.cp(self.registers.a, v); 8 }

            // INC/DEC r
            0x04 => { self.registers.b = self.inc(self.registers.b); 4 }
            0x05 => { self.registers.b = self.dec(self.registers.b); 4 }
            0x0C => { self.registers.c = self.inc(self.registers.c); 4 }
            0x0D => { self.registers.c = self.dec(self.registers.c); 4 }
            0x14 => { self.registers.d = self.inc(self.registers.d); 4 }
            0x15 => { self.registers.d = self.dec(self.registers.d); 4 }
            0x1C => { self.registers.e = self.inc(self.registers.e); 4 }
            0x1D => { self.registers.e = self.dec(self.registers.e); 4 }
            0x24 => { self.registers.h = self.inc(self.registers.h); 4 }
            0x25 => { self.registers.h = self.dec(self.registers.h); 4 }
            0x2C => { self.registers.l = self.inc(self.registers.l); 4 }
            0x2D => { self.registers.l = self.dec(self.registers.l); 4 }
            0x34 => { let hl = self.registers.hl(); let v = self.inc(memory.read(hl)); memory.write(hl, v); 12 }
            0x35 => { let hl = self.registers.hl(); let v = self.dec(memory.read(hl)); memory.write(hl, v); 12 }
            0x3C => { self.registers.a = self.inc(self.registers.a); 4 }
            0x3D => { self.registers.a = self.dec(self.registers.a); 4 }

            // Special Loads
            0x02 => { memory.write(self.registers.bc(), self.registers.a); 8 } // LD (BC), A
            0x0A => { self.registers.a = memory.read(self.registers.bc()); 8 } // LD A, (BC)
            0x12 => { memory.write(self.registers.de(), self.registers.a); 8 } // LD (DE), A
            0x1A => { self.registers.a = memory.read(self.registers.de()); 8 } // LD A, (DE)
            
            0x22 => { // LD (HL+), A
                let hl = self.registers.hl();
                memory.write(hl, self.registers.a);
                self.registers.set_hl(hl.wrapping_add(1));
                8
            }
            0x2A => { // LD A, (HL+)
                let hl = self.registers.hl();
                self.registers.a = memory.read(hl);
                self.registers.set_hl(hl.wrapping_add(1));
                8
            }
            0x32 => { // LD (HL-), A
                let hl = self.registers.hl();
                memory.write(hl, self.registers.a);
                self.registers.set_hl(hl.wrapping_sub(1));
                8
            }
            0x3A => { // LD A, (HL-)
                let hl = self.registers.hl();
                self.registers.a = memory.read(hl);
                self.registers.set_hl(hl.wrapping_sub(1));
                8
            }

            0xE0 => { let n = self.fetch(memory); memory.write(0xFF00 + n as u16, self.registers.a); 12 } // LDH (n), A
            0xF0 => { let n = self.fetch(memory); self.registers.a = memory.read(0xFF00 + n as u16); 12 } // LDH A, (n)
            0xE2 => { memory.write(0xFF00 + self.registers.c as u16, self.registers.a); 8 } // LD (C), A
            0xF2 => { self.registers.a = memory.read(0xFF00 + self.registers.c as u16); 8 } // LD A, (C)
            
            0xEA => { // LD (nn), A
                let lo = self.fetch(memory);
                let hi = self.fetch(memory);
                memory.write(u16::from_le_bytes([lo, hi]), self.registers.a);
                16
            }
            0xFA => { // LD A, (nn)
                let lo = self.fetch(memory);
                let hi = self.fetch(memory);
                self.registers.a = memory.read(u16::from_le_bytes([lo, hi]));
                16
            }

            // 16-bit Loads
            0x01 => { let lo = self.fetch(memory); let hi = self.fetch(memory); self.registers.set_bc(u16::from_le_bytes([lo, hi])); 12 } // LD BC, nn
            0x11 => { let lo = self.fetch(memory); let hi = self.fetch(memory); self.registers.set_de(u16::from_le_bytes([lo, hi])); 12 } // LD DE, nn
            0x21 => { let lo = self.fetch(memory); let hi = self.fetch(memory); self.registers.set_hl(u16::from_le_bytes([lo, hi])); 12 } // LD HL, nn
            0x31 => { let lo = self.fetch(memory); let hi = self.fetch(memory); self.registers.sp = u16::from_le_bytes([lo, hi]); 12 } // LD SP, nn
            
            0x08 => { // LD (nn), SP
                let lo = self.fetch(memory);
                let hi = self.fetch(memory);
                let addr = u16::from_le_bytes([lo, hi]);
                let sp_bytes = self.registers.sp.to_le_bytes();
                memory.write(addr, sp_bytes[0]);
                memory.write(addr + 1, sp_bytes[1]);
                20
            }
            0xF8 => { // LD HL, SP+n
                let n = self.fetch(memory) as i8 as i16 as u16;
                let sp = self.registers.sp;
                let res = sp.wrapping_add(n);
                self.registers.set_hl(res);
                self.registers.set_z(false);
                self.registers.set_n(false);
                self.registers.set_h((sp & 0x0F) + (n & 0x0F) > 0x0F);
                self.registers.set_c((sp & 0xFF) + (n & 0xFF) > 0xFF);
                12
            }
            0xF9 => { self.registers.sp = self.registers.hl(); 8 } // LD SP, HL

            0xC5 => { self.push(self.registers.bc(), memory); 16 } // PUSH BC
            0xD5 => { self.push(self.registers.de(), memory); 16 } // PUSH DE
            0xE5 => { self.push(self.registers.hl(), memory); 16 } // PUSH HL
            0xF5 => { self.push(self.registers.af(), memory); 16 } // PUSH AF
            
            0xC1 => { let v = self.pop(memory); self.registers.set_bc(v); 12 } // POP BC
            0xD1 => { let v = self.pop(memory); self.registers.set_de(v); 12 } // POP DE
            0xE1 => { let v = self.pop(memory); self.registers.set_hl(v); 12 } // POP HL
            0xF1 => { let v = self.pop(memory); self.registers.set_af(v); 12 } // POP AF

            // 16-bit Arithmetic
            0x03 => { self.registers.set_bc(self.registers.bc().wrapping_add(1)); 8 } // INC BC
            0x13 => { self.registers.set_de(self.registers.de().wrapping_add(1)); 8 } // INC DE
            0x23 => { self.registers.set_hl(self.registers.hl().wrapping_add(1)); 8 } // INC HL
            0x33 => { self.registers.sp = self.registers.sp.wrapping_add(1); 8 } // INC SP
            
            0x0B => { self.registers.set_bc(self.registers.bc().wrapping_sub(1)); 8 } // DEC BC
            0x1B => { self.registers.set_de(self.registers.de().wrapping_sub(1)); 8 } // DEC DE
            0x2B => { self.registers.set_hl(self.registers.hl().wrapping_sub(1)); 8 } // DEC HL
            0x3B => { self.registers.sp = self.registers.sp.wrapping_sub(1); 8 } // DEC SP

            0x09 => { // ADD HL, BC
                let hl = self.registers.hl();
                let bc = self.registers.bc();
                let res = hl.wrapping_add(bc);
                self.registers.set_hl(res);
                self.registers.set_n(false);
                self.registers.set_h((hl & 0x0FFF) + (bc & 0x0FFF) > 0x0FFF);
                self.registers.set_c((hl as u32) + (bc as u32) > 0xFFFF);
                8
            }
            0x19 => { // ADD HL, DE
                let hl = self.registers.hl();
                let de = self.registers.de();
                let res = hl.wrapping_add(de);
                self.registers.set_hl(res);
                self.registers.set_n(false);
                self.registers.set_h((hl & 0x0FFF) + (de & 0x0FFF) > 0x0FFF);
                self.registers.set_c((hl as u32) + (de as u32) > 0xFFFF);
                8
            }
            0x29 => { // ADD HL, HL
                let hl = self.registers.hl();
                let res = hl.wrapping_add(hl);
                self.registers.set_hl(res);
                self.registers.set_n(false);
                self.registers.set_h((hl & 0x0FFF) + (hl & 0x0FFF) > 0x0FFF);
                self.registers.set_c((hl as u32) + (hl as u32) > 0xFFFF);
                8
            }
            0x39 => { // ADD HL, SP
                let hl = self.registers.hl();
                let sp = self.registers.sp;
                let res = hl.wrapping_add(sp);
                self.registers.set_hl(res);
                self.registers.set_n(false);
                self.registers.set_h((hl & 0x0FFF) + (sp & 0x0FFF) > 0x0FFF);
                self.registers.set_c((hl as u32) + (sp as u32) > 0xFFFF);
                8
            }

            0xE8 => { // ADD SP, n
                let n = self.fetch(memory) as i8 as i16 as u16;
                let sp = self.registers.sp;
                let res = sp.wrapping_add(n);
                self.registers.sp = res;
                self.registers.set_z(false);
                self.registers.set_n(false);
                self.registers.set_h((sp & 0x0F) + (n & 0x0F) > 0x0F);
                self.registers.set_c((sp & 0xFF) + (n & 0xFF) > 0xFF);
                16
            }

            // Control Flow
            0xC3 => { // JP nn
                let lo = self.fetch(memory);
                let hi = self.fetch(memory);
                self.registers.pc = u16::from_le_bytes([lo, hi]);
                16
            }
            0xC2 => { // JP NZ, nn
                let lo = self.fetch(memory); let hi = self.fetch(memory);
                if !self.registers.z() { self.registers.pc = u16::from_le_bytes([lo, hi]); 16 } else { 12 }
            }
            0xCA => { // JP Z, nn
                let lo = self.fetch(memory); let hi = self.fetch(memory);
                if self.registers.z() { self.registers.pc = u16::from_le_bytes([lo, hi]); 16 } else { 12 }
            }
            0xD2 => { // JP NC, nn
                let lo = self.fetch(memory); let hi = self.fetch(memory);
                if !self.registers.c() { self.registers.pc = u16::from_le_bytes([lo, hi]); 16 } else { 12 }
            }
            0xDA => { // JP C, nn
                let lo = self.fetch(memory); let hi = self.fetch(memory);
                if self.registers.c() { self.registers.pc = u16::from_le_bytes([lo, hi]); 16 } else { 12 }
            }
            0xE9 => { self.registers.pc = self.registers.hl(); 4 } // JP HL

            0x18 => { // JR n
                let n = self.fetch(memory) as i8;
                self.registers.pc = self.registers.pc.wrapping_add(n as i16 as u16);
                12
            }
            0x20 => { // JR NZ, n
                let n = self.fetch(memory) as i8;
                if !self.registers.z() { self.registers.pc = self.registers.pc.wrapping_add(n as i16 as u16); 12 } else { 8 }
            }
            0x28 => { // JR Z, n
                let n = self.fetch(memory) as i8;
                if self.registers.z() { self.registers.pc = self.registers.pc.wrapping_add(n as i16 as u16); 12 } else { 8 }
            }
            0x30 => { // JR NC, n
                let n = self.fetch(memory) as i8;
                if !self.registers.c() { self.registers.pc = self.registers.pc.wrapping_add(n as i16 as u16); 12 } else { 8 }
            }
            0x38 => { // JR C, n
                let n = self.fetch(memory) as i8;
                if self.registers.c() { self.registers.pc = self.registers.pc.wrapping_add(n as i16 as u16); 12 } else { 8 }
            }

            0xCD => { // CALL nn
                let lo = self.fetch(memory);
                let hi = self.fetch(memory);
                let addr = u16::from_le_bytes([lo, hi]);
                self.push(self.registers.pc, memory);
                self.registers.pc = addr;
                24
            }
            0xC4 => { // CALL NZ, nn
                let lo = self.fetch(memory); let hi = self.fetch(memory); let addr = u16::from_le_bytes([lo, hi]);
                if !self.registers.z() { self.push(self.registers.pc, memory); self.registers.pc = addr; 24 } else { 12 }
            }
            0xCC => { // CALL Z, nn
                let lo = self.fetch(memory); let hi = self.fetch(memory); let addr = u16::from_le_bytes([lo, hi]);
                if self.registers.z() { self.push(self.registers.pc, memory); self.registers.pc = addr; 24 } else { 12 }
            }
            0xD4 => { // CALL NC, nn
                let lo = self.fetch(memory); let hi = self.fetch(memory); let addr = u16::from_le_bytes([lo, hi]);
                if !self.registers.c() { self.push(self.registers.pc, memory); self.registers.pc = addr; 24 } else { 12 }
            }
            0xDC => { // CALL C, nn
                let lo = self.fetch(memory); let hi = self.fetch(memory); let addr = u16::from_le_bytes([lo, hi]);
                if self.registers.c() { self.push(self.registers.pc, memory); self.registers.pc = addr; 24 } else { 12 }
            }

            0xC9 => { self.registers.pc = self.pop(memory); 16 } // RET
            0xC0 => { if !self.registers.z() { self.registers.pc = self.pop(memory); 20 } else { 8 } } // RET NZ
            0xC8 => { if self.registers.z() { self.registers.pc = self.pop(memory); 20 } else { 8 } } // RET Z
            0xD0 => { if !self.registers.c() { self.registers.pc = self.pop(memory); 20 } else { 8 } } // RET NC
            0xD8 => { if self.registers.c() { self.registers.pc = self.pop(memory); 20 } else { 8 } } // RET C
            0xD9 => { self.registers.pc = self.pop(memory); self.ime = true; 16 } // RETI

            0xC7 => { self.push(self.registers.pc, memory); self.registers.pc = 0x00; 16 } // RST 00H
            0xCF => { self.push(self.registers.pc, memory); self.registers.pc = 0x08; 16 } // RST 08H
            0xD7 => { self.push(self.registers.pc, memory); self.registers.pc = 0x10; 16 } // RST 10H
            0xDF => { self.push(self.registers.pc, memory); self.registers.pc = 0x18; 16 } // RST 18H
            0xE7 => { self.push(self.registers.pc, memory); self.registers.pc = 0x20; 16 } // RST 20H
            0xEF => { self.push(self.registers.pc, memory); self.registers.pc = 0x28; 16 } // RST 28H
            0xF7 => { self.push(self.registers.pc, memory); self.registers.pc = 0x30; 16 } // RST 30H
            0xFF => { self.push(self.registers.pc, memory); self.registers.pc = 0x38; 16 } // RST 38H

            // Miscellaneous
            0x27 => { // DAA
                let mut a = self.registers.a as u16;
                if !self.registers.n() {
                    if self.registers.h_flag() || (a & 0x0F) > 0x09 { a += 0x06; }
                    if self.registers.c() || a > 0x9F { a += 0x60; self.registers.set_c(true); }
                } else {
                    if self.registers.h_flag() { a = a.wrapping_sub(0x06); }
                    if self.registers.c() { a = a.wrapping_sub(0x60); }
                }
                self.registers.a = a as u8;
                self.registers.set_z(self.registers.a == 0);
                self.registers.set_h(false);
                4
            }
            0x2F => { self.registers.a = !self.registers.a; self.registers.set_n(true); self.registers.set_h(true); 4 } // CPL
            0x37 => { self.registers.set_n(false); self.registers.set_h(false); self.registers.set_c(true); 4 } // SCF
            0x3F => { self.registers.set_n(false); self.registers.set_h(false); self.registers.set_c(!self.registers.c()); 4 } // CCF
            0xF3 => { self.ime = false; 4 } // DI
            0xFB => { self.ime = true; 4 } // EI
            
            // Rotates - A
            0x07 => { // RLCA
                let bit = (self.registers.a & 0x80) >> 7;
                self.registers.a = (self.registers.a << 1) | bit;
                self.registers.set_z(false);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(bit == 1);
                4
            }
            0x17 => { // RLA
                let carry = if self.registers.c() { 1 } else { 0 };
                let bit = (self.registers.a & 0x80) >> 7;
                self.registers.a = (self.registers.a << 1) | carry;
                self.registers.set_z(false);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(bit == 1);
                4
            }
            0x0F => { // RRCA
                let bit = self.registers.a & 0x01;
                self.registers.a = (self.registers.a >> 1) | (bit << 7);
                self.registers.set_z(false);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(bit == 1);
                4
            }
            0x1F => { // RRA
                let carry = if self.registers.c() { 1 } else { 0 };
                let bit = self.registers.a & 0x01;
                self.registers.a = (self.registers.a >> 1) | (carry << 7);
                self.registers.set_z(false);
                self.registers.set_n(false);
                self.registers.set_h(false);
                self.registers.set_c(bit == 1);
                4
            }

            0xCB => self.execute_cb(memory),
            
            _ => {
                // Log unimplemented opcode
                4
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::Cpu;

    struct MockMemory {
        pub data: [u8; 0x10000],
    }

    impl MockMemory {
        fn new() -> Self {
            Self { data: [0; 0x10000] }
        }
    }

    impl Memory for MockMemory {
        fn read(&self, address: u16) -> u8 {
            self.data[address as usize]
        }
        fn write(&mut self, address: u16, value: u8) {
            self.data[address as usize] = value;
        }
    }

    #[test]
    fn test_ld_r_n() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.pc = 0x0000;
        mem.write(0x0000, 0x06); // LD B, 0x42
        mem.write(0x0001, 0x42);
        
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.b, 0x42);
        assert_eq!(cpu.registers.pc, 0x0002);
    }

    #[test]
    fn test_add_a_r() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.a = 0x0F;
        cpu.registers.b = 0x01;
        cpu.registers.pc = 0x0000;
        mem.write(0x0000, 0x80); // ADD A, B
        
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.a, 0x10);
        assert_eq!(cpu.registers.z(), false);
        assert_eq!(cpu.registers.n(), false);
        assert_eq!(cpu.registers.h_flag(), true);
        assert_eq!(cpu.registers.c(), false);
    }

    #[test]
    fn test_add_a_r_carry() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.a = 0xFF;
        cpu.registers.c = 0x01;
        cpu.registers.pc = 0x0000;
        mem.write(0x0000, 0x81); // ADD A, C
        
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.a, 0x00);
        assert_eq!(cpu.registers.z(), true);
        assert_eq!(cpu.registers.c(), true);
        assert_eq!(cpu.registers.h_flag(), true);
    }

    #[test]
    fn test_sub_a_r() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.a = 0x10;
        cpu.registers.b = 0x01;
        mem.write(0x0000, 0x90); // SUB A, B
        cpu.registers.pc = 0x0000;

        cpu.step(&mut mem);
        assert_eq!(cpu.registers.a, 0x0F);
        assert_eq!(cpu.registers.z(), false);
        assert_eq!(cpu.registers.n(), true);
        assert_eq!(cpu.registers.h_flag(), true);
        assert_eq!(cpu.registers.c(), false);
    }

    #[test]
    fn test_daa() {
        // Test case 1: 0x45 + 0x38 = 0x83 in BCD
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.a = 0x45;
        cpu.registers.b = 0x38;
        mem.write(0x0000, 0x80); // ADD A, B -> A=0x7D, H=0, C=0
        mem.write(0x0001, 0x27); // DAA -> A=0x83, C=0
        
        cpu.registers.pc = 0x0000;
        cpu.step(&mut mem); // ADD
        cpu.step(&mut mem); // DAA
        
        assert_eq!(cpu.registers.a, 0x83);
        assert_eq!(cpu.registers.c(), false);
        
        // Test case 2: 0x83 - 0x38 = 0x45 in BCD
        cpu.registers.a = 0x83;
        cpu.registers.b = 0x38;
        mem.write(0x0002, 0x90); // SUB A, B -> A=0x4B, N=1, H=1
        mem.write(0x0003, 0x27); // DAA -> A=0x45
        
        cpu.registers.pc = 0x0002;
        cpu.step(&mut mem); // SUB
        cpu.step(&mut mem); // DAA
        
        assert_eq!(cpu.registers.a, 0x45);
    }

    #[test]
    fn test_cb_rlc() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.a = 0x80;
        mem.write(0x0000, 0xCB);
        mem.write(0x0001, 0x07); // RLC A
        
        cpu.registers.pc = 0x0000;
        cpu.step(&mut mem);
        
        assert_eq!(cpu.registers.a, 0x01);
        assert_eq!(cpu.registers.c(), true);
        assert_eq!(cpu.registers.z(), false);
    }

    #[test]
    fn test_inc_dec() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.b = 0x0F;
        mem.write(0x0000, 0x04); // INC B
        cpu.registers.pc = 0x0000;
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.b, 0x10);
        assert_eq!(cpu.registers.h_flag(), true);
        
        mem.write(0x0001, 0x05); // DEC B
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.b, 0x0F);
        assert_eq!(cpu.registers.h_flag(), true);
        assert_eq!(cpu.registers.n(), true);
    }

    #[test]
    fn test_push_pop() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.sp = 0xFFFE;
        cpu.registers.set_bc(0x1234);
        mem.write(0x0000, 0xC5); // PUSH BC
        mem.write(0x0001, 0xD1); // POP DE
        
        cpu.registers.pc = 0x0000;
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.sp, 0xFFFC);
        assert_eq!(mem.read(0xFFFD), 0x12);
        assert_eq!(mem.read(0xFFFC), 0x34);
        
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.de(), 0x1234);
        assert_eq!(cpu.registers.sp, 0xFFFE);
    }

    #[test]
    fn test_jr() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.pc = 0x1000;
        mem.write(0x1000, 0x18); // JR -2
        mem.write(0x1001, 0xFE as u8); // -2 in two's complement
        
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.pc, 0x1000);
    }

    #[test]
    fn test_call_ret() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.sp = 0xFFFE;
        cpu.registers.pc = 0x1000;
        mem.write(0x1000, 0xCD); // CALL 0x2000
        mem.write(0x1001, 0x00);
        mem.write(0x1002, 0x20);
        
        mem.write(0x2000, 0xC9); // RET
        
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.pc, 0x2000);
        assert_eq!(cpu.registers.sp, 0xFFFC);
        
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.pc, 0x1003);
        assert_eq!(cpu.registers.sp, 0xFFFE);
    }

    #[test]
    fn test_add_sp_n() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.sp = 0x000F;
        mem.write(0x0000, 0xE8); // ADD SP, 1
        mem.write(0x0001, 0x01);
        
        cpu.registers.pc = 0x0000;
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.sp, 0x0010);
        assert_eq!(cpu.registers.h_flag(), true);
        assert_eq!(cpu.registers.c(), false);
        
        cpu.registers.sp = 0x00FF;
        mem.write(0x0002, 0xE8); // ADD SP, 1
        mem.write(0x0003, 0x01);
        cpu.registers.pc = 0x0002;
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.sp, 0x0100);
        assert_eq!(cpu.registers.c(), true);
    }

    #[test]
    fn test_bit_ops() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();
        
        cpu.registers.b = 0x80;
        mem.write(0x0000, 0xCB);
        mem.write(0x0001, 0x78); // BIT 7, B
        
        cpu.registers.pc = 0x0000;
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.z(), false);
        assert_eq!(cpu.registers.h_flag(), true);
        assert_eq!(cpu.registers.n(), false);
        
        cpu.registers.b = 0x7F;
        cpu.registers.pc = 0x0000;
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.z(), true);
    }

    #[test]
    fn test_adc_sbc_complex() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();

        // ADC: 0x01 + 0x01 + Carry(1) = 0x03
        cpu.registers.a = 0x01;
        cpu.registers.b = 0x01;
        cpu.registers.set_c(true);
        mem.write(0x0000, 0x88); // ADC A, B
        cpu.registers.pc = 0x0000;
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.a, 0x03);
        assert_eq!(cpu.registers.c(), false);

        // SBC: 0x01 - 0x01 - Carry(1) = 0xFF
        cpu.registers.a = 0x01;
        cpu.registers.b = 0x01;
        cpu.registers.set_c(true);
        mem.write(0x0001, 0x98); // SBC A, B
        cpu.registers.pc = 0x0001;
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.a, 0xFF);
        assert_eq!(cpu.registers.c(), true);
        assert_eq!(cpu.registers.h_flag(), true);
    }

    #[test]
    fn test_ld_hl_sp_n() {
        let mut cpu = Cpu::new();
        let mut mem = MockMemory::new();

        cpu.registers.sp = 0x000F;
        mem.write(0x0000, 0xF8); // LD HL, SP+1
        mem.write(0x0001, 0x01);
        cpu.registers.pc = 0x0000;
        cpu.step(&mut mem);
        assert_eq!(cpu.registers.hl(), 0x0010);
        assert_eq!(cpu.registers.h_flag(), true);
        assert_eq!(cpu.registers.c(), false);
        assert_eq!(cpu.registers.z(), false);
    }
}
