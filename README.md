65hell is a cycle-exact, memory access exact simulator of a NMOS MOS 6502 CPU, written in the fancy Rust language.
It is somewhat brisky, simulating a future past 75Mhz 8-bit 6502 on my 1.7 GHz Macbook Air (sandy bridge i5m).

I've written this simulator as a method to compare two bold and diverse languages: Scheme and Rust, to learn
something about them. It is in no way to be taken as an example of "idiomatic" code, whatever it means.

The original Scheme implementation has been left behind a bit, so it is not included here.

Rust, as unpleasant as it can be, is by far the fastest language, overpowering the best AOT scheme implementations
-- Gambit and Chez - by a factor of 15 and 8, respectively. These values may change, for better or worse, as I
understand more about the languages and their unholy optimizations.

65hell is currently usable, as a rust module. Memory is modelled outside of the CPU, so 
you can implement your bus discipline and run peripherals alongside the processor, for example to emulate a 
time split bus architecture, like our beloved 80's micros, and memory interferences from other DMA devices.

Included is an executable sample program which loads 6502 a few programs and simulate a very barebone architecture.

Instructions:
` cargo run --release `

As you probably know debug mode is a painful ~15 times slower vs. release, a common Rust weakness. 

Things to do:

`cargo run --release --  -k f004 -t f001 C000:tests/ehbasic.bin`

`cargo run --release -- 000A:tests/fxa.bin -j 0400`

Setup minimal i/o and launch enhanced basic. 
Answer "C" to Warm/Cold question, or the
simulator will panic, because basic will try to execute things into uninitialized memory.
There is no "delete" and "backspace" does what says, but does not clear text under cursor.
Play with it!
EhBASIC by Lee Davidson: https://github.com/jefftranter/6502/tree/master/asm/ehbasic

6502 functional tests are here: https://github.com/Klaus2m5/6502_65C02_functional_tests.
Check the source: usually memory at 0x200 will have a peculiar value upon successful termination
of the various tests.



TODO Shortlist:

- sample timed execution in main.rs -- LIMIT PERFORMANCE
- crateize (or cratify?) cpu.rs
- full out of band monitor

Todo:

- RESET still to be tested
- check that irqs etc match exactly this: https://wiki.nesdev.com/w/index.php/CPU_interrupts
- interrupts are sampled a bit too much and a bit too early, can be more precise
- unimplemented opcodes, and their side effects;
- NMOS 6502 quirky flags in decimal mode (documented flags are fine);
- support for 6502 variants: 6510, at a minimum;
- implement a new memory subsystem, to support pages and layered mapping, also r/o;
- faster sub-exact mode: correct cycle count but simplified memory accesses;
- stun mode, where CPU skips the current cycle
- SYNC/RDY for the 6502. Sync is an OUTPUT signal, high at T0. RDY is an INPUT signal that halts the processor

Evaluating performance:

One free run of 6502_functional_test runs in 1.46s on my machine (i5m 1.7 GHz, MB Air 2011), around ~75MHz 6502.
Please run in release mode, debug mode is slow.