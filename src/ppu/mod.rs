/// Game Boy PPU (Picture Processing Unit)
///
/// Memory Map:
///   0x8000-0x97FF: Tile Data (384 tiles × 16 bytes each)
///   0x9800-0x9BFF: BG Tile Map 1 (32×32)
///   0x9C00-0x9FFF: BG Tile Map 2 (32×32)
///   0xFE00-0xFE9F: OAM (40 sprites × 4 bytes each)
///
/// LCD Registers (0xFF40 - 0xFF4B):
///   LCDC (0xFF40): LCD Control
///   STAT (0xFF41): LCD Status
///   SCY  (0xFF42): Scroll Y
///   SCX  (0xFF43): Scroll X
///   LY   (0xFF44): Current scanline (read-only)
///   LYC  (0xFF45): LY Compare
///   DMA  (0xFF46): OAM DMA Transfer
///   BGP  (0xFF47): BG Palette Data
///   OBP0 (0xFF48): Object Palette 0
///   OBP1 (0xFF49): Object Palette 1
///   WY   (0xFF4A): Window Y Position
///   WX   (0xFF4B): Window X Position

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

/// Total scanlines including VBlank (144 visible + 10 VBlank = 154)
const TOTAL_SCANLINES: u8 = 154;

/// Cycle counts for each PPU mode
const OAM_SCAN_CYCLES: u32 = 80;
const DRAWING_CYCLES: u32 = 172;   // Variable in real hardware, we use a fixed approximation
const HBLANK_CYCLES: u32 = 204;    // Fills the rest to 456
const SCANLINE_CYCLES: u32 = 456;  // Total cycles per scanline

/// LCDC bit masks
const LCDC_BG_ENABLE: u8         = 0x01; // Bit 0: BG & Window enable
const LCDC_OBJ_ENABLE: u8        = 0x02; // Bit 1: OBJ (Sprite) enable
const LCDC_OBJ_SIZE: u8          = 0x04; // Bit 2: OBJ size (0=8x8, 1=8x16)
const LCDC_BG_TILE_MAP: u8       = 0x08; // Bit 3: BG Tile Map area (0=0x9800, 1=0x9C00)
const LCDC_TILE_DATA: u8         = 0x10; // Bit 4: BG & Window Tile Data (0=0x8800, 1=0x8000)
const LCDC_WINDOW_ENABLE: u8     = 0x20; // Bit 5: Window enable
const LCDC_WINDOW_TILE_MAP: u8   = 0x40; // Bit 6: Window Tile Map area (0=0x9800, 1=0x9C00)
const LCDC_LCD_ENABLE: u8        = 0x80; // Bit 7: LCD enable

/// STAT bit masks
const STAT_LYC_FLAG: u8          = 0x04; // Bit 2: LYC=LY coincidence flag (read-only)
const STAT_HBLANK_INT: u8        = 0x08; // Bit 3: Mode 0 HBlank interrupt
const STAT_VBLANK_INT: u8        = 0x10; // Bit 4: Mode 1 VBlank interrupt
const STAT_OAM_INT: u8           = 0x20; // Bit 5: Mode 2 OAM interrupt
const STAT_LYC_INT: u8           = 0x40; // Bit 6: LYC=LY interrupt

/// Interrupt flag bits
pub const INT_VBLANK: u8 = 0x01;
pub const INT_STAT: u8   = 0x02;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PpuMode {
    HBlank  = 0,
    VBlank  = 1,
    OamScan = 2,
    Drawing = 3,
}

/// Sprite attributes from OAM
#[derive(Clone, Copy, Default)]
struct SpriteEntry {
    y: u8,         // Y position (on-screen Y + 16)
    x: u8,         // X position (on-screen X + 8)
    tile: u8,      // Tile index
    flags: u8,     // Attributes/Flags
}

impl SpriteEntry {
    /// Bit 4: Palette number (0=OBP0, 1=OBP1)
    fn palette(&self) -> bool { (self.flags & 0x10) != 0 }
    /// Bit 5: X flip
    fn x_flip(&self) -> bool { (self.flags & 0x20) != 0 }
    /// Bit 6: Y flip
    fn y_flip(&self) -> bool { (self.flags & 0x40) != 0 }
    /// Bit 7: BG priority (0=above BG, 1=behind BG colors 1-3)
    fn bg_priority(&self) -> bool { (self.flags & 0x80) != 0 }
}

