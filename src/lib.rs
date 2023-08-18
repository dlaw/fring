//! Fast ring buffer intended for no_std targets.
//!
//! `fring` ("fast ring") is a fast, lightweight circular buffer, designed
//! for embedded systems and other no_std targets.  The memory footprint
//! is the buffer itself plus two `usize` indices, and that's it.  The
//! buffer allows a single producer and a single consumer, which may
//! operate concurrently.  Memory safety and thread safety are enforced at
//! compile time; the buffer is lock-free at runtime.  The buffer length
//! is required to be a power of two, and the only arithmetic operations
//! used by buffer operations are addition/subtraction and bitwise-and.

#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

/// A `Buffer<N>` consists of a `[u8; N]` array along with two `usize`
/// indices into the array.  `N` must be a power of two.  If you need
/// more flexibility with sizing, consider using a `bbqueue` instead.
pub struct Buffer<const N: usize> {
    data: core::cell::UnsafeCell<[u8; N]>,
    head: AtomicUsize,  // head = next index to be read
    tail: AtomicUsize,  // tail = next index to be written
    // `head` and `tail` are allowed to increment all the way to `usize::MAX`
    // and wrap around.  We maintain the invariants `tail - head >= 0` and
    // `head + N - tail >= 0` (note that these may be *wrapping* subtractions).
    // Indices into `data` are given by `head % N` and `tail % N`.  Since `N`
    // is a power of 2, these are equal to `head & (N - 1)` and `tail & (N - 1)`.
    // When the buffer is empty, `head == tail`.  When the buffer is full,
    // `head + N == tail` (and note this may be a *wrapping* addition).
}

/// A `Producer` is a smart pointer to a `Buffer`, which is endowed with
/// the right to add data into the buffer.  Only one `Producer` may exist
/// at one time for any given buffer.  Requesting a `WriteRegion` from a
/// `Producer` is the only way to insert data into a `Buffer`.
pub struct Producer<'a, const N: usize> {
    buffer: &'a Buffer<N>,
    // The Producer is allowed to increment buffer.tail, but may not
    // modify buffer.head.
}

/// A `Consumer` is a smart pointer to a `Buffer`, which is endowed with
/// the right to remove data from the buffer.  Only one `Consumer` may exist
/// at one time for any given buffer.  Requesting a `ReadRegion` from a
/// `Consumer` is the only way to read data out of a `Buffer`.
pub struct Consumer<'a, const N: usize> {
    buffer: &'a Buffer<N>,
    // The Consumer is allowed to increment buffer.head, but may not
    // modify buffer.tail.
}

/// A `WriteRegion` is a smart pointer to a specific region of data in a
/// `Buffer`.  The `WriteRegion` derefs to `[u8]` and may generally be used
/// in the same way as a slice (e.g. `w[i]`, `w.len()`).  When a `WriteRegion`
/// is dropped, it updates the associated `Buffer` to indicate that its memory
/// region now contains data which is ready to be read.  If a `WriteRegion` is
/// forgotten instead of dropped, the buffer will not be updated and its memory
/// will be overwritten by the next write to the buffer.
pub struct WriteRegion<'a, 'b, const N: usize> {
    // Lifetime 'a is the lifetime of the associated Producer.
    // Lifetime 'b is the lifetime of this WriteRegion.
    // These lifetimes enforce the constraint that each Producer can only have
    // a single WriteRegion existing at one time, but the Producer outlives the
    // WriteRegion and can produce another WriteRegion once this one is dropped.
    producer: &'b mut Producer<'a, N>,
    region: &'b mut [u8],
}

/// A `ReadRegion` is a smart pointer to a specific region of data in a
/// `Buffer`.  The `ReadRegion` derefs to `[u8]` and may generally be used
/// in the same way as a slice (e.g. `r[i]`, `r.len()`).  When a `ReadRegion`
/// is dropped, it updates the associated `Buffer` to indicate that the its
/// memory region may now be overwritten.  If a `ReadRegion` is forgotten
/// instead of dropped, the buffer will not be updated and its memory will
/// be read again by the next read from the buffer.
pub struct ReadRegion<'a, 'b, const N: usize> {
    // Lifetime 'a is the lifetime of the associated Consumer.
    // Lifetime 'b is the lifetime of this ReadRegion.
    // These lifetimes enforce the constraint that each Consumer can only have
    // a single ReadRegion existing at one time, but the Consumer outlives the
    // ReadRegion and can produce another ReadRegion once this one is dropped.
    consumer: &'b mut Consumer<'a, N>,
    region: &'b mut [u8],
}

impl<const N: usize> Buffer<N> {
    /// Return a new, empty buffer.  The memory backing the buffer is zero-initialized.
    pub const fn new() -> Self {
        assert!(N != 0 && N - 1 & N == 0); // N must be a power of 2
        Buffer {
            data: core::cell::UnsafeCell::new([0; N]),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }
    /// Split the `Buffer` into a `Producer` and a `Consumer`.  This function is the
    /// only way to use a `Buffer`, and it's also the only way to create a `Producer`
    /// or a `Consumer`.  This function requires a mutable (i.e. exclusive) reference
    /// to the buffer, and the lifetime of that reference is equal to the lifetimes of
    /// the producer and consumer which are returned.  Therefore, for a given buffer,
    /// only one producer and one consumer can exist at one time.
    pub fn split<'a>(&'a mut self) -> (Producer<'a, N>, Consumer<'a, N>) {
        (
            Producer::<N> { buffer: self },
            Consumer::<N> { buffer: self },
        )
    }
    /// Internal use only. Return a u8 slice starting from `start` and ending at `end`,
    /// except that the slice shall not be longer than `target_len`, and the slice shall
    /// not wrap around the end of the buffer.  `start` and `end` are wrapped to the
    /// buffer length.  UNSAFE: caller is responsible for ensuring that overlapping
    /// slices are never created, since we return a mutable (i.e. exclusive) slice.
    unsafe fn slice<'a>(&self, start: usize, end: usize, target_len: usize) -> &'a mut [u8] {
        let start_ptr = (self.data.get() as *mut u8).add(start & (N - 1));
        let wrap_len = N - (start & (N - 1));
        let max_len = end.wrapping_sub(start);
        let len = core::cmp::min(target_len, core::cmp::min(max_len, wrap_len));
        core::slice::from_raw_parts_mut(start_ptr, len)
    }
}

