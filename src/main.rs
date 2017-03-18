extern crate termion;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::async_stdin;
use termion::event::{Event,Key};
use std::io::{Read, stdin, stdout, Write};
use std::num::Wrapping;
use std::fmt;
use std::io;
use std::io::prelude::*;
use std::fs::File;

struct P65Flags {
            n: bool, 
            v: bool, 
            bit5: bool, 
            b: bool, 
            d: bool, 
            i: bool, 
            z: bool, 
            c: bool, 
}       

impl fmt::Debug for P65Flags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let val = (if self.n { 0x80 } else { 0x00 }) |
        if self.v { 0x40 } else { 0x00 } |
        0x20 |
        if self.b { 0x10 } else { 0x00 } |
        if self.d { 0x08 } else { 0x00 } |
        if self.i { 0x04 } else { 0x00 } |
        if self.z { 0x02 } else { 0x00 } |
        if self.c { 0x01 } else { 0x00 } ;
        write!(f, "{:x}", val)
    }
}


struct P65 {
    a: Wrapping<u8>,
    x: Wrapping<u8>,
    y: Wrapping<u8>,
    p: P65Flags, 
    s: Wrapping<u8>,
    pc: Wrapping<u16>,

// emulator state
    cycle: u64,
    ts: u8,
    op: u8,
    v1: Wrapping<u8>,
    v2: Wrapping<u8>,
    ah: Wrapping<u8>,
    al: Wrapping<u8>,
}

impl fmt::Debug for P65 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "P65 {{ a: {:x}, x: {:x}, y: {:x}, p: {:x}, s: {:x}, pc: {:x}, op: {:x}, v1: {:x}, v2: {:x}, ah-al: {:x}, T{} }}",
            self.a,self.x,self.y,self.pack_p(), self.s, self.pc, self.op, self.v1, self.v2, self.ah_al(), self.ts)
    }
}

impl P65 {
    fn new() -> P65 {
        P65 { 
            a:Wrapping(0xaa), 
            x:Wrapping(0), 
            y:Wrapping(0), 
            p: P65Flags {n: false, v: false, bit5: true, b: true, d: false, i: true, z: true, c: false,},
            s: Wrapping(0xfd), 
            pc: Wrapping(0), 
            cycle: 0, 
            ts: 0, 
            op: 0, 
            v1: Wrapping(0), 
            v2: Wrapping(0), 
            ah: Wrapping(0), 
            al: Wrapping(0),
            }
    }

    fn cycle_inc(&mut self) -> u64 {
        self.cycle = self.cycle + 1;
        self.cycle
    }

    fn ts_inc (&mut self) -> u8 {
        self.ts = self.ts + 1;
        self.ts
    }

    fn fetch_op(&mut self, mem: &mut Mem) -> u8 {
        self.op = mem.read(self.pc.0 as usize);
        self.inc_pc();
        self.ts = 0;    // will be incremented to 1 by tick
        self.op
    }

    fn tick(&mut self) {
        self.ts_inc();
        self.cycle_inc();
    }

    fn ah_al(&self) -> u16 { (self.ah.0 as u16) << 8 | (self.al.0 as u16) }
    fn inc_pc(&mut self) { self.pc += Wrapping(1); }
    fn inc_sp(&mut self) { self.s += Wrapping(1); }
    fn dec_sp(&mut self) { self.s -= Wrapping(1); }

    fn pack_p(&self) -> u8 {
        (if self.p.n { 0x80 } else { 0x00 }) |
        if self.p.v { 0x40 } else { 0x00 } |
        0x20 |
        if self.p.b { 0x10 } else { 0x00 } |
        if self.p.d { 0x08 } else { 0x00 } |
        if self.p.i { 0x04 } else { 0x00 } |
        if self.p.z { 0x02 } else { 0x00 } |
        if self.p.c { 0x01 } else { 0x00 } 
    }

    fn unpack_set_p(&mut self, flags: u8) {
        self.p.n = flags & 0x80 != 0; 
        self.p.v = flags & 0x40 != 0; 
        self.p.bit5 = true; 
        self.p.b = flags & 0x10 != 0; 
        self.p.d = flags & 0x08 != 0; 
        self.p.i = flags & 0x04 != 0; 
        self.p.z = flags & 0x02 != 0; 
        self.p.c = flags & 0x01 != 0; 
    }

    fn fix_nz(&mut self) {
        self.p.z = self.v1 == Wrapping(0);
        self.p.n = self.v1 >= Wrapping(0x80);
    }

    // operations
    fn op_asl(&mut self) {
        self.p.c = self.v1 & Wrapping(0x80) != Wrapping(0);
        self.v1 = self.v1 << 1;
        self.fix_nz();
    }