pub struct Ppu {
    // VRAM and OAM (shared with bus for DMA, but PPU uses its own copies during rendering)
    pub vram: [u8; 0x2000],
    pub oam: [u8; 0xA0],

    // LCD Registers
    pub lcdc: u8,   // 0xFF40
    pub stat: u8,   // 0xFF41
    pub scy: u8,    // 0xFF42
    pub scx: u8,    // 0xFF43
    pub ly: u8,     // 0xFF44
    pub lyc: u8,    // 0xFF45
    pub bgp: u8,    // 0xFF47
    pub obp0: u8,   // 0xFF48
    pub obp1: u8,   // 0xFF49
    pub wy: u8,     // 0xFF4A
    pub wx: u8,     // 0xFF4B

    // Internal state
    pub mode: PpuMode,
    pub cycles: u32,
    pub frame_buffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4], // RGBA
    pub interrupt_flags: u8,

    // Window internal line counter (increments only when window is actually rendered)
    window_line_counter: u8,

    /// Set to true when a full frame has been rendered (VBlank entered)
    pub frame_ready: bool,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            lcdc: 0x91, // Post-boot values
            stat: 0x85,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            mode: PpuMode::OamScan,
            cycles: 0,
            frame_buffer: [0xFF; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            interrupt_flags: 0,
            window_line_counter: 0,
            frame_ready: false,
        }
    }

    /// Read a PPU register
    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            0xFF40 => self.lcdc,
            0xFF41 => {
                // STAT register: bits 0-1 are the current mode, bit 2 is coincidence flag
                let mode_bits = self.mode as u8;
                let coincidence = if self.ly == self.lyc { STAT_LYC_FLAG } else { 0 };
                // Upper bits from stat register (interrupt enable flags) + read-only bits
                (self.stat & 0x78) | coincidence | mode_bits | 0x80
            }
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF,
        }
    }

    /// Write a PPU register
    pub fn write_register(&mut self, address: u16, value: u8) {
        match address {
            0xFF40 => {
                let was_enabled = self.lcdc & LCDC_LCD_ENABLE != 0;
                self.lcdc = value;
                let is_enabled = self.lcdc & LCDC_LCD_ENABLE != 0;

                // When LCD is turned off, reset PPU state
                if was_enabled && !is_enabled {
                    self.ly = 0;
                    self.cycles = 0;
                    self.mode = PpuMode::HBlank;
                    self.window_line_counter = 0;
                    // Clear the STAT mode bits
                    self.stat &= 0xFC;
                }
                // When LCD is turned on, start in OAM scan
                if !was_enabled && is_enabled {
                    self.mode = PpuMode::OamScan;
                    self.cycles = 0;
                }
            }
            0xFF41 => {
                // Bits 0-2 are read-only, only write bits 3-6
                self.stat = (self.stat & 0x07) | (value & 0x78);
            }
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {} // LY is read-only
            0xFF45 => self.lyc = value,
            0xFF47 => self.bgp = value,
            0xFF48 => self.obp0 = value,
            0xFF49 => self.obp1 = value,
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            _ => {}
        }
    }

    /// Step the PPU by the given number of T-cycles
    pub fn step(&mut self, cycles: u32) {
        if self.lcdc & LCDC_LCD_ENABLE == 0 {
            return; // LCD is off
        }

        self.cycles += cycles;

        match self.mode {
            PpuMode::OamScan => {
                if self.cycles >= OAM_SCAN_CYCLES {
                    self.cycles -= OAM_SCAN_CYCLES;
                    self.mode = PpuMode::Drawing;
                }
            }
            PpuMode::Drawing => {
                if self.cycles >= DRAWING_CYCLES {
                    self.cycles -= DRAWING_CYCLES;
                    self.mode = PpuMode::HBlank;

                    // Render the current scanline
                    self.render_scanline();

                    // HBlank STAT interrupt
                    if self.stat & STAT_HBLANK_INT != 0 {
                        self.interrupt_flags |= INT_STAT;
                    }
                }
            }
            PpuMode::HBlank => {
                if self.cycles >= HBLANK_CYCLES {
                    self.cycles -= HBLANK_CYCLES;
                    self.ly += 1;

                    // Check LYC coincidence
                    self.check_lyc();

                    if self.ly >= 144 {
                        // Enter VBlank
                        self.mode = PpuMode::VBlank;
                        self.interrupt_flags |= INT_VBLANK;
                        self.frame_ready = true;

                        if self.stat & STAT_VBLANK_INT != 0 {
                            self.interrupt_flags |= INT_STAT;
                        }
                    } else {
                        // Next scanline
                        self.mode = PpuMode::OamScan;
                        if self.stat & STAT_OAM_INT != 0 {
                            self.interrupt_flags |= INT_STAT;
                        }
                    }
                }
            }
            PpuMode::VBlank => {
                if self.cycles >= SCANLINE_CYCLES {
                    self.cycles -= SCANLINE_CYCLES;
                    self.ly += 1;

                    if self.ly >= TOTAL_SCANLINES {
                        // Frame complete, back to top
                        self.ly = 0;
                        self.window_line_counter = 0;
                        self.mode = PpuMode::OamScan;

                        // Check LYC for line 0
                        self.check_lyc();

                        if self.stat & STAT_OAM_INT != 0 {
                            self.interrupt_flags |= INT_STAT;
                        }
                    } else {
                        self.check_lyc();
                    }
                }
            }
        }
    }

    /// Check LY == LYC coincidence and request STAT interrupt if enabled
    fn check_lyc(&mut self) {
        if self.ly == self.lyc {
            self.stat |= STAT_LYC_FLAG;
            if self.stat & STAT_LYC_INT != 0 {
                self.interrupt_flags |= INT_STAT;
            }
        } else {
            self.stat &= !STAT_LYC_FLAG;
        }
    }

    /// Take and reset pending interrupt flags
    pub fn take_interrupts(&mut self) -> u8 {
        let flags = self.interrupt_flags;
        self.interrupt_flags = 0;
        flags
    }

    /// Render a single scanline into the frame buffer
    fn render_scanline(&mut self) {
        let ly = self.ly as usize;
        if ly >= SCREEN_HEIGHT {
            return;
        }

        // Track which BG pixels are non-zero (for sprite priority)
        let mut bg_priority_map = [false; SCREEN_WIDTH];

        // 1. Render Background
        if self.lcdc & LCDC_BG_ENABLE != 0 {
            self.render_bg_scanline(ly, &mut bg_priority_map);
        } else {
            // When BG is disabled, fill with white (color 0)
            let color = self.get_color_from_palette(0, self.bgp);
            for x in 0..SCREEN_WIDTH {
                self.set_pixel(x, ly, color);
            }
        }

        // 2. Render Window
        if self.lcdc & LCDC_BG_ENABLE != 0 && self.lcdc & LCDC_WINDOW_ENABLE != 0 {
            self.render_window_scanline(ly, &mut bg_priority_map);
        }

        // 3. Render Sprites
        if self.lcdc & LCDC_OBJ_ENABLE != 0 {
            self.render_sprites_scanline(ly, &bg_priority_map);
        }
    }

    /// Render the background layer for a single scanline
    fn render_bg_scanline(&mut self, ly: usize, bg_priority_map: &mut [bool; SCREEN_WIDTH]) {
        let tile_map_base: u16 = if self.lcdc & LCDC_BG_TILE_MAP != 0 {
            0x9C00
        } else {
            0x9800
        };
        let signed_addressing = self.lcdc & LCDC_TILE_DATA == 0;

        let y = self.scy.wrapping_add(ly as u8);
        let tile_row = (y / 8) as u16;
        let tile_y_offset = (y % 8) as u16;

        for x in 0..SCREEN_WIDTH {
            let scrolled_x = self.scx.wrapping_add(x as u8);
            let tile_col = (scrolled_x / 8) as u16;
            let tile_x_offset = scrolled_x % 8;

            // Get tile index from tile map
            let map_addr = tile_map_base + tile_row * 32 + tile_col;
            let tile_index = self.vram_read(map_addr);

            // Get tile data address
            let tile_data_addr = self.get_tile_data_address(tile_index, signed_addressing);
            let line_offset = tile_y_offset * 2;

            // Read the two bytes for this line of the tile
            let byte1 = self.vram_read(tile_data_addr + line_offset);
            let byte2 = self.vram_read(tile_data_addr + line_offset + 1);

            // Extract color ID (2bpp: bit from byte2 is high bit, byte1 is low bit)
            let bit_pos = 7 - tile_x_offset;
            let color_id = ((byte2 >> bit_pos) & 1) << 1 | ((byte1 >> bit_pos) & 1);

            bg_priority_map[x] = color_id != 0;

            let color = self.get_color_from_palette(color_id, self.bgp);
            self.set_pixel(x, ly, color);
        }
    }

    /// Render the window layer for a single scanline
    fn render_window_scanline(&mut self, ly: usize, bg_priority_map: &mut [bool; SCREEN_WIDTH]) {
        // Window is only visible if WY <= LY and WX <= 166
        if ly < self.wy as usize || self.wx > 166 {
            return;
        }

        let tile_map_base: u16 = if self.lcdc & LCDC_WINDOW_TILE_MAP != 0 {
            0x9C00
        } else {
            0x9800
        };
        let signed_addressing = self.lcdc & LCDC_TILE_DATA == 0;

        let win_y = self.window_line_counter;
        let tile_row = (win_y / 8) as u16;
        let tile_y_offset = (win_y % 8) as u16;

        let wx = self.wx.wrapping_sub(7) as usize; // WX is offset by 7
        let mut rendered = false;

        for x in wx..SCREEN_WIDTH {
            rendered = true;
            let win_x = (x - wx) as u8;
            let tile_col = (win_x / 8) as u16;
            let tile_x_offset = win_x % 8;

            let map_addr = tile_map_base + tile_row * 32 + tile_col;
            let tile_index = self.vram_read(map_addr);

            let tile_data_addr = self.get_tile_data_address(tile_index, signed_addressing);
            let line_offset = tile_y_offset * 2;

            let byte1 = self.vram_read(tile_data_addr + line_offset);
            let byte2 = self.vram_read(tile_data_addr + line_offset + 1);

            let bit_pos = 7 - tile_x_offset;
            let color_id = ((byte2 >> bit_pos) & 1) << 1 | ((byte1 >> bit_pos) & 1);

            bg_priority_map[x] = color_id != 0;

            let color = self.get_color_from_palette(color_id, self.bgp);
            self.set_pixel(x, ly, color);
        }

        if rendered {
            self.window_line_counter += 1;
        }
    }

    /// Render sprites for a single scanline
    fn render_sprites_scanline(&mut self, ly: usize, bg_priority_map: &[bool; SCREEN_WIDTH]) {
        let sprite_height: u8 = if self.lcdc & LCDC_OBJ_SIZE != 0 { 16 } else { 8 };

        // Collect all sprites visible on this scanline (max 10 per Game Boy hardware)
        let mut visible_sprites: Vec<(usize, SpriteEntry)> = Vec::with_capacity(10);

        for i in 0..40 {
            let sprite = self.get_sprite(i);
            let sprite_y = sprite.y.wrapping_sub(16) as i16;

            if (ly as i16) >= sprite_y && (ly as i16) < sprite_y + sprite_height as i16 {
                visible_sprites.push((i, sprite));
                if visible_sprites.len() >= 10 {
                    break;
                }
            }
        }

        // Sort by X coordinate, then by OAM index (lower index = higher priority)
        // Game Boy renders sprites with lower X first; on tie, lower index wins
        visible_sprites.sort_by(|a, b| {
            a.1.x.cmp(&b.1.x).then(a.0.cmp(&b.0))
        });

        // Render sprites in reverse order so higher priority sprites overwrite
        for &(_, sprite) in visible_sprites.iter().rev() {
            let sprite_x = sprite.x.wrapping_sub(8) as i16;
            let sprite_y = sprite.y.wrapping_sub(16) as i16;

            let mut tile_line = (ly as i16 - sprite_y) as u8;

            let tile_index = if sprite_height == 16 {
                // In 8x16 mode, bit 0 of tile index is ignored
                if sprite.y_flip() {
                    tile_line = sprite_height - 1 - tile_line;
                }
                if tile_line < 8 {
                    sprite.tile & 0xFE
                } else {
                    tile_line -= 8;
                    sprite.tile | 0x01
                }
            } else {
                if sprite.y_flip() {
                    tile_line = 7 - tile_line;
                }
                sprite.tile
            };

            // Sprites always use unsigned addressing at 0x8000
            let tile_addr = 0x8000 + (tile_index as u16) * 16 + (tile_line as u16) * 2;
            let byte1 = self.vram_read(tile_addr);
            let byte2 = self.vram_read(tile_addr + 1);

            let palette = if sprite.palette() { self.obp1 } else { self.obp0 };

            for pixel_x in 0..8u8 {
                let target_x = sprite_x + pixel_x as i16;
                if target_x < 0 || target_x >= SCREEN_WIDTH as i16 {
                    continue;
                }
                let target_x = target_x as usize;

                let bit = if sprite.x_flip() { pixel_x } else { 7 - pixel_x };
                let color_id = ((byte2 >> bit) & 1) << 1 | ((byte1 >> bit) & 1);

                // Color 0 is transparent for sprites
                if color_id == 0 {
                    continue;
                }

                // BG priority: if sprite has bg_priority flag and BG pixel is non-zero, skip
                if sprite.bg_priority() && bg_priority_map[target_x] {
                    continue;
                }

                let color = self.get_color_from_palette(color_id, palette);
                self.set_pixel(target_x, ly, color);
            }
        }
    }

    /// Get a sprite entry from OAM
    fn get_sprite(&self, index: usize) -> SpriteEntry {
        let base = index * 4;
        SpriteEntry {
            y: self.oam[base],
            x: self.oam[base + 1],
            tile: self.oam[base + 2],
            flags: self.oam[base + 3],
        }
    }

    /// Get the tile data address given a tile index
    fn get_tile_data_address(&self, tile_index: u8, signed_addressing: bool) -> u16 {
        if signed_addressing {
            // Mode 0x8800: tile_index is signed, base at 0x9000
            let signed_index = tile_index as i8 as i16;
            (0x9000_i32 + (signed_index as i32 * 16)) as u16
        } else {
            // Mode 0x8000: tile_index is unsigned, base at 0x8000
            0x8000 + (tile_index as u16) * 16
        }
    }

    /// Read from VRAM using absolute address (converts to VRAM-relative)
    fn vram_read(&self, address: u16) -> u8 {
        let offset = (address.wrapping_sub(0x8000)) as usize;
        if offset < self.vram.len() {
            self.vram[offset]
        } else {
            0xFF
        }
    }

    /// Map a 2-bit color ID through a palette register to a shade
    fn get_color_from_palette(&self, color_id: u8, palette: u8) -> [u8; 4] {
        let shade = (palette >> (color_id * 2)) & 0x03;
        self.shade_to_rgba(shade)
    }

    /// Convert a 2-bit shade to RGBA
    /// Shades: 0=White, 1=Light Gray, 2=Dark Gray, 3=Black
    /// Uses the classic Game Boy green-ish color palette
    fn shade_to_rgba(&self, shade: u8) -> [u8; 4] {
        match shade {
            0 => [0xE0, 0xF8, 0xD0, 0xFF], // Lightest (white-green)
            1 => [0x88, 0xC0, 0x70, 0xFF], // Light green
            2 => [0x34, 0x68, 0x56, 0xFF], // Dark green
            3 => [0x08, 0x18, 0x20, 0xFF], // Darkest (near-black)
            _ => [0xE0, 0xF8, 0xD0, 0xFF],
        }
    }

    /// Set a pixel in the frame buffer (RGBA format)
    fn set_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        let offset = (y * SCREEN_WIDTH + x) * 4;
        if offset + 3 < self.frame_buffer.len() {
            self.frame_buffer[offset..offset + 4].copy_from_slice(&color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppu_initial_state() {
        let ppu = Ppu::new();
        assert_eq!(ppu.lcdc, 0x91);
        assert_eq!(ppu.mode, PpuMode::OamScan);
        assert_eq!(ppu.ly, 0);
        assert_eq!(ppu.cycles, 0);
    }

    #[test]
    fn test_ppu_mode_transitions() {
        let mut ppu = Ppu::new();
        assert_eq!(ppu.mode, PpuMode::OamScan);

        // OAM Scan -> Drawing
        ppu.step(OAM_SCAN_CYCLES);
        assert_eq!(ppu.mode, PpuMode::Drawing);

        // Drawing -> HBlank
        ppu.step(DRAWING_CYCLES);
        assert_eq!(ppu.mode, PpuMode::HBlank);

        // HBlank -> next line OAM Scan
        ppu.step(HBLANK_CYCLES);
        assert_eq!(ppu.mode, PpuMode::OamScan);
        assert_eq!(ppu.ly, 1);
    }

    #[test]
    fn test_ppu_enters_vblank() {
        let mut ppu = Ppu::new();

        // Run through 144 scanlines
        for _ in 0..144 {
            ppu.step(OAM_SCAN_CYCLES);
            ppu.step(DRAWING_CYCLES);
            ppu.step(HBLANK_CYCLES);
        }

        assert_eq!(ppu.mode, PpuMode::VBlank);
        assert_eq!(ppu.ly, 144);
        assert!(ppu.frame_ready);
    }

    #[test]
    fn test_ppu_full_frame() {
        let mut ppu = Ppu::new();

        // Run through all 154 lines
        for _ in 0..144 {
            ppu.step(SCANLINE_CYCLES);
        }
        assert_eq!(ppu.mode, PpuMode::VBlank);

        for _ in 144..154 {
            ppu.step(SCANLINE_CYCLES);
        }
        assert_eq!(ppu.mode, PpuMode::OamScan);
        assert_eq!(ppu.ly, 0);
    }

    #[test]
    fn test_shade_to_rgba() {
        let ppu = Ppu::new();
        let white = ppu.shade_to_rgba(0);
        let black = ppu.shade_to_rgba(3);
        assert_eq!(white[3], 0xFF); // Alpha
        assert!(white[0] > black[0]); // White is brighter
    }

    #[test]
    fn test_palette_mapping() {
        let ppu = Ppu::new();
        // Default BGP = 0xFC = 0b11_11_11_00
        // Color 0 -> shade 0 (white), Color 1 -> shade 3 (black), etc.
        let c0 = ppu.get_color_from_palette(0, 0xFC);
        let c1 = ppu.get_color_from_palette(1, 0xFC);
        assert_eq!(c0, ppu.shade_to_rgba(0)); // Color 0 maps to shade 0
        assert_eq!(c1, ppu.shade_to_rgba(3)); // Color 1 maps to shade 3
    }

    #[test]
    fn test_tile_address_unsigned() {
        let ppu = Ppu::new();
        assert_eq!(ppu.get_tile_data_address(0, false), 0x8000);
        assert_eq!(ppu.get_tile_data_address(1, false), 0x8010);
        assert_eq!(ppu.get_tile_data_address(255, false), 0x8FF0);
    }

    #[test]
    fn test_tile_address_signed() {
        let ppu = Ppu::new();
        assert_eq!(ppu.get_tile_data_address(0, true), 0x9000);
        assert_eq!(ppu.get_tile_data_address(128, true), 0x8800); // -128 signed
        assert_eq!(ppu.get_tile_data_address(127, true), 0x97F0); // 127 signed
    }

    #[test]
    fn test_lcd_disable_resets_state() {
        let mut ppu = Ppu::new();
        ppu.ly = 100;
        ppu.cycles = 200;
        ppu.mode = PpuMode::Drawing;

        // Disable LCD
        ppu.write_register(0xFF40, 0x00);
        assert_eq!(ppu.ly, 0);
        assert_eq!(ppu.cycles, 0);
        assert_eq!(ppu.mode, PpuMode::HBlank);
    }
}
