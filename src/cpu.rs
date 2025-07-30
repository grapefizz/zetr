use crate::bus::Bus;

#[derive(Debug)]
pub struct CPU {
    pub a: u8,      // Accumulator
    pub x: u8,      // X register
    pub y: u8,      // Y register
    pub pc: u16,    // Program counter
    pub sp: u8,     // Stack pointer
    pub status: u8, // Status register
    pub cycles: u64,
}

// Status flags
const FLAG_CARRY: u8 = 0x01;
const FLAG_ZERO: u8 = 0x02;
const FLAG_INTERRUPT: u8 = 0x04;
const FLAG_DECIMAL: u8 = 0x08;
const FLAG_BREAK: u8 = 0x10;
const FLAG_UNUSED: u8 = 0x20;
const FLAG_OVERFLOW: u8 = 0x40;
const FLAG_NEGATIVE: u8 = 0x80;

impl CPU {
    pub fn new() -> Self {
        CPU {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            sp: 0xFD,
            status: FLAG_INTERRUPT | FLAG_UNUSED,
            cycles: 0,
        }
    }
    
    pub fn reset(&mut self, bus: &mut Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = FLAG_INTERRUPT | FLAG_UNUSED;
        
        // Load reset vector
        let lo = bus.read(0xFFFC) as u16;
        let hi = bus.read(0xFFFD) as u16;
        self.pc = (hi << 8) | lo;
        
        self.cycles = 0;
    }
    
    pub fn step(&mut self, read_fn: &mut dyn FnMut(u16) -> u8, write_fn: &mut dyn FnMut(u16, u8)) -> u8 {
        let opcode = read_fn(self.pc);
        self.pc = self.pc.wrapping_add(1);
        
        let cycles = self.execute_instruction(opcode, read_fn, write_fn);
        self.cycles = self.cycles.wrapping_add(cycles as u64);
        cycles
    }
    