    fn op_lsr(&mut self) {
        self.p.c = self.v1 & Wrapping(0x01) != Wrapping(0);
        self.v1 = self.v1 >> 1;
        self.fix_nz();
    }        
    fn op_rol(&mut self) {
        let tmp = self.v1;
        self.v1 = self.v1 << 1 | Wrapping(if self.p.c { 1 } else { 0 })  ;
        self.p.c = tmp & Wrapping(0x80) != Wrapping(0);
        self.fix_nz();
    }
    fn op_ror(&mut self) {
        let tmp = self.v1;
        self.v1 = self.v1 >> 1 | Wrapping(if self.p.c { 0x80 } else { 0 })  ;
        self.p.c = tmp & Wrapping(0x1) != Wrapping(0);
        self.fix_nz();
    }
    fn op_unk(&mut self) {
        assert!(1==0, "Unknown Opcode!");
    }
    fn op_nil(&mut self) { }     // nil means the opcode is managed elsewhere
    fn op_nop(&mut self) { }
    fn op_adc(&mut self) {
        let tsum = (self.a.0 as u16 + self.v1.0 as u16 + if self.p.c { 0x1 } else { 0x0 }) as u16;
        self.p.c = tsum >= 0x100;
        self.p.v = !((self.a.0 & 0x80) ^ (self.v1.0 & 0x80)) & ((self.a.0 & 0x80) ^ ((tsum & 0x80) as u8)) != 0;
        self.v1 = Wrapping((tsum & 0xff) as u8);
        self.fix_nz();
        self.a = self.v1;
    }
    fn op_sbc(&mut self) {
        let tsub = (self.a.0 as u16 - self.v1.0 as u16 - if self.p.c { 0x0 } else { 0x1 }) as u16;
        self.p.c = tsub < 0x100;   // carry is not borrow
        self.p.v = ((self.a.0 & 0x80) ^ (self.v1.0 & 0x80)) & ((self.a.0 & 0x80) ^ ((tsub & 0x80) as u8)) != 0;
        self.v1 = Wrapping((tsub & 0xff) as u8);
        self.fix_nz();
        self.a = self.v1;
    }
    fn op_and(&mut self) { self.a = self.a & self.v1; }
    fn op_ora(&mut self) { self.a = self.a | self.v1; }
    fn op_eor(&mut self) { self.a = self.a ^ self.v1; }
    fn op_cmp(&mut self) {
        let tmpa = self.a;
        let tmpv = self.p.v;
        self.p.c = true;
        self.op_sbc();
        self.p.v = tmpv;
        self.a = tmpa;
    }
    fn op_cpx(&mut self) {
        let tmpa = self.a;
        let tmpv = self.p.v;
        self.a = self.x;
        self.op_sbc();
        self.p.v = tmpv;
        self.a = tmpa;
    }
    fn op_cpy(&mut self) {
        let tmpa = self.a;
        let tmpv = self.p.v;
        self.a = self.y;
        self.op_sbc();
        self.p.v = tmpv;
        self.a = tmpa;
    }
    fn op_dec(&mut self) { self.v1 -= Wrapping(1); }
    fn op_inc(&mut self) { self.v1 += Wrapping(1); }
    fn op_lda(&mut self) { self.a = self.v1; self.fix_nz(); }
    fn op_ldx(&mut self) { self.x = self.v1; self.fix_nz(); }
    fn op_ldy(&mut self) { self.y = self.v1; self.fix_nz(); }
    fn op_bit(&mut self) {
        self.p.z = self.v1.0 & self.a.0 == 0;
        self.p.n = self.v1.0 & 0x80 != 0;
        self.p.v = self.v1.0 & 0x40 != 0;
    }
    fn op_sta(&mut self) { self.v1 = self.a; }
    fn op_stx(&mut self) { self.v1 = self.x; }
    fn op_sty(&mut self) { self.v1 = self.y; }
    fn op_pha(&mut self) { self.v1 = self.a; }
    fn op_php(&mut self) { self.v1 = Wrapping(self.pack_p()) }
    fn op_sec(&mut self) { self.p.c = true; }     
    fn op_clc(&mut self) { self.p.c = false; }     
    fn op_sei(&mut self) { self.p.i = true; }     
    fn op_cli(&mut self) { self.p.i = false; }     
    fn op_sed(&mut self) { self.p.d = true; }     
    fn op_cld(&mut self) { self.p.d = false; }     
    fn op_clv(&mut self) { self.p.v = false; }     
    fn op_inx(&mut self) { self.x += Wrapping(1); }
    fn op_dex(&mut self) { self.x -= Wrapping(1); }
    fn op_iny(&mut self) { self.y += Wrapping(1); }
    fn op_dey(&mut self) { self.y -= Wrapping(1); }
    fn op_tax(&mut self) { self.x = self.a; }
    fn op_tay(&mut self) { self.y = self.a; }
    fn op_tsx(&mut self) { self.x = self.s; }
    fn op_txa(&mut self) { self.a = self.x; }
    fn op_txs(&mut self) { self.s = self.x; }
    fn op_tya(&mut self) { self.a = self.y; }
    fn op_pla(&mut self) { self.a = self.v1; }
    fn op_plp(&mut self) {  let stupid_borrow = self.v1.0; 
                            let tmpb = self.p.b;    // B is unaffected by plp
                            self.unpack_set_p(stupid_borrow); 
                            self.p.b = tmpb;
                            }
    
