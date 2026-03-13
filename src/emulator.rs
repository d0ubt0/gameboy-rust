use crate::cpu::Cpu;
use crate::mmu::{Bus, Memory};
use crate::cartridge::Cartridge;
use crate::ppu::SCREEN_WIDTH;
use crate::ppu::SCREEN_HEIGHT;

use std::path::Path;

/// Number of CPU cycles per frame (~70224 cycles at 4.194304 MHz ≈ 59.73 fps)
const CYCLES_PER_FRAME: u32 = 70224;

pub struct GameBoy {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl GameBoy {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            cpu: Cpu::new(),
            bus: Bus::new(sample_rate),
        }
    }

    /// Load a ROM from a file path
    pub fn load_rom_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let cartridge = Cartridge::from_file(path)?;
        self.bus.cartridge = Some(cartridge);
        Ok(())
    }

    /// Load a ROM from raw bytes (for testing)
    pub fn load_rom(&mut self, rom: Vec<u8>) {
        self.bus.cartridge = Some(Cartridge::new(rom));
    }

    /// Run a single CPU instruction and advance all subsystems by the consumed cycles
    pub fn step(&mut self) -> u32 {
        // Execute one CPU instruction, get cycles consumed
        let cycles = self.cpu.step(&mut self.bus);

        // Advance the PPU by the same number of cycles
        self.bus.ppu.step(cycles);

        // Advance the Timer by the same number of T-cycles
        self.bus.timer.step(cycles);

        // Advance the APU by the same number of T-cycles
        self.bus.apu.step(cycles);

        // Tick DMA transfer
        // (In real hardware this is per machine cycle, simplified here)
        for _ in 0..(cycles / 4) {
            self.bus.tick_dma();
        }

        // Collect interrupt flags from PPU
        let ppu_interrupts = self.bus.ppu.take_interrupts();
        self.bus.interrupt_flags |= ppu_interrupts;

        // Collect interrupt flags from Timer
        let timer_interrupts = self.bus.timer.take_interrupts();
        self.bus.interrupt_flags |= timer_interrupts;

        // Collect interrupt flags from Joypad
        let joypad_interrupts = self.bus.joypad.take_interrupts();
        self.bus.interrupt_flags |= joypad_interrupts;

        // Handle CPU interrupts
        self.handle_interrupts();

        cycles
    }

    /// Run the emulator for one complete frame (~70224 cycles)
    pub fn run_frame(&mut self) {
        self.bus.ppu.frame_ready = false;

        let mut total_cycles: u32 = 0;
        while total_cycles < CYCLES_PER_FRAME {
            total_cycles += self.step();

            // Early exit if frame is ready (useful for synchronization)
            if self.bus.ppu.frame_ready {
                break;
            }
        }
    }

    /// Get the current frame buffer for rendering
    pub fn frame_buffer(&self) -> &[u8] {
        &self.bus.ppu.frame_buffer
    }

    /// Get screen dimensions
    pub fn screen_size() -> (usize, usize) {
        (SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    /// Handle pending interrupts
    fn handle_interrupts(&mut self) {
        if !self.cpu.ime && !self.cpu.halted {
            return;
        }

        let pending = self.bus.interrupt_flags & self.bus.interrupt_enable & 0x1F;
        if pending == 0 {
            return;
        }

        // Wake from HALT regardless of IME
        self.cpu.halted = false;

        if !self.cpu.ime {
            return;
        }

        // Disable further interrupts
        self.cpu.ime = false;

        // Find the highest priority interrupt (bit 0 = highest)
        for bit in 0..5 {
            if pending & (1 << bit) != 0 {
                // Clear the interrupt flag
                self.bus.interrupt_flags &= !(1 << bit);

                // Push current PC onto stack
                let pc = self.cpu.registers.pc;
                self.cpu.registers.sp = self.cpu.registers.sp.wrapping_sub(2);
                let sp = self.cpu.registers.sp;
                self.bus.write_word(sp, pc);

                // Jump to interrupt vector
                self.cpu.registers.pc = match bit {
                    0 => 0x0040, // VBlank
                    1 => 0x0048, // LCD STAT
                    2 => 0x0050, // Timer
                    3 => 0x0058, // Serial
                    4 => 0x0060, // Joypad
                    _ => unreachable!(),
                };

                break; // Only service one interrupt at a time
            }
        }
    }
}
