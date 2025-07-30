use crate::cpu::CPU;
use crate::ppu::PPU;
use crate::cartridge::Cartridge;

pub struct Bus {
    pub cpu: CPU,
    pub ppu: PPU,
    pub cartridge: Option<Cartridge>,
    pub ram: [u8; 2048], // 2KB internal RAM
    pub controller1: u8,
    pub controller2: u8,
    pub controller1_shift: u8,
    pub controller2_shift: u8,
}

impl Bus {
    pub fn new() -> Self {
        Bus {
            cpu: CPU::new(),
            ppu: PPU::new(),
            cartridge: None,
            ram: [0; 2048],
            controller1: 0,
            controller2: 0,
            controller1_shift: 0,
            controller2_shift: 0,
        }
    }
    
    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }
    
    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                // Internal RAM (mirrored every 2KB)
                self.ram[addr as usize % 2048]
            }
            0x2000..=0x3FFF => {
                // PPU registers (mirrored every 8 bytes)
                self.ppu.cpu_read(0x2000 + (addr % 8))
            }
            0x4016 => {
                // Controller 1
                let result = self.controller1_shift & 1;
                self.controller1_shift >>= 1;
                result
            }
            0x4017 => {
                // Controller 2
                let result = self.controller2_shift & 1;
                self.controller2_shift >>= 1;
                result
            }
            0x4000..=0x4017 => {
                // APU and I/O registers (not implemented)
                0
            }
            0x4018..=0x401F => {
                // APU and I/O test functionality (not implemented)
                0
            }
            0x4020..=0xFFFF => {
                // Cartridge space
                if let Some(ref cartridge) = self.cartridge {
                    cartridge.read_prg(addr - 0x8000)
                } else {
                    0
                }
            }
        }
    }
    
    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => {
                // Internal RAM (mirrored every 2KB)
                self.ram[addr as usize % 2048] = data;
            }
            0x2000..=0x3FFF => {
                // PPU registers (mirrored every 8 bytes)
                self.ppu.cpu_write(0x2000 + (addr % 8), data);
            }
            0x4014 => {
                // OAM DMA
                let page = (data as u16) << 8;
                for i in 0..256 {
                    let byte = self.read(page + i);
                    self.ppu.oam[i as usize] = byte;
                }
            }
            0x4016 => {
                // Controller strobe
                if data & 1 != 0 {
                    self.controller1_shift = self.controller1;
                    self.controller2_shift = self.controller2;
                }
            }
            0x4000..=0x4017 => {
                // APU and I/O registers (not implemented)
            }
            0x4018..=0x401F => {
                // APU and I/O test functionality (not implemented)
            }
            0x4020..=0xFFFF => {
                // Cartridge space
                if let Some(ref mut cartridge) = self.cartridge {
                    cartridge.write_prg(addr - 0x8000, data);
                }
            }
        }
    }
    
    pub fn reset(&mut self) {
        if self.cartridge.is_some() {
            // Get reset vector directly
            let lo = self.read(0xFFFC) as u16;
            let hi = self.read(0xFFFD) as u16;
            self.cpu.pc = (hi << 8) | lo;
            self.cpu.a = 0;
            self.cpu.x = 0;
            self.cpu.y = 0;
            self.cpu.sp = 0xFD;
            self.cpu.status = 0x24; // I flag set
            self.cpu.cycles = 0;
            self.ppu.reset();
        }
    }
    
    pub fn step(&mut self) -> bool {
        // Step PPU 3 times for every CPU step (PPU runs at 3x CPU speed)
        let mut nmi = false;
        for _ in 0..3 {
            if let Some(ref mut cartridge) = self.cartridge {
                nmi |= self.ppu.step(cartridge);
            }
        }
        
        // Handle NMI
        if nmi {
            // NMI interrupt
            let pc_hi = (self.cpu.pc >> 8) as u8;
            let pc_lo = self.cpu.pc as u8;
            self.write(0x0100 + self.cpu.sp as u16, pc_hi);
            self.cpu.sp = self.cpu.sp.wrapping_sub(1);
            self.write(0x0100 + self.cpu.sp as u16, pc_lo);
            self.cpu.sp = self.cpu.sp.wrapping_sub(1);
            self.write(0x0100 + self.cpu.sp as u16, self.cpu.status & !0x10 | 0x20);
            self.cpu.sp = self.cpu.sp.wrapping_sub(1);
            self.cpu.status |= 0x04; // Set interrupt flag
            
            let lo = self.read(0xFFFA) as u16;
            let hi = self.read(0xFFFB) as u16;
            self.cpu.pc = (hi << 8) | lo;
        }
        
        // Step CPU
        self.cpu.step(self);
        
        self.ppu.frame_ready()
    }
    
    pub fn set_controller_state(&mut self, controller: u8, buttons: u8) {
        match controller {
            1 => self.controller1 = buttons,
            2 => self.controller2 = buttons,
            _ => {}
        }
    }
}
