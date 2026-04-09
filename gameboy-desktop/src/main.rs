use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};
use pixels::{Pixels, SurfaceTexture};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use gameboy_core::emulator::GameBoy;
use gameboy_core::joypad::Button;
use gameboy_core::ppu::{SCREEN_WIDTH, SCREEN_HEIGHT};

// Scale factor for the window
const SCALE: u32 = 4;

struct GameBoyApp {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    gameboy: Arc<Mutex<GameBoy>>,
    audio_stream: Option<cpal::Stream>,
    rom_loaded: bool,
}

impl GameBoyApp {
    fn new(rom_path: Option<String>) -> Self {
        // 1. Detect host audio sample rate (default to 44100 if anything fails)
        let sample_rate = {
            let host = cpal::default_host();
            let device = host.default_output_device();
            device.and_then(|d| d.default_output_config().ok())
                  .map(|c| c.sample_rate())
                  .unwrap_or(44100)
        };

        let mut gb = GameBoy::new(sample_rate);
        let mut rom_loaded = false;

        if let Some(path) = rom_path {
            match gb.load_rom_file(&path) {
                Ok(()) => {
                    log::info!("ROM loaded successfully: {}", path);
                    rom_loaded = true;
                }
                Err(e) => {
                    log::error!("Failed to load ROM: {}", e);
                    eprintln!("Error: {}", e);
                }
            }
        } else {
            log::info!("No ROM file specified. Running without cartridge.");
            log::info!("Usage: gameboy-rust <rom_file.gb>");
        }

        let gameboy = Arc::new(Mutex::new(gb));
        let audio_stream = Self::setup_audio(gameboy.clone());

        Self {
            window: None,
            pixels: None,
            gameboy,
            audio_stream,
            rom_loaded,
        }
    }

    fn setup_audio(gameboy: Arc<Mutex<GameBoy>>) -> Option<cpal::Stream> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let config = device.default_output_config().ok()?;

        log::info!("Audio device: {}", device.name().unwrap_or_default());
        log::info!("Audio config: {:?}", config);

        let stream_config: cpal::StreamConfig = config.clone().into();
        let _channels = stream_config.channels as usize;

        let stream = device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut gb = gameboy.lock().unwrap();
                let apu = &mut gb.bus.apu;
                
                // Read stereo samples from APU (interleaved)
                // We need data.len() / channels sample pairs
                let requested_pairs = data.len() / 2;
                let read_pairs = apu.read_samples(data, requested_pairs);

                // If we don't have enough samples, fill with silence
                if read_pairs < requested_pairs {
                    for i in (read_pairs * 2)..data.len() {
                        data[i] = 0.0;
                    }
                }
            },
            |err| log::error!("Audio stream error: {}", err),
            None
        ).ok()?;

        stream.play().ok()?;
        Some(stream)
    }
}

impl ApplicationHandler for GameBoyApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let title = if self.rom_loaded {
                let gb = self.gameboy.lock().unwrap();
                if let Some(cart) = &gb.bus.cartridge {
                    format!("Game Boy — {}", cart.header.title)
                } else {
                    "Game Boy Emulator".to_string()
                }
            } else {
                "Game Boy Emulator (No ROM)".to_string()
            };

            let window_attributes = Window::default_attributes()
                .with_title(title)
                .with_inner_size(winit::dpi::LogicalSize::new(
                    SCREEN_WIDTH as f64 * SCALE as f64,
                    SCREEN_HEIGHT as f64 * SCALE as f64,
                ));
            
            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            self.window = Some(window.clone());

            let pixels = {
                let window_size = window.inner_size();
                let surface_texture = SurfaceTexture::new(
                    window_size.width,
                    window_size.height,
                    window.clone(),
                );
                Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface_texture).unwrap()
            };
            self.pixels = Some(pixels);
            
            log::info!("Window and pixel buffer initialized ({}x{} @ {}x scale).", 
                SCREEN_WIDTH, SCREEN_HEIGHT, SCALE);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        if let Some(window) = &self.window {
            if window.id() == window_id {
                match event {
                    WindowEvent::CloseRequested => {
                        event_loop.exit();
                    }
                    WindowEvent::KeyboardInput {
                        event: KeyEvent {
                            physical_key: PhysicalKey::Code(key_code),
                            state,
                            ..
                        },
                        ..
                    } => {
                        // Escape exits the emulator
                        if key_code == KeyCode::Escape && state == ElementState::Pressed {
                            event_loop.exit();
                            return;
                        }

                        // Map keyboard keys to Game Boy buttons
                        //   Arrow keys  -> D-Pad
                        //   Z           -> B
                        //   X           -> A
                        //   Enter       -> Start
                        //   Backspace   -> Select
                        let button = match key_code {
                            KeyCode::ArrowRight => Some(Button::Right),
                            KeyCode::ArrowLeft  => Some(Button::Left),
                            KeyCode::ArrowUp    => Some(Button::Up),
                            KeyCode::ArrowDown  => Some(Button::Down),
                            KeyCode::KeyZ       => Some(Button::B),
                            KeyCode::KeyX       => Some(Button::A),
                            KeyCode::Enter      => Some(Button::Start),
                            KeyCode::Backspace  => Some(Button::Select),
                            _ => None,
                        };

                        if let Some(btn) = button {
                            let mut gb = self.gameboy.lock().unwrap();
                            match state {
                                ElementState::Pressed  => gb.bus.joypad.press(btn),
                                ElementState::Released => gb.bus.joypad.release(btn),
                            }
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        // Run one frame of emulation
                        if self.rom_loaded {
                            let mut gb = self.gameboy.lock().unwrap();
                            gb.run_frame();
                        }

                        if let Some(pixels) = &mut self.pixels {
                            // Copy the PPU frame buffer to the pixel buffer
                            let frame = pixels.frame_mut();
                            let gb = self.gameboy.lock().unwrap();
                            let fb = gb.frame_buffer();
                            
                            let copy_len = frame.len().min(fb.len());
                            frame[..copy_len].copy_from_slice(&fb[..copy_len]);

                            if let Err(err) = pixels.render() {
                                log::error!("pixels.render() failed: {err}");
                                event_loop.exit();
                            }
                        }
                    }
                    WindowEvent::Resized(size) => {
                        if let Some(pixels) = &mut self.pixels {
                            if size.width > 0 && size.height > 0 {
                                if let Err(err) = pixels.resize_surface(size.width, size.height) {
                                    log::error!("pixels.resize_surface() failed: {err}");
                                    event_loop.exit();
                                }
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
    }
    
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();
    
    let args: Vec<String> = std::env::args().collect();
    let rom_path = args.get(1).cloned();

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    
    let mut app = GameBoyApp::new(rom_path);
    
    log::info!("Starting Game Boy emulator event loop...");
    event_loop.run_app(&mut app).expect("Event loop failed");
}
