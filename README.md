`fring` ("fast ring") is a fast, lightweight circular buffer, designed
for embedded systems and other no_std targets.  The memory footprint
is the buffer itself plus two `usize` indices, and that's it.  The
buffer allows a single producer and a single consumer, which may
operate concurrently.  Memory safety and thread safety are enforced at
compile time; the buffer is lock-free at runtime.  The buffer length
is required to be a power of two, and the only arithmetic operations
used by buffer operations are addition/subtraction and bitwise-and.
Compared to other Rust ring buffers (such as
[bbqueue](https://docs.rs/bbqueue/latest/bbqueue/)), `fring` is
less flexible, but offers reduced storage and computational overhead.
