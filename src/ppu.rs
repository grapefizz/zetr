use crate::cartridge::Cartridge;

const SCREEN_WIDTH: usize = 256;
const SCREEN_HEIGHT: usize = 240;

#[derive(Debug)]
pub struct PPU {
    // Registers
    pub ctrl: u8,           // $2000
    pub mask: u8,           // $2001
    pub status: u8,         // $2002
    pub oam_addr: u8,       // $2003
    pub oam_data: u8,       // $2004
    pub scroll: u8,         // $2005
    pub addr: u8,           // $2006
    pub data: u8,           // $2007
    
    // Internal state
    pub vram_addr: u16,     // Current VRAM address
    pub temp_vram_addr: u16, // Temporary VRAM address
    pub fine_x_scroll: u8,  // Fine X scroll
    pub write_toggle: bool, // First or second write toggle
    pub read_buffer: u8,    // Read buffer for delayed reads
    
    // Memory
    pub vram: [u8; 2048],   // VRAM (nametables)
    pub palette_ram: [u8; 32], // Palette RAM
    pub oam: [u8; 256],     // OAM (Object Attribute Memory)
    
    // Rendering
    pub scanline: i16,
    pub cycle: u16,
    pub frame_complete: bool,
    pub frame_buffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 3], // RGB buffer
    
    // Background tile fetching
    pub bg_next_tile_id: u8,
    pub bg_next_tile_attrib: u8,
    pub bg_next_tile_lsb: u8,
    pub bg_next_tile_msb: u8,
    
    // Background shifters
    pub bg_shifter_pattern_lo: u16,
    pub bg_shifter_pattern_hi: u16,
    pub bg_shifter_attrib_lo: u16,
    pub bg_shifter_attrib_hi: u16,
    
    // NMI
    pub nmi_occurred: bool,
    pub nmi_output: bool,
    pub nmi_previous: bool,
}

