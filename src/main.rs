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


// Simple test program for my 6502 simulator. Will load EHBASIC at 0xC000 and run it. 

fn main() {

    let new_stdout = stdout();
    let mut stdout = new_stdout.lock().into_raw_mode().unwrap();
    let mut stdin = async_stdin().bytes();

    let mut pr = P65::new();
    let mut mem = MemoryArray::new(65536).unwrap();        // the full awesome power of 64KB at the tip of your fingers


    let mut f = std::fs::File::open("tests/ehbasic.bin").unwrap();
    let rs = f.read_exact(&mut mem.0[0xC000 ..]).unwrap();
    
    pr.reset(&mut mem);

//    per i test
//    pr.pc =  0x400;
//    pr.fetch_op(&mut mem);
//    pr.tick();
//    pr.cycle = 8;
        
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
        pr.run(&mut mem, 10_000);
        stdout.flush().unwrap();
    }
    
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
