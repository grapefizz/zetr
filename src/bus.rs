use crate::ppu::PPU;
use crate::cartridge::Cartridge;

pub struct Bus<'a> {
    pub ppu: &'a mut PPU,
    pub cartridge: &'a mut Cartridge,
    pub ram: &'a mut [u8; 2048],
    pub controller1: u8,
    pub controller1_shift: u8,
    pub controller_strobe: bool,
}

impl<'a> Bus<'a> {
    pub fn new(ppu: &'a mut PPU, cartridge: &'a mut Cartridge, ram: &'a mut [u8; 2048]) -> Self {
        Bus {
            ppu,
            cartridge,
            ram,
            controller1: 0,
            controller1_shift: 0,
            controller_strobe: false,
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.cpu_read(0x2000 + (addr & 0x0007), self.cartridge),
            0x4016 => {
                let data = (self.controller1_shift & 0x80) >> 7;
                self.controller1_shift <<= 1;
                data
            }
            0x4017 => 0, // Controller 2 not implemented
            0x8000..=0xFFFF => self.cartridge.read_prg(addr - 0x8000),
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = data,
            0x2000..=0x3FFF => self.ppu.cpu_write(0x2000 + (addr & 0x0007), data, self.cartridge),
            0x4014 => {
                // OAM DMA
                self.ppu.oam_addr = data;
                // DMA should be handled in the main loop, not here.
                // This write just sets the OAM address.
            }
            0x4016 => {
                self.controller_strobe = data & 1 != 0;
                if self.controller_strobe {
                    self.controller1_shift = self.controller1;
                }
            }
            0x8000..=0xFFFF => self.cartridge.write_prg(addr - 0x8000, data),
            _ => {}
        }
    }
}