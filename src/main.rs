extern crate termion;

mod cpu;
mod disasm;

use std::ascii::AsciiExt;
use std::fmt::Write as whatever;
use std::io::{Read, stdout, Write};
use termion::raw::IntoRawMode;
use termion::async_stdin;
use cpu::Memory;
use cpu::P65;


const EXPLAIN: &'static str = "[options] hex-address:file [addr:file ..]\r\n\
                                \r\n\
                                Load one or more blobs at specified hexadecimal addresses, \r\n\
                                then launch via RESET vector or by direct jump (-j)\r\n\
                                The RESET vector will be populated with meaningful value only if a binary\r\n\
                                has been loaded to the appropriate location, covering top of memory, or if -p poke has been used.\r\n\
                                Memory size is 64KB of 8 bit RAM\r\n\
                                Press Ctrl+q to quit, Ctrl+e to dump current status.\r\n\
                                \r\n\
                                Options:\r\n\
                                \t-h: help\r\n\
                                \t-d: dump trace to stderr\r\n\
                                \t-k address: of the optional keyboard (mapped to stdin)\r\n\
                                \t-t address: of the optional screen/printer character (stdout\r\n\
                                \t-i address: of the optional irq/nmi generator, useful for tests\r\n\
                                \t-j address: jump start to address\r\n\
                                \t-p address:value poke value, multiple usage allowed\r\n";

// Simple test program for my 6502 simulator.

fn main() {

    let mut mem = MemoryArrayMess::new(65536).unwrap(); // the full awesome power of 64KB at your fingertips
    let mut dump = false;
    let mut loaded = false; // must load at least some code
    let mut jump: Option<u16> = None;

    // not exceptional argument parsing. TODO: refactor this mess
    let mut ai = std::env::args();
    let exe = ai.next().unwrap();
    while let Some(s) = ai.next() {
        match s.as_ref() {
            s if !s.starts_with("-") => {
                let tup: Result<(u16,&str),&str> = s.split(':')
                            .zip(s.split(':').skip(1))
                            .next()
                            .ok_or("Missing address and file name")
                            .and_then(|(s1,s2)| u16::from_str_radix(s1,16)
                                                        .map(|a| (a,s2))
                                                        .or(Err("Missing Address")))
                            .and_then(|(s1,s2)| if s2.len() > 0 { Ok((s1,s2)) } else { Err("Missing File name") });
                match tup {
                    Ok((a,f)) => match load_binary(&mut mem, f, a) {
                                    Ok(_) => {
                                        loaded = true;
                                    },
                                    Err(e) =>  { println!("Error: {}", e); return },
                                },
                    Err(s) => { println!("{}\r",s); return; },
                }
            }
            "-d" => {
                dump = true;
            }
            "-p" => {
                let arg = ai.next();
                match arg {
                    None => {
                        println!("You must specify an (hex:hex) address:value pair with -p");
                        return;
                    }
                    Some(s) => {
                        let mut p = s.split(':');
                        let a2 = p.next()
                            .ok_or("Missing Address")
                            .and_then(|v| u16::from_str_radix(v, 16).or(Err("Address error")));
                        let v2 =
                            p.next().ok_or("Missing Value").and_then(|v| {
                                                                         u8::from_str_radix(v, 16)
                                                                             .or(Err("Value error"))
                                                                     });
                        match (a2, v2) {
                            (Ok(a), Ok(v)) => {
                                mem.write(a as usize, v);
                            }
                            (Err(a), _) | (_, Err(a)) => {
                                println!("-p: {}", a);
                                return;
                            }
                        }
                    }
                }
            }
            p @ "-k" | p @ "-j" | p @ "-t" | p @ "-i" => {
                let arg = ai.next();
                if arg.is_none() {
                    println!("{} needs an argument.", p);
                    return;
                }
                let mut target = match p {
                    "-k" => &mut mem.keyboard,
                    "-j" => &mut jump,
                    "-t" => &mut mem.printer,
                    "-i" => &mut mem.irq_generator,
                    _ => {
                        panic!();
                    }
                };
                if let Ok(v) = u16::from_str_radix(&arg.unwrap(), 16) {
                    *target = Some(v);
                } else {
                    println!("Usage: {} {}", exe, EXPLAIN);
                    return;
                }
            }
            _ => {
                println!("Usage: {} {}", exe, EXPLAIN);
                return;
            }
        }
    }
    if !loaded {
        println!("Must specify at least one file to load");
        return;
    }


    let new_stdout = stdout();
    let mut stdout = new_stdout.lock().into_raw_mode().unwrap();
    let mut stdin = async_stdin().bytes();

    let mut status_print = false;
    let mut last_flush = 0u64;
    let mut pr = P65::new();
    pr.reset(&mut mem);
    if jump.is_some() {
        println!("jump: {}", jump.unwrap());
        pr.jump(&mut mem, jump.unwrap());
    }

    loop {
        match stdin.next() {
            Some(Ok(c)) => {
                match c {
                    0x11 => {
                        // Ctrl+q
                        break;
                    }
                    0x05 => {
                        // ctrl+e
                        status_print = true;
                    }
                    c => {
                        if mem.keyboard.is_some() {
                            let btmp = mem.keyboard.unwrap() as usize;
                            mem.write(btmp as usize, c.to_ascii_uppercase());
                        }
                    }
                }
            }
            Some(Err(_)) => {}
            None => {}
        }

        pr.run(&mut mem, 1);             // one cycle at a time. best performance can be achieved in single/multiple op or cycle mode 
        if mem.irq_generator.is_some() {
            if mem.fire_irq {
                pr.irq_set();
            } else {
                pr.irq_clear();
                mem.fire_irq = false;
            }
            if mem.fire_nmi {
                pr.nmi_set();
            } else {
                pr.nmi_clear();
                mem.fire_nmi = false;
            }
        }
        if pr.cycle - last_flush >= 50_000 {
            // flush output every 50K cycles. Gross!
            stdout.flush().unwrap(); // we must flush to keep terminal operating
            last_flush = pr.cycle;
        }
        if status_print && pr.ts == 1 {
            println!("{}\r", status_string(&pr, &mut mem));
            status_print = false;
        }
        if dump {
            println!("{}\r", status_string(&pr, &mut mem));
        }
    }
}