impl<'a, const N: usize> Producer<'a, N> {
    /// Return a `WriteRegion` for up to `target_len` bytes to be written into
    /// the buffer. The returned region may be shorter than `target_len`.
    /// If the returned region has length zero, then the buffer is full.
    /// To write the largest possible contiguous length, provide
    /// `target_len = usize::MAX`.
    pub fn write<'b>(&'b mut self, target_len: usize) -> WriteRegion<'a, 'b, N> {
        let start = self.buffer.tail.load(Relaxed);
        let end = self.buffer.head.load(Relaxed).wrapping_add(N);
        let region = unsafe { self.buffer.slice(start, end, target_len) };
        WriteRegion {
            producer: self,
            region,
        }
    }
    /// Return the amount of empty space currently available in the buffer.
    /// If the consumer is reading concurrently with this call, then the amount
    /// of empty space may increase, but it will not decrease below the value
    /// which is returned.
    pub fn empty_size(&self) -> usize {
        let start = self.buffer.tail.load(Relaxed);
        let end = self.buffer.head.load(Relaxed).wrapping_add(N);
        end.wrapping_sub(start)
    }
}

impl<'a, const N: usize> Consumer<'a, N> {
    /// Return a `ReadRegion` for up to `target_len` bytes to be read from
    /// the buffer. The returned region may be shorter than `target_len`.
    /// If the returned region has length zero, then the buffer is empty.
    /// To read the largest possible contiguous length, provide
    /// `target_len = usize::MAX`.
    pub fn read<'b>(&'b mut self, target_len: usize) -> ReadRegion<'a, 'b, N> {
        let start = self.buffer.head.load(Relaxed);
        let end = self.buffer.tail.load(Relaxed);
        let region = unsafe { self.buffer.slice(start, end, target_len) };
        ReadRegion {
            consumer: self,
            region,
        }
    }
    /// Return the amount of data currently stored in the buffer.
    /// If the producer is writing concurrently with this call, then the amount
    /// of data may increase, but it will not decrease below the value which is
    /// returned.
    pub fn data_size(&self) -> usize {
        let start = self.buffer.head.load(Relaxed);
        let end = self.buffer.tail.load(Relaxed);
        end.wrapping_sub(start)
    }
    /// Discard all data which is currently stored in the buffer.
    /// If the producer is writing concurrently with this call, then the producer's
    /// newest data may not be discarded.
    pub fn flush(&mut self) {
        self.buffer
            .head
            .store(self.buffer.tail.load(Relaxed), Relaxed);
    }
}

/// On drop, a `WriteRegion` updates its associated buffer to
/// indicate that the memory being written is ready for reading.
impl<'a, 'b, const N: usize> Drop for WriteRegion<'a, 'b, N> {
    /// Dropping a `WriteRegion` requires a single addition operation.
    fn drop(&mut self) {
        self.producer
            .buffer
            .tail
            .fetch_add(self.region.len(), Relaxed);
    }
}

/// On drop, a `ReadRegion` updates its associated buffer to
/// indicate that the memory being read is no longer in use.
impl<'a, 'b, const N: usize> Drop for ReadRegion<'a, 'b, N> {
    /// Dropping a `ReadRegion` requires a single addition operation.
    fn drop(&mut self) {
        self.consumer
            .buffer
            .head
            .fetch_add(self.region.len(), Relaxed);
    }
}

impl<'a, 'b, const N: usize> core::ops::Deref for WriteRegion<'a, 'b, N> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.region
    }
}

impl<'a, 'b, const N: usize> core::ops::DerefMut for WriteRegion<'a, 'b, N> {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.region
    }
}

impl<'a, 'b, const N: usize> core::ops::Deref for ReadRegion<'a, 'b, N> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.region
    }
}

impl<'a, 'b, const N: usize> core::ops::DerefMut for ReadRegion<'a, 'b, N> {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.region
    }
}

unsafe impl<const N: usize> Send for Buffer<N> {}
/// `Buffer<N>` is `Send` and `Sync` because accesses to its internal data are
/// only possible via a single `Producer` and a single `Consumer` at any time.
unsafe impl<const N: usize> Sync for Buffer<N> {}

#[test]
fn index_wraparound() {
    // This can't be tested using the public interface because it would
    // take too long to get `head` and `tail` incremented to usize::MAX.
    let mut b = Buffer::<64>::new();
    b.head.fetch_sub(128, Relaxed);
    b.tail.fetch_sub(128, Relaxed);
    // Now b.head == b.tail == usize::MAX - 127
    let (mut p, mut c) = b.split();
    for _ in 0..8 {
        assert!(p.write(32).len() == 32);
        assert!(c.read(usize::MAX).len() == 32);
        assert!(c.read(usize::MAX).len() == 0);
    }
}
