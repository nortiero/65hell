65hell is a cycle-exact, memory access exact simulator of a NMOS MOS 6502 CPU, written in the fancy Rust language.
It is somewhat brisky given those vexing requirements, simulating a future past 75Mhz 8-bit 6502 on my 1.6 GHz 
Macbook Air (sandy bridge i5-M).

I've written this simulator as a method to compare two bold and diverse languages: Scheme and Rust, and to learn
something about them. It is in no way to be taken as an example of "idiomatic" code, whatever it means.
The original Scheme implementation has been left behind a bit, in need of a few fixes: I will post it later.
Rust, as unpleasant as it can be, is by far the fastest language, overpowering the best AOT scheme implementations
-- Gambit and Chez - by a factor of 15 and 8, respectively. These values may change, for better or worse, as I
understand more about the languages and their unholy optimizations.

65hell is currently usable, as a crate ** or as source. Memory is modelled outside of the CPU, so 
you can implement your bus discipline and run peripherals alongside the processor, for example to emulate a 
time split bus architecture, like our beloved 80's micros, and memory interferences from other DMA devices.

Included is an executable sample program which loads 6502 functional tests, or EHBASIC by ****.

Instructions:
` cargo run --release `

As you probably know debug mode is a painful ~15 times slower vs. release, a common Rust weakness. 
"Govern yourself accordingly" -- and have fun!




Todo:

- test RESET
- sample timed execution in main
- check that irqs etc match exactly this: https://wiki.nesdev.com/w/index.php/CPU_interrupts
- unimplemented opcodes, and their side effects;
- NMOS 6502 quirky flags in decimal mode  (documented flags are OK);
- support for 6502 variants: 6510, at a minimum;
- maybe some speedup, but it's no slouch even now;
- implement a new memory subsystem, to support pages and layered mapping, also r/o;
- ** crateize (or cratify?) it (for Rust);
- faster sub-exact mode: correct cycle count but simplified memory accesses;
- stun mode for 6510 etc, where the CPU skips the current cycle
- SYNC/RDY for the 6502. Sync is an OUTPUT signal, high at T0. RDY is an INPUT signal that halts the processor