fn load_binary<M: Memory>(mem: &mut M, name: &str, address: u16) -> std::io::Result<()> {
    let f = try!(std::fs::File::open(name));
    for (i, v) in f.bytes().enumerate() {
        mem.write(address.wrapping_add(i as u16) as usize, v?);
    }
    Ok(())
}

// we need something better for memory. should manage roms, stratified layouts, etc.
// TODO: devices should be attached/detached to some bus manager object.
// Memory should be memory, an array and nothing more.
struct MemoryArrayMess {
    pub m: Vec<u8>,
    pub fire_irq: bool,
    pub fire_nmi: bool,
    pub keyboard: Option<u16>,
    pub printer: Option<u16>,
    pub irq_generator: Option<u16>,
}

impl MemoryArrayMess {
    pub fn new(size: usize) -> Result<MemoryArrayMess, &'static str> {
        if size > 65536 {
            Err("Too much!")
        } else {
            Ok(MemoryArrayMess {
                   m: vec![0u8; size],
                   fire_irq: false,
                   fire_nmi: false,
                   keyboard: None,
                   printer: None,
                   irq_generator: None,
               })
        }
    }
}

impl Memory for MemoryArrayMess {
    fn read(&mut self, a: usize) -> u8 {
        if self.keyboard.is_some() && self.keyboard.unwrap() == a as u16 {
            let tmp = self.m[a];
            self.m[a] = 0x00;
            tmp
        } else {
            self.m[a]
        }
    }
    // FIXME. probably a would be a fine u16, instead of usize. check trait
    fn write(&mut self, a: usize, v: u8) {
        if self.printer.is_some() && self.printer.unwrap() == a as u16 {
            if v == 0x7f {
                print!("\x08"); // hack for backspace in raw mode
            } else {
                print!("{}", v as char); // cheap term.
            }
        } else if self.irq_generator.is_some() && self.irq_generator.unwrap() == a as u16 {
            if v & 0x01 != 0 {
                // bit 0 è /IRQ
                self.fire_irq = false;
            } else {
                self.fire_irq = true;
            }
        } else {
            self.m[a] = v;
        }
    }
}
// to be called only in T1 , to have meaningful information
// todo: use a side effect free version of mem.read
pub fn status_string<M: Memory>(pr: &P65, mem: &mut M) -> String {
    use disasm;
    let op = pr.op;
    let param = (mem.read(pr.pc.wrapping_sub(1) as usize) as u16) |
                ((mem.read(pr.pc as usize) as u16) << 8);
    let mut status = String::new();
    write!(&mut status,
           "{:3} {:7} {:?}",
           disasm::op_name(op).to_uppercase(),
           disasm::addr_name(op, param).to_uppercase(),
           pr)
            .expect("Error writing processor status");
    status
}
