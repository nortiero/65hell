#![allow(dead_code)]
use std::fmt;

// simple trait for memory operations
pub trait Memory {
    fn read(&mut self, a: usize) -> u8;
    fn write(&mut self, a: usize, v: u8); 
}

// 6502 flags
pub struct P65Flags {
            n: bool, 
            v: bool, 
            bit5: bool, 
            b: bool, 
            d: bool, 
            i: bool, 
            z: bool, 
            c: bool,
}       

impl P65Flags {
    fn pack(&self) -> u8 {
        (if self.n { 0x80 } else { 0x00 })  |
         if self.v { 0x40 } else { 0x00 }   |
         0x20                               |
         if self.b { 0x10 } else { 0x00 }   |
         if self.d { 0x08 } else { 0x00 }   |
         if self.i { 0x04 } else { 0x00 }   |
         if self.z { 0x02 } else { 0x00 }   |
         if self.c { 0x01 } else { 0x00 } 
    }

    fn unpack(&mut self, flags: u8) {
        self.n = flags & 0x80 != 0; 
        self.v = flags & 0x40 != 0; 
        self.bit5 = true; 
        self.b = flags & 0x10 != 0;    // FIXME. B is full of sad
        self.d = flags & 0x08 != 0; 
        self.i = flags & 0x04 != 0; 
        self.z = flags & 0x02 != 0; 
        self.c = flags & 0x01 != 0; 
    }
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


// processor state machine container
pub struct P65 {
    a: u8,
    x: u8,
    y: u8,
    p: P65Flags, 
    s: u8,
    pub pc: u16,        // fixme privatize

// emulator state
    pub cycle: u64,
    pub ts: u8,
    pub op: u8,
    pub v1: u8,
    v2: u8,
    ah: u8,
    al: u8,
    nmi: bool, nmi_cycle: u64, nmi_triggered: bool,
    irq: bool, irq_cycle: u64, irq_triggered: bool,
    reset_triggered: bool,
}

type AddrModeF<M: Memory> = fn(&mut P65, &mut M,  fn(&mut P65));
type OpcodeF = fn(&mut P65);

impl fmt::Debug for P65 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "T{:01x} pc:{:04x} a:{:02x} x:{:02x} y:{:02x} p:{:02x} s:{:02x} op:{:02x} v1:{:02x} v2:{:02x} ah/al: {:04x} i:{:1} Cy:{:06}",
             self.ts, self.pc, self.a,self.x,self.y,self.p.pack(), self.s, self.op, self.v1, self.v2, self.ah_al(), if self.irq { 1 } else { 0 }, self.cycle % 1000000 )
    }
}