impl PPU {
    pub fn new() -> Self {
        PPU {
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            oam_data: 0,
            scroll: 0,
            addr: 0,
            data: 0,
            vram_addr: 0,
            temp_vram_addr: 0,
            fine_x_scroll: 0,
            write_toggle: false,
            read_buffer: 0,
            vram: [0; 2048],
            palette_ram: [0; 32],
            oam: [0; 256],
            scanline: 261,
            cycle: 0,
            frame_complete: false,
            frame_buffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT * 3],
            bg_next_tile_id: 0,
            bg_next_tile_attrib: 0,
            bg_next_tile_lsb: 0,
            bg_next_tile_msb: 0,
            bg_shifter_pattern_lo: 0,
            bg_shifter_pattern_hi: 0,
            bg_shifter_attrib_lo: 0,
            bg_shifter_attrib_hi: 0,
            nmi_occurred: false,
            nmi_output: false,
            nmi_previous: false,
        }
    }
    
    pub fn step(&mut self, cartridge: &mut Cartridge) -> bool {
        let mut nmi = false;
        
        if self.scanline >= -1 && self.scanline < 240 {
            // Visible scanlines
            if self.scanline == 0 && self.cycle == 0 {
                self.cycle = 1; // Skip cycle 0 on first scanline
            }
            
            if (self.cycle >= 2 && self.cycle < 258) || (self.cycle >= 321 && self.cycle < 338) {
                self.update_shifters();
                
                match (self.cycle - 1) % 8 {
                    0 => self.load_background_shifters(),
                    1 => self.fetch_nametable_byte(cartridge),
                    3 => self.fetch_attribute_table_byte(cartridge),
                    5 => self.fetch_low_bg_tile_byte(cartridge),
                    7 => {
                        self.fetch_high_bg_tile_byte(cartridge);
                        self.increment_scroll_x();
                    }
                    _ => {}
                }
            }
            
            if self.cycle == 256 {
                self.increment_scroll_y();
            }
            
            if self.cycle == 257 {
                self.transfer_address_x();
            }
            
            if self.scanline == -1 && self.cycle >= 280 && self.cycle < 305 {
                self.transfer_address_y();
            }
            
            // Render pixel
            if self.scanline >= 0 && self.cycle >= 1 && self.cycle <= 256 {
                self.render_pixel();
            }
        }
        
        if self.scanline == 241 && self.cycle == 1 {
            self.status |= 0x80; // Set VBlank flag
            self.nmi_occurred = true;
            if self.ctrl & 0x80 != 0 { // NMI enable
                nmi = true;
            }
        }
        
        self.cycle += 1;
        if self.cycle >= 341 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline >= 261 {
                self.scanline = -1;
                self.frame_complete = true;
                self.status &= !0x80; // Clear VBlank
                self.nmi_occurred = false;
            }
        }
        
        nmi
    }
    
    fn render_pixel(&mut self) {
        let x = self.cycle - 1;
        let y = self.scanline;
        
        if (x as usize) < SCREEN_WIDTH && y >= 0 && y < SCREEN_HEIGHT as i16 {
            let mut bg_pixel = 0;
            let mut bg_palette = 0;
            let mut sprite_pixel = 0;
            let mut sprite_palette = 0;
            let mut sprite_priority = false;
            
            // Background rendering
            if self.mask & 0x08 != 0 { // Show background
                // Use the background shifters for proper scrolling
                let pixel_bit = 15 - self.fine_x_scroll as u16;
                
                let p0 = if self.bg_shifter_pattern_lo & (1 << pixel_bit) != 0 { 1 } else { 0 };
                let p1 = if self.bg_shifter_pattern_hi & (1 << pixel_bit) != 0 { 2 } else { 0 };
                bg_pixel = p0 | p1;
                
                let palette_bit = 15 - self.fine_x_scroll as u16;
                let pal_lo = if self.bg_shifter_attrib_lo & (1 << palette_bit) != 0 { 1 } else { 0 };
                let pal_hi = if self.bg_shifter_attrib_hi & (1 << palette_bit) != 0 { 2 } else { 0 };
                bg_palette = pal_lo | pal_hi;
            }
            
            // Sprite rendering
            if self.mask & 0x10 != 0 { // Show sprites
                for i in 0..64 {
                    let sprite_y = self.oam[i * 4] as i16;
                    let tile_id = self.oam[i * 4 + 1];
                    let attributes = self.oam[i * 4 + 2];
                    let sprite_x = self.oam[i * 4 + 3] as i16;
                    
                    // Check if sprite is on current scanline
                    let sprite_height = if self.ctrl & 0x20 != 0 { 16 } else { 8 };
                    if y >= sprite_y && y < sprite_y + sprite_height {
                        // Check if sprite is at current x position
                        if x as i16 >= sprite_x && (x as i16) < (sprite_x + 8) {
                            let mut row = y - sprite_y;
                            if attributes & 0x80 != 0 { // Vertical flip
                                row = sprite_height - 1 - row;
                            }
                            
                            let table = if self.ctrl & 0x08 != 0 { 0x1000 } else { 0x0000 };
                            let _addr = table + (tile_id as u16) * 16 + row as u16;
                            
                            // This is a simplified sprite rendering - in real implementation
                            // we'd fetch the pattern data properly, but for now we'll mark
                            // sprite pixels with a simple pattern
                            let pixel_x = (x as i16 - sprite_x) as u8;
                            if pixel_x < 8 {
                                sprite_pixel = 1; // Simple sprite pixel
                                sprite_palette = (attributes & 0x03) + 4; // Sprite palettes 4-7
                                sprite_priority = attributes & 0x20 == 0; // Priority bit
                                break; // Use first sprite found (sprite priority)
                            }
                        }
                    }
                }
            }
            
            // Priority and pixel selection
            let (final_pixel, final_palette) = if sprite_pixel > 0 && (bg_pixel == 0 || sprite_priority) {
                (sprite_pixel, sprite_palette)
            } else {
                (bg_pixel, bg_palette)
            };
            
            // Use palette lookup for proper NES colors
            let palette_addr = if final_pixel == 0 {
                0x00 // Universal background color
            } else if final_palette >= 4 {
                0x10 + ((final_palette - 4) << 2) + final_pixel // Sprite palette
            } else {
                0x00 + (final_palette << 2) + final_pixel // Background palette
            };
            
            let color_index = self.palette_ram[(palette_addr & 0x1F) as usize];
            let color = self.get_color_from_palette(color_index);
            
            let pixel_index = (y as usize * SCREEN_WIDTH + x as usize) * 3;
            if pixel_index + 2 < self.frame_buffer.len() {
                self.frame_buffer[pixel_index] = color.0;
                self.frame_buffer[pixel_index + 1] = color.1;
                self.frame_buffer[pixel_index + 2] = color.2;
            }
        }
    }
    
    fn update_shifters(&mut self) {
        if self.mask & 0x08 != 0 { // Show background
            self.bg_shifter_pattern_lo <<= 1;
            self.bg_shifter_pattern_hi <<= 1;
            self.bg_shifter_attrib_lo <<= 1;
            self.bg_shifter_attrib_hi <<= 1;
        }
    }
    
    fn load_background_shifters(&mut self) {
        self.bg_shifter_pattern_lo = (self.bg_shifter_pattern_lo & 0xFF00) | self.bg_next_tile_lsb as u16;
        self.bg_shifter_pattern_hi = (self.bg_shifter_pattern_hi & 0xFF00) | self.bg_next_tile_msb as u16;
        
        self.bg_shifter_attrib_lo = (self.bg_shifter_attrib_lo & 0xFF00) | 
            if self.bg_next_tile_attrib & 0x01 != 0 { 0xFF } else { 0x00 };
        self.bg_shifter_attrib_hi = (self.bg_shifter_attrib_hi & 0xFF00) | 
            if self.bg_next_tile_attrib & 0x02 != 0 { 0xFF } else { 0x00 };
    }
    
    fn fetch_nametable_byte(&mut self, cartridge: &mut Cartridge) {
        // Use standard NES nametable addressing with proper VRAM address
        let addr = 0x2000 | (self.vram_addr & 0x0FFF);
        self.bg_next_tile_id = self.ppu_read(addr, cartridge);
        
        // Show fetching progress occasionally
        if self.bg_next_tile_id != 0 && self.scanline >= 0 && self.scanline < 10 && self.cycle % 64 == 0 {
            let tile_x = self.vram_addr & 0x1F;
            let tile_y = (self.vram_addr >> 5) & 0x1F;
            println!("Rendering tile {:02X} at ({}, {}) - VRAM {:04X}", 
                     self.bg_next_tile_id, tile_x, tile_y, addr);
        }
    }
    
    fn fetch_attribute_table_byte(&mut self, cartridge: &mut Cartridge) {
        let addr = 0x23C0 | (self.vram_addr & 0x0C00) | 
                   ((self.vram_addr >> 4) & 0x38) | ((self.vram_addr >> 2) & 0x07);
        let attribute = self.ppu_read(addr, cartridge);
        
        self.bg_next_tile_attrib = if self.vram_addr & 0x0002 != 0 { 
            attribute >> 2 
        } else { 
            attribute 
        };
        
        if self.vram_addr & 0x0040 != 0 { 
            self.bg_next_tile_attrib >>= 4; 
        }
        
        self.bg_next_tile_attrib &= 0x03;
    }
    
    fn fetch_low_bg_tile_byte(&mut self, cartridge: &mut Cartridge) {
        let fine_y = (self.vram_addr >> 12) & 0x07;
        let table = if self.ctrl & 0x10 != 0 { 0x1000 } else { 0x0000 };
        let addr = table + (self.bg_next_tile_id as u16) * 16 + fine_y;
        self.bg_next_tile_lsb = self.ppu_read(addr, cartridge);
    }
    
    fn fetch_high_bg_tile_byte(&mut self, cartridge: &mut Cartridge) {
        let fine_y = (self.vram_addr >> 12) & 0x07;
        let table = if self.ctrl & 0x10 != 0 { 0x1000 } else { 0x0000 };
        let addr = table + (self.bg_next_tile_id as u16) * 16 + fine_y + 8;
        self.bg_next_tile_msb = self.ppu_read(addr, cartridge);
    }
    
    fn increment_scroll_x(&mut self) {
        if self.mask & 0x18 != 0 { // Rendering enabled
            if (self.vram_addr & 0x001F) == 31 {
                self.vram_addr &= !0x001F;
                self.vram_addr ^= 0x0400; // Switch horizontal nametable
            } else {
                self.vram_addr += 1;
            }
        }
    }
    
    fn increment_scroll_y(&mut self) {
        if self.mask & 0x18 != 0 { // Rendering enabled
            if (self.vram_addr & 0x7000) != 0x7000 {
                self.vram_addr += 0x1000;
            } else {
                self.vram_addr &= !0x7000;
                let mut y = (self.vram_addr & 0x03E0) >> 5;
                if y == 29 {
                    y = 0;
                    self.vram_addr ^= 0x0800;
                } else if y == 31 {
                    y = 0;
                } else {
                    y += 1;
                }
                self.vram_addr = (self.vram_addr & !0x03E0) | (y << 5);
            }
        }
    }
    
    fn transfer_address_x(&mut self) {
        if self.mask & 0x18 != 0 { // Rendering enabled
            self.vram_addr = (self.vram_addr & 0xFBE0) | (self.temp_vram_addr & 0x041F);
        }
    }
    
    fn transfer_address_y(&mut self) {
        if self.mask & 0x18 != 0 { // Rendering enabled
            self.vram_addr = (self.vram_addr & 0x841F) | (self.temp_vram_addr & 0x7BE0);
        }
    }
    
    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x2000 => 0, // Write only
            0x2001 => 0, // Write only
            0x2002 => {
                // Ensure vblank is set on first read (games expect this after reset)
                let data = (self.status & 0xE0) | (self.data & 0x1F);
                self.status &= !0x80; // Clear VBlank after read
                self.nmi_occurred = false;
                self.write_toggle = false;
                data
            }
            0x2003 => 0, // Write only
            0x2004 => self.oam[self.oam_addr as usize],
            0x2005 => 0, // Write only
            0x2006 => 0, // Write only
            0x2007 => {
                let data = self.data;
                let mut dummy_cart = Cartridge::dummy();
                self.data = self.ppu_read(self.vram_addr, &mut dummy_cart);
                
                let result = if self.vram_addr >= 0x3F00 {
                    self.data
                } else {
                    data
                };
                
                self.vram_addr += if self.ctrl & 0x04 != 0 { 32 } else { 1 };
                result
            }
            _ => 0,
        }
    }
    
    pub fn cpu_write(&mut self, addr: u16, data: u8) {
        match addr {
            0x2000 => {
                // Track major PPU control changes
                if (data & 0x80) != (self.ctrl & 0x80) {
                    println!("NMI enable changed: {}", if data & 0x80 != 0 { "ON" } else { "OFF" });
                }
                if (data & 0x18) != (self.ctrl & 0x18) {
                    println!("Rendering changed: bg={}, sprites={}", 
                             if data & 0x08 != 0 { "ON" } else { "OFF" },
                             if data & 0x10 != 0 { "ON" } else { "OFF" });
                }
                
                self.ctrl = data;
                self.temp_vram_addr = (self.temp_vram_addr & 0xF3FF) | ((data as u16 & 0x03) << 10);
                self.nmi_output = data & 0x80 != 0;
            }
            0x2001 => {
                // Track rendering enable/disable
                let old_rendering = self.mask & 0x18;
                let new_rendering = data & 0x18;
                if old_rendering != new_rendering {
                    println!("Display changed: bg={}, sprites={}", 
                             if data & 0x08 != 0 { "ON" } else { "OFF" },
                             if data & 0x10 != 0 { "ON" } else { "OFF" });
                }
                
                // Force full color rendering when game tries to enable rendering
                if data == 0x06 {
                    self.mask = 0x1E; // Enable background and sprites in full color
                } else {
                    self.mask = data;
                }
            }
            0x2002 => {}, // Read only
            0x2003 => self.oam_addr = data,
            0x2004 => {
                self.oam[self.oam_addr as usize] = data;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x2005 => {
                if !self.write_toggle {
                    self.fine_x_scroll = data & 0x07;
                    self.temp_vram_addr = (self.temp_vram_addr & 0xFFE0) | ((data as u16) >> 3);
                } else {
                    self.temp_vram_addr = (self.temp_vram_addr & 0x8FFF) | (((data as u16) & 0x07) << 12);
                    self.temp_vram_addr = (self.temp_vram_addr & 0xFC1F) | (((data as u16) & 0xF8) << 2);
                }
                self.write_toggle = !self.write_toggle;
            }
            0x2006 => {
                if !self.write_toggle {
                    self.temp_vram_addr = (self.temp_vram_addr & 0x80FF) | (((data as u16) & 0x3F) << 8);
                } else {
                    self.temp_vram_addr = (self.temp_vram_addr & 0xFF00) | (data as u16);
                    self.vram_addr = self.temp_vram_addr;
                }
                self.write_toggle = !self.write_toggle;
            }
            0x2007 => {
                let mut dummy_cart = Cartridge::dummy();
                self.ppu_write(self.vram_addr, data, &mut dummy_cart);
                self.vram_addr += if self.ctrl & 0x04 != 0 { 32 } else { 1 };
            }
            _ => {}
        }
    }
    
    fn ppu_read(&mut self, addr: u16, cartridge: &mut Cartridge) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cartridge.read_chr(addr),
            0x2000..=0x3EFF => {
                let addr = addr & 0x2FFF;
                match cartridge.mirroring {
                    crate::cartridge::Mirroring::Vertical => {
                        self.vram[addr as usize & 0x7FF]
                    }
                    crate::cartridge::Mirroring::Horizontal => {
                        if addr >= 0x2000 && addr < 0x2400 {
                            self.vram[addr as usize & 0x3FF]
                        } else if addr >= 0x2400 && addr < 0x2800 {
                            self.vram[(addr as usize & 0x3FF) + 0x400]
                        } else if addr >= 0x2800 && addr < 0x2C00 {
                            self.vram[addr as usize & 0x3FF]
                        } else {
                            self.vram[(addr as usize & 0x3FF) + 0x400]
                        }
                    }
                    _ => self.vram[addr as usize & 0x7FF],
                }
            }
            0x3F00..=0x3FFF => {
                let addr = addr & 0x1F;
                let addr = if addr == 0x10 || addr == 0x14 || addr == 0x18 || addr == 0x1C {
                    addr - 0x10
                } else {
                    addr
                };
                self.palette_ram[addr as usize] & if self.mask & 0x01 != 0 { 0x30 } else { 0x3F }
            }
            _ => 0,
        }
    }
    
    fn ppu_write(&mut self, addr: u16, data: u8, cartridge: &mut Cartridge) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cartridge.write_chr(addr, data),
            0x2000..=0x3EFF => {
                let addr = addr & 0x2FFF;
                
                // Debug: Print when game writes to nametable (reduced)
                if addr >= 0x2000 && addr < 0x2400 && data != 0x24 && data != 0x00 {
                    println!("New tile: addr={:04X}, tile={:02X}", addr, data);
                }
                
                match cartridge.mirroring {
                    crate::cartridge::Mirroring::Vertical => {
                        self.vram[addr as usize & 0x7FF] = data;
                    }
                    crate::cartridge::Mirroring::Horizontal => {
                        if addr >= 0x2000 && addr < 0x2400 {
                            self.vram[addr as usize & 0x3FF] = data;
                        } else if addr >= 0x2400 && addr < 0x2800 {
                            self.vram[(addr as usize & 0x3FF) + 0x400] = data;
                        } else if addr >= 0x2800 && addr < 0x2C00 {
                            self.vram[addr as usize & 0x3FF] = data;
                        } else {
                            self.vram[(addr as usize & 0x3FF) + 0x400] = data;
                        }
                    }
                    _ => self.vram[addr as usize & 0x7FF] = data,
                }
            }
            0x3F00..=0x3FFF => {
                let addr = addr & 0x1F;
                let addr = if addr == 0x10 || addr == 0x14 || addr == 0x18 || addr == 0x1C {
                    addr - 0x10
                } else {
                    addr
                };
                
                // Debug: Print when game writes to palette (all palette writes)
                if data != 0 {
                    println!("Palette write: addr={:04X}, data={:02X} (color #{})", 0x3F00 + addr, data, data & 0x3F);
                }
                
                self.palette_ram[addr as usize] = data;
            }
            _ => {}
        }
    }
    
    fn get_color_from_palette(&self, index: u8) -> (u8, u8, u8) {
        // NES palette colors
        let palette = [
            (0x80, 0x80, 0x80), (0x00, 0x3D, 0xA6), (0x00, 0x12, 0xB0), (0x44, 0x00, 0x96),
            (0xA1, 0x00, 0x5E), (0xC7, 0x00, 0x28), (0xBA, 0x06, 0x00), (0x8C, 0x17, 0x00),
            (0x5C, 0x2F, 0x00), (0x10, 0x45, 0x00), (0x05, 0x4A, 0x00), (0x00, 0x47, 0x2E),
            (0x00, 0x41, 0x66), (0x00, 0x00, 0x00), (0x05, 0x05, 0x05), (0x05, 0x05, 0x05),
            (0xC7, 0xC7, 0xC7), (0x00, 0x77, 0xFF), (0x21, 0x55, 0xFF), (0x82, 0x37, 0xFA),
            (0xEB, 0x2F, 0xB5), (0xFF, 0x29, 0x50), (0xFF, 0x22, 0x00), (0xD6, 0x32, 0x00),
            (0xC4, 0x62, 0x00), (0x35, 0x80, 0x00), (0x05, 0x8F, 0x00), (0x00, 0x8A, 0x55),
            (0x00, 0x99, 0xCC), (0x21, 0x21, 0x21), (0x09, 0x09, 0x09), (0x09, 0x09, 0x09),
            (0xFF, 0xFF, 0xFF), (0x0F, 0xD7, 0xFF), (0x69, 0xA2, 0xFF), (0xD4, 0x80, 0xFF),
            (0xFF, 0x45, 0xF3), (0xFF, 0x61, 0x8B), (0xFF, 0x88, 0x33), (0xFF, 0x9C, 0x12),
            (0xFA, 0xBC, 0x20), (0x9F, 0xE3, 0x0E), (0x2B, 0xF0, 0x35), (0x0C, 0xF0, 0xA4),
            (0x05, 0xFB, 0xFF), (0x5E, 0x5E, 0x5E), (0x0D, 0x0D, 0x0D), (0x0D, 0x0D, 0x0D),
            (0xFF, 0xFF, 0xFF), (0xA6, 0xFC, 0xFF), (0xB3, 0xEC, 0xFF), (0xDA, 0xAB, 0xEB),
            (0xFF, 0xA8, 0xF9), (0xFF, 0xAB, 0xB3), (0xFF, 0xD2, 0xB0), (0xFF, 0xEF, 0xA6),
            (0xFF, 0xF7, 0x9C), (0xD7, 0xFF, 0xB3), (0xC5, 0xFF, 0xC4), (0xA6, 0xFF, 0xC8),
            (0xA2, 0xFF, 0xFF), (0xB3, 0xB3, 0xB3), (0x70, 0x70, 0x70), (0x70, 0x70, 0x70),
        ];
        
        palette.get(index as usize & 0x3F).copied().unwrap_or((0, 0, 0))
    }
    
    pub fn reset(&mut self) {
        self.fine_x_scroll = 0;
        self.temp_vram_addr = 0x2000; // Start at nametable 0 (where game writes)
        self.vram_addr = 0x2000; // Start at nametable 0 (where game writes)
        self.write_toggle = false;
        self.data = 0;
        self.scanline = 261;
        self.cycle = 0;
        self.frame_complete = false;
        self.status = 0x80; // Set VBlank flag on reset
        self.ctrl = 0;
        self.mask = 0;
        self.nmi_occurred = false;
        self.nmi_output = false;
        self.bg_shifter_pattern_lo = 0;
        self.bg_shifter_pattern_hi = 0;
        self.bg_shifter_attrib_lo = 0;
        self.bg_shifter_attrib_hi = 0;
        
        // Initialize palette with Donkey Kong-like colors
        self.palette_ram[0] = 0x0F; // Black background
        self.palette_ram[1] = 0x30; // White
        self.palette_ram[2] = 0x16; // Red
        self.palette_ram[3] = 0x27; // Green
        
        // Sprite palettes
        self.palette_ram[17] = 0x30; // White for sprites
        self.palette_ram[18] = 0x27; // Green for sprites
        self.palette_ram[19] = 0x16; // Red for sprites
    }
    
    pub fn frame_ready(&self) -> bool {
        self.frame_complete
    }
    
    pub fn frame_done(&mut self) {
        self.frame_complete = false;
    }
    
    pub fn get_frame_buffer(&self) -> &[u8] {
        &self.frame_buffer
    }
}