    fn execute_instruction(&mut self, opcode: u8, read_fn: &mut dyn FnMut(u16) -> u8, write_fn: &mut dyn FnMut(u16, u8)) -> u8 {
        match opcode {
            // LDA - Load Accumulator
            0xA9 => { let val = self.immediate(bus); self.lda(val); 2 }
            0xA5 => { let val = self.zero_page(bus); self.lda(val); 3 }
            0xB5 => { let val = self.zero_page_x(bus); self.lda(val); 4 }
            0xAD => { let val = self.absolute(bus); self.lda(val); 4 }
            0xBD => { let val = self.absolute_x(bus); self.lda(val); 4 }
            0xB9 => { let val = self.absolute_y(bus); self.lda(val); 4 }
            0xA1 => { let val = self.indexed_indirect(bus); self.lda(val); 6 }
            0xB1 => { let val = self.indirect_indexed(bus); self.lda(val); 5 }
            
            // LDX - Load X Register
            0xA2 => { let val = self.immediate(bus); self.ldx(val); 2 }
            0xA6 => { let val = self.zero_page(bus); self.ldx(val); 3 }
            0xB6 => { let val = self.zero_page_y(bus); self.ldx(val); 4 }
            0xAE => { let val = self.absolute(bus); self.ldx(val); 4 }
            0xBE => { let val = self.absolute_y(bus); self.ldx(val); 4 }
            
            // LDY - Load Y Register
            0xA0 => { let val = self.immediate(bus); self.ldy(val); 2 }
            0xA4 => { let val = self.zero_page(bus); self.ldy(val); 3 }
            0xB4 => { let val = self.zero_page_x(bus); self.ldy(val); 4 }
            0xAC => { let val = self.absolute(bus); self.ldy(val); 4 }
            0xBC => { let val = self.absolute_x(bus); self.ldy(val); 4 }
            
            // STA - Store Accumulator
            0x85 => { self.zero_page_write(bus, self.a); 3 }
            0x95 => { self.zero_page_x_write(bus, self.a); 4 }
            0x8D => { self.absolute_write(bus, self.a); 4 }
            0x9D => { self.absolute_x_write(bus, self.a); 5 }
            0x99 => { self.absolute_y_write(bus, self.a); 5 }
            0x81 => { self.indexed_indirect_write(bus, self.a); 6 }
            0x91 => { self.indirect_indexed_write(bus, self.a); 6 }
            
            // JMP - Jump
            0x4C => { self.pc = self.absolute_address(bus); 3 }
            0x6C => { self.pc = self.indirect_address(bus); 5 }
            
            // JSR - Jump to Subroutine
            0x20 => { self.jsr(bus); 6 }
            
            // RTS - Return from Subroutine
            0x60 => { self.rts(bus); 6 }
            
            // BNE - Branch if Not Equal
            0xD0 => { self.branch(!self.get_flag(FLAG_ZERO), bus) }
            
            // BEQ - Branch if Equal
            0xF0 => { self.branch(self.get_flag(FLAG_ZERO), bus) }
            
            // BPL - Branch if Positive
            0x10 => { self.branch(!self.get_flag(FLAG_NEGATIVE), bus) }
            
            // BMI - Branch if Minus
            0x30 => { self.branch(self.get_flag(FLAG_NEGATIVE), bus) }
            
            // BCC - Branch if Carry Clear
            0x90 => { self.branch(!self.get_flag(FLAG_CARRY), bus) }
            
            // BCS - Branch if Carry Set
            0xB0 => { self.branch(self.get_flag(FLAG_CARRY), bus) }
            
            // BVC - Branch if Overflow Clear
            0x50 => { self.branch(!self.get_flag(FLAG_OVERFLOW), bus) }
            
            // BVS - Branch if Overflow Set
            0x70 => { self.branch(self.get_flag(FLAG_OVERFLOW), bus) }
            
            // CMP - Compare Accumulator
            0xC9 => { let val = self.immediate(bus); self.cmp(val); 2 }
            0xC5 => { let val = self.zero_page(bus); self.cmp(val); 3 }
            0xD5 => { let val = self.zero_page_x(bus); self.cmp(val); 4 }
            0xCD => { let val = self.absolute(bus); self.cmp(val); 4 }
            0xDD => { let val = self.absolute_x(bus); self.cmp(val); 4 }
            0xD9 => { let val = self.absolute_y(bus); self.cmp(val); 4 }
            0xC1 => { let val = self.indexed_indirect(bus); self.cmp(val); 6 }
            0xD1 => { let val = self.indirect_indexed(bus); self.cmp(val); 5 }
            
            // INX - Increment X
            0xE8 => { self.inx(); 2 }
            
            // INY - Increment Y
            0xC8 => { self.iny(); 2 }
            
            // DEX - Decrement X
            0xCA => { self.dex(); 2 }
            
            // DEY - Decrement Y
            0x88 => { self.dey(); 2 }
            
            // TAX - Transfer A to X
            0xAA => { self.tax(); 2 }
            
            // TAY - Transfer A to Y
            0xA8 => { self.tay(); 2 }
            
            // TXA - Transfer X to A
            0x8A => { self.txa(); 2 }
            
            // TYA - Transfer Y to A
            0x98 => { self.tya(); 2 }
            
            // NOP - No Operation
            0xEA => { 2 }
            
            // SEC - Set Carry
            0x38 => { self.set_flag(FLAG_CARRY, true); 2 }
            
            // CLC - Clear Carry
            0x18 => { self.set_flag(FLAG_CARRY, false); 2 }
            
            // SEI - Set Interrupt Disable
            0x78 => { self.set_flag(FLAG_INTERRUPT, true); 2 }
            
            // CLI - Clear Interrupt Disable
            0x58 => { self.set_flag(FLAG_INTERRUPT, false); 2 }
            
            // CLD - Clear Decimal
            0xD8 => { self.set_flag(FLAG_DECIMAL, false); 2 }
            
            // SED - Set Decimal
            0xF8 => { self.set_flag(FLAG_DECIMAL, true); 2 }
            
            // CLV - Clear Overflow
            0xB8 => { self.set_flag(FLAG_OVERFLOW, false); 2 }
            
            // ADC - Add with Carry
            0x69 => { let val = self.immediate(bus); self.adc(val); 2 }
            0x65 => { let val = self.zero_page(bus); self.adc(val); 3 }
            0x75 => { let val = self.zero_page_x(bus); self.adc(val); 4 }
            0x6D => { let val = self.absolute(bus); self.adc(val); 4 }
            0x7D => { let val = self.absolute_x(bus); self.adc(val); 4 }
            0x79 => { let val = self.absolute_y(bus); self.adc(val); 4 }
            0x61 => { let val = self.indexed_indirect(bus); self.adc(val); 6 }
            0x71 => { let val = self.indirect_indexed(bus); self.adc(val); 5 }
            
            // SBC - Subtract with Carry
            0xE9 => { let val = self.immediate(bus); self.sbc(val); 2 }
            0xE5 => { let val = self.zero_page(bus); self.sbc(val); 3 }
            0xF5 => { let val = self.zero_page_x(bus); self.sbc(val); 4 }
            0xED => { let val = self.absolute(bus); self.sbc(val); 4 }
            0xFD => { let val = self.absolute_x(bus); self.sbc(val); 4 }
            0xF9 => { let val = self.absolute_y(bus); self.sbc(val); 4 }
            0xE1 => { let val = self.indexed_indirect(bus); self.sbc(val); 6 }
            0xF1 => { let val = self.indirect_indexed(bus); self.sbc(val); 5 }
            
            // PHA - Push Accumulator
            0x48 => { self.push(bus, self.a); 3 }
            
            // PLA - Pull Accumulator
            0x68 => { let val = self.pull(bus); self.lda(val); 4 }
            
            // PHP - Push Processor Status
            0x08 => { self.push(bus, self.status | FLAG_BREAK | FLAG_UNUSED); 3 }
            
            // PLP - Pull Processor Status
            0x28 => { self.status = (self.pull(bus) & !FLAG_BREAK) | FLAG_UNUSED; 4 }
            
            // TXS - Transfer X to Stack Pointer
            0x9A => { self.sp = self.x; 2 }
            
            // TSX - Transfer Stack Pointer to X
            0xBA => { self.x = self.sp; self.set_zn(self.x); 2 }
            
            // RTI - Return from Interrupt
            0x40 => { self.rti(bus); 6 }
            
            // BRK - Break
            0x00 => { self.brk(bus); 7 }
            
            _ => {
                // Unknown opcode, treat as NOP
                2
            }
        }
    }
    
