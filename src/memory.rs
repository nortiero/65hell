pub struct MemoryArray<'a>(pub &'a mut [u8]);

pub trait Memory {
    fn read(&mut self, a: usize) -> u8;
    fn write(&mut self, a: usize, v: u8); 
}

impl<'a> Memory for MemoryArray<'a> {
    fn read(&mut self, a: usize) -> u8 {
        if a != 0xF004 {
            self.0[a] 
        } else {
            let tmp = self.0[a];         // temp hack for ehbasic i/o
            self.0[a] = 0x00;
            tmp
        }
    }
    fn write(&mut self, a: usize, v: u8) { 
        if a == 0xF001 { print!("{}",v as char); } else {
            self.0[a] = v; 
        }
    }
}