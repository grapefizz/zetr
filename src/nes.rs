use crate::cartridge::Cartridge;
use crate::cpu::CPU;
use crate::ppu::PPU;
use crate::bus::Bus;

// Controller button constants
const BUTTON_A: u8 = 0x01;
const BUTTON_B: u8 = 0x02;
const BUTTON_SELECT: u8 = 0x04;
const BUTTON_START: u8 = 0x08;
const BUTTON_UP: u8 = 0x10;
const BUTTON_DOWN: u8 = 0x20;
const BUTTON_LEFT: u8 = 0x40;
const BUTTON_RIGHT: u8 = 0x80;

pub struct NES {
    cpu: CPU,
    ppu: PPU,
    ram: [u8; 2048],
    cartridge: Option<Cartridge>,
    controller1: u8,
    cycles: u64,

    // DMA state
    dma_page: u8,
    dma_addr: u8,
    dma_data: u8,
    dma_transfer: bool,
    dma_dummy: bool,
}

impl NES {
    pub fn new() -> Self {
        NES {
            cpu: CPU::new(),
            ppu: PPU::new(),
            ram: [0; 2048],
            cartridge: None,
            controller1: 0,
            cycles: 0,
            dma_page: 0,
            dma_addr: 0,
            dma_data: 0,
            dma_transfer: false,
            dma_dummy: true,
        }
    }

    pub fn load_cartridge(&mut self, rom_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let cartridge = Cartridge::new(rom_path)?;
        self.cartridge = Some(cartridge);
        self.reset();
        Ok(())
    }

    pub fn reset(&mut self) {
        if let Some(cart) = self.cartridge.as_mut() {
            let mut bus = Bus::new(&mut self.ppu, cart, &mut self.ram);
            self.cpu.reset(&mut bus);
        }
        self.cycles = 0;
    }

    pub fn run_frame(&mut self) {
        if self.cartridge.is_none() {
            return;
        }

        while !self.ppu.frame_complete {
            self.clock();
        }
    }

    fn clock(&mut self) {
        let cart = self.cartridge.as_mut().unwrap();
        
        self.ppu.step(cart);

        if self.cycles % 3 == 0 {
            if self.cpu.dma_request {
                self.dma_transfer = true;
                self.dma_page = self.cpu.dma_page;
                self.dma_addr = 0;
                self.dma_dummy = true;
                self.cpu.dma_request = false;
            }

            if self.dma_transfer {
                if self.dma_dummy {
                    if self.cycles % 2 == 1 {
                        self.dma_dummy = false;
                    }
                } else {
                    if self.cycles % 2 == 0 {
                        let addr = (self.dma_page as u16) << 8 | self.dma_addr as u16;
                        let mut bus = Bus::new(&mut self.ppu, cart, &mut self.ram);
                        self.dma_data = bus.read(addr);
                    } else {
                        self.ppu.oam[self.dma_addr as usize] = self.dma_data;
                        self.dma_addr = self.dma_addr.wrapping_add(1);
                        if self.dma_addr == 0 {
                            self.dma_transfer = false;
                            self.dma_dummy = true;
                        }
                    }
                }
            } else {
                let mut bus = Bus::new(&mut self.ppu, cart, &mut self.ram);
                bus.controller1 = self.controller1;
                self.cpu.step(&mut bus);
            }
        }

        if self.ppu.nmi_occurred {
            self.ppu.nmi_occurred = false;
            let mut bus = Bus::new(&mut self.ppu, cart, &mut self.ram);
            self.cpu.nmi(&mut bus);
        }

        self.cycles += 1;
    }

    pub fn handle_key_down(&mut self, keycode: sdl2::keyboard::Keycode) {
        use sdl2::keyboard::Keycode;
        match keycode {
            Keycode::Z => self.controller1 |= BUTTON_A,
            Keycode::X => self.controller1 |= BUTTON_B,
            Keycode::A => self.controller1 |= BUTTON_SELECT,
            Keycode::S => self.controller1 |= BUTTON_START,
            Keycode::Up => self.controller1 |= BUTTON_UP,
            Keycode::Down => self.controller1 |= BUTTON_DOWN,
            Keycode::Left => self.controller1 |= BUTTON_LEFT,
            Keycode::Right => self.controller1 |= BUTTON_RIGHT,
            _ => {}
        }
    }

    pub fn handle_key_up(&mut self, keycode: sdl2::keyboard::Keycode) {
        use sdl2::keyboard::Keycode;
        match keycode {
            Keycode::Z => self.controller1 &= !BUTTON_A,
            Keycode::X => self.controller1 &= !BUTTON_B,
            Keycode::A => self.controller1 &= !BUTTON_SELECT,
            Keycode::S => self.controller1 &= !BUTTON_START,
            Keycode::Up => self.controller1 &= !BUTTON_UP,
            Keycode::Down => self.controller1 &= !BUTTON_DOWN,
            Keycode::Left => self.controller1 &= !BUTTON_LEFT,
            Keycode::Right => self.controller1 &= !BUTTON_RIGHT,
            _ => {}
        }
    }

    pub fn frame_ready(&self) -> bool {
        self.ppu.frame_complete
    }

    pub fn get_frame_buffer(&self) -> &[u8] {
        self.ppu.get_frame_buffer()
    }

    pub fn frame_done(&mut self) {
        self.ppu.frame_complete = false;
    }
}
