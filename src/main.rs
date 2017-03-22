#![allow(unused_parens)]
#![allow(unused_must_use)]
#![allow(unused_variables)]

extern crate termion;
use termion::raw::IntoRawMode;
use termion::async_stdin;
use std::io::{Read, stdin, stdout, Write};
use std::fmt;
use std::ascii::AsciiExt;

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
    a: u8,
    x: u8,
    y: u8,
    p: P65Flags, 
    s: u8,
    pc: u16,

// emulator state
    cycle: u64,
    ts: u8,
    op: u8,
    v1: u8,
    v2: u8,
    ah: u8,
    al: u8,
}

type AddrModeF = fn(&mut P65, &mut Mem,  fn(&mut P65));
type OpcodeF = fn(&mut P65);


impl fmt::Debug for P65 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "T{:01x} pc:{:04x} a:{:02x} x:{:02x} y:{:02x} p:{:02x} s:{:02x} op:{:02x} v1:{:02x} v2:{:02x} ah/al: {:04x} Cy:{:06}",
             self.ts, self.pc, self.a,self.x,self.y,self.pack_p(), self.s, self.op, self.v1, self.v2, self.ah_al(), self.cycle % 1000000 )
    }
}

impl P65 {
    fn new() -> P65 {
        P65 { 
            a:0xaa, 
            x:0, 
            y:0, 
            p: P65Flags {n: false, v: false, bit5: true, b: true, d: false, i: true, z: true, c: false,},
            s: (0xfd), 
            pc: (0), 
            cycle: 0, 
            ts: 0, 
            op: 0, 
            v1: (0), 
            v2: (0), 
            ah: (0), 
            al: (0),
            }
    }

    fn cycle_inc(&mut self) -> u64 {
        self.cycle = self.cycle + 1;
        self.cycle
    }

    fn ts_inc (&mut self) -> u8 {
        self.ts = self.ts.wrapping_add(1);
        self.ts
    }

    fn fetch_op(&mut self, mem: &mut Mem) -> u8 {
        self.op = mem.read(self.pc as usize);
        self.inc_pc();
        self.ts = 0;    // will be incremented to 1 by tick
        self.op
    }

    fn tick(&mut self) {
        self.ts_inc();
        self.cycle_inc();
    }

    fn ah_al(&self) -> u16 { (self.ah as u16) << 8 | (self.al as u16) }
    fn inc_pc(&mut self) { self.pc = self.pc.wrapping_add(1); }
    #[allow(dead_code)]
    fn set_pc(&mut self, pch: u8, pcl: u8) { self.pc = ((pch as u16) << 8) | (pcl as u16); }

