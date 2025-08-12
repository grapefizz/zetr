use crate::cartridge::Cartridge;

const SCREEN_WIDTH: usize = 256;
const SCREEN_HEIGHT: usize = 240;

#[derive(Debug, Default, Clone, Copy)]
struct Sprite {
    y: u8,
    tile_id: u8,
    attributes: u8,
    x: u8,
    pattern_lo: u8,
    pattern_hi: u8,
}

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
    
    // Sprite rendering
    scanline_sprites: [Sprite; 8],
    sprite_count: usize,
    
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
            scanline_sprites: [Sprite::default(); 8],
            sprite_count: 0,
            nmi_occurred: false,
            nmi_output: false,
            nmi_previous: false,
        }
    }
    
    pub fn step(&mut self, cartridge: &mut Cartridge) {
        if self.scanline >= -1 && self.scanline < 240 {
            if self.scanline == 0 && self.cycle == 0 {
                self.cycle = 1;
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
                self.evaluate_sprites();
            }

            if self.cycle == 320 {
                self.fetch_sprite_patterns(cartridge);
            }
            
            if self.scanline == -1 && self.cycle >= 280 && self.cycle < 305 {
                self.transfer_address_y();
            }
            
            if self.scanline >= 0 && self.cycle >= 1 && self.cycle <= 256 {
                self.render_pixel();
            }
        }
        
        if self.scanline == 241 && self.cycle == 1 {
            self.status |= 0x80;
            if self.ctrl & 0x80 != 0 {
                self.nmi_occurred = true;
            }
        }
        
        self.cycle += 1;
        if self.cycle >= 341 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline >= 261 {
                self.scanline = -1;
                self.frame_complete = true;
                self.status &= !0x80;
                self.nmi_occurred = false;
            }
        }
    }
    
    fn render_pixel(&mut self) {
        let x = self.cycle - 1;
        let y = self.scanline;
        
        if (x as usize) < SCREEN_WIDTH && y >= 0 && y < SCREEN_HEIGHT as i16 {
            let mut bg_pixel = 0;
            let mut bg_palette = 0;
            
            if self.mask & 0x08 != 0 {
                let pixel_bit = 15 - self.fine_x_scroll as u16;
                let p0 = (self.bg_shifter_pattern_lo >> pixel_bit) & 1;
                let p1 = (self.bg_shifter_pattern_hi >> pixel_bit) & 1;
                bg_pixel = (p1 << 1) | p0;
                
                let palette_bit = 15 - self.fine_x_scroll as u16;
                let pal_lo = (self.bg_shifter_attrib_lo >> palette_bit) & 1;
                let pal_hi = (self.bg_shifter_attrib_hi >> palette_bit) & 1;
                bg_palette = (pal_hi << 1) | pal_lo;
            }
            
            let mut sprite_pixel = 0;
            let mut sprite_palette = 0;
            let mut sprite_priority = false;

            if self.mask & 0x10 != 0 {
                for i in 0..self.sprite_count {
                    let sprite = &mut self.scanline_sprites[i];
                    if sprite.x == 0 {
                        let p0 = (sprite.pattern_lo & 0x80) >> 7;
                        let p1 = (sprite.pattern_hi & 0x80) >> 7;
                        let pixel = (p1 << 1) | p0;

                        if pixel != 0 {
                            sprite_pixel = pixel;
                            sprite_palette = (sprite.attributes & 0x03) + 4;
                            sprite_priority = (sprite.attributes & 0x20) == 0;
                            
                            if self.mask & 0x08 != 0 && i == 0 && sprite_pixel != 0 && bg_pixel != 0 {
                                self.status |= 0x40;
                            }
                            
                            break;
                        }
                    }
                }
            }
            
            let (final_pixel, final_palette) = if sprite_pixel > 0 && (bg_pixel == 0 || sprite_priority) {
                (sprite_pixel, sprite_palette)
            } else {
                (bg_pixel as u8, bg_palette as u8)
            };
            
            let palette_addr = if final_pixel == 0 { 0 } else { (final_palette << 2) | final_pixel };
            let color_index = self.palette_ram[palette_addr as usize & 0x1F];
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
        if self.mask & 0x08 != 0 {
            self.bg_shifter_pattern_lo <<= 1;
            self.bg_shifter_pattern_hi <<= 1;
            self.bg_shifter_attrib_lo <<= 1;
            self.bg_shifter_attrib_hi <<= 1;
        }
        if self.mask & 0x10 != 0 && self.cycle >= 1 && self.cycle < 258 {
            for i in 0..self.sprite_count {
                if self.scanline_sprites[i].x > 0 {
                    self.scanline_sprites[i].x -= 1;
                } else {
                    self.scanline_sprites[i].pattern_lo <<= 1;
                    self.scanline_sprites[i].pattern_hi <<= 1;
                }
            }
        }
    }

    fn evaluate_sprites(&mut self) {
        self.sprite_count = 0;
        let sprite_height = if self.ctrl & 0x20 != 0 { 16 } else { 8 };

        for i in 0..64 {
            let y = self.oam[i * 4] as i16;
            let diff = self.scanline - y;

            if diff >= 0 && diff < sprite_height {
                if self.sprite_count < 8 {
                    self.scanline_sprites[self.sprite_count].y = self.oam[i * 4];
                    self.scanline_sprites[self.sprite_count].tile_id = self.oam[i * 4 + 1];
                    self.scanline_sprites[self.sprite_count].attributes = self.oam[i * 4 + 2];
                    self.scanline_sprites[self.sprite_count].x = self.oam[i * 4 + 3];
                    self.sprite_count += 1;
                } else {
                    self.status |= 0x20;
                    break;
                }
            }
        }
    }

    fn fetch_sprite_patterns(&mut self, cartridge: &mut Cartridge) {
        let sprite_height = if self.ctrl & 0x20 != 0 { 16 } else { 8 };

        for i in 0..self.sprite_count {
            let sprite_y = self.scanline_sprites[i].y;
            let sprite_tile_id = self.scanline_sprites[i].tile_id;
            let sprite_attributes = self.scanline_sprites[i].attributes;
            
            let mut row = (self.scanline as u16) - (sprite_y as u16);

            if sprite_attributes & 0x80 != 0 {
                row = sprite_height as u16 - 1 - row;
            }

            let (tile_id, table) = if sprite_height == 16 {
                let table = if sprite_tile_id & 0x01 != 0 { 0x1000 } else { 0x0000 };
                let tile_id = sprite_tile_id & 0xFE;
                let tile_id = if row < 8 { tile_id } else { tile_id + 1 };
                (tile_id, table)
            } else {
                let table = if self.ctrl & 0x08 != 0 { 0x1000 } else { 0x0000 };
                (sprite_tile_id, table)
            };
            
            let row = row & 0x07;
            let addr_lo = table + (tile_id as u16) * 16 + row;
            let addr_hi = addr_lo + 8;

            let mut lo = self.ppu_read(addr_lo, cartridge);
            let mut hi = self.ppu_read(addr_hi, cartridge);

            if sprite_attributes & 0x40 != 0 {
                lo = lo.reverse_bits();
                hi = hi.reverse_bits();
            }

            self.scanline_sprites[i].pattern_lo = lo;
            self.scanline_sprites[i].pattern_hi = hi;
        }
    }
    
    fn load_background_shifters(&mut self) {
        self.bg_shifter_pattern_lo = (self.bg_shifter_pattern_lo & 0xFF00) | self.bg_next_tile_lsb as u16;
        self.bg_shifter_pattern_hi = (self.bg_shifter_pattern_hi & 0xFF00) | self.bg_next_tile_msb as u16;
        self.bg_shifter_attrib_lo = (self.bg_shifter_attrib_lo & 0xFF00) | if self.bg_next_tile_attrib & 1 != 0 { 0xFF } else { 0 };
        self.bg_shifter_attrib_hi = (self.bg_shifter_attrib_hi & 0xFF00) | if self.bg_next_tile_attrib & 2 != 0 { 0xFF } else { 0 };
    }
    
    fn fetch_nametable_byte(&mut self, cartridge: &mut Cartridge) {
        let addr = 0x2000 | (self.vram_addr & 0x0FFF);
        self.bg_next_tile_id = self.ppu_read(addr, cartridge);
    }
    
    fn fetch_attribute_table_byte(&mut self, cartridge: &mut Cartridge) {
        let addr = 0x23C0 | (self.vram_addr & 0x0C00) | ((self.vram_addr >> 4) & 0x38) | ((self.vram_addr >> 2) & 0x07);
        let attribute = self.ppu_read(addr, cartridge);
        let shift = ((self.vram_addr >> 4) & 4) | (self.vram_addr & 2);
        self.bg_next_tile_attrib = (attribute >> shift) & 3;
    }
    
    fn fetch_low_bg_tile_byte(&mut self, cartridge: &mut Cartridge) {
        let fine_y = (self.vram_addr >> 12) & 7;
        let table = if self.ctrl & 0x10 != 0 { 0x1000 } else { 0x0000 };
        let addr = table + (self.bg_next_tile_id as u16) * 16 + fine_y;
        self.bg_next_tile_lsb = self.ppu_read(addr, cartridge);
    }
    
    fn fetch_high_bg_tile_byte(&mut self, cartridge: &mut Cartridge) {
        let fine_y = (self.vram_addr >> 12) & 7;
        let table = if self.ctrl & 0x10 != 0 { 0x1000 } else { 0x0000 };
        let addr = table + (self.bg_next_tile_id as u16) * 16 + fine_y + 8;
        self.bg_next_tile_msb = self.ppu_read(addr, cartridge);
    }
    
    fn increment_scroll_x(&mut self) {
        if self.mask & 0x18 != 0 {
            if (self.vram_addr & 0x001F) == 31 {
                self.vram_addr &= !0x001F;
                self.vram_addr ^= 0x0400;
            } else {
                self.vram_addr += 1;
            }
        }
    }
    
    fn increment_scroll_y(&mut self) {
        if self.mask & 0x18 != 0 {
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
        if self.mask & 0x18 != 0 {
            self.vram_addr = (self.vram_addr & 0xFBE0) | (self.temp_vram_addr & 0x041F);
        }
    }
    
    fn transfer_address_y(&mut self) {
        if self.mask & 0x18 != 0 {
            self.vram_addr = (self.vram_addr & 0x841F) | (self.temp_vram_addr & 0x7BE0);
        }
    }
    
    pub fn cpu_read(&mut self, addr: u16, cartridge: &mut Cartridge) -> u8 {
        match addr {
            0x2002 => {
                let data = (self.status & 0xE0) | (self.read_buffer & 0x1F);
                self.status &= !0x80;
                self.write_toggle = false;
                data
            }
            0x2004 => self.oam[self.oam_addr as usize],
            0x2007 => {
                let mut data = self.read_buffer;
                self.read_buffer = self.ppu_read(self.vram_addr, cartridge);
                if self.vram_addr >= 0x3F00 { data = self.read_buffer; }
                self.vram_addr = self.vram_addr.wrapping_add(if self.ctrl & 4 != 0 { 32 } else { 1 });
                data
            }
            _ => 0,
        }
    }
    
    pub fn cpu_write(&mut self, addr: u16, data: u8, cartridge: &mut Cartridge) {
        match addr {
            0x2000 => {
                self.ctrl = data;
                self.temp_vram_addr = (self.temp_vram_addr & 0xF3FF) | ((data as u16 & 3) << 10);
            }
            0x2001 => self.mask = data,
            0x2003 => self.oam_addr = data,
            0x2004 => {
                self.oam[self.oam_addr as usize] = data;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x2005 => {
                if !self.write_toggle {
                    self.fine_x_scroll = data & 7;
                    self.temp_vram_addr = (self.temp_vram_addr & 0xFFE0) | ((data as u16) >> 3);
                } else {
                    self.temp_vram_addr = (self.temp_vram_addr & 0x8C1F) | ((data as u16 & 0xF8) << 2) | ((data as u16 & 7) << 12);
                }
                self.write_toggle = !self.write_toggle;
            }
            0x2006 => {
                if !self.write_toggle {
                    self.temp_vram_addr = (self.temp_vram_addr & 0x00FF) | ((data as u16 & 0x3F) << 8);
                } else {
                    self.temp_vram_addr = (self.temp_vram_addr & 0xFF00) | (data as u16);
                    self.vram_addr = self.temp_vram_addr;
                }
                self.write_toggle = !self.write_toggle;
            }
            0x2007 => {
                self.ppu_write(self.vram_addr, data, cartridge);
                self.vram_addr = self.vram_addr.wrapping_add(if self.ctrl & 4 != 0 { 32 } else { 1 });
            }
            _ => {}
        }
    }
    
    fn ppu_read(&mut self, addr: u16, cartridge: &mut Cartridge) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0..=0x1FFF => cartridge.read_chr(addr),
            0x2000..=0x3EFF => {
                let addr = addr & 0x0FFF;
                match cartridge.mirroring {
                    crate::cartridge::Mirroring::Vertical => self.vram[(addr & 0x07FF) as usize],
                    crate::cartridge::Mirroring::Horizontal => self.vram[(addr & 0x03FF | ((addr >> 1) & 0x0400)) as usize],
                    _ => self.vram[addr as usize],
                }
            }
            0x3F00..=0x3FFF => {
                let mut addr = addr & 0x1F;
                if addr == 0x10 || addr == 0x14 || addr == 0x18 || addr == 0x1C { addr -= 0x10; }
                self.palette_ram[addr as usize] & if self.mask & 1 != 0 { 0x30 } else { 0x3F }
            }
            _ => 0,
        }
    }
    
    fn ppu_write(&mut self, addr: u16, data: u8, cartridge: &mut Cartridge) {
        let addr = addr & 0x3FFF;
        match addr {
            0..=0x1FFF => cartridge.write_chr(addr, data),
            0x2000..=0x3EFF => {
                let addr = addr & 0x0FFF;
                match cartridge.mirroring {
                    crate::cartridge::Mirroring::Vertical => self.vram[(addr & 0x07FF) as usize] = data,
                    crate::cartridge::Mirroring::Horizontal => self.vram[(addr & 0x03FF | ((addr >> 1) & 0x0400)) as usize] = data,
                    _ => self.vram[addr as usize] = data,
                }
            }
            0x3F00..=0x3FFF => {
                let mut addr = addr & 0x1F;
                if addr == 0x10 || addr == 0x14 || addr == 0x18 || addr == 0x1C { addr -= 0x10; }
                self.palette_ram[addr as usize] = data;
            }
            _ => {}
        }
    }
    
    fn get_color_from_palette(&self, index: u8) -> (u8, u8, u8) {
        let palette = [
            (84, 84, 84), (0, 30, 116), (8, 16, 144), (48, 0, 136), (68, 0, 100), (92, 0, 48), (84, 4, 0), (60, 24, 0),
            (32, 42, 0), (8, 58, 0), (0, 64, 0), (0, 60, 40), (0, 50, 88), (0, 0, 0), (0, 0, 0), (0, 0, 0),
            (152, 150, 152), (8, 76, 196), (48, 50, 236), (92, 30, 228), (136, 20, 176), (160, 20, 100), (152, 34, 32),
            (120, 60, 0), (84, 90, 0), (40, 114, 0), (8, 124, 0), (0, 118, 40), (0, 102, 120), (0, 0, 0), (0, 0, 0),
            (0, 0, 0), (236, 238, 236), (76, 154, 236), (120, 124, 236), (176, 98, 236), (228, 84, 236), (236, 88, 180),
            (236, 106, 100), (212, 136, 32), (160, 170, 0), (116, 196, 0), (76, 208, 32), (56, 204, 108), (56, 180, 220),
            (60, 60, 60), (0, 0, 0), (0, 0, 0), (236, 238, 236), (168, 204, 236), (188, 188, 236), (212, 178, 236),
            (236, 174, 236), (236, 174, 212), (236, 180, 176), (228, 196, 144), (204, 210, 120), (180, 222, 120),
            (168, 226, 144), (152, 226, 180), (160, 214, 228), (160, 162, 160), (0, 0, 0), (0, 0, 0),
        ];
        palette.get(index as usize & 0x3F).copied().unwrap_or((0, 0, 0))
    }
    
    pub fn reset(&mut self) {
        self.fine_x_scroll = 0;
        self.write_toggle = false;
        self.data = 0;
        self.scanline = 261;
        self.cycle = 0;
        self.frame_complete = false;
        self.status = 0;
        self.ctrl = 0;
        self.mask = 0;
        self.nmi_occurred = false;
        self.nmi_output = false;
        self.bg_shifter_pattern_lo = 0;
        self.bg_shifter_pattern_hi = 0;
        self.bg_shifter_attrib_lo = 0;
        self.bg_shifter_attrib_hi = 0;
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
