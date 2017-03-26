use std::ascii::AsciiExt;

extern crate termion;
use termion::raw::IntoRawMode;
use termion::async_stdin;
use std::io::{Read, stdout, Write};

mod cpu;
mod memory;
use memory::MemoryArray;
use cpu::Memory;
use cpu::P65;
mod disasm;


// Simple test program for my 6502 simulator. Will load EHBASIC at 0xC000 and run it. 

fn main() {

    let new_stdout = stdout();
    let mut stdout = new_stdout.lock().into_raw_mode().unwrap();
    let mut stdin = async_stdin().bytes();


/* basic

    let mut f = std::fs::File::open("tests/ehbasic.bin").unwrap();
    let rs = f.read_exact(&mut mem.0[0xC000 ..]).unwrap();
    
    pr.reset(&mut mem);

*/

  //tests
  // test actually run in 1.46s on my machine when --release'd

    let mut pr = P65::new();

// we need something different for memory


    struct MemoryArrayMess{ 
        pub m: Vec<u8>,
        pub fire_irq: bool,
        pub fire_nmi: bool,
    }

    impl MemoryArrayMess{
        pub fn new(size: usize) -> Result<MemoryArrayMess, &'static str> {
            if size > 65536 {
                Err("Too much!")
            } else {
                Ok(MemoryArrayMess { m: vec![0u8; size], fire_irq: false, fire_nmi: false })
            }
        }
    }

    impl Memory for MemoryArrayMess {
        fn read(&mut self, a: usize) -> u8 {
            match a {
                0xF004 => {
                    let tmp = self.m[a];         // temp hack for ehbasic i/o
                    self.m[a] = 0x00;
                    tmp
                },
                0xBFFC => {
                    println!("READ BFFC!");
                    self.m[a]
                }
                _ => self.m[a],
            }
        }

        fn write(&mut self, a: usize, v: u8) { 
            match a {
                0xF001 => { print!("{}",v as char); },
                0xBFFC => {
                    println!("Write BFFC <- {:x}\r", v);
                    if v & 0x01 != 0 {                     // bit 0 è /IRQ
                        self.fire_irq = false;
                    } else {
                        self.fire_irq = true;
                    }
                }
                _ => { self.m[a] = v; },
            }
        }
    }


    let mut mem = MemoryArrayMess::new(65536).unwrap();        // the full awesome power of 64KB at the tip of your fingers
    let mut f = std::fs::File::open("tests/6502_interrupt_test.bin").unwrap();
    let _ = f.read_exact(&mut mem.m[0x000A ..]).unwrap();



    mem.write(0xFFFC, 0x00);
    mem.write(0xFFFD, 0x04);
    pr.reset(&mut mem);
  // end tests

//    let mut oldpc = 0xFFFFu16;
    loop {
        let c = stdin.next();
        match c {
            Some(Ok(c)) => {
                match c {
                    0x11 => {
                            break; 
                    },
                    c => {
                            mem.write(0xF004,c.to_ascii_uppercase());
                    },
                }
            },
            Some(Err(_)) => { write!(stdout, "Error char\r\n").unwrap(); },
            None => {},
        }
        pr.run(&mut mem, 1);
        if mem.fire_irq { println!("Fire IRQ!\r"); pr.irq_set(); } else { pr.irq_clear(); mem.fire_irq = false; }
        if mem.fire_nmi { println!("Fire NMI!\r"); pr.nmi_set(); } else { pr.nmi_clear(); mem.fire_nmi = false; }
        if pr.cycle % 10_000 == 0 {     // flush sometimes, gross!
            stdout.flush().unwrap();
        }
    }


// we want to: 
// intercept some memory reads/writes  (for emulating mmapped devices/peripherals)
// allow the cpu and other peripherals to read / write some signals




    
/*
        if pr.cycle >= 100_000_000 {
            if pr.ts == 1 {
                print!("\r\n");
                print!("{:3}",P65::op_name(pr.op).to_uppercase());
                print!(" {:7}", P65::addr_string(pr.op, (mem.read(pr.pc as usize) as u16) | ((mem.read(pr.pc.wrapping_add(1) as usize) as u16) << 8)).to_uppercase());
            } else {
                print!("           ");
            }
            print!(" {:?}", pr);  
            print!("\r\n");
            break;
        }
    }
*/        
}
