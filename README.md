# gameboy-rust

A functional Game Boy (DMG) emulator written in Rust. The goal is a modular and readable implementation of the original hardware.

## Current Status

The emulator is mostly functional and can run many early titles and test ROMs.

- **CPU**: Full LR35902 instruction set with accurate cycle counting.
- **Video (PPU)**: Scanline-based renderer. Supports background, window, and sprites (OAM).
- **Audio (APU)**: 4-channel stereo support (Pulse 1, Pulse 2, Wave, and Noise) using `cpal`.
- **Memory**: Basic MBC1 implementation.
- **Input**: Full joypad support via `winit`.
- **Serial**: Text output from the serial port is supported (useful for Blargg's tests).

## Building and Running

You need the Rust toolchain installed.

```bash
cargo run --release <path_to_rom>
```

### Controls

| Game Boy | Keyboard |
| :--- | :--- |
| **D-Pad** | Arrow Keys |
| **A** | X |
| **B** | Z |
| **Start** | Enter |
| **Select** | Backspace |

## Technical Details

- **Rendering**: Uses `pixels` for a simple hardware-accelerated frame buffer.
- **Windowing**: Managed by `winit`.
- **Audio Output**: Handled by `cpal` to maintain cross-platform compatibility.
- **Structure**: The project is split into separate modules for the Bus, CPU, PPU, APU, and Timer to make the logic easier to follow.

## References

- [Pan Docs](https://gbdev.io/pandocs/) - The main reference for Game Boy technical details.
- [Game Boy CPU Manual](http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf)
- [Blargg's Test ROMs](https://github.com/retrio/gb-test-roms)
