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
    instruction_count: u32, // For debugging
    frame_count: u32, // Track total frames rendered
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
            instruction_count: 0,
            frame_count: 0,
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
            self.instruction_count = 0;
            
            // Wait a few frames for PPU to warm up (like real NES)
            // Most games expect this delay
            for _ in 0..3 {
                self.run_frame();
            }
            
            println!("NES reset complete, PPU warmed up, starting game...");
        }
    }
    
    pub fn run_frame(&mut self) {
        if self.cartridge.is_some() {
            self.frame_complete = false;
            
            // Run until PPU signals frame complete
            while !self.frame_complete {
                // Execute one CPU instruction
                self.cpu_step();
                
                // PPU runs 3 times per CPU cycle
                for _ in 0..3 {
                    let nmi = if let Some(ref mut cartridge) = self.cartridge {
                        self.ppu.step(cartridge)
                    } else {
                        false
                    };
                    
                    if nmi && (self.cpu.p & 0x04) == 0 { // NMI not masked
                        self.nmi();
                    }
                    
                    if self.ppu.frame_ready() {
                        self.frame_complete = true;
                        self.ppu.frame_done();
                        self.frame_count += 1;
                        
                        // Provide feedback every few seconds
                        if self.frame_count % 300 == 0 { // Every 5 seconds at 60 FPS
                            println!("Game running: {} frames completed ({:.1}s)", 
                                     self.frame_count, self.frame_count as f32 / 60.0);
                        }
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
            
            // Debug output disabled
            if false {
                println!("Instruction {}: PC=${:04X} opcode=${:02X} A=${:02X} X=${:02X} Y=${:02X} P=${:02X} S=${:02X}", 
                    self.instruction_count, self.cpu.pc, opcode, self.cpu.a, self.cpu.x, self.cpu.y, self.cpu.p, self.cpu.sp);
                self.instruction_count += 1;
            }
            
            self.cpu.pc = self.cpu.pc.wrapping_add(1);
            
            // Execute basic instructions (expanded for better game support)
            match opcode {
                0xEA => {}, // NOP
                
                // Jump/Branch instructions
                0x4C => {
                    // JMP absolute
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    self.cpu.pc = (hi << 8) | lo;
                }
                0x20 => {
                    // JSR - Jump to subroutine
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = (hi << 8) | lo;
                    
                    // Push return address (PC + 1) to stack
                    let ret_addr = self.cpu.pc + 1;
                    self.cpu_write(0x0100 + self.cpu.sp as u16, ((ret_addr >> 8) & 0xFF) as u8);
                    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
                    self.cpu_write(0x0100 + self.cpu.sp as u16, (ret_addr & 0xFF) as u8);
                    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
                    
                    self.cpu.pc = addr;
                }
                0x60 => {
                    // RTS - Return from subroutine
                    self.cpu.sp = self.cpu.sp.wrapping_add(1);
                    let lo = self.cpu_read(0x0100 + self.cpu.sp as u16) as u16;
                    self.cpu.sp = self.cpu.sp.wrapping_add(1);
                    let hi = self.cpu_read(0x0100 + self.cpu.sp as u16) as u16;
                    self.cpu.pc = ((hi << 8) | lo) + 1;
                }
                0x40 => {
                    // RTI - Return from interrupt
                    self.cpu.sp = self.cpu.sp.wrapping_add(1);
                    self.cpu.p = self.cpu_read(0x0100 + self.cpu.sp as u16) & !0x10;
                    self.cpu.sp = self.cpu.sp.wrapping_add(1);
                    let lo = self.cpu_read(0x0100 + self.cpu.sp as u16) as u16;
                    self.cpu.sp = self.cpu.sp.wrapping_add(1);
                    let hi = self.cpu_read(0x0100 + self.cpu.sp as u16) as u16;
                    self.cpu.pc = (hi << 8) | lo;
                }
                0x10 => {
                    // BPL - Branch if positive
                    let offset = self.cpu_read(self.cpu.pc) as i8;
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    if (self.cpu.p & 0x80) == 0 {
                        self.cpu.pc = ((self.cpu.pc as i32) + (offset as i32)) as u16;
                    }
                }
                0x30 => {
                    // BMI - Branch if minus
                    let offset = self.cpu_read(self.cpu.pc) as i8;
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    if (self.cpu.p & 0x80) != 0 {
                        self.cpu.pc = ((self.cpu.pc as i32) + (offset as i32)) as u16;
                    }
                }
                0x50 => {
                    // BVC - Branch if overflow clear
                    let offset = self.cpu_read(self.cpu.pc) as i8;
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    if (self.cpu.p & 0x40) == 0 {
                        self.cpu.pc = ((self.cpu.pc as i32) + (offset as i32)) as u16;
                    }
                }
                0x70 => {
                    // BVS - Branch if overflow set
                    let offset = self.cpu_read(self.cpu.pc) as i8;
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    if (self.cpu.p & 0x40) != 0 {
                        self.cpu.pc = ((self.cpu.pc as i32) + (offset as i32)) as u16;
                    }
                }
                0x90 => {
                    // BCC - Branch if carry clear
                    let offset = self.cpu_read(self.cpu.pc) as i8;
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    if (self.cpu.p & 0x01) == 0 {
                        self.cpu.pc = ((self.cpu.pc as i32) + (offset as i32)) as u16;
                    }
                }
                0xB0 => {
                    // BCS - Branch if carry set
                    let offset = self.cpu_read(self.cpu.pc) as i8;
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    if (self.cpu.p & 0x01) != 0 {
                        self.cpu.pc = ((self.cpu.pc as i32) + (offset as i32)) as u16;
                    }
                }
                0xD0 => {
                    // BNE - Branch if not equal
                    let offset = self.cpu_read(self.cpu.pc) as i8;
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    if (self.cpu.p & 0x02) == 0 {
                        self.cpu.pc = ((self.cpu.pc as i32) + (offset as i32)) as u16;
                    }
                }
                0xF0 => {
                    // BEQ - Branch if equal
                    let offset = self.cpu_read(self.cpu.pc) as i8;
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    if (self.cpu.p & 0x02) != 0 {
                        self.cpu.pc = ((self.cpu.pc as i32) + (offset as i32)) as u16;
                    }
                }
                
                // Load instructions
                0xA9 => {
                    // LDA immediate
                    self.cpu.a = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(self.cpu.a);
                }
                0xA5 => {
                    // LDA zero page
                    let addr = self.cpu_read(self.cpu.pc) as u16;
                    self.cpu.a = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(self.cpu.a);
                }
                0xAD => {
                    // LDA absolute
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = (hi << 8) | lo;
                    self.cpu.a = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                    self.set_zero_negative(self.cpu.a);
                }
                0xA2 => {
                    // LDX immediate
                    self.cpu.x = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(self.cpu.x);
                }
                0xA0 => {
                    // LDY immediate
                    self.cpu.y = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(self.cpu.y);
                }
                
                // Store instructions
                0x8D => {
                    // STA absolute
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = (hi << 8) | lo;
                    self.cpu_write(addr, self.cpu.a);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                }
                0x85 => {
                    // STA zero page
                    let addr = self.cpu_read(self.cpu.pc) as u16;
                    self.cpu_write(addr, self.cpu.a);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                }
                0x95 => {
                    // STA zero page,X
                    let addr = (self.cpu_read(self.cpu.pc) as u16 + self.cpu.x as u16) & 0xFF;
                    self.cpu_write(addr, self.cpu.a);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                }
                0x8E => {
                    // STX absolute
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = (hi << 8) | lo;
                    self.cpu_write(addr, self.cpu.x);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                }
                0x86 => {
                    // STX zero page
                    let addr = self.cpu_read(self.cpu.pc) as u16;
                    self.cpu_write(addr, self.cpu.x);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                }
                0x8C => {
                    // STY absolute
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = (hi << 8) | lo;
                    self.cpu_write(addr, self.cpu.y);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                }
                0x84 => {
                    // STY zero page
                    let addr = self.cpu_read(self.cpu.pc) as u16;
                    self.cpu_write(addr, self.cpu.y);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                }
                0x91 => {
                    // STA (zero page),Y - indirect indexed
                    let zp_addr = self.cpu_read(self.cpu.pc) as u16;
                    let lo = self.cpu_read(zp_addr) as u16;
                    let hi = self.cpu_read((zp_addr + 1) & 0xFF) as u16;
                    let addr = ((hi << 8) | lo).wrapping_add(self.cpu.y as u16);
                    self.cpu_write(addr, self.cpu.a);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                }
                
                // Compare instructions
                0xC9 => {
                    // CMP immediate
                    let value = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    let result = self.cpu.a.wrapping_sub(value);
                    self.set_zero_negative(result);
                    if self.cpu.a >= value { self.cpu.p |= 0x01; } else { self.cpu.p &= !0x01; }
                }
                0xE0 => {
                    // CPX immediate
                    let value = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    let result = self.cpu.x.wrapping_sub(value);
                    self.set_zero_negative(result);
                    if self.cpu.x >= value { self.cpu.p |= 0x01; } else { self.cpu.p &= !0x01; }
                }
                0xC0 => {
                    // CPY immediate
                    let value = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    let result = self.cpu.y.wrapping_sub(value);
                    self.set_zero_negative(result);
                    if self.cpu.y >= value { self.cpu.p |= 0x01; } else { self.cpu.p &= !0x01; }
                }
                
                // Increment/Decrement
                0xE8 => {
                    // INX
                    self.cpu.x = self.cpu.x.wrapping_add(1);
                    self.set_zero_negative(self.cpu.x);
                }
                0xCA => {
                    // DEX
                    self.cpu.x = self.cpu.x.wrapping_sub(1);
                    self.set_zero_negative(self.cpu.x);
                }
                0xC8 => {
                    // INY
                    self.cpu.y = self.cpu.y.wrapping_add(1);
                    self.set_zero_negative(self.cpu.y);
                }
                0x88 => {
                    // DEY
                    self.cpu.y = self.cpu.y.wrapping_sub(1);
                    self.set_zero_negative(self.cpu.y);
                }
                
                // Stack operations
                0x48 => {
                    // PHA - Push accumulator
                    self.cpu_write(0x0100 + self.cpu.sp as u16, self.cpu.a);
                    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
                }
                0x68 => {
                    // PLA - Pull accumulator
                    self.cpu.sp = self.cpu.sp.wrapping_add(1);
                    self.cpu.a = self.cpu_read(0x0100 + self.cpu.sp as u16);
                    self.set_zero_negative(self.cpu.a);
                }
                0x08 => {
                    // PHP - Push processor status
                    self.cpu_write(0x0100 + self.cpu.sp as u16, self.cpu.p | 0x10);
                    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
                }
                0x28 => {
                    // PLP - Pull processor status
                    self.cpu.sp = self.cpu.sp.wrapping_add(1);
                    self.cpu.p = self.cpu_read(0x0100 + self.cpu.sp as u16) & !0x10;
                }
                
                // Flag operations
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
                0x18 => {
                    // CLC - Clear carry
                    self.cpu.p &= !0x01;
                }
                0x38 => {
                    // SEC - Set carry
                    self.cpu.p |= 0x01;
                }
                0xB8 => {
                    // CLV - Clear overflow
                    self.cpu.p &= !0x40;
                }
                
                // Transfer instructions
                0xAA => {
                    // TAX
                    self.cpu.x = self.cpu.a;
                    self.set_zero_negative(self.cpu.x);
                }
                0x8A => {
                    // TXA
                    self.cpu.a = self.cpu.x;
                    self.set_zero_negative(self.cpu.a);
                }
                0xA8 => {
                    // TAY
                    self.cpu.y = self.cpu.a;
                    self.set_zero_negative(self.cpu.y);
                }
                0x98 => {
                    // TYA
                    self.cpu.a = self.cpu.y;
                    self.set_zero_negative(self.cpu.a);
                }
                0x9A => {
                    // TXS
                    self.cpu.sp = self.cpu.x;
                }
                0xBA => {
                    // TSX
                    self.cpu.x = self.cpu.sp;
                    self.set_zero_negative(self.cpu.x);
                }
                
                // Logical operations
                0x29 => {
                    // AND immediate
                    let value = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.cpu.a &= value;
                    self.set_zero_negative(self.cpu.a);
                }
                0x09 => {
                    // ORA immediate
                    let value = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.cpu.a |= value;
                    self.set_zero_negative(self.cpu.a);
                }
                0x49 => {
                    // EOR immediate
                    let value = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.cpu.a ^= value;
                    self.set_zero_negative(self.cpu.a);
                }
                
                // Arithmetic operations
                0x69 => {
                    // ADC immediate
                    let value = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    let carry = if self.cpu.p & 0x01 != 0 { 1 } else { 0 };
                    let result = self.cpu.a as u16 + value as u16 + carry;
                    
                    if result > 255 { self.cpu.p |= 0x01; } else { self.cpu.p &= !0x01; }
                    self.cpu.a = result as u8;
                    self.set_zero_negative(self.cpu.a);
                }
                0xE9 => {
                    // SBC immediate  
                    let value = self.cpu_read(self.cpu.pc);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    let carry = if self.cpu.p & 0x01 != 0 { 0 } else { 1 };
                    let result = self.cpu.a as i16 - value as i16 - carry;
                    
                    if result < 0 { self.cpu.p &= !0x01; } else { self.cpu.p |= 0x01; }
                    self.cpu.a = result as u8;
                    self.set_zero_negative(self.cpu.a);
                }
                
                // More addressing modes for LDA
                0xB5 => {
                    // LDA zero page,X
                    let addr = (self.cpu_read(self.cpu.pc) as u16 + self.cpu.x as u16) & 0xFF;
                    self.cpu.a = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(self.cpu.a);
                }
                0xBD => {
                    // LDA absolute,X
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = ((hi << 8) | lo).wrapping_add(self.cpu.x as u16);
                    self.cpu.a = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                    self.set_zero_negative(self.cpu.a);
                }
                0xB9 => {
                    // LDA absolute,Y
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = ((hi << 8) | lo).wrapping_add(self.cpu.y as u16);
                    self.cpu.a = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                    self.set_zero_negative(self.cpu.a);
                }
                
                // Memory operations
                0xE6 => {
                    // INC zero page
                    let addr = self.cpu_read(self.cpu.pc) as u16;
                    let value = self.cpu_read(addr).wrapping_add(1);
                    self.cpu_write(addr, value);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(value);
                }
                0xC6 => {
                    // DEC zero page
                    let addr = self.cpu_read(self.cpu.pc) as u16;
                    let value = self.cpu_read(addr).wrapping_sub(1);
                    self.cpu_write(addr, value);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(value);
                }
                
                // Bit operations
                0x24 => {
                    // BIT zero page
                    let addr = self.cpu_read(self.cpu.pc) as u16;
                    let value = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    
                    if (self.cpu.a & value) == 0 { self.cpu.p |= 0x02; } else { self.cpu.p &= !0x02; }
                    if value & 0x80 != 0 { self.cpu.p |= 0x80; } else { self.cpu.p &= !0x80; }
                    if value & 0x40 != 0 { self.cpu.p |= 0x40; } else { self.cpu.p &= !0x40; }
                }
                0x2C => {
                    // BIT absolute
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = (hi << 8) | lo;
                    let value = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                    
                    if (self.cpu.a & value) == 0 { self.cpu.p |= 0x02; } else { self.cpu.p &= !0x02; }
                    if value & 0x80 != 0 { self.cpu.p |= 0x80; } else { self.cpu.p &= !0x80; }
                    if value & 0x40 != 0 { self.cpu.p |= 0x40; } else { self.cpu.p &= !0x40; }
                }
                
                // Shift operations
                0x0A => {
                    // ASL accumulator
                    if self.cpu.a & 0x80 != 0 { self.cpu.p |= 0x01; } else { self.cpu.p &= !0x01; }
                    self.cpu.a <<= 1;
                    self.set_zero_negative(self.cpu.a);
                }
                0x4A => {
                    // LSR accumulator
                    if self.cpu.a & 0x01 != 0 { self.cpu.p |= 0x01; } else { self.cpu.p &= !0x01; }
                    self.cpu.a >>= 1;
                    self.set_zero_negative(self.cpu.a);
                }
                0x2A => {
                    // ROL accumulator
                    let carry = if self.cpu.p & 0x01 != 0 { 1 } else { 0 };
                    if self.cpu.a & 0x80 != 0 { self.cpu.p |= 0x01; } else { self.cpu.p &= !0x01; }
                    self.cpu.a = (self.cpu.a << 1) | carry;
                    self.set_zero_negative(self.cpu.a);
                }
                0x6A => {
                    // ROR accumulator
                    let carry = if self.cpu.p & 0x01 != 0 { 0x80 } else { 0 };
                    if self.cpu.a & 0x01 != 0 { self.cpu.p |= 0x01; } else { self.cpu.p &= !0x01; }
                    self.cpu.a = (self.cpu.a >> 1) | carry;
                    self.set_zero_negative(self.cpu.a);
                }
                
                // More load instructions
                0xB6 => {
                    // LDX zero page,Y
                    let addr = (self.cpu_read(self.cpu.pc) as u16 + self.cpu.y as u16) & 0xFF;
                    self.cpu.x = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(self.cpu.x);
                }
                0xBE => {
                    // LDX absolute,Y
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = ((hi << 8) | lo).wrapping_add(self.cpu.y as u16);
                    self.cpu.x = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                    self.set_zero_negative(self.cpu.x);
                }
                0xB4 => {
                    // LDY zero page,X
                    let addr = (self.cpu_read(self.cpu.pc) as u16 + self.cpu.x as u16) & 0xFF;
                    self.cpu.y = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    self.set_zero_negative(self.cpu.y);
                }
                0xBC => {
                    // LDY absolute,X
                    let lo = self.cpu_read(self.cpu.pc) as u16;
                    let hi = self.cpu_read(self.cpu.pc.wrapping_add(1)) as u16;
                    let addr = ((hi << 8) | lo).wrapping_add(self.cpu.x as u16);
                    self.cpu.y = self.cpu_read(addr);
                    self.cpu.pc = self.cpu.pc.wrapping_add(2);
                    self.set_zero_negative(self.cpu.y);
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
        self.set_controller_state(self.controller1);
        
        // Print button presses for debugging
        match keycode {
            Keycode::Z => println!("A button pressed"),
            Keycode::X => println!("B button pressed"),
            Keycode::A => println!("Select pressed"),
            Keycode::S => println!("Start pressed"),
            Keycode::Up => println!("Up pressed"),
            Keycode::Down => println!("Down pressed"),
            Keycode::Left => println!("Left pressed"),
            Keycode::Right => println!("Right pressed"),
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
        self.set_controller_state(self.controller1);
        
        // Print button releases for debugging
        match keycode {
            Keycode::Z => println!("A button released"),
            Keycode::X => println!("B button released"),
            Keycode::A => println!("Select released"),
            Keycode::S => println!("Start released"),
            Keycode::Up => println!("Up released"),
            Keycode::Down => println!("Down released"),
            Keycode::Left => println!("Left released"),
            Keycode::Right => println!("Right released"),
            _ => {}
        }
    }
    
    pub fn frame_ready(&self) -> bool {
        self.frame_complete
    }
    
    pub fn get_frame_buffer(&self) -> &[u8] { 
        self.ppu.get_frame_buffer()
    }
}