    fn inc_sp(&mut self) { self.s = self.s.wrapping_add(1); }
    fn dec_sp(&mut self) { self.s = self.s.wrapping_sub(1); }

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
        self.p.b = flags & 0x10 != 0;    // FIXME. B is full of sad
        self.p.d = flags & 0x08 != 0; 
        self.p.i = flags & 0x04 != 0; 
        self.p.z = flags & 0x02 != 0; 
        self.p.c = flags & 0x01 != 0; 
    }

    // adjust negative and zero flags, a common operation.
    // i am using a parameter, v, instead of relying on v1 etc because of the ease of making mistakes here. 
    fn fix_nz(&mut self, v: u8) {
        self.p.z = v == 0;
        self.p.n = v >= 0x80;
    }

    // operations
    fn op_asl(&mut self) {
        self.p.c = self.v1 & 0x80 != 0;
        self.v1 = self.v1 << 1;
        let tmp = self.v1; self.fix_nz(tmp);
    }

    fn op_lsr(&mut self) {
        self.p.c = self.v1 & 0x01 != 0;
        self.v1 = self.v1 >> 1;
        let tmp = self.v1; self.fix_nz(tmp);
    }        
    fn op_rol(&mut self) {
        let tmp = self.v1;
        self.v1 = self.v1 << 1 |  (if self.p.c { 1 } else { 0 })  ;
        self.p.c = tmp &  (0x80) !=  (0);
        let tmp = self.v1; self.fix_nz(tmp);
    }
    fn op_ror(&mut self) {
        let tmp = self.v1;
        self.v1 = self.v1 >> 1 |  (if self.p.c { 0x80 } else { 0 })  ;
        self.p.c = tmp &  (0x1) !=  (0);
        let tmp = self.v1; self.fix_nz(tmp);
    }
    fn op_unk(&mut self) {
        assert!(1==0, "Unknown Opcode!");
    }
    fn op_nil(&mut self) { }     // nil means the opcode is managed elsewhere
    fn op_nop(&mut self) { }
    fn op_adc(&mut self) {
        if self.p.d { 
            self.op_adc_dec(); } else { self.op_adc_bin(); }
    }
    fn op_adc_bin(&mut self) {
            let tsum = (self.a as u16 + self.v1 as u16 + if self.p.c { 0x1 } else { 0x0 }) as u16;
            self.p.c = tsum >= 0x100;
            self.p.v = !((self.a as u8 & 0x80) ^ (self.v1 & 0x80)) & ((self.a as u8 & 0x80) ^ ((tsum & 0x80) as u8)) != 0;
            self.v1 =  ((tsum & 0xff) as u8);
            let tmp = self.v1; self.fix_nz(tmp);
            self.a = self.v1;
    }
    fn op_adc_dec(&mut self) {
            let mut c1: u8 = (self.a & 0x0F) + (self.v1 & 0x0F) + if self.p.c { 1 } else { 0 };
            let mut c2: u8 = self.a.wrapping_shr(4) + self.v1.wrapping_shr(4);
            if c1 >= 0xA { c1 -= 0xA; c2 += 1; }
            if c2 >= 0xA { c2 -= 0xA; self.p.c = true; } else { self.p.c = false; }
            self.a = c2.wrapping_shl(4) | c1;
            self.p.z = (self.a == 0);
    }
    fn op_sbc(&mut self) {
        if self.p.d { 
            self.op_sbc_dec(); 
        } else { 
            self.op_sbc_bin(); 
        }
    }
    fn op_sbc_bin(&mut self) {
            let tsub = (self.a as u16).wrapping_sub(self.v1 as u16).wrapping_sub(if self.p.c { 0x0 } else { 0x1 } as u16);
            self.p.c = !(tsub >= 0x100);
            self.p.v = ((self.a as u8 & 0x80) ^ (self.v1 & 0x80)) & ((self.a as u8 & 0x80) ^ ((tsub & 0x80) as u8)) != 0;
            self.v1 = (tsub & 0xff) as u8;
            let tmp = self.v1; self.fix_nz(tmp);
            self.a = self.v1 ;
    }
    fn op_sbc_dec(&mut self) {
            let mut c1 = 0xA + (self.a & 0x0F) - (self.v1 & 0x0F) - (if self.p.c { 0 } else { 1 });
            let mut c2 = 0xA + self.a.wrapping_shr(4) -self.v1.wrapping_shr(4);
            if c1 >= 0xA { c1 -= 0xA; } else { c2 -= 1; }
            if c2 >= 0xA { c2 -= 0xA; self.p.c = true; } else { self.p.c = false; }
            self.a = c2.wrapping_shl(4) | (c1);
            self.p.z = self.a == 0;
    }
    fn op_and(&mut self) { self.a = self.a & self.v1; let tmp = self.a; self.fix_nz(tmp); }
    fn op_ora(&mut self) { self.a = self.a | self.v1; let tmp = self.a; self.fix_nz(tmp); }
    fn op_eor(&mut self) { self.a = self.a ^ self.v1; let tmp = self.a; self.fix_nz(tmp); }
    fn op_cmp(&mut self) {
        let tsub = self.a.wrapping_sub(self.v1);
        self.p.c = self.a >= self.v1;
        self.fix_nz(tsub);
    }
    fn op_cpx(&mut self) {
        let tsub = self.x.wrapping_sub(self.v1);
        self.p.c = self.x >= self.v1;
        self.fix_nz(tsub);
    }
    fn op_cpy(&mut self) {
        let tsub = self.y.wrapping_sub(self.v1);
        self.p.c = self.y >= self.v1;
        self.fix_nz(tsub);
    }
    fn op_dec(&mut self) { self.v1 = self.v1.wrapping_sub(1); let tmp = self.v1; self.fix_nz(tmp); }
    fn op_inc(&mut self) { self.v1 = self.v1.wrapping_add(1); let tmp = self.v1; self.fix_nz(tmp); }
    fn op_lda(&mut self) { self.a = self.v1; let tmp = self.a; self.fix_nz(tmp); }
    fn op_ldx(&mut self) { self.x = self.v1; let tmp = self.x; self.fix_nz(tmp); }
    fn op_ldy(&mut self) { self.y = self.v1; let tmp = self.y; self.fix_nz(tmp); }
    fn op_bit(&mut self) {
        self.p.z = self.v1 & self.a == 0;
        self.p.n = self.v1 & 0x80 != 0;
        self.p.v = self.v1 & 0x40 != 0;
    }
    fn op_sta(&mut self) { self.v1 = self.a; }
    fn op_stx(&mut self) { self.v1 = self.x; }
    fn op_sty(&mut self) { self.v1 = self.y; }
    fn op_pha(&mut self) { self.v1 = self.a; }
    fn op_php(&mut self) { self.v1 = self.pack_p(); }
    fn op_sec(&mut self) { self.p.c = true; }     
    fn op_clc(&mut self) { self.p.c = false; }     
    fn op_sei(&mut self) { self.p.i = true; }     
    fn op_cli(&mut self) { self.p.i = false; }     
    fn op_sed(&mut self) { self.p.d = true; }     
    fn op_cld(&mut self) { self.p.d = false; }     
    fn op_clv(&mut self) { self.p.v = false; }     
    fn op_inx(&mut self) { self.x = self.x.wrapping_add(1); let tmp = self.x; self.fix_nz(tmp); }  //  borrow check is retard
    fn op_dex(&mut self) { self.x = self.x.wrapping_sub(1); let tmp = self.x; self.fix_nz(tmp); }
    fn op_iny(&mut self) { self.y = self.y.wrapping_add(1); let tmp = self.y; self.fix_nz(tmp); }
    fn op_dey(&mut self) { self.y = self.y.wrapping_sub(1); let tmp = self.y; self.fix_nz(tmp); }
    fn op_tax(&mut self) { self.x = self.a; let tmp = self.x; self.fix_nz(tmp); }
    fn op_tay(&mut self) { self.y = self.a; let tmp = self.y; self.fix_nz(tmp); }
    fn op_tsx(&mut self) { self.x = self.s; let tmp = self.x; self.fix_nz(tmp); }
    fn op_txa(&mut self) { self.a = self.x; let tmp = self.a; self.fix_nz(tmp); }
    fn op_txs(&mut self) { self.s = self.x;  }  // TXS doesn't change flags
    fn op_tya(&mut self) { self.a = self.y; let tmp = self.a; self.fix_nz(tmp); }
    fn op_pla(&mut self) { self.a = self.v1; let tmp = self.a; self.fix_nz(tmp); }
    fn op_plp(&mut self) {  let stupid_borrow = self.v1; 
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



    fn decode_op(op: u8) -> OpcodeF {
        const OPTABLE: [OpcodeF; 256] = [
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
    #[allow(dead_code)]
    fn op_name(op: u8) -> &'static str {
        const OPTABLE: [&'static str; 256] = [
// MSD LSD-> 0            1            2            3            4            5            6            7            8            9            a            b            c            d            e            f
             "brk", "ora", "unk", "unk", "unk", "ora", "asl", "unk", "php", "ora", "asl", "unk", "unk", "ora", "asl", "unk",
 		     "bpl", "ora", "unk", "unk", "unk", "ora", "asl", "unk", "clc", "ora", "unk", "unk", "unk", "ora", "asl", "unk", 
		     "jsr", "and", "unk", "unk", "bit", "and", "rol", "unk", "plp", "and", "rol", "unk", "bit", "and", "rol", "unk", 
		     "bmi", "and", "unk", "unk", "unk", "and", "rol", "unk", "sec", "and", "unk", "unk", "unk", "and", "rol", "unk", 
		     "rti", "eor", "unk", "unk", "unk", "eor", "lsr", "unk", "pha", "eor", "lsr", "unk", "jmp", "eor", "lsr", "unk", 
		     "bvc", "eor", "unk", "unk", "unk", "eor", "lsr", "unk", "cli", "eor", "unk", "unk", "unk", "eor", "lsr", "unk", 
		     "rts", "adc", "unk", "unk", "unk", "adc", "ror", "unk", "pla", "adc", "ror", "unk", "jmp", "adc", "ror", "unk", 
		     "bvs", "adc", "unk", "unk", "unk", "adc", "ror", "unk", "sei", "adc", "unk", "unk", "unk", "adc", "ror", "unk", 
		     "unk", "sta", "unk", "unk", "sty", "sta", "stx", "unk", "dey", "unk", "txa", "unk", "sty", "sta", "stx", "unk", 
		     "bcc", "sta", "unk", "unk", "sty", "sta", "stx", "unk", "tya", "sta", "txs", "unk", "unk", "sta", "unk", "unk", 
		     "ldy", "lda", "ldx", "unk", "ldy", "lda", "ldx", "unk", "tay", "lda", "tax", "unk", "ldy", "lda", "ldx", "unk", 
		     "bcs", "lda", "unk", "unk", "ldy", "lda", "ldx", "unk", "clv", "lda", "tsx", "unk", "ldy", "lda", "ldx", "unk", 
		     "cpy", "cmp", "unk", "unk", "cpy", "cmp", "dec", "unk", "iny", "cmp", "dex", "unk", "cpy", "cmp", "dec", "unk", 
		     "bne", "cmp", "unk", "unk", "unk", "cmp", "dec", "unk", "cld", "cmp", "unk", "unk", "unk", "cmp", "dec", "unk", 
		     "cpx", "sbc", "unk", "unk", "cpx", "sbc", "inc", "unk", "inx", "sbc", "nop", "unk", "cpx", "sbc", "inc", "unk", 
		     "beq", "sbc", "unk", "unk", "unk", "sbc", "inc", "unk", "sed", "sbc", "unk", "unk", "unk", "sbc", "inc", "unk",];
            
        OPTABLE[op as usize]
    }       


    // now the addressing modes, divided by group of opcodes

    fn a1_ac(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { mem.read(self.pc as usize); },      // discard read
            2 => { self.v1 = self.a; opfun(self); self.a = self.v1; self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a1_imp(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { mem.read(self.pc as usize); },      // discard read
            2 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_ix(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { mem.read(self.al as usize);  self.v1 = self.al.wrapping_add(self.x); },    // discd read
            3 => { self.al =  (mem.read(self.v1 as usize));  self.v1 = self.v1.wrapping_add(1); },
            4 => { self.ah =  (mem.read(self.v1 as usize)); },
            5 => { self.v1 =  (mem.read(self.ah_al() as usize)); },
            6 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a2_imm(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.v1 =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }

// what is the type for something that is "usizeable" ? Doesn't exist. sht
//    fn mem_read<T>(m: &mut Mem, a: T) ->  <u8> {
//         (m.read(usize::try_from(a).unwrap()))
//    }

    fn a2_zp(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { self.v1 =  (mem.read(self.al as usize)); },
            3 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_abs(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize)); self.inc_pc(); },
            3 => { self.v1 =  (mem.read(self.ah_al() as usize)); },
            4 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_iy(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.v1 =  (mem.read(self.pc as usize));  self.inc_pc(); },
            2 => { self.al =  (mem.read(self.v1 as usize));  self.v1 = self.v1.wrapping_add(1); },
            3 => { self.ah =  (mem.read(self.v1 as usize));  
                                self.v2 =  (((self.al as u32 + self.y as u32) >> 8) as u8);
                                self.al = self.al.wrapping_add(self.y); },
            4 => { self.v1 =  (mem.read(self.ah_al() as usize)); 
                                self.ah = self.ah.wrapping_add(self.v2); 
                                if self.v2 ==  (0) { self.ts_inc(); }; },
            5 => { self.v1 =  (mem.read(self.ah_al() as usize));  },
            6 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a2_zpx(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { mem.read(self.al as usize); self.al = self.al.wrapping_add(self.x); },  //discard read
            3 => { self.v1 =  (mem.read(self.al as usize)); },
            4 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_zpy(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { mem.read(self.al as usize); self.al = self.al.wrapping_add(self.y); },  //discard read
            3 => { self.v1 =  (mem.read(self.al as usize)); },
            4 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_ay(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize));  self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize));  self.inc_pc(); 
                                self.v2 =  (((self.al as u16 + self.y as u16) >> 8) as u8);  // here v2 is max 1
                                self.al = self.al.wrapping_add(self.y); },
            3 => { self.v1 =  (mem.read(self.ah_al() as usize)); 
                                self.ah = self.ah.wrapping_add(self.v2); 
                                if self.v2 ==  (0) { self.ts_inc(); }; },
            4 => { self.v1 =  (mem.read(self.ah_al() as usize));  },
            5 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a2_ax(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize));  self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize));  self.inc_pc();
                                self.v2 =  (((self.al as u32 + self.x as u32) >> 8) as u8);
                                self.al = self.al.wrapping_add(self.x); },
            3 => { self.v1 =  (mem.read(self.ah_al() as usize)); 
                                self.ah = self.ah.wrapping_add(self.v2); 
                                if self.v2 == 0 { self.ts_inc(); }; },
            4 => { self.v1 =  (mem.read(self.ah_al() as usize));  },
            5 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    
    fn a3_zp(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { opfun(self); mem.write(self.al as usize, self.v1); },
            3 => { self.fetch_op(mem); }
            _ => {},
        }
    }

    fn a3_abs(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize)); self.inc_pc(); },
            3 => { opfun(self); mem.write(self.ah_al() as usize, self.v1); },
            4 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_ix(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize));  self.inc_pc(); },
            2 => { mem.read(self.al as usize);                     self.v1 = self.al.wrapping_add(self.x); },     // discard read
            3 => { self.al =  (mem.read(self.v1 as usize));  self.v1 = self.v1.wrapping_add(1); },
            4 => { self.ah =  (mem.read(self.v1 as usize)); },
            5 => { opfun(self); mem.write(self.ah_al() as usize, self.v1); },
            6 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_ax(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize));  self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize));  self.inc_pc();
                                self.v2 =  (((self.al as u32 + self.x as u32) >> 8) as u8);
                                self.al = self.al.wrapping_add(self.x); },
            3 => { self.v1 =  (mem.read(self.ah_al() as usize)); 
                                self.ah = self.ah.wrapping_add(self.v2); },
            4 => { opfun(self); mem.write(self.ah_al() as usize, self.v1);  },
            5 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a3_ay(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize));  self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize));  self.inc_pc();
                                self.v2 =  (((self.al as u32 + self.y as u32) >> 8) as u8);
                                self.al = self.al.wrapping_add(self.y); },
            3 => { self.v1 =  (mem.read(self.ah_al() as usize)); 
                                self.ah = self.ah.wrapping_add(self.v2); },
            4 => { opfun(self); mem.write(self.ah_al() as usize, self.v1);  },
            5 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a3_zpx(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { mem.read(self.al as usize);       self.al = self.al.wrapping_add(self.x);          },  // discard  
            3 => { opfun(self); mem.write(self.al as usize, self.v1); },
            4 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_zpy(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { mem.read(self.al as usize);       self.al = self.al.wrapping_add(self.y);          },  // discard  
            3 => { opfun(self); mem.write(self.al as usize, self.v1); },
            4 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_iy(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.v1 =  (mem.read(self.pc as usize));  self.inc_pc(); },
            2 => { self.al =  (mem.read(self.v1 as usize));  self.v1 = self.v1.wrapping_add(1); },
            3 => { self.ah =  (mem.read(self.v1 as usize));  
                                self.v2 =  (((self.al as u32 + self.y as u32) >> 8) as u8);
                                self.al = self.al.wrapping_add(self.y); },
            4 => { mem.read(self.ah_al() as usize);          self.ah = self.ah.wrapping_add(self.v2); },
            5 => { opfun(self); mem.write(self.ah_al() as usize, self.v1);  },
            6 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a4_zp(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { self.v1 =  (mem.read(self.al as usize));                },
            3 => { mem.write(self.al as usize, self.v1); },                          // wasted write
            4 => { opfun(self); mem.write(self.al as usize, self.v1); },
            5 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a4_zpx(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { mem.read(self.al as usize); self.al = self.al.wrapping_add(self.x); },                 //discard read
            3 => { self.v1 =  (mem.read(self.al as usize)); },
            4 => { mem.write(self.al as usize, self.v1); },                          // wasted write
            5 => { opfun(self); mem.write(self.al as usize, self.v1); },
            6 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a4_ax(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize));  self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize));  self.inc_pc();
                                self.v2 =  (((self.al as u32 + self.x as u32) >> 8) as u8);
                                self.al = self.al.wrapping_add(self.x); },
            3 => { mem.read(self.ah_al() as usize); self.ah = self.ah.wrapping_add(self.v2); },        // discard read
            4 => { self.v1 =  (mem.read(self.ah_al() as usize)); },
            5 => { mem.write(self.ah_al() as usize, self.v1);  },               // wasted write
            6 => { opfun(self); mem.write(self.ah_al() as usize, self.v1);  },
            7 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn a4_abs(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize)); self.inc_pc(); },
            3 => { self.v1 =  (mem.read(self.ah_al() as usize));                },
            4 => { mem.write(self.ah_al() as usize, self.v1); },                          // wasted write
            5 => { opfun(self); mem.write(self.ah_al() as usize, self.v1); },
            6 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    
    fn jsr_abs(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
 //       println!("qui!!");
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { mem.read((self.s as usize) + 0x100);  },        // discard read. s tack pointer is always 1xx
            3 => { mem.write((self.s as usize) + 0x100, (self.pc >> 8) as u8);  self.dec_sp();  },
            4 => { mem.write((self.s as usize) + 0x100, (self.pc & 0xFF) as u8); self.dec_sp(); },    // check with a real 6502
            5 => { self.ah =  (mem.read(self.pc as usize));  },
            6 => { self.pc =  (self.ah_al());      self.fetch_op(mem); },   // load PC and fetch. only one mem read       
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
            1 => { mem.read(self.pc as usize); self.inc_pc(); self.p.b = true; },                // discard read. pc has been incremented earlier
            2 => { mem.write((self.s as usize) + 0x100, (self.pc >> 8) as u8);   self.dec_sp();  },
            3 => { mem.write((self.s as usize) + 0x100, (self.pc & 0xFF) as u8); self.dec_sp();  },    
            4 => { mem.write((self.s as usize) + 0x100, (self.pack_p()) as u8);  self.dec_sp(); },
            5 => { self.pc =  (mem.read(0xFFFE) as u16);  },     // PCL
            6 => { self.pc = (self.pc & 0xFF) | ((mem.read(0xFFFF) as u16) << 8); }, // PCH
            7 => { self.fetch_op(mem); self.p.i = true; },        // remember to set up i
            _ => {},
        }
    }
    fn rti_imp(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc as usize);    self.inc_pc(); },   // discard read
            2 => { mem.read((self.s as usize) + 0x100); self.inc_sp(); },     // discard read too
            3 => {  let pedante=mem.read(self.s as usize + 0x100);
//                    println!("pload: ss {}  {:x}",self.s,pedante); 
                    let tmpb = self.p.b; 
                    self.unpack_set_p(pedante); self.inc_sp();
                    self.p.b = tmpb;       // b is unaffected by rti & plp
                        },
            4 => { self.pc =  (mem.read((self.s as usize) + 0x100) as u16 ); self.inc_sp(); }, 
            5 => { self.pc = (self.pc & 0x00ff) | ((mem.read((self.s as usize) + 0x100) as u16) << 8 );  }, 
            6 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn jmp_abs(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize)); self.inc_pc(); },
            3 => { self.pc =  (self.ah_al());      self.fetch_op(mem); },   // load PC and fetch. only one mem read       
            _ => {},
        }
    }

    fn jmp_ind(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { self.al =  (mem.read(self.pc as usize)); self.inc_pc(); },
            2 => { self.ah =  (mem.read(self.pc as usize)); self.inc_pc(); },
            3 => { self.pc =  (mem.read(self.ah_al() as usize) as u16 ); self.al= self.al.wrapping_add(1) },  // carry IS NOT propagated.. don't jump from (XXFF)!
            4 => { self.pc = (self.pc & 0xFF) |  ((mem.read(self.ah_al() as usize) as u16) << 8); },   // load PC and fetch. only one mem read       
            5 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    // FIXME check in RTS l'indirizzo di ritorno è ok
    // in RTI è l'indirizzo -1



    // FIXME DAVVERO STACK
    fn rts_imp(&mut self, mem: &mut Mem, _: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc as usize);    self.inc_pc(); },   // discard read
            2 => { mem.read((self.s as usize) + 0x100); self.inc_sp(); },     // discard read too
            3 => { self.pc =  (mem.read((self.s as usize) + 0x100) as u16 ); self.inc_sp(); }, 
            4 => { self.pc = (self.pc & 0xFF) |  ((mem.read((self.s as usize) + 0x100) as u16) << 8 );  }, 
            5 => { mem.read(self.pc as usize);    self.inc_pc(); },   // discard read, inc pc
            6 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn a5_bxx(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { self.v1 =  (mem.read(self.pc as usize)); self.inc_pc();  opfun(self);   },    // skip to 4 if branch not taken. relative jump is calculated from nextop address
            2 => { mem.read(self.pc as usize);
                        let newpc = self.pc as i16 as i32 + self.v1 as i8 as i32;  // we extend sign
//                        println!("newpc: {:x}\r", newpc);
                        self.pc =  ((self.pc & 0xFF00) | (newpc & 0xFF) as u16);   // modify pcl only
                        if (newpc & 0xFF00) as u16 == self.pc & 0xFF00 { self.ts += 1; }   // skip if not page
                        self.v2 =  (((newpc & 0xFF00) >> 8) as u8);   // save pch for later
 //                       println!("v2: {:x}", self.v2);
            },
            3 => { mem.read(self.pc as usize);  
                        self.pc =  (self.pc & 0xFF | ((self.v2 as u16) << 8));
            },                          // eventually complete carry propagation
            4 => { self.fetch_op(mem); },                                                                // finally fetch new opcode
            _ => {},
        }
    }

    fn a5_plx(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { mem.read(self.pc as usize); },                                 // discard read
            2 => { mem.read((self.s as usize) + 0x100); self.inc_sp();  },        // discard read
            3 => { self.v1 =  (mem.read((self.s as usize) +0x100)); },
            4 => { opfun(self);   self.fetch_op(mem);  },                              // place a or p in its right place
            _ => {},
        }
    }

    fn a5_phx(&mut self, mem: &mut Mem, opfun: OpcodeF) {
        match self.ts {
            1 => { mem.read(self.pc as usize); },                                 // discard read (don't incpc)
            2 => { opfun(self);  mem.write((self.s as usize) + 0x100, self.v1);     self.dec_sp(); },
            3 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn ad_unk(&mut self, _: &mut Mem, _: fn(&mut Self)) {
        panic!("Unknown OP: {:x}  PC: {:x}", self.op, self.pc);
    }

    fn decode_addr_mode(op: u8) -> AddrModeF {
        const ADDRTABLE: [AddrModeF; 256] = [
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
			 P65::a2_imm  ,P65::a2_ix ,P65::a2_imm  ,P65::ad_unk ,P65::a2_zp  ,P65::a2_zp  ,P65::a2_zp  ,P65::ad_unk ,P65::a1_imp ,P65::a2_imm ,P65::a1_imp ,P65::ad_unk ,P65::a2_abs  ,P65::a2_abs ,P65::a2_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::a2_zpx ,P65::a2_zpx ,P65::a2_zpy ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::a1_imp ,P65::ad_unk ,P65::a2_ax   ,P65::a2_ax  ,P65::a2_ay  ,P65::ad_unk,
			 P65::a2_imm  ,P65::a2_ix ,P65::ad_unk  ,P65::ad_unk ,P65::a2_zp  ,P65::a2_zp  ,P65::a4_zp  ,P65::ad_unk ,P65::a1_imp ,P65::a2_imm ,P65::a1_imp ,P65::ad_unk ,P65::a2_abs  ,P65::a2_abs ,P65::a4_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zpx ,P65::a4_zpx ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::ad_unk ,P65::ad_unk ,P65::ad_unk  ,P65::a2_ax  ,P65::a4_ax  ,P65::ad_unk,
			 P65::a2_imm  ,P65::a2_ix ,P65::ad_unk  ,P65::ad_unk ,P65::a2_zp  ,P65::a2_zp  ,P65::a4_zp  ,P65::ad_unk ,P65::a1_imp ,P65::a2_imm ,P65::a1_imp ,P65::ad_unk ,P65::a2_abs  ,P65::a2_abs ,P65::a4_abs ,P65::ad_unk,
			 P65::a5_bxx  ,P65::a2_iy ,P65::ad_unk  ,P65::ad_unk ,P65::ad_unk ,P65::a2_zpx ,P65::a4_zpx ,P65::ad_unk ,P65::a1_imp ,P65::a2_ay  ,P65::ad_unk ,P65::ad_unk ,P65::ad_unk  ,P65::a2_ax  ,P65::a4_ax  ,P65::ad_unk,
        ]   ;
        ADDRTABLE[op as usize]
    }
    
    // quick'n'dirty addressing mode disassembler, very useful to debug the simulator itself
    #[allow(dead_code)]
    fn addr_string(op: u8, v1: u16) -> String {
        match op & 0x0F {
        0x00 =>  {   
            if op & 0x10 != 0 {
                format!("${:02x}",(v1 & 0xFF) as i8) /* bxx */
            } else {
                if op == 0x00 || op == 0x40 || op == 0x60  { 
                    "".to_string() /* imp */
                } else if op == 0x20 { 
                    format!("${:04x}", v1)  /* jsr abs */ 
                } else if op == 0x80 { 
                    "NOP*".to_string()
                } else { 
                    format!("#${:2x}", (v1 & 0xFF) as u8) /* imm */ 
                }
            }},
        0x01 => {
            if op & 0x10 == 0 { /* ix */
                 format!("(${:02x},X)", (v1 & 0xFF) as u8)
            }
            else { /* iy */ 
                 format!("(${:02x},Y)", (v1 & 0xFF) as u8)
            }},
        0x02 => {
            if op == 0xa2 { /* imm */ 
                format!("#${:02x}", (v1 & 0xFF) as u8)
            } else { "UNK".to_string() }},
        0x03 => { "UNK".to_string() },
        0x04 => { 
            if op == 0x24 || op == 0x84 || op == 0xa4 || op == 0xc4 || op == 0xe4 { /* zp */
                format!("${:02x}", (v1 & 0xFF) as u8)                
            } else if op == 0x94 || op == 0xb4 { /* zpx */
                format!("${:02x},X", (v1 &0xFF) as u8)                
            } else { 
                "UNK".to_string()
            }},
        0x05 => { 
            if op & 0x10 == 0 { /* zp */
                format!("${:02x}", (v1 & 0xFF) as u8)                
            } else { /* zpx */
                format!("${:02x},X", (v1 & 0xFF) as u8)
            }},
        0x06 => { 
            if op & 0x10 == 0 {
                if op == 0x96 || op == 0xb6 { /* zpy */ 
                    format!("${:02x},Y", (v1 & 0xFF) as u8)
                } else { /* zpx */ 
                    format!("${:02x},X", (v1 & 0xFF) as u8)
                }
            } else {  /* zp */ 
                format!("${:02x}", (v1 & 0xFF) as u8)                
            }},
        0x07 => { "UNK".to_string() /* unk */ },
        0x08 => { "".to_string() /* imp */ },
        0x09 => {
            if op & 0x10 == 0 {
                if op == 0x89 { 
                    "UNK".to_string()
                } else { /* imm */ 
                    format!("#${:02x}", (v1 & 0xFF) as u8)
                }
            } else { 
                /* ay */ 
                format!("${:04x},Y", v1) 
            }},
        0x0A => { 
            if op < 0x8A {
                if op & 0x10 == 0 { 
                    "".to_string() /* acc */ 
                } else { 
                    "UNK".to_string() /* unk */ 
                }
            } else {
                if op == 0xDA || op == 0xFA { 
                    "UNK".to_string() /* unk */ 
                } else { "".to_string() /* imp */ }
            }},
        0x0B => { "UNK".to_string() /* unk */ },
        0x0C => { 
            if op & 0x10 == 0 {
                if op == 0x0C { 
                    "UNK".to_string() 
                } else if op == 0x4C { /* jmp ind */ 
                    format!("(${:04x})", v1)
                } else { /* abs */
                    format!("${:04x}", v1) 
                }
            } else if op == 0xBC { /* ax */ 
                format!("${:04x},X", v1)  
            } else { 
                "UNK".to_string() 
            }}, 
        0x0D => { 
            if op & 0x10 == 0 { /* abs */ 
                format!("${:04x}", v1) 
            } else { /* ax */
                format!("${:04x},X", v1)  
            }},
        0x0E => { 
            if op & 0x10 == 0 {                /* abs */
                format!("${:04x}", v1) 
            } else {
                if op == 0x9e { 
                    "UNK".to_string()
                } else if op == 0xbe { /* ay */
                    format!("${:04x},Y", v1)  
                } else { /* ax */
                    format!("${:04x},X", v1)  
                }
            }},
        0x0F => { "UNK".to_string() /* UNK */ },
        _ => { "".to_string() /* not that smart rust, there are at most 16 cases */ },     
        }
    }

    // we fake this.
    fn reset(&mut self, mem: &mut Mem) {
        self.s =  (0xFD);
        self.op = 0x00;
        self.al =  (mem.read(0xFFFC));
        self.ah =  (mem.read(0xFFFD));
        self.pc =  (self.ah_al());
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

struct Mem<'a>(&'a mut [u8]);

impl<'a> Mem<'a> {
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
    

    let mut mem = Mem(&mut mem_store);
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