impl P65 {
    pub fn new() -> P65 {
        P65 { 
            a:  0xaa, 
            x:  0, 
            y:  0, 
            p:  P65Flags {n: false, v: false, bit5: true, b: true, d: false, i: true, z: true, c: false,},
            s:  0xfd, 
            pc: 0, 
            cycle: 0, 
            ts: 0, 
            op: 0, 
            v1: 0, 
            v2: 0, 
            ah: 0, 
            al: 0,
            nmi_cycle: 0, nmi: false, nmi_triggered: false,
            irq_cycle: 0, irq: false, irq_triggered: false,
            reset_triggered: false,
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

    fn fetch_op<M: Memory>(&mut self, mem: &mut M) -> u8 {
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
    fn set_pc (&mut self, pch: u8, pcl: u8) { self.pc = ((pch as u16) << 8)| (pcl as u16); }
    #[allow(dead_code)]
    fn set_pcl(&mut self, pcl: u8)          { self.pc = (self.pc & 0xFF00) | (pcl as u16); }
    #[allow(dead_code)]
    fn set_pch(&mut self, pch: u8)          { self.pc = ((pch as u16) << 8)| (self.pc & 0x00FF); }
    fn inc_sp(&mut self) { self.s = self.s.wrapping_add(1); }
    fn dec_sp(&mut self) { self.s = self.s.wrapping_sub(1); }


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
        self.v1 = (self.v1 << 1) | (if self.p.c { 1 } else { 0 })  ;
        self.p.c = tmp & 0x80 != 0;
        let tmp = self.v1; self.fix_nz(tmp);
    }
    fn op_ror(&mut self) {
        let tmp = self.v1;
        self.v1 = (self.v1 >> 1) | (if self.p.c { 0x80 } else { 0 })  ;
        self.p.c = tmp & 0x1 != 0;
        let tmp = self.v1; self.fix_nz(tmp);
    }
    fn op_unk(&mut self) {
        assert!(1==0, "Unknown Opcode!");
    }
    fn op_nil(&mut self) { }     // nil means the opcode is managed elsewhere
    fn op_nop(&mut self) { }
    fn op_adc(&mut self) {
        if self.p.d { 
            self.op_adc_dec(); 
        } else { 
            self.op_adc_bin(); 
        }
    }
    fn op_adc_bin(&mut self) {
            let tsum = (self.a as u16 + self.v1 as u16 + if self.p.c { 0x1 } else { 0x0 }) as u16;
            self.p.c = tsum >= 0x100;
            self.p.v = !((self.a as u8 & 0x80) ^ (self.v1 & 0x80)) & ((self.a as u8 & 0x80) ^ ((tsum & 0x80) as u8)) != 0;
            self.v1 =  (tsum & 0xff) as u8;
            let tmp = self.v1; self.fix_nz(tmp);
            self.a = self.v1;
    }
    fn op_adc_dec(&mut self) {
            let mut c1: u8 = (self.a & 0x0F) + (self.v1 & 0x0F) + if self.p.c { 1 } else { 0 };
            let mut c2: u8 = self.a.wrapping_shr(4) + self.v1.wrapping_shr(4);
            if c1 >= 0xA { c1 -= 0xA; c2 += 1; }
            if c2 >= 0xA { c2 -= 0xA; self.p.c = true; } else { self.p.c = false; }
            self.a = c2.wrapping_shl(4) | c1;
            self.p.z = self.a == 0;
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
    fn op_php(&mut self) { self.v1 = self.p.pack(); }
    fn op_sec(&mut self) { self.p.c = true;  }     
    fn op_clc(&mut self) { self.p.c = false; }     
    fn op_sei(&mut self) { self.p.i = true;  }     
    fn op_cli(&mut self) { self.p.i = false; }     
    fn op_sed(&mut self) { self.p.d = true;  }     
    fn op_cld(&mut self) { self.p.d = false; }     
    fn op_clv(&mut self) { self.p.v = false; }     
    fn op_inx(&mut self) { self.x = self.x.wrapping_add(1); let tmp = self.x; self.fix_nz(tmp); }  //  borrow check is retard
    fn op_dex(&mut self) { self.x = self.x.wrapping_sub(1); let tmp = self.x; self.fix_nz(tmp); }
    fn op_iny(&mut self) { self.y = self.y.wrapping_add(1); let tmp = self.y; self.fix_nz(tmp); }
    fn op_dey(&mut self) { self.y = self.y.wrapping_sub(1); let tmp = self.y; self.fix_nz(tmp); }
    fn op_tax(&mut self) { self.x = self.a;  let tmp = self.x; self.fix_nz(tmp); }
    fn op_tay(&mut self) { self.y = self.a;  let tmp = self.y; self.fix_nz(tmp); }
    fn op_tsx(&mut self) { self.x = self.s;  let tmp = self.x; self.fix_nz(tmp); }
    fn op_txa(&mut self) { self.a = self.x;  let tmp = self.a; self.fix_nz(tmp); }
    fn op_txs(&mut self) { self.s = self.x;  }  // TXS doesn't touch flags
    fn op_tya(&mut self) { self.a = self.y;  let tmp = self.a; self.fix_nz(tmp); }
    fn op_pla(&mut self) { self.a = self.v1; let tmp = self.a; self.fix_nz(tmp); }
    fn op_plp(&mut self) { 
        let stupid_borrow = self.v1; 
        let tmpb = self.p.b;    // B is unaffected by plp
        self.p.unpack(stupid_borrow); 
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
        match op {
             0x00 => P65::op_nil, 0x01 => P65::op_ora, 0x02 => P65::op_unk, 0x03 => P65::op_unk, 0x04 => P65::op_unk, 0x05 => P65::op_ora, 0x06 => P65::op_asl, 0x07 => P65::op_unk, 0x08 => P65::op_php, 0x09 => P65::op_ora, 0x0a => P65::op_asl, 0x0b => P65::op_unk, 0x0c => P65::op_unk, 0x0d => P65::op_ora, 0x0e => P65::op_asl, 0x0f => P65::op_unk,
 		     0x10 => P65::op_bpl, 0x11 => P65::op_ora, 0x12 => P65::op_unk, 0x13 => P65::op_unk, 0x14 => P65::op_unk, 0x15 => P65::op_ora, 0x16 => P65::op_asl, 0x17 => P65::op_unk, 0x18 => P65::op_clc, 0x19 => P65::op_ora, 0x1a => P65::op_unk, 0x1b => P65::op_unk, 0x1c => P65::op_unk, 0x1d => P65::op_ora, 0x1e => P65::op_asl, 0x1f => P65::op_unk, 
		     0x20 => P65::op_nil, 0x21 => P65::op_and, 0x22 => P65::op_unk, 0x23 => P65::op_unk, 0x24 => P65::op_bit, 0x25 => P65::op_and, 0x26 => P65::op_rol, 0x27 => P65::op_unk, 0x28 => P65::op_plp, 0x29 => P65::op_and, 0x2a => P65::op_rol, 0x2b => P65::op_unk, 0x2c => P65::op_bit, 0x2d => P65::op_and, 0x2e => P65::op_rol, 0x2f => P65::op_unk, 
		     0x30 => P65::op_bmi, 0x31 => P65::op_and, 0x32 => P65::op_unk, 0x33 => P65::op_unk, 0x34 => P65::op_unk, 0x35 => P65::op_and, 0x36 => P65::op_rol, 0x37 => P65::op_unk, 0x38 => P65::op_sec, 0x39 => P65::op_and, 0x3a => P65::op_unk, 0x3b => P65::op_unk, 0x3c => P65::op_unk, 0x3d => P65::op_and, 0x3e => P65::op_rol, 0x3f => P65::op_unk, 
		     0x40 => P65::op_nil, 0x41 => P65::op_eor, 0x42 => P65::op_unk, 0x43 => P65::op_unk, 0x44 => P65::op_unk, 0x45 => P65::op_eor, 0x46 => P65::op_lsr, 0x47 => P65::op_unk, 0x48 => P65::op_pha, 0x49 => P65::op_eor, 0x4a => P65::op_lsr, 0x4b => P65::op_unk, 0x4c => P65::op_nil, 0x4d => P65::op_eor, 0x4e => P65::op_lsr, 0x4f => P65::op_unk, 
		     0x50 => P65::op_bvc, 0x51 => P65::op_eor, 0x52 => P65::op_unk, 0x53 => P65::op_unk, 0x54 => P65::op_unk, 0x55 => P65::op_eor, 0x56 => P65::op_lsr, 0x57 => P65::op_unk, 0x58 => P65::op_cli, 0x59 => P65::op_eor, 0x5a => P65::op_unk, 0x5b => P65::op_unk, 0x5c => P65::op_unk, 0x5d => P65::op_eor, 0x5e => P65::op_lsr, 0x5f => P65::op_unk, 
		     0x60 => P65::op_nil, 0x61 => P65::op_adc, 0x62 => P65::op_unk, 0x63 => P65::op_unk, 0x64 => P65::op_unk, 0x65 => P65::op_adc, 0x66 => P65::op_ror, 0x67 => P65::op_unk, 0x68 => P65::op_pla, 0x69 => P65::op_adc, 0x6a => P65::op_ror, 0x6b => P65::op_unk, 0x6c => P65::op_nil, 0x6d => P65::op_adc, 0x6e => P65::op_ror, 0x6f => P65::op_unk, 
		     0x70 => P65::op_bvs, 0x71 => P65::op_adc, 0x72 => P65::op_unk, 0x73 => P65::op_unk, 0x74 => P65::op_unk, 0x75 => P65::op_adc, 0x76 => P65::op_ror, 0x77 => P65::op_unk, 0x78 => P65::op_sei, 0x79 => P65::op_adc, 0x7a => P65::op_unk, 0x7b => P65::op_unk, 0x7c => P65::op_unk, 0x7d => P65::op_adc, 0x7e => P65::op_ror, 0x7f => P65::op_unk, 
		     0x80 => P65::op_unk, 0x81 => P65::op_sta, 0x82 => P65::op_unk, 0x83 => P65::op_unk, 0x84 => P65::op_sty, 0x85 => P65::op_sta, 0x86 => P65::op_stx, 0x87 => P65::op_unk, 0x88 => P65::op_dey, 0x89 => P65::op_unk, 0x8a => P65::op_txa, 0x8b => P65::op_unk, 0x8c => P65::op_sty, 0x8d => P65::op_sta, 0x8e => P65::op_stx, 0x8f => P65::op_unk, 
		     0x90 => P65::op_bcc, 0x91 => P65::op_sta, 0x92 => P65::op_unk, 0x93 => P65::op_unk, 0x94 => P65::op_sty, 0x95 => P65::op_sta, 0x96 => P65::op_stx, 0x97 => P65::op_unk, 0x98 => P65::op_tya, 0x99 => P65::op_sta, 0x9a => P65::op_txs, 0x9b => P65::op_unk, 0x9c => P65::op_unk, 0x9d => P65::op_sta, 0x9e => P65::op_unk, 0x9f => P65::op_unk, 
		     0xa0 => P65::op_ldy, 0xa1 => P65::op_lda, 0xa2 => P65::op_ldx, 0xa3 => P65::op_unk, 0xa4 => P65::op_ldy, 0xa5 => P65::op_lda, 0xa6 => P65::op_ldx, 0xa7 => P65::op_unk, 0xa8 => P65::op_tay, 0xa9 => P65::op_lda, 0xaa => P65::op_tax, 0xab => P65::op_unk, 0xac => P65::op_ldy, 0xad => P65::op_lda, 0xae => P65::op_ldx, 0xaf => P65::op_unk, 
		     0xb0 => P65::op_bcs, 0xb1 => P65::op_lda, 0xb2 => P65::op_unk, 0xb3 => P65::op_unk, 0xb4 => P65::op_ldy, 0xb5 => P65::op_lda, 0xb6 => P65::op_ldx, 0xb7 => P65::op_unk, 0xb8 => P65::op_clv, 0xb9 => P65::op_lda, 0xba => P65::op_tsx, 0xbb => P65::op_unk, 0xbc => P65::op_ldy, 0xbd => P65::op_lda, 0xbe => P65::op_ldx, 0xbf => P65::op_unk, 
		     0xc0 => P65::op_cpy, 0xc1 => P65::op_cmp, 0xc2 => P65::op_unk, 0xc3 => P65::op_unk, 0xc4 => P65::op_cpy, 0xc5 => P65::op_cmp, 0xc6 => P65::op_dec, 0xc7 => P65::op_unk, 0xc8 => P65::op_iny, 0xc9 => P65::op_cmp, 0xca => P65::op_dex, 0xcb => P65::op_unk, 0xcc => P65::op_cpy, 0xcd => P65::op_cmp, 0xce => P65::op_dec, 0xcf => P65::op_unk, 
		     0xd0 => P65::op_bne, 0xd1 => P65::op_cmp, 0xd2 => P65::op_unk, 0xd3 => P65::op_unk, 0xd4 => P65::op_unk, 0xd5 => P65::op_cmp, 0xd6 => P65::op_dec, 0xd7 => P65::op_unk, 0xd8 => P65::op_cld, 0xd9 => P65::op_cmp, 0xda => P65::op_unk, 0xdb => P65::op_unk, 0xdc => P65::op_unk, 0xdd => P65::op_cmp, 0xde => P65::op_dec, 0xdf => P65::op_unk, 
		     0xe0 => P65::op_cpx, 0xe1 => P65::op_sbc, 0xe2 => P65::op_unk, 0xe3 => P65::op_unk, 0xe4 => P65::op_cpx, 0xe5 => P65::op_sbc, 0xe6 => P65::op_inc, 0xe7 => P65::op_unk, 0xe8 => P65::op_inx, 0xe9 => P65::op_sbc, 0xea => P65::op_nop, 0xeb => P65::op_unk, 0xec => P65::op_cpx, 0xed => P65::op_sbc, 0xee => P65::op_inc, 0xef => P65::op_unk, 
		     0xf0 => P65::op_beq, 0xf1 => P65::op_sbc, 0xf2 => P65::op_unk, 0xf3 => P65::op_unk, 0xf4 => P65::op_unk, 0xf5 => P65::op_sbc, 0xf6 => P65::op_inc, 0xf7 => P65::op_unk, 0xf8 => P65::op_sed, 0xf9 => P65::op_sbc, 0xfa => P65::op_unk, 0xfb => P65::op_unk, 0xfc => P65::op_unk, 0xfd => P65::op_sbc, 0xfe => P65::op_inc, 0xff => P65::op_unk,
             _ => P65::op_unk,  /* silly silly, op is a u8 */
        }
    }       

    // now the addressing modes, divided by group of opcodes

    fn a1_ac<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { mem.read(self.pc as usize); },      // discard read
            2 => { self.v1 = self.a; opfun(self); self.a = self.v1; self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a1_imp<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { mem.read(self.pc as usize); },      // discard read
            2 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_ix<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al = mem.read(self.pc as usize); self.inc_pc(); },
            2 => { mem.read(self.al as usize);  self.v1 = self.al.wrapping_add(self.x); },    // discd read
            3 => { self.al = mem.read(self.v1 as usize);  self.v1 = self.v1.wrapping_add(1); },
            4 => { self.ah = mem.read(self.v1 as usize); },
            5 => { self.v1 = mem.read(self.ah_al() as usize); },
            6 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a2_imm<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.v1 =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_zp<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { self.v1 =  mem.read(self.al as usize); },
            3 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_abs<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { self.ah =  mem.read(self.pc as usize); self.inc_pc(); },
            3 => { self.v1 =  mem.read(self.ah_al() as usize); },
            4 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_iy<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.v1 =  mem.read(self.pc as usize);  self.inc_pc(); },
            2 => { self.al =  mem.read(self.v1 as usize);  self.v1 = self.v1.wrapping_add(1); },
            3 => { self.ah =  mem.read(self.v1 as usize);  
                                self.v2 =  ((self.al as u32 + self.y as u32) >> 8) as u8;
                                self.al = self.al.wrapping_add(self.y); },
            4 => { self.v1 =  mem.read(self.ah_al() as usize); 
                                self.ah = self.ah.wrapping_add(self.v2); 
                                if self.v2 ==  (0) { self.ts_inc(); }; },
            5 => { self.v1 =  mem.read(self.ah_al() as usize);  },
            6 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a2_zpx<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { mem.read(self.al as usize); self.al = self.al.wrapping_add(self.x); },  //discard read
            3 => { self.v1 =  mem.read(self.al as usize); },
            4 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_zpy<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { mem.read(self.al as usize); self.al = self.al.wrapping_add(self.y); },  //discard read
            3 => { self.v1 =  mem.read(self.al as usize); },
            4 => { opfun(self); self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a2_ay<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize);  self.inc_pc(); },
            2 => { self.ah =  mem.read(self.pc as usize);  self.inc_pc(); 
                                self.v2 =  ((self.al as u16 + self.y as u16) >> 8) as u8;  // here v2 is max 1
                                self.al = self.al.wrapping_add(self.y); },
            3 => { self.v1 =  mem.read(self.ah_al() as usize); 
                                self.ah = self.ah.wrapping_add(self.v2); 
                                if self.v2 ==  (0) { self.ts_inc(); }; },
            4 => { self.v1 =  mem.read(self.ah_al() as usize);  },
            5 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a2_ax<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize);  self.inc_pc(); },
            2 => { self.ah =  mem.read(self.pc as usize);  self.inc_pc();
                                self.v2 =  ((self.al as u32 + self.x as u32) >> 8) as u8;
                                self.al = self.al.wrapping_add(self.x); },
            3 => { self.v1 =  mem.read(self.ah_al() as usize); 
                                self.ah = self.ah.wrapping_add(self.v2); 
                                if self.v2 == 0 { self.ts_inc(); }; },
            4 => { self.v1 =  mem.read(self.ah_al() as usize);  },
            5 => { opfun(self); self.fetch_op(mem); },
            _ => {},
        }
    }
    
    fn a3_zp<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { opfun(self); mem.write(self.al as usize, self.v1); },
            3 => { self.fetch_op(mem); }
            _ => {},
        }
    }

    fn a3_abs<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { self.ah =  mem.read(self.pc as usize); self.inc_pc(); },
            3 => { opfun(self); mem.write(self.ah_al() as usize, self.v1); },
            4 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_ix<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize);  self.inc_pc(); },
            2 => { mem.read(self.al as usize);             self.v1 = self.al.wrapping_add(self.x); },     // discard read
            3 => { self.al =  mem.read(self.v1 as usize);  self.v1 = self.v1.wrapping_add(1); },
            4 => { self.ah =  mem.read(self.v1 as usize); },
            5 => { opfun(self); mem.write(self.ah_al() as usize, self.v1); },
            6 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_ax<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize);  self.inc_pc(); },
            2 => { self.ah =  mem.read(self.pc as usize);  self.inc_pc();
                                self.v2 =  ((self.al as u32 + self.x as u32) >> 8) as u8;
                                self.al = self.al.wrapping_add(self.x); },
            3 => { self.v1 =  mem.read(self.ah_al() as usize); 
                                self.ah = self.ah.wrapping_add(self.v2); },
            4 => { opfun(self); mem.write(self.ah_al() as usize, self.v1);  },
            5 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a3_ay<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize);  self.inc_pc(); },
            2 => { self.ah =  mem.read(self.pc as usize);  self.inc_pc();
                                self.v2 =  ((self.al as u32 + self.y as u32) >> 8) as u8;
                                self.al = self.al.wrapping_add(self.y); },
            3 => { self.v1 =  mem.read(self.ah_al() as usize); 
                                self.ah = self.ah.wrapping_add(self.v2); },
            4 => { opfun(self); mem.write(self.ah_al() as usize, self.v1);  },
            5 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a3_zpx<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { mem.read(self.al as usize);       self.al = self.al.wrapping_add(self.x);          },  // discard  
            3 => { opfun(self); mem.write(self.al as usize, self.v1); },
            4 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_zpy<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { mem.read(self.al as usize);       self.al = self.al.wrapping_add(self.y);          },  // discard  
            3 => { opfun(self); mem.write(self.al as usize, self.v1); },
            4 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a3_iy<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.v1 =  mem.read(self.pc as usize);  self.inc_pc(); },
            2 => { self.al =  mem.read(self.v1 as usize);  self.v1 = self.v1.wrapping_add(1); },
            3 => { self.ah =  mem.read(self.v1 as usize);  
                                self.v2 =  ((self.al as u32 + self.y as u32) >> 8) as u8;
                                self.al = self.al.wrapping_add(self.y); },
            4 => { mem.read(self.ah_al() as usize);          self.ah = self.ah.wrapping_add(self.v2); },
            5 => { opfun(self); mem.write(self.ah_al() as usize, self.v1);  },
            6 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a4_zp<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { self.v1 =  mem.read(self.al as usize);                },
            3 => { mem.write(self.al as usize, self.v1); },                          // wasted write
            4 => { opfun(self); mem.write(self.al as usize, self.v1); },
            5 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a4_zpx<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { mem.read(self.al as usize); self.al = self.al.wrapping_add(self.x); },                 //discard read
            3 => { self.v1 =  mem.read(self.al as usize); },
            4 => { mem.write(self.al as usize, self.v1); },                          // wasted write
            5 => { opfun(self); mem.write(self.al as usize, self.v1); },
            6 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    fn a4_ax<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize);  self.inc_pc(); },
            2 => { self.ah =  mem.read(self.pc as usize);  self.inc_pc();
                                self.v2 =  ((self.al as u32 + self.x as u32) >> 8) as u8;
                                self.al = self.al.wrapping_add(self.x); },
            3 => { mem.read(self.ah_al() as usize); self.ah = self.ah.wrapping_add(self.v2); },        // discard read
            4 => { self.v1 = mem.read(self.ah_al() as usize); },
            5 => { mem.write(self.ah_al() as usize, self.v1);  },               // wasted write
            6 => { opfun(self); mem.write(self.ah_al() as usize, self.v1);  },
            7 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn a4_abs<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc();   },
            2 => { self.ah =  mem.read(self.pc as usize); self.inc_pc();   },
            3 => { self.v1 =  mem.read(self.ah_al() as usize);             },
            4 => { mem.write(self.ah_al() as usize, self.v1);              },    // wasted write
            5 => { opfun(self); mem.write(self.ah_al() as usize, self.v1); },
            6 => { self.fetch_op(mem); }
            _ => {},
        }
    }
    
    fn jsr_abs<M: Memory>(&mut self, mem: &mut M, _: fn(&mut Self)) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { mem.read((self.s as usize) + 0x100);  },        // discard read. s tack pointer is always 1xx
            3 => { mem.write((self.s as usize) + 0x100, (self.pc >> 8) as u8);  self.dec_sp();  },
            4 => { mem.write((self.s as usize) + 0x100, (self.pc & 0xFF) as u8); self.dec_sp(); },    // check with a real 6502
            5 => { self.ah =  mem.read(self.pc as usize);  },
            6 => { self.pc =  self.ah_al();      self.fetch_op(mem); },   // load PC and fetch. only one mem read       
            _ => {},
        }
    }

    // check: http://visual6502.org/wiki/index.php?title=6502_BRK_and_B_bit
    // please note that BRK is 2 byte long, its opcode 0x00, followed by an unused byte
    // the reason is to fix old PROMs, where programmed bits were zero. The second byte would
    // be used by the hot fix to discover where the BRK came from
    // about B:
    // - is not a real flag
    // - real nmos 6502 set it always in T5, even for IRQs
    // - IRQs and NMIs enter at T2, so they skip pc increment and b = 1. Nice! 
    // - if a NMI kicks in during BRK between T0 and T3 it will store B as 1 on the stack.. that's original cpu behavior (BUG). I am unsure of the extent of the bug. Where is B set? CHECK with virtual6502
    // - if you CLI during a NMI, you can serve other NMIs before RTI (and other IRQs too!)
    // CHECK/FIXME . we drop the triggers at T6. A fast bouncing NMI or IRQ could be retriggered early.. What a real CPU would do?
    fn brk_imp<M: Memory>(&mut self, mem: &mut M, _: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc as usize); self.inc_pc(); self.p.b = true; },  // discard read. note that ONLY the real BRK will be in T1, IRQ/NMI START FROM T2
            2 => {       // ENTRY POINT for IRQs & NMIs. b MUST be false now , except for note bug
                
                if self.reset_triggered {   // this hack is from the cpu
                    mem.read((self.s as usize) + 0x100); self.dec_sp();
                } else { 
                    mem.write((self.s as usize) + 0x100, (self.pc >> 8) as u8);   self.dec_sp();
                }},
            3 => { 
                if self.reset_triggered {
                    mem.read((self.s as usize) + 0x100); self.dec_sp();
                } else {
                    mem.write((self.s as usize) + 0x100, (self.pc & 0xFF) as u8); self.dec_sp();    
                }},
            4 => {
                if self.reset_triggered {
                    mem.read((self.s as usize) + 0x100); self.dec_sp();
                } else {
                    mem.write((self.s as usize) + 0x100, (self.p.pack()) as u8);  self.dec_sp();  
                }},
            5 => { 
                // the actual vector is chosen late in the process, e.g. http://forum.6502.org/viewtopic.php?t=1797
                // this will also "steal" the BRK, if we happen to be executing it during irq/nmi triggering
                // we use a little hack to ensure that a late NMI won't switch vectors at T6. the processor
                // uses logic to ensure the same behavior
                self.p.i = true;                           // now I must be set, to avoid retriggering. cpu does the same
                self.p.b = true;
                if self.reset_triggered {
                    self.set_pcl(mem.read(0xFFFC));
                    self.ah = 0xFF; self.al = 0xFD;
                    self.reset_triggered = false;          // drop trigger
                } else if self.nmi_triggered {
                    self.set_pcl(mem.read(0xFFFA));
                    self.ah = 0xFF; self.al = 0xFB;
                    self.nmi_triggered = false;            
                } else {
                    self.set_pcl(mem.read(0xFFFE));
                    self.ah = 0xFF; self.al = 0xFF;
                    if self.irq_triggered { self.irq_triggered = false; };
                }
            },
            6 => { let tmp = self.ah_al() as usize; self.set_pch(mem.read(tmp)); },
            7 => { self.fetch_op(mem); },        // remember to set I. Is too late here? -> Yes 
            _ => {},
        }
    }
    // An interrupt is more or less like a BRK.
    // Do not clear the interrupt (CLI) before the issuing peripheral interrupt flag is cleared, or will fire twice! 
    pub fn irq_set(&mut self) {
        if !self.irq && self.cycle - self.irq_cycle >= 2 {
            self.irq_cycle = self.cycle;
            self.irq = true;
        }
    }
    pub fn irq_clear(&mut self) {
        if  self.irq && self.cycle - self.irq_cycle >= 2 {
            self.irq_cycle = self.cycle;
            self.irq = false;
        }
    }
    // firing an NMI will get it serviced, then ignored until took down and then up (logically).
    // this is called being "edge sensitive". Must stay up / low for at least two cycles to be sensed.
    pub fn nmi_set(&mut self) {
        if !self.nmi && self.cycle - self.nmi_cycle >= 2 {
            self.nmi_cycle = self.cycle;
            self.nmi = true;
        }
    }
    // please note that once triggered, within 2 cycles, NMI will happen anyway
    pub fn nmi_clear(&mut self) {
        if  self.nmi && self.cycle - self.nmi_cycle >= 2 {
            self.nmi_cycle = self.cycle;
            self.nmi = false;
        }
    }

    fn rti_imp<M: Memory>(&mut self, mem: &mut M, _: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc as usize);    self.inc_pc(); },   // discard read
            2 => { mem.read((self.s as usize) + 0x100); self.inc_sp(); },     // discard read too
            3 => {  let pedante=mem.read(self.s as usize + 0x100);
                    let tmpb = self.p.b; 
                    self.p.unpack(pedante); self.inc_sp();
                    self.p.b = tmpb;       // b is unaffected by rti & plp
                        },
            4 => { self.pc = mem.read((self.s as usize) + 0x100) as u16; self.inc_sp(); }, 
            5 => { self.pc = (self.pc & 0x00ff) | ((mem.read((self.s as usize) + 0x100) as u16) << 8 );  }, 
            6 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn jmp_abs<M: Memory>(&mut self, mem: &mut M, _: fn(&mut Self)) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { self.ah =  mem.read(self.pc as usize); self.inc_pc(); },
            3 => { self.pc =  self.ah_al();      self.fetch_op(mem); },   // load PC and fetch. only one mem read       
            _ => {},
        }
    }

    fn jmp_ind<M: Memory>(&mut self, mem: &mut M, _: fn(&mut Self)) {
        match self.ts {
            1 => { self.al =  mem.read(self.pc as usize); self.inc_pc(); },
            2 => { self.ah =  mem.read(self.pc as usize); self.inc_pc(); },
            3 => { self.pc =  mem.read(self.ah_al() as usize) as u16; self.al= self.al.wrapping_add(1) },  // carry IS NOT propagated.. don't jump from (XXFF)!
            4 => { self.pc = (self.pc & 0xFF) |  ((mem.read(self.ah_al() as usize) as u16) << 8); },   // load PC and fetch. only one mem read       
            5 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn rts_imp<M: Memory>(&mut self, mem: &mut M, _: fn(&mut Self)) {
        match self.ts {
            1 => { mem.read(self.pc as usize);                            self.inc_pc(); },   // discard read
            2 => { mem.read((self.s as usize) + 0x100);                   self.inc_sp(); },     // discard read too
            3 => { self.pc =  mem.read((self.s as usize) + 0x100) as u16; self.inc_sp(); }, 
            4 => { self.pc = (self.pc & 0xFF) |  ((mem.read((self.s as usize) + 0x100) as u16) << 8 );  }, 
            5 => { mem.read(self.pc as usize);    self.inc_pc(); },   // discard read, inc pc
            6 => { self.fetch_op(mem); },
            _ => {},
        }
    }
    fn a5_bxx<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { self.v1 =  mem.read(self.pc as usize); self.inc_pc();  opfun(self);   },    // skip to 4 if branch not taken. relative jump is calculated from nextop address
            2 => { mem.read(self.pc as usize);
                        let newpc = self.pc as i16 as i32 + self.v1 as i8 as i32;  // we extend sign
                        self.pc =  (self.pc & 0xFF00) | (newpc & 0xFF) as u16;   // modify pcl only
                        if (newpc & 0xFF00) as u16 == self.pc & 0xFF00 { self.ts += 1; }   // skip if not page
                        self.v2 =  ((newpc & 0xFF00) >> 8) as u8;   // save pch for later
            },
            3 => { mem.read(self.pc as usize);  
                        self.pc =  self.pc & 0xFF | ((self.v2 as u16) << 8);
            },                          // eventually complete carry propagation
            4 => { self.fetch_op(mem); },                                                                // finally fetch new opcode
            _ => {},
        }
    }

    fn a5_plx<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { mem.read(self.pc as usize); },                                 // discard read
            2 => { mem.read((self.s as usize) + 0x100); self.inc_sp();  },        // discard read
            3 => { self.v1 =  mem.read((self.s as usize) +0x100); },
            4 => { opfun(self);   self.fetch_op(mem);  },                              // place a or p in its right place
            _ => {},
        }
    }

    fn a5_phx<M: Memory>(&mut self, mem: &mut M, opfun: OpcodeF) {
        match self.ts {
            1 => { mem.read(self.pc as usize); },                                 // discard read (don't incpc)
            2 => { opfun(self);  mem.write((self.s as usize) + 0x100, self.v1);     self.dec_sp(); },
            3 => { self.fetch_op(mem); },
            _ => {},
        }
    }

    fn ad_unk<M: Memory>(&mut self, _: &mut M, _: fn(&mut Self)) {
        panic!("Unknown OP: {:x}  PC: {:x}", self.op, self.pc);
    }

    // luckily it is optimized as a jump table by the compiler, because it's impossible in rust to make a const array of generic function pointers
    fn decode_addr_mode<M: Memory>(op: u8) -> AddrModeF<M> {
        match op {
            0x00 => { P65::brk_imp },0x01 => { P65::a2_ix} ,0x02 => { P65::ad_unk } ,0x03 => { P65::ad_unk },0x04 => { P65::ad_unk },0x05 => { P65::a2_zp  },0x06 => { P65::a4_zp  },0x07 => { P65::ad_unk },0x08 => { P65::a5_phx },0x09 => { P65::a2_imm },0x0a => { P65::a1_ac  },0x0b => { P65::ad_unk },0x0c => { P65::ad_unk } ,0x0d => { P65::a2_abs} ,0x0e => { P65::a4_abs }, 0x0f => { P65::ad_unk },
            0x10 => { P65::a5_bxx  },0x11 => { P65::a2_iy} ,0x12 => { P65::ad_unk } ,0x13 => { P65::ad_unk },0x14 => { P65::ad_unk },0x15 => { P65::a2_zpx },0x16 => { P65::a4_zpx },0x17 => { P65::ad_unk },0x18 => { P65::a1_imp },0x19 => { P65::a2_ay  },0x1a => { P65::ad_unk },0x1b => { P65::ad_unk },0x1c => { P65::ad_unk } ,0x1d => { P65::a2_ax } ,0x1e => { P65::a4_ax  }, 0x1f => { P65::ad_unk },
            0x20 => { P65::jsr_abs },0x21 => { P65::a2_ix} ,0x22 => { P65::ad_unk } ,0x23 => { P65::ad_unk },0x24 => { P65::a2_zp  },0x25 => { P65::a2_zp  },0x26 => { P65::a4_zp  },0x27 => { P65::ad_unk },0x28 => { P65::a5_plx },0x29 => { P65::a2_imm },0x2a => { P65::a1_ac  },0x2b => { P65::ad_unk },0x2c => { P65::a2_abs } ,0x2d => { P65::a2_abs} ,0x2e => { P65::a4_abs }, 0x2f => { P65::ad_unk },
            0x30 => { P65::a5_bxx  },0x31 => { P65::a2_iy} ,0x32 => { P65::ad_unk } ,0x33 => { P65::ad_unk },0x34 => { P65::ad_unk },0x35 => { P65::a2_zpx },0x36 => { P65::a4_zpx },0x37 => { P65::ad_unk },0x38 => { P65::a1_imp },0x39 => { P65::a2_ay  },0x3a => { P65::ad_unk },0x3b => { P65::ad_unk },0x3c => { P65::ad_unk } ,0x3d => { P65::a2_ax } ,0x3e => { P65::a4_ax  }, 0x3f => { P65::ad_unk },
            0x40 => { P65::rti_imp },0x41 => { P65::a2_ix} ,0x42 => { P65::ad_unk } ,0x43 => { P65::ad_unk },0x44 => { P65::ad_unk },0x45 => { P65::a2_zp  },0x46 => { P65::a4_zp  },0x47 => { P65::ad_unk },0x48 => { P65::a5_phx },0x49 => { P65::a2_imm },0x4a => { P65::a1_ac  },0x4b => { P65::ad_unk },0x4c => { P65::jmp_abs} ,0x4d => { P65::a2_abs} ,0x4e => { P65::a4_abs }, 0x4f => { P65::ad_unk },
            0x50 => { P65::a5_bxx  },0x51 => { P65::a2_iy} ,0x52 => { P65::ad_unk } ,0x53 => { P65::ad_unk },0x54 => { P65::ad_unk },0x55 => { P65::a2_zpx },0x56 => { P65::a4_zpx },0x57 => { P65::ad_unk },0x58 => { P65::a1_imp },0x59 => { P65::a2_ay  },0x5a => { P65::ad_unk },0x5b => { P65::ad_unk },0x5c => { P65::ad_unk } ,0x5d => { P65::a2_ax } ,0x5e => { P65::a4_ax  }, 0x5f => { P65::ad_unk },
            0x60 => { P65::rts_imp },0x61 => { P65::a2_ix} ,0x62 => { P65::ad_unk } ,0x63 => { P65::ad_unk },0x64 => { P65::ad_unk },0x65 => { P65::a2_zp  },0x66 => { P65::a4_zp  },0x67 => { P65::ad_unk },0x68 => { P65::a5_plx },0x69 => { P65::a2_imm },0x6a => { P65::a1_ac  },0x6b => { P65::ad_unk },0x6c => { P65::jmp_ind} ,0x6d => { P65::a2_abs} ,0x6e => { P65::a4_abs }, 0x6f => { P65::ad_unk },
            0x70 => { P65::a5_bxx  },0x71 => { P65::a2_iy} ,0x72 => { P65::ad_unk } ,0x73 => { P65::ad_unk },0x74 => { P65::ad_unk },0x75 => { P65::a2_zpx },0x76 => { P65::a4_zpx },0x77 => { P65::ad_unk },0x78 => { P65::a1_imp },0x79 => { P65::a2_ay  },0x7a => { P65::ad_unk },0x7b => { P65::ad_unk },0x7c => { P65::ad_unk } ,0x7d => { P65::a2_ax } ,0x7e => { P65::a4_ax  }, 0x7f => { P65::ad_unk },
            0x80 => { P65::ad_unk  },0x81 => { P65::a3_ix} ,0x82 => { P65::ad_unk } ,0x83 => { P65::ad_unk },0x84 => { P65::a3_zp  },0x85 => { P65::a3_zp  },0x86 => { P65::a3_zp  },0x87 => { P65::ad_unk },0x88 => { P65::a1_imp },0x89 => { P65::ad_unk },0x8a => { P65::a1_imp },0x8b => { P65::ad_unk },0x8c => { P65::a3_abs } ,0x8d => { P65::a3_abs} ,0x8e => { P65::a3_abs }, 0x8f => { P65::ad_unk },
            0x90 => { P65::a5_bxx  },0x91 => { P65::a3_iy} ,0x92 => { P65::ad_unk } ,0x93 => { P65::ad_unk },0x94 => { P65::a3_zpx },0x95 => { P65::a3_zpx },0x96 => { P65::a3_zpy },0x97 => { P65::ad_unk },0x98 => { P65::a1_imp },0x99 => { P65::a3_ay  },0x9a => { P65::a1_imp },0x9b => { P65::ad_unk },0x9c => { P65::ad_unk } ,0x9d => { P65::a3_ax } ,0x9e => { P65::ad_unk }, 0x9f => { P65::ad_unk },
            0xa0 => { P65::a2_imm  },0xa1 => { P65::a2_ix} ,0xa2 => { P65::a2_imm } ,0xa3 => { P65::ad_unk },0xa4 => { P65::a2_zp  },0xa5 => { P65::a2_zp  },0xa6 => { P65::a2_zp  },0xa7 => { P65::ad_unk },0xa8 => { P65::a1_imp },0xa9 => { P65::a2_imm },0xaa => { P65::a1_imp },0xab => { P65::ad_unk },0xac => { P65::a2_abs } ,0xad => { P65::a2_abs} ,0xae => { P65::a2_abs }, 0xaf => { P65::ad_unk },
            0xb0 => { P65::a5_bxx  },0xb1 => { P65::a2_iy} ,0xb2 => { P65::ad_unk } ,0xb3 => { P65::ad_unk },0xb4 => { P65::a2_zpx },0xb5 => { P65::a2_zpx },0xb6 => { P65::a2_zpy },0xb7 => { P65::ad_unk },0xb8 => { P65::a1_imp },0xb9 => { P65::a2_ay  },0xba => { P65::a1_imp },0xbb => { P65::ad_unk },0xbc => { P65::a2_ax  } ,0xbd => { P65::a2_ax } ,0xbe => { P65::a2_ay  }, 0xbf => { P65::ad_unk },
            0xc0 => { P65::a2_imm  },0xc1 => { P65::a2_ix} ,0xc2 => { P65::ad_unk } ,0xc3 => { P65::ad_unk },0xc4 => { P65::a2_zp  },0xc5 => { P65::a2_zp  },0xc6 => { P65::a4_zp  },0xc7 => { P65::ad_unk },0xc8 => { P65::a1_imp },0xc9 => { P65::a2_imm },0xca => { P65::a1_imp },0xcb => { P65::ad_unk },0xcc => { P65::a2_abs } ,0xcd => { P65::a2_abs} ,0xce => { P65::a4_abs }, 0xcf => { P65::ad_unk },
            0xd0 => { P65::a5_bxx  },0xd1 => { P65::a2_iy} ,0xd2 => { P65::ad_unk } ,0xd3 => { P65::ad_unk },0xd4 => { P65::ad_unk },0xd5 => { P65::a2_zpx },0xd6 => { P65::a4_zpx },0xd7 => { P65::ad_unk },0xd8 => { P65::a1_imp },0xd9 => { P65::a2_ay  },0xda => { P65::ad_unk },0xdb => { P65::ad_unk },0xdc => { P65::ad_unk } ,0xdd => { P65::a2_ax } ,0xde => { P65::a4_ax  }, 0xdf => { P65::ad_unk },
            0xe0 => { P65::a2_imm  },0xe1 => { P65::a2_ix} ,0xe2 => { P65::ad_unk } ,0xe3 => { P65::ad_unk },0xe4 => { P65::a2_zp  },0xe5 => { P65::a2_zp  },0xe6 => { P65::a4_zp  },0xe7 => { P65::ad_unk },0xe8 => { P65::a1_imp },0xe9 => { P65::a2_imm },0xea => { P65::a1_imp },0xeb => { P65::ad_unk },0xec => { P65::a2_abs } ,0xed => { P65::a2_abs} ,0xee => { P65::a4_abs }, 0xef => { P65::ad_unk },
            0xf0 => { P65::a5_bxx  },0xf1 => { P65::a2_iy} ,0xf2 => { P65::ad_unk } ,0xf3 => { P65::ad_unk },0xf4 => { P65::ad_unk },0xf5 => { P65::a2_zpx },0xf6 => { P65::a4_zpx },0xf7 => { P65::ad_unk },0xf8 => { P65::a1_imp },0xf9 => { P65::a2_ay  },0xfa => { P65::ad_unk },0xfb => { P65::ad_unk },0xfc => { P65::ad_unk } ,0xfd => { P65::a2_ax } ,0xfe => { P65::a4_ax  }, 0xff => { P65::ad_unk },
            _ => { P65::ad_unk }
        }
    }    

    pub fn reset<M: Memory>(&mut self, mem: &mut M) {
        self.s =   0xFD;
        self.op =  0x00;
        self.al =  mem.read(0xFFFC);
        self.ah =  mem.read(0xFFFD);
        self.pc =  self.ah_al();
        self.fetch_op(mem);
        self.tick();
        self.cycle = 8;
    }

    // 


    // QUESTION: CAN A NMI interrupt another NMI ?
    fn check_interrupts(&mut self) {
        if self.nmi && self.cycle - self.nmi_cycle == 2 {  // == 2:  nmi will be triggered only once, then needs to be reset
            self.nmi_triggered = true;  // FIXME. checking after 2 cycles only is a bit of a hack to avoid bouncing. we can do better
        }
        if !self.p.i && self.irq && self.cycle - self.irq_cycle >= 2 {   // irqs are always retriggered if not blocked by SEI
            println!("IRQ Triggered!");
            self.irq_triggered = true;
        }
    }

    /* run will run count cycles, eventually stopping in the midst of an instruction */
    pub fn run<M: Memory>(&mut self, mem: &mut M, count: u64) -> u64 {
        self.check_interrupts(); // FIXME: interrupts should be polled at the end of T1 or early T2. see: https://wiki.nesdev.com/w/index.php/CPU_interrupts
        for _ in 0 .. count {
            if (self.ts == 1) 
            {
                use disasm;
                let op = self.op;
                let param = (mem.read(self.pc as usize) as u16) | ((mem.read(self.pc.wrapping_add(1) as usize) as u16) << 8);
                println!("{} {}\r", disasm::op_name(op).to_uppercase(), disasm::addr_name(op, param).to_uppercase());
                println!("0x200: {}, {:?}\r", mem.read(0x200), self);
            }

            let opaddr: AddrModeF<M> = P65::decode_addr_mode::<M>(self.op);
            let opfun: OpcodeF = P65::decode_op(self.op);
            opaddr(self, mem, opfun);
            if (self.nmi_triggered || self.irq_triggered) && self.ts == 0 {  // current instruction has been depleted. we can service irq/nmi
                self.op = 0x00;       // brk_imp, see implementation
                self.ts = 1;          // we skip reading brk operand
                self.pc = self.pc.wrapping_sub(1);
                self.p.b = false;     // we clear B here, because of entering BRK at T2 (and to simulate BRK/IRQ & IRQ/NMI B shadowing)
            }

            self.tick();
        }
        self.cycle
    }


    /*
     * step will execute exactly count instructions, including the current one, if not yet terminated.
     * Due to the way 6502 overlaps execute of the previous instruction (except for memory operations) with
     * fetch of the next one, next op may have been already loaded by the time a step terminates.
     * You will never see the processor in state T0, only in T1, after fetching opcode and before addressing and execute.
     * If the controlling program has to start executing at an arbitrary location it should
     * a) set the PC and call fetch_op, or
     * b) call jump
     * c) set 0xFFFE/0xFFFF and reset
     *
     * Note that by repetitive calling to run, step may be substantially slower
     */
    pub fn step<M: Memory>(&mut self, mem: &mut M, count: u64) {
        let mut count = count;
        while count > 0 {
            self.run(mem,1);
            if self.ts == 1 { count -= 1; }
        }
    }
}



