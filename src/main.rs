use std::ascii::AsciiExt;

extern crate termion;
use termion::raw::IntoRawMode;
use termion::async_stdin;
use std::io::{Read, stdout, Write};

mod cpu;
mod memory;
use memory::MemoryArray;
use memory::Memory;
use cpu::P65;

fn main() {
    let mut mem_store = [0u8; 65536];

    let new_stdout = stdout();
    let mut stdout = new_stdout.lock().into_raw_mode().unwrap();
    let mut stdin = async_stdin().bytes();



    let mut f = std::fs::File::open("tests/ehbasic.bin").unwrap();
    let rs = f.read_exact(&mut mem_store[0xC000 ..]);
    if let Ok(_) = rs {
        println!("Good read ");
    } else {
        panic!("File not read in full");
    }
    

    let mut mem = MemoryArray(&mut mem_store);
    let mut pr = P65::new();

    pr.reset(&mut mem);

    // per i test
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
