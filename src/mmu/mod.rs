pub mod bus;

pub use bus::Bus;

pub trait Memory {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);

    /// Read a 16-bit word (little-endian)
    fn read_word(&self, address: u16) -> u16 {
        let lo = self.read(address) as u16;
        let hi = self.read(address.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    /// Write a 16-bit word (little-endian)
    fn write_word(&mut self, address: u16, value: u16) {
        self.write(address, value as u8);
        self.write(address.wrapping_add(1), (value >> 8) as u8);
    }
}