    fn op_bcs(&mut self) { if !self.p.c { self.ts = 3 }; }       // ts = 3 means to skip to T3 (+ 1), branch not taken
    fn op_bcc(&mut self) { if  self.p.c { self.ts = 3 }; }
    fn op_beq(&mut self) { if !self.p.z { self.ts = 3 }; }
    fn op_bne(&mut self) { if  self.p.z { self.ts = 3 }; }
    fn op_bvs(&mut self) { if !self.p.v { self.ts = 3 }; }
    fn op_bvc(&mut self) { if  self.p.v { self.ts = 3 }; }
    fn op_bmi(&mut self) { if !self.p.n { self.ts = 3 }; }
    fn op_bpl(&mut self) { if  self.p.n { self.ts = 3 }; }



    fn decode_op(op: u8) -> fn(&mut Self) {
        const OPTABLE: [fn(&mut P65); 256] = [
// MSD LSD-> 0            1            2            3            4            5            6            7            8            9            a            b            c            d            e            f
/*  0  */    P65::op_nil, P65::op_ora, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_ora, P65::op_asl, P65::op_unk, P65::op_php, P65::op_ora, P65::op_asl, P65::op_unk, P65::op_unk, P65::op_ora, P65::op_asl, P65::op_unk,
 		     P65::op_bpl, P65::op_ora, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_ora, P65::op_asl, P65::op_unk, P65::op_clc, P65::op_ora, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_ora, P65::op_asl, P65::op_unk, 
		     P65::op_nil, P65::op_and, P65::op_unk, P65::op_unk, P65::op_bit, P65::op_and, P65::op_rol, P65::op_unk, P65::op_plp, P65::op_and, P65::op_rol, P65::op_unk, P65::op_bit, P65::op_and, P65::op_rol, P65::op_unk, 
		     P65::op_bmi, P65::op_and, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_and, P65::op_rol, P65::op_unk, P65::op_sec, P65::op_and, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_and, P65::op_rol, P65::op_unk, 
		     P65::op_nil, P65::op_eor, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_eor, P65::op_lsr, P65::op_unk, P65::op_pha, P65::op_eor, P65::op_lsr, P65::op_unk, P65::op_nil, P65::op_eor, P65::op_lsr, P65::op_unk, 
		     P65::op_bvc, P65::op_eor, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_eor, P65::op_lsr, P65::op_unk, P65::op_cli, P65::op_eor, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_eor, P65::op_lsr, P65::op_unk, 
		     P65::op_nil, P65::op_adc, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_adc, P65::op_ror, P65::op_unk, P65::op_pla, P65::op_adc, P65::op_ror, P65::op_unk, P65::op_nil, P65::op_adc, P65::op_ror, P65::op_unk, 
		     P65::op_bvs, P65::op_adc, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_adc, P65::op_ror, P65::op_unk, P65::op_sei, P65::op_adc, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_adc, P65::op_ror, P65::op_unk, 
		     P65::op_unk, P65::op_sta, P65::op_unk, P65::op_unk, P65::op_sty, P65::op_sta, P65::op_stx, P65::op_unk, P65::op_dey, P65::op_unk, P65::op_txa, P65::op_unk, P65::op_sty, P65::op_sta, P65::op_stx, P65::op_unk, 
		     P65::op_bcc, P65::op_sta, P65::op_unk, P65::op_unk, P65::op_sty, P65::op_sta, P65::op_stx, P65::op_unk, P65::op_tya, P65::op_sta, P65::op_txs, P65::op_unk, P65::op_unk, P65::op_sta, P65::op_unk, P65::op_unk, 
		     P65::op_ldy, P65::op_lda, P65::op_ldx, P65::op_unk, P65::op_ldy, P65::op_lda, P65::op_ldx, P65::op_unk, P65::op_tay, P65::op_lda, P65::op_tax, P65::op_unk, P65::op_ldy, P65::op_lda, P65::op_ldx, P65::op_unk, 
		     P65::op_bcs, P65::op_lda, P65::op_unk, P65::op_unk, P65::op_ldy, P65::op_lda, P65::op_ldx, P65::op_unk, P65::op_clv, P65::op_lda, P65::op_tsx, P65::op_unk, P65::op_ldy, P65::op_lda, P65::op_ldx, P65::op_unk, 
		     P65::op_cpy, P65::op_cmp, P65::op_unk, P65::op_unk, P65::op_cpy, P65::op_cmp, P65::op_dec, P65::op_unk, P65::op_iny, P65::op_cmp, P65::op_dex, P65::op_unk, P65::op_cpy, P65::op_cmp, P65::op_dec, P65::op_unk, 
		     P65::op_bne, P65::op_cmp, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_cmp, P65::op_dec, P65::op_unk, P65::op_cld, P65::op_cmp, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_cmp, P65::op_dec, P65::op_unk, 
		     P65::op_cpx, P65::op_sbc, P65::op_unk, P65::op_unk, P65::op_cpx, P65::op_sbc, P65::op_inc, P65::op_unk, P65::op_inx, P65::op_sbc, P65::op_nop, P65::op_unk, P65::op_cpx, P65::op_sbc, P65::op_inc, P65::op_unk, 
		     P65::op_beq, P65::op_sbc, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_sbc, P65::op_inc, P65::op_unk, P65::op_sed, P65::op_sbc, P65::op_unk, P65::op_unk, P65::op_unk, P65::op_sbc, P65::op_inc, P65::op_unk,];
            
        OPTABLE[op as usize]
    }       

