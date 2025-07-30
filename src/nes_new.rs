use crate::cartridge::Cartridge;
use crate::ppu::PPU;

// Controller button constants
const BUTTON_A: u8 = 0x80;
const BUTTON_B: u8 = 0x40;
const BUTTON_SELECT: u8 = 0x20;
const BUTTON_START: u8 = 0x10;
const BUTTON_UP: u8 = 0x08;
const BUTTON_DOWN: u8 = 0x04;
const BUTTON_LEFT: u8 = 0x02;
const BUTTON_RIGHT: u8 = 0x01;

// Simple 6502 CPU
struct CPU {
    a: u8,      // Accumulator
    x: u8,      // X register
    y: u8,      // Y register
    pc: u16,    // Program counter
    sp: u8,     // Stack pointer
    p: u8,      // Status register
    cycles: u64,
}

impl CPU {
    fn new() -> Self {
        CPU {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            sp: 0xFD,
            p: 0x24,
            cycles: 0,
        }
    }
    
    fn reset(&mut self, cartridge: &Cartridge) {
        // Reset vector is at 0xFFFC-0xFFFD
        let lo = cartridge.read_prg(0x7FFC) as u16; // Adjust for 0x8000 base
        let hi = cartridge.read_prg(0x7FFD) as u16;
        self.pc = (hi << 8) | lo;
        self.sp = 0xFD;
        self.p = 0x24;
        self.cycles = 0;
    }
}

pub struct NES {
    cpu: CPU,
    ppu: PPU,
    ram: [u8; 2048], // 2KB internal RAM
    pub frame_complete: bool,
    pub cartridge: Option<Cartridge>,
    controller1: u8,
    controller1_shift: u8,
    controller_strobe: bool,
    cycles: u64,
}

impl NES {
    pub fn new() -> Self {
        NES {
            cpu: CPU::new(),
            ppu: PPU::new(),
            ram: [0; 2048],
            frame_complete: false,
            cartridge: None,
            controller1: 0,
            controller1_shift: 0,
            controller_strobe: false,
            cycles: 0,
        }
    }
    
    pub fn load_cartridge(&mut self, rom_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let cartridge = Cartridge::new(rom_path)?;
        println!("Loaded ROM: {} KB PRG, {} KB CHR", 
                 cartridge.prg_rom.len() / 1024, 
                 cartridge.chr_rom.len() / 1024);
        self.cartridge = Some(cartridge);
        Ok(())
    }
    
    pub fn reset(&mut self) {
        if let Some(ref cartridge) = self.cartridge {
            self.cpu.reset(cartridge);
            self.ppu.reset();
            self.cycles = 0;
        }
    }
    
    pub fn run_frame(&mut self) {
        if let Some(ref mut cartridge) = self.cartridge {
            self.frame_complete = false;
            
            // Run until PPU signals frame complete
            while !self.frame_complete {
                // Execute one CPU instruction
                self.cpu_step();
                
                // PPU runs 3 times per CPU cycle
                for _ in 0..3 {
                    let nmi = self.ppu.step(cartridge);
                    if nmi && (self.cpu.p & 0x04) == 0 { // NMI not masked
                        self.nmi();
                    }
                    
                    if self.ppu.frame_ready() {
                        self.frame_complete = true;
                        self.ppu.frame_done();
                        break;
                    }
                }
            }
        }
    }
    
    fn nmi(&mut self) {
        // Push PC and status to stack
        self.cpu_write(0x0100 + self.cpu.sp as u16, ((self.cpu.pc >> 8) & 0xFF) as u8);
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
        self.cpu_write(0x0100 + self.cpu.sp as u16, (self.cpu.pc & 0xFF) as u8);
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
        self.cpu_write(0x0100 + self.cpu.sp as u16, self.cpu.p);
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
        
        // Jump to NMI vector
        if let Some(ref cartridge) = self.cartridge {
            let lo = cartridge.read_prg(0x7FFA) as u16; // NMI vector at 0xFFFA
            let hi = cartridge.read_prg(0x7FFB) as u16;
            self.cpu.pc = (hi << 8) | lo;
        }
    }
    
    // Simple CPU step - basic instruction execution
    fn cpu_step(&mut self) {
        if self.cartridge.is_some() {
            // Fetch instruction
            let opcode = self.cpu_read(self.cpu.pc);
            self.cpu.pc = self.cpu.pc.wrapping_add(1);
            
            // Execute basic instructions (very simplified)
            match opcode {
                0xEA => {}, // NOP
                0x4C => {
                    // JMP absolute
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    self.cpu.pc = (hi << 8) | lo;
                }
                0xA9 => {
                    // LDA immediate
                    self.cpu.a = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(self.cpu.a);
                }
                0x8D => {
                    // STA absolute
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = (hi << 8) | lo;
                    self.cpu_write(addr, self.cpu.a);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                }
                0x78 => {
                    // SEI - Set interrupt disable
                    self.cpu.p |= 0x04;
                }
                0x58 => {
                    // CLI - Clear interrupt disable
                    self.cpu.p &= !0x04;
                }
                0xD8 => {
                    // CLD - Clear decimal mode
                    self.cpu.p &= !0x08;
                }
                _ => {
                    // Unknown instruction - just increment PC
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                }
            }
            
            self.cpu.cycles = self.cpu.cycles.wrapping_add(1);
        }
    }
    
    fn set_zero_negative(&mut self, value: u8) {
        if value == 0 {
            self.cpu.p |= 0x02; // Zero flag
        } else {
            self.cpu.p &= !0x02;
        }
        
        if value & 0x80 != 0 {
            self.cpu.p |= 0x80; // Negative flag
        } else {
            self.cpu.p &= !0x80;
        }
    }
    
    // Memory read for CPU
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                // Internal RAM (2KB, mirrored)
                self.ram[(addr & 0x07FF) as usize]
            }
            0x2000..=0x3FFF => {
                // PPU registers (8 bytes, mirrored)
                self.ppu.cpu_read(0x2000 + (addr & 0x0007))
            }
            0x4016 => {
                // Controller 1
                let bit = self.controller1_shift & 0x80;
                self.controller1_shift <<= 1;
                if bit != 0 { 1 } else { 0 }
            }
            0x4017 => 0, // Controller 2 (not implemented)
            0x8000..=0xFFFF => {
                // Cartridge PRG ROM
                if let Some(ref cartridge) = self.cartridge {
                    cartridge.read_prg(addr - 0x8000)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }
    
    // Memory write for CPU
    fn cpu_write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => {
                // Internal RAM (2KB, mirrored)
                self.ram[(addr & 0x07FF) as usize] = data;
            }
            0x2000..=0x3FFF => {
                // PPU registers (8 bytes, mirrored)
                self.ppu.cpu_write(0x2000 + (addr & 0x0007), data);
            }
            0x4016 => {
                // Controller strobe
                self.controller_strobe = data & 1 != 0;
                if self.controller_strobe {
                    self.controller1_shift = self.controller1;
                }
            }
            0x8000..=0xFFFF => {
                // Cartridge space (usually read-only)
            }
            _ => {}
        }
    }
    
    // Controller input handling
    pub fn set_controller_state(&mut self, buttons: u8) {
        self.controller1 = buttons;
        if self.controller_strobe {
            self.controller1_shift = self.controller1;
        }
    }
    
    pub fn get_frame_buffer(&self) -> &[u8] { 
        self.ppu.get_frame_buffer()
    }
}
