pub mod registers;
pub mod instructions;

pub use registers::Registers;
use crate::mmu::Memory;

pub struct Cpu {
    pub registers: Registers,
    pub ime: bool,      // Interrupt Master Enable
    pub halted: bool,
    pub stopped: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            ime: false,
            halted: false,
            stopped: false,
        }
    }

    pub fn step<M: Memory>(&mut self, memory: &mut M) -> u32 {
        if self.halted {
            // Check for interrupts to wake up
            return 4; // Cycles consumed while halted
        }

        let opcode = self.fetch(memory);
        self.execute(opcode, memory)
    }

    fn fetch<M: Memory>(&mut self, memory: &M) -> u8 {
        let opcode = memory.read(self.registers.pc);
        self.registers.pc = self.registers.pc.wrapping_add(1);
        opcode
    }

    fn execute<M: Memory>(&mut self, opcode: u8, memory: &mut M) -> u32 {
        self.execute_instruction(opcode, memory)
    }
}