    // Addressing modes
    fn immediate(&mut self, bus: &mut Bus) -> u8 {
        let val = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }
    
    fn zero_page(&mut self, bus: &mut Bus) -> u8 {
        let addr = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        bus.read(addr)
    }
    
    fn zero_page_x(&mut self, bus: &mut Bus) -> u8 {
        let addr = (bus.read(self.pc).wrapping_add(self.x)) as u16;
        self.pc = self.pc.wrapping_add(1);
        bus.read(addr)
    }
    
    fn zero_page_y(&mut self, bus: &mut Bus) -> u8 {
        let addr = (bus.read(self.pc).wrapping_add(self.y)) as u16;
        self.pc = self.pc.wrapping_add(1);
        bus.read(addr)
    }
    
    fn absolute(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.absolute_address(bus);
        bus.read(addr)
    }
    
    fn absolute_address(&mut self, bus: &mut Bus) -> u16 {
        let lo = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        let hi = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        (hi << 8) | lo
    }
    
    fn absolute_x(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.absolute_address(bus).wrapping_add(self.x as u16);
        bus.read(addr)
    }
    
    fn absolute_y(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.absolute_address(bus).wrapping_add(self.y as u16);
        bus.read(addr)
    }
    
    fn indexed_indirect(&mut self, bus: &mut Bus) -> u8 {
        let base = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        let addr_lo = (base.wrapping_add(self.x)) as u16;
        let addr_hi = (base.wrapping_add(self.x).wrapping_add(1)) as u16;
        let lo = bus.read(addr_lo) as u16;
        let hi = bus.read(addr_hi) as u16;
        let addr = (hi << 8) | lo;
        bus.read(addr)
    }
    
