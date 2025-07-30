use std::fs::File;
use std::io::Read;

#[derive(Debug)]
pub struct Cartridge {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper: u8,
    pub mirroring: Mirroring,
}

#[derive(Debug, Clone, Copy)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
}

impl Cartridge {
    pub fn new(rom_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(rom_path)?;
        let mut header = [0u8; 16];
        file.read_exact(&mut header)?;
        
        // Check for iNES header
        if &header[0..4] != b"NES\x1A" {
            return Err("Invalid ROM file format".into());
        }
        
        let prg_rom_size = header[4] as usize * 16384; // 16KB units
        let chr_rom_size = header[5] as usize * 8192;  // 8KB units
        
        let flags6 = header[6];
        let flags7 = header[7];
        
        let mapper = (flags7 & 0xF0) | (flags6 >> 4);
        
        let mirroring = if flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };
        
        // Skip trainer if present
        if flags6 & 0x04 != 0 {
            let mut trainer = [0u8; 512];
            file.read_exact(&mut trainer)?;
        }
        
        // Read PRG ROM
        let mut prg_rom = vec![0u8; prg_rom_size];
        file.read_exact(&mut prg_rom)?;
        
        // Read CHR ROM
        let mut chr_rom = vec![0u8; chr_rom_size];
        if chr_rom_size > 0 {
            file.read_exact(&mut chr_rom)?;
        } else {
            // CHR RAM
            chr_rom = vec![0u8; 8192];
        }
        
        Ok(Cartridge {
            prg_rom,
            chr_rom,
            mapper,
            mirroring,
        })
    }
    
    pub fn read_prg(&self, address: u16) -> u8 {
        let address = address as usize;
        match self.prg_rom.len() {
            16384 => {
                // 16KB PRG ROM, mirrored
                self.prg_rom[address % 16384]
            }
            32768 => {
                // 32KB PRG ROM
                self.prg_rom[address]
            }
            _ => {
                // Other sizes, just use modulo
                self.prg_rom[address % self.prg_rom.len()]
            }
        }
    }
    
    pub fn write_prg(&mut self, _address: u16, _data: u8) {
        // Most cartridges don't support writing to PRG ROM
        // Mapper-specific implementations would go here
    }
    
    pub fn read_chr(&self, address: u16) -> u8 {
        if self.chr_rom.is_empty() {
            0 // Return 0 if no CHR ROM
        } else {
            self.chr_rom[address as usize % self.chr_rom.len()]
        }
    }
    
    pub fn write_chr(&mut self, address: u16, data: u8) {
        // CHR RAM write
        if self.chr_rom.len() == 8192 {
            self.chr_rom[address as usize % 8192] = data;
        }
    }
    
    pub fn dummy() -> Self {
        Cartridge {
            prg_rom: vec![],
            chr_rom: vec![],
            mapper: 0,
            mirroring: Mirroring::Horizontal,
        }
    }
}
