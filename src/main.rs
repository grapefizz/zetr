mod cartridge;
mod ppu;
mod nes;

use std::env;
use std::time::{Duration, Instant};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::TextureAccess;

use nes::NES;

const SCREEN_WIDTH: usize = 256;
const SCREEN_HEIGHT: usize = 240;
const SCALE: u32 = 3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <rom_file>", args[0]);
        eprintln!("Example: {} donkeykong.nes", args[0]);
        return Ok(());
    }

    let rom_path = &args[1];
    
    // Initialize SDL2
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    
    let window = video_subsystem
        .window("ZETR - NES Emulator", SCREEN_WIDTH as u32 * SCALE, SCREEN_HEIGHT as u32 * SCALE)
        .position_centered()
        .build()?;
    
    let mut canvas = window.into_canvas().build()?;
    let texture_creator = canvas.texture_creator();
    
    let mut texture = texture_creator.create_texture(
        PixelFormatEnum::RGB24,
        TextureAccess::Streaming,
        SCREEN_WIDTH as u32,
        SCREEN_HEIGHT as u32,
    )?;
    
    // Initialize NES
    let mut nes = NES::new();
    if let Err(e) = nes.load_cartridge(rom_path) {
        eprintln!("Error loading ROM: {}", e);
        return Ok(());
    }
    nes.reset();
    
    let mut event_pump = sdl_context.event_pump()?;
    let frame_duration = Duration::from_nanos(1_000_000_000 / 60); // 60 FPS
    
    println!("Controls:");
    println!("Arrow keys: D-pad");
    println!("Z: A button");
    println!("X: B button");
    println!("A: Select");
    println!("S: Start");
    println!("ESC: Quit");
    
    'running: loop {
        let frame_start = Instant::now();
        
        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'running,
                Event::KeyDown { keycode: Some(keycode), .. } => {
                    nes.handle_key_down(keycode);
                }
                Event::KeyUp { keycode: Some(keycode), .. } => {
                    nes.handle_key_up(keycode);
                }
                _ => {}
            }
        }
        
        // Run NES for one frame
        nes.run_frame();
        
        // Render
        if nes.frame_ready() {
            let frame_buffer = nes.get_frame_buffer();
            texture.update(None, frame_buffer, SCREEN_WIDTH * 3)?;
            canvas.copy(&texture, None, None)?;
            canvas.present();
        }
        
        // Frame rate limiting
        let frame_time = frame_start.elapsed();
        if frame_time < frame_duration {
            std::thread::sleep(frame_duration - frame_time);
        }
    }
    
    Ok(())
}