    fn indirect_indexed(&mut self, bus: &mut Bus) -> u8 {
        let base = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        let lo = bus.read(base) as u16;
        let hi = bus.read((base + 1) & 0xFF) as u16;
        let addr = ((hi << 8) | lo).wrapping_add(self.y as u16);
        bus.read(addr)
    }
    
    fn indirect_address(&mut self, bus: &mut Bus) -> u16 {
        let addr_lo = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        let addr_hi = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        let addr = (addr_hi << 8) | addr_lo;
        
        // JMP indirect bug: if addr_lo is 0xFF, high byte comes from addr & 0xFF00
        let lo = bus.read(addr) as u16;
        let hi_addr = if addr_lo == 0xFF {
            addr & 0xFF00
        } else {
            addr + 1
        };
        let hi = bus.read(hi_addr) as u16;
        (hi << 8) | lo
    }
    
    // Write addressing modes
    fn zero_page_write(&mut self, bus: &mut Bus, data: u8) {
        let addr = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        bus.write(addr, data);
    }
    
    fn zero_page_x_write(&mut self, bus: &mut Bus, data: u8) {
        let addr = (bus.read(self.pc).wrapping_add(self.x)) as u16;
        self.pc = self.pc.wrapping_add(1);
        bus.write(addr, data);
    }
    
    fn absolute_write(&mut self, bus: &mut Bus, data: u8) {
        let addr = self.absolute_address(bus);
        bus.write(addr, data);
    }
    
    fn absolute_x_write(&mut self, bus: &mut Bus, data: u8) {
        let addr = self.absolute_address(bus).wrapping_add(self.x as u16);
        bus.write(addr, data);
    }
    
    fn absolute_y_write(&mut self, bus: &mut Bus, data: u8) {
        let addr = self.absolute_address(bus).wrapping_add(self.y as u16);
        bus.write(addr, data);
    }
    
    fn indexed_indirect_write(&mut self, bus: &mut Bus, data: u8) {
        let base = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        let addr_lo = (base.wrapping_add(self.x)) as u16;
        let addr_hi = (base.wrapping_add(self.x).wrapping_add(1)) as u16;
        let lo = bus.read(addr_lo) as u16;
        let hi = bus.read(addr_hi) as u16;
        let addr = (hi << 8) | lo;
        bus.write(addr, data);
    }
    
    fn indirect_indexed_write(&mut self, bus: &mut Bus, data: u8) {
        let base = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        let lo = bus.read(base) as u16;
        let hi = bus.read((base + 1) & 0xFF) as u16;
        let addr = ((hi << 8) | lo).wrapping_add(self.y as u16);
        bus.write(addr, data);
    }
    
    // Instructions
    fn lda(&mut self, val: u8) {
        self.a = val;
        self.set_zn(self.a);
    }
    
    fn ldx(&mut self, val: u8) {
        self.x = val;
        self.set_zn(self.x);
    }
    
    fn ldy(&mut self, val: u8) {
        self.y = val;
        self.set_zn(self.y);
    }
    
    fn cmp(&mut self, val: u8) {
        let result = self.a.wrapping_sub(val);
        self.set_flag(FLAG_CARRY, self.a >= val);
        self.set_zn(result);
    }
    
    fn adc(&mut self, val: u8) {
        let carry = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
        let result = self.a as u16 + val as u16 + carry;
        
        self.set_flag(FLAG_CARRY, result > 0xFF);
        self.set_flag(FLAG_OVERFLOW, 
            (self.a ^ result as u8) & (val ^ result as u8) & 0x80 != 0);
        
        self.a = result as u8;
        self.set_zn(self.a);
    }
    
