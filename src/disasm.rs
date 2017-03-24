/*
 * A small disassembler, very useful to debug cpu module 
 *
 */


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