    // now the addressing modes, divided by group of opcodes

    fn a1_ac(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc.0 as usize); },      // discard read
            2 => { self.v1 = self.a; opfun(self); self.a = self.v1; self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a1_imp(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc.0 as usize); },      // discard read
            2 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_ix(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { mem.read(self.al.0 as usize);  self.v1 = self.al + self.x; },    // discd read
            3 => { self.al = Wrapping(mem.read(self.v1.0 as usize));  self.v1 += Wrapping(1); },
            4 => { self.ah = Wrapping(mem.read(self.v1.0 as usize)); },
            5 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize)); },
            6 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a2_imm(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.v1 = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }

// what is the type for something that is "usizeable" ? Doesn't exist. sht
//    fn mem_read<T>(m: &mut Mem, a: T) -> Wrapping<u8> {
//        Wrapping(m.read(usize::try_from(a).unwrap()))
//    }

    fn a2_zp(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { self.v1 = Wrapping(mem.read(self.al.0 as usize)); },
            3 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_abs(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            3 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize)); },
            4 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_iy(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.v1 = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc(); },
            2 => { self.al = Wrapping(mem.read(self.v1.0 as usize));  self.v1 += Wrapping(1); },
            3 => { self.ah = Wrapping(mem.read(self.v1.0 as usize));  
                                self.v2 = Wrapping(((self.al.0 as u32 + self.y.0 as u32) >> 8) as u8);
                                self.al = self.al + self.y; },
            4 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize)); 
                                self.ah += self.v2; 
                                if self.v2 == Wrapping(0) { self.ts_inc(); }; },
            5 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize));  },
            6 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a2_zpx(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { mem.read(self.al.0 as usize); self.al += self.x; },  //discard read
            3 => { self.v1 = Wrapping(mem.read(self.al.0 as usize)); },
            4 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_zpy(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { mem.read(self.al.0 as usize); self.al += self.y; },  //discard read
            3 => { self.v1 = Wrapping(mem.read(self.al.0 as usize)); },
            4 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_ay(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc(); 
                                self.v2 = Wrapping(((self.al.0 as u16 + self.y.0 as u16) >> 8) as u8);  // here v2 is max 1
                                self.al = self.al + self.y; },
            3 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize)); 
                                self.ah += self.v2; 
                                if self.v2 == Wrapping(0) { self.ts_inc(); }; },
            4 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize));  },
            5 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a2_ax(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc();
                                self.v2 = Wrapping(((self.al.0 as u32 + self.x.0 as u32) >> 8) as u8);
                                self.al = self.al + self.x; },
            3 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize)); 
                                self.ah += self.v2; 
                                if self.v2 == Wrapping(0) { self.ts_inc(); }; },
            4 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize));  },
            5 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    
    fn a3_zp(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { opfun(self); mem.write(self.al.0 as usize, self.v1.0); },
            3 => { self.fetch_op(mem); }
            _ => {},
        }
    }

    fn a3_abs(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            3 => { opfun(self); mem.write(self.ah_al() as usize, self.v1.0); },
            4 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_ix(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc(); },
            2 => { mem.read(self.al.0 as usize);                     self.v1 = self.al + self.x; },     // discard read
            3 => { self.al = Wrapping(mem.read(self.v1.0 as usize));  self.v1 += Wrapping(1); },
            4 => { self.ah = Wrapping(mem.read(self.v1.0 as usize)); },
            5 => { opfun(self); mem.write(self.ah_al() as usize, self.v1.0); },
            6 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_ax(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc();
                                self.v2 = Wrapping(((self.al.0 as u32 + self.x.0 as u32) >> 8) as u8);
                                self.al = self.al + self.x; },
            3 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize)); 
                                self.ah += self.v2; },
            4 => { opfun(self); mem.write(self.ah_al() as usize, self.v1.0);  },
            5 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a3_ay(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc();
                                self.v2 = Wrapping(((self.al.0 as u32 + self.y.0 as u32) >> 8) as u8);
                                self.al = self.al + self.y; },
            3 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize)); 
                                self.ah += self.v2; },
            4 => { opfun(self); mem.write(self.ah_al() as usize, self.v1.0);  },
            5 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a3_zpx(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { mem.read(self.al.0 as usize);       self.al += self.x;          },  // discard  
            3 => { opfun(self); mem.write(self.al.0 as usize, self.v1.0); },
            4 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_zpy(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { mem.read(self.al.0 as usize);       self.al += self.y;          },  // discard  
            3 => { opfun(self); mem.write(self.al.0 as usize, self.v1.0); },
            4 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_iy(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.v1 = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc(); },
            2 => { self.al = Wrapping(mem.read(self.v1.0 as usize));  self.v1 += Wrapping(1); },
            3 => { self.ah = Wrapping(mem.read(self.v1.0 as usize));  
                                self.v2 = Wrapping(((self.al.0 as u32 + self.y.0 as u32) >> 8) as u8);
                                self.al = self.al + self.y; },
            4 => { mem.read(self.ah_al() as usize);                  self.ah += self.v2; },
            5 => { opfun(self); mem.write(self.ah_al() as usize, self.v1.0);  },
            6 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a4_zp(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { self.v1 = Wrapping(mem.read(self.al.0 as usize));                },
            3 => { mem.write(self.al.0 as usize, self.v1.0); },                          // wasted write
            4 => { opfun(self); mem.write(self.al.0 as usize, self.v1.0); },
            5 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a4_zpx(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { mem.read(self.al.0 as usize); self.al += self.x; },                 //discard read
            3 => { self.v1 = Wrapping(mem.read(self.al.0 as usize)); },
            4 => { mem.write(self.al.0 as usize, self.v1.0); },                          // wasted write
            5 => { opfun(self); mem.write(self.al.0 as usize, self.v1.0); },
            6 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a4_ax(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize));  self.inc_pc();
                                self.v2 = Wrapping(((self.al.0 as u32 + self.x.0 as u32) >> 8) as u8);
                                self.al = self.al + self.x; },
            3 => { mem.read(self.ah_al() as usize); self.ah += self.v2; },        // discard read
            4 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize)); },
            5 => { mem.write(self.ah_al() as usize, self.v1.0);  },               // wasted write
            6 => { opfun(self); mem.write(self.ah_al() as usize, self.v1.0);  },
            7 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn a4_abs(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            3 => { self.v1 = Wrapping(mem.read(self.ah_al() as usize));                },
            4 => { mem.write(self.ah_al() as usize, self.v1.0); },                          // wasted write
            5 => { opfun(self); mem.write(self.ah_al() as usize, self.v1.0); },
            6 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    
    fn jsr_abs(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { mem.read((self.s.0 as usize) + 0x100);  },        // discard read. s tack pointer is always 1xx
            3 => { mem.write((self.s.0 as usize) + 0x100, (self.pc.0 >> 8) as u8);  self.dec_sp();  },
            4 => { mem.write((self.s.0 as usize) + 0x100, (self.pc.0 & 0xFF) as u8); self.dec_sp(); },    // check with a real 6502
            5 => { self.ah = Wrapping(mem.read(self.pc.0 as usize));  },
            6 => { self.pc = Wrapping(self.ah_al());      self.fetch_op(mem); },   // load PC and fetch. only one mem read       
            _ => {},
        }
    }

    // TODO: NMI & RESET (FFFA e FFFC)
    // FIXME
    // please note that B is always set by BRK
    // and is pushed on the stack with P
    // when serving IRQs (caveats) B will be zero, at least on the stack -- don't know
    // if you can push it and it is 1/0
    // check: http://visual6502.org/wiki/index.php?title=6502_BRK_and_B_bit
    fn brk_imp(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc.0 as usize); self.p.b = true; },                // discard read. pc has been incremented earlier
            2 => { mem.write((self.s.0 as usize) + 0x100, (self.pc.0 >> 8) as u8);   self.dec_sp();  },
            3 => { mem.write((self.s.0 as usize) + 0x100, (self.pc.0 & 0xFF) as u8); self.dec_sp();  },    
            4 => { mem.write((self.s.0 as usize) + 0x100, (self.pack_p()) as u8);      self.dec_sp(); },
            5 => { self.pc = Wrapping(mem.read(0xFFFE) as u16);  },     // PCL
            6 => { self.pc = Wrapping((mem.read(0xFFFF) as u16) << 8); }, // PCH
            7 => { self.fetch_op(mem); },        
            _ => {},
        }
    }
    fn rti_imp(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc.0 as usize);    self.inc_pc(); },   // discard read
            2 => { mem.read((self.s.0 as usize) + 0x100); self.inc_sp(); },     // discard read too
            3 => {  let pedante=mem.read(self.s.0 as usize);
                    let tmpb = self.p.b; 
                    self.unpack_set_p(pedante); self.inc_sp();
                    self.p.b = tmpb;       // b is unaffected by rti & plp
                        },
            4 => { self.pc = Wrapping(mem.read((self.s.0 as usize) + 0x100) as u16 ); self.inc_sp(); }, 
            5 => { self.pc += Wrapping((mem.read((self.s.0 as usize) + 0x100) as u16) << 8 );  }, 
            6 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn jmp_abs(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            3 => { self.pc = Wrapping(self.ah_al());      self.fetch_op(mem); },   // load PC and fetch. only one mem read       
            _ => {},
        }
    }

    fn jmp_ind(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { self.al = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            2 => { self.ah = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc(); },
            3 => { self.pc = Wrapping(mem.read(self.ah_al() as usize) as u16 ); self.al += Wrapping(1) },  // carry IS NOT propagated.. don't jump from (XXFF)!
            4 => { self.pc += Wrapping((mem.read(self.ah_al() as usize) as u16) << 8); },   // load PC and fetch. only one mem read       
            5 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    // FIXME check in RTS l'indirizzo di ritorno è ok
    // in RTI è l'indirizzo -1
    fn rts_imp(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc.0 as usize);    self.inc_pc(); },   // discard read
            2 => { mem.read((self.s.0 as usize) + 0x100); self.inc_sp(); },     // discard read too
            3 => { self.pc = Wrapping(mem.read((self.s.0 as usize) + 0x100) as u16 ); self.inc_sp(); }, 
            4 => { self.pc += Wrapping((mem.read((self.s.0 as usize) + 0x100) as u16) << 8 );  }, 
            5 => { mem.read(self.pc.0 as usize);    self.inc_pc(); },   // discard read, inc pc
            6 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn a5_bxx(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { self.v1 = Wrapping(mem.read(self.pc.0 as usize)); self.inc_pc();  opfun(self);   },    // skip to 4 if branch not taken. relative jump is calculated from nextop address
            2 => { mem.read(self.pc.0 as usize);
                                self.v2 = Wrapping(((self.v1.0 as u16 + (self.pc.0 & 0xFF)) >> 8) as u8);    // possible error in appendix a 5.8 hw manual, here fixed with discard
                                self.pc = Wrapping((self.pc.0 & 0xFF00) | 
                                    (self.pc.0.wrapping_add(self.v1.0 as u16) & 0xFF));             // increment only pcl
                                if self.v2 == Wrapping(0) { self.ts += 1; } },                        // skip next if not carry 
            3 => { mem.read(self.pc.0 as usize);  self.pc += Wrapping((self.v2.0 as u16) << 8);  },                          // eventually complete carry propagation
            4 => { self.fetch_op(mem); },                                                                // finally fetch new opcode
            _ => {},
        }
    }

    fn a5_plx(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc.0 as usize); },                                 // discard read
            2 => { mem.read((self.s.0 as usize) + 0x100); self.inc_sp();  },        // discard read
            3 => { self.v1 = Wrapping(mem.read((self.s.0 as usize) +0x100)); },
            4 => { opfun(self);   self.fetch_op(mem);  },                              // place a or p in its right place
            _ => {},
        }
    }

    fn a5_phx(&mut self, mem: &mut Mem, opfun: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc.0 as usize); },                                 // discard read (don't incpc)
            2 => { opfun(self);  mem.write((self.s.0 as usize) + 0x100, self.v1.0);     self.dec_sp(); },
            3 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn ad_unk(&mut self, _: &mut Mem, _: fn(&mut Self)) {}


    fn decode_addr_mode(op: u8) -> fn(&mut Self, &mut Mem, fn(&mut Self)) {
        const ADDRTABLE: [fn(&mut P65, &mut Mem, fn(&mut P65)); 256] = [
			 P65::brk_imp ,P65::a2_ix ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zp  ,P65::a4_zp  ,P65::ad_unk ,P65::a5_phx ,P65::a2_imm, P65::a1_ac  ,P65::ad_unk ,P65::ad_unk  ,P65::a2_abs ,P65::a4_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zpx ,P65::a4_zpx ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::ad_unk ,P65::ad_unk ,P65::ad_unk  ,P65::a2_ax  ,P65::a4_ax  ,P65::ad_unk,
			 P65::jsr_abs ,P65::a2_ix ,P65::ad_unk  ,P65::ad_unk ,P65::a2_zp  ,P65::a2_zp  ,P65::a4_zp  ,P65::ad_unk ,P65::a5_plx ,P65::a2_imm ,P65::a1_ac  ,P65::ad_unk ,P65::a2_abs  ,P65::a2_abs ,P65::a4_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zpx ,P65::a4_zpx ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::ad_unk ,P65::ad_unk ,P65::ad_unk  ,P65::a2_ax  ,P65::a4_ax  ,P65::ad_unk,
			 P65::rti_imp ,P65::a2_ix ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zp  ,P65::a4_zp  ,P65::ad_unk ,P65::a5_phx ,P65::a2_imm ,P65::a1_ac  ,P65::ad_unk ,P65::jmp_abs ,P65::a2_abs ,P65::a4_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zpx ,P65::a4_zpx ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::ad_unk ,P65::ad_unk ,P65::ad_unk  ,P65::a2_ax  ,P65::a4_ax  ,P65::ad_unk,
			 P65::rts_imp ,P65::a2_ix ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zp  ,P65::a4_zp  ,P65::ad_unk ,P65::a5_plx ,P65::a2_imm ,P65::a1_ac  ,P65::ad_unk ,P65::jmp_ind ,P65::a2_abs ,P65::a4_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zpx ,P65::a4_zpx ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::ad_unk ,P65::ad_unk ,P65::ad_unk  ,P65::a2_ax  ,P65::a4_ax  ,P65::ad_unk,
			 P65::ad_unk  ,P65::a3_ix ,P65::ad_unk  ,P65::ad_unk ,P65::a3_zp  ,P65::a3_zp  ,P65::a3_zp  ,P65::ad_unk ,P65::a1_imp ,P65::ad_unk ,P65::a1_imp ,P65::ad_unk ,P65::a3_abs  ,P65::a3_abs ,P65::a3_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a3_iy ,P65::ad_unk  ,P65::ad_unk ,P65::a3_zpx ,P65::a3_zpx ,P65::a3_zpy ,P65::ad_unk ,P65::a1_imp ,P65::a3_ay  ,P65::a1_imp ,P65::ad_unk ,P65::ad_unk  ,P65::a3_ax  ,P65::ad_unk ,P65::ad_unk,
			 P65::a2_imm  ,P65::a2_ix ,P65::a2_imm  ,P65::ad_unk ,P65::a2_zp  ,P65::a2_zp  ,P65::a2_zp  ,P65::ad_unk ,P65::a1_imp ,P65::a2_imm ,P65::a1_imp ,P65::ad_unk ,P65::a2_abs  ,P65::a2_abs ,P65::a2_abs  ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::a2_zpx ,P65::a2_zpx ,P65::a2_zpy ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::a1_imp ,P65::ad_unk ,P65::a2_ax   ,P65::a2_ax  ,P65::a2_ay  ,P65::ad_unk,
			 P65::a2_imm  ,P65::a2_ix ,P65::ad_unk  ,P65::ad_unk ,P65::a2_zp  ,P65::a2_zp  ,P65::a4_zp  ,P65::ad_unk ,P65::a1_imp ,P65::a2_imm ,P65::a1_imp ,P65::ad_unk ,P65::a2_abs  ,P65::a2_abs ,P65::a4_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zpx ,P65::a4_zpx ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::ad_unk ,P65::ad_unk ,P65::ad_unk  ,P65::a2_ax  ,P65::a4_ax  ,P65::ad_unk,
			 P65::a2_imm  ,P65::a2_ix ,P65::ad_unk  ,P65::ad_unk ,P65::a2_zp  ,P65::a2_zp  ,P65::a4_zp  ,P65::ad_unk ,P65::a1_imp ,P65::a2_imm ,P65::a1_imp ,P65::ad_unk ,P65::a2_abs  ,P65::a2_abs ,P65::a4_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zpx ,P65::a4_zpx ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::ad_unk ,P65::ad_unk ,P65::ad_unk  ,P65::a2_ax  ,P65::a4_ax  ,P65::ad_unk,
        ]   ;
        ADDRTABLE[op as usize]
    }

    // we fake this.
    fn reset(&mut self, mem: &mut Mem) {
        self.s = Wrapping(0xFD);
        self.op = 0x00;
        self.al = Wrapping(mem.read(0xFFFC));
        self.ah = Wrapping(mem.read(0xFFFD));
        self.pc = Wrapping(self.ah_al());
        self.fetch_op(mem);
        self.tick();
        self.cycle = 8;
    }

    fn run(&mut self, mem: &mut Mem, count: u64) -> u64 {
        for _ in 0..count {
            let opfun = P65::decode_op(self.op);
            let opaddr = P65::decode_addr_mode(self.op);
            opaddr(self, mem, opfun);
            self.tick();
        }
        0
    }
}

