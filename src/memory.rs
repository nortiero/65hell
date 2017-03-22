// A simple memory array, implementing Memory trait, with provisions for console I/O at locations F001(output) and F004(input) 

/*
Please note that not all the memory map shall be RAM.
Real hardware may have parts of memory directly mapped in ROM or with special hardware, so this simplification may not apply.
A better implementation would allow partitioning of memory between RAM/ROM/specially mapped, to allow a more sophisticated simulation.

TODO: a better memory mapper. Specify range of addresses, mode of operation (r/o, r/w, trigger action, protected write,etc.)
*/

use cpu::Memory;

pub struct MemoryArray(pub Vec<u8>);

impl MemoryArray {
    pub fn new(size: usize) -> Result<MemoryArray, &'static str> {
        if size > 65536 {
            Err("Too much!")
        } else {
            Ok(MemoryArray(vec![0u8; size]))
        }
    }
}

impl Memory for MemoryArray {
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