    fn sbc(&mut self, val: u8) {
        let carry = if self.get_flag(FLAG_CARRY) { 0 } else { 1 };
        let result = self.a as i16 - val as i16 - carry as i16;
        
        self.set_flag(FLAG_CARRY, result >= 0);
        self.set_flag(FLAG_OVERFLOW,
            (self.a ^ result as u8) & ((255 - val) ^ result as u8) & 0x80 != 0);
        
        self.a = result as u8;
        self.set_zn(self.a);
    }
    
    fn inx(&mut self) {
        self.x = self.x.wrapping_add(1);
        self.set_zn(self.x);
    }
    
    fn iny(&mut self) {
        self.y = self.y.wrapping_add(1);
        self.set_zn(self.y);
    }
    
    fn dex(&mut self) {
        self.x = self.x.wrapping_sub(1);
        self.set_zn(self.x);
    }
    
    fn dey(&mut self) {
        self.y = self.y.wrapping_sub(1);
        self.set_zn(self.y);
    }
    
    fn tax(&mut self) {
        self.x = self.a;
        self.set_zn(self.x);
    }
    
    fn tay(&mut self) {
        self.y = self.a;
        self.set_zn(self.y);
    }
    
    fn txa(&mut self) {
        self.a = self.x;
        self.set_zn(self.a);
    }
    
    fn tya(&mut self) {
        self.a = self.y;
        self.set_zn(self.a);
    }
    
    fn jsr(&mut self, bus: &mut Bus) {
        let ret_addr = self.pc + 1;
        self.push(bus, (ret_addr >> 8) as u8);
        self.push(bus, ret_addr as u8);
        self.pc = self.absolute_address(bus);
    }
    
    fn rts(&mut self, bus: &mut Bus) {
        let lo = self.pull(bus) as u16;
        let hi = self.pull(bus) as u16;
        self.pc = ((hi << 8) | lo) + 1;
    }
    
    fn rti(&mut self, bus: &mut Bus) {
        self.status = (self.pull(bus) & !FLAG_BREAK) | FLAG_UNUSED;
        let lo = self.pull(bus) as u16;
        let hi = self.pull(bus) as u16;
        self.pc = (hi << 8) | lo;
    }
    
    fn brk(&mut self, bus: &mut Bus) {
        self.pc = self.pc.wrapping_add(1);
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, self.pc as u8);
        self.push(bus, self.status | FLAG_BREAK | FLAG_UNUSED);
        self.set_flag(FLAG_INTERRUPT, true);
        
        let lo = bus.read(0xFFFE) as u16;
        let hi = bus.read(0xFFFF) as u16;
        self.pc = (hi << 8) | lo;
    }
    
    fn branch(&mut self, condition: bool, bus: &mut Bus) -> u8 {
        let offset = self.immediate(bus) as i8;
        if condition {
            let old_pc = self.pc;
            self.pc = self.pc.wrapping_add(offset as u16);
            // Page crossing adds extra cycle
            if (old_pc & 0xFF00) != (self.pc & 0xFF00) {
                4
            } else {
                3
            }
        } else {
            2
        }
    }
    
    fn push(&mut self, bus: &mut Bus, data: u8) {
        bus.write(0x0100 + self.sp as u16, data);
        self.sp = self.sp.wrapping_sub(1);
    }
    
    fn pull(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 + self.sp as u16)
    }
    
    // Flag operations
    fn get_flag(&self, flag: u8) -> bool {
        self.status & flag != 0
    }
    
    fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.status |= flag;
        } else {
            self.status &= !flag;
        }
    }
    
    fn set_zn(&mut self, val: u8) {
        self.set_flag(FLAG_ZERO, val == 0);
        self.set_flag(FLAG_NEGATIVE, val & 0x80 != 0);
    }
    
    pub fn nmi(&mut self, bus: &mut Bus) {
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, self.pc as u8);
        self.push(bus, self.status & !FLAG_BREAK | FLAG_UNUSED);
        self.set_flag(FLAG_INTERRUPT, true);
        
        let lo = bus.read(0xFFFA) as u16;
        let hi = bus.read(0xFFFB) as u16;
        self.pc = (hi << 8) | lo;
    }
}
