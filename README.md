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

Things to do:

`cargo run --  -k f004 -t f001 C000:tests/ehbasic.bin -j c000`

Setup minimal i/o and launch enhanced basic. 
Answer "C" to Warm/Cold question, or the
simulator will panic, because basic will try to execute things into uninitialized memory.
There is no "delete" and "backspace" does what says, but does not clear text under cursor.
Play with it!



Todo:

- RESET signal must be tested
- sample timed execution in main -- LIMIT PERFORMANCE
- check that irqs etc match exactly this: https://wiki.nesdev.com/w/index.php/CPU_interrupts
- interrupts are sampled a bit too much and a bit too early, can be more precise
- unimplemented opcodes, and their side effects;
- NMOS 6502 quirky flags in decimal mode  (documented flags are OK);
- support for 6502 variants: 6510, at a minimum;
- implement a new memory subsystem, to support pages and layered mapping, also r/o;
- ** crateize (or cratify?) it (for Rust);
- faster sub-exact mode: correct cycle count but simplified memory accesses;
- stun mode for 6510 etc, where the CPU skips the current cycle
- SYNC/RDY for the 6502. Sync is an OUTPUT signal, high at T0. RDY is an INPUT signal that halts the processor
- a full program monitor outside memory. That should not be hard.

Evaluating performance:
a free run of 6502_functional_test runs in 1.46s on my machine. Please use cargo build --release !