static mut lastchar: u8 = 0;

struct Mem<'a>(&'a mut [u8]);

impl<'a> Mem<'a> {
    fn read(&mut self, a: usize) -> u8 {
        if a == 0xF004 {
            println!("leggo!");
            unsafe {
                let tmp = lastchar; 
                lastchar = 0;
                tmp
            }
        } else {
            self.0[a] 
        }
    }
    fn write(&mut self, a: usize, v: u8) { 
//        if a == 0xF001 { println!("bem!{}\r",v); }
        self.0[a] = v; 
    }
}

fn main() {
    let mut mem_store = [0u8; 65536];
    let prog = [0xa9u8 ,0x00 ,0x20 ,0x10 ,0x00 ,0x4c ,0x02 ,0x00 ,0x00 ,0x00 ,0x00 ,0x00 ,0x00 ,0x00 ,0x00 ,0x40 ,0xe8 ,0x88 ,0xe6 ,0x0f ,0x38 ,0x69 ,0x02 ,0x60];
    for x in 0..prog.len() {
        mem_store[x] = prog[x];
    }

    let new_stdout = stdout();
    let mut stdout = new_stdout.lock().into_raw_mode().unwrap();
    let mut stdin = async_stdin().bytes();


    let mut f = std::fs::File::open("tests/ehbasic.bin").unwrap();
    let rs = f.read_exact(&mut mem_store[0xC000..]);
    if let Ok(_) = rs {
        println!("Good read ");
    } else {
        panic!("File not read in full");
    }
    
    

    let mut mem = Mem(&mut mem_store);
    let mut pr = P65::new();
    
    pr.reset(&mut mem);
    
    for x in 0..100000 {
        let c = stdin.next();
        match c {
            Some(Ok(c)) => {
                match c {
                    0x3 => {
                       write!(stdout,"Good-bye!\r\n").unwrap();
                       break;
                    },
                    c => { unsafe { lastchar = c; } },
                }
            },
            Some(Error) => { write!(stdout, "Error char\r\n").unwrap(); },
            _ => { },
        }
        pr.run(&mut mem, 1);
//        if pr.pc == Wrapping(0x45c0+1) { break; }   // +1 because pc is autoincremented during fetch
        println!("({}): {:?}\r", x, pr);
    }
    println!("mem a #xF: {}\r", mem.read(0xF));
    println!("mem a #x0210: {}\r", mem.read(0x210));
    println!("mem a #x71: {}\r", mem.read(0x71));
    println!("mem a #x202: {}\r", mem.read(0x202));
    println!("mem a #x22a: {}\r", mem.read(0x22a));
}


// NOTA: il negativo ha un problema al ciclo 1000 sul visual 6502, non mi combacia. fixare qui.

/*
    11.82%  $A5 LDA zero-page
    10.37%  $D0 BNE
     7.33%  $4C JMP absolute
     6.97%  $E8 INX
     4.46%  $10 BPL
     3.82%  $C9 CMP immediate
     3.49%  $30 BMI
     3.32%  $F0 BEQ
     3.32%  $24 BIT zero-page
     2.94%  $85 STA zero-page
     2.00%  $88 DEX
     1.98%  $C8 INY
     1.77%  $A8 TAY
     1.74%  $E6 INC zero-page
     1.74%  $B0 BCS
     1.66%  $BD LDA absolute,X
     1.64%  $B5 LDA zero-page,X
     1.51%  $AD LDA absolute
     1.41%  $20 JSR absolute
     1.38%  $4A LSR A
     1.37%  $60 RTS
     1.35%  $B1 LDA (zero-page),Y
     1.32%  $29 AND immediate
     1.27%  $9D STA absolute,X
     1.24%  $8D STA absolute
     1.08%  $18 CLC
     1.03%  $A9 LDA immediate
     */