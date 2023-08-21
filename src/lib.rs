//! Fast ring buffer intended for no_std targets.
//!
//! `fring` ("fast ring") is a fast and lightweight circular buffer,
//! designed for embedded systems and other no_std targets.  The memory
//! footprint is the buffer itself plus two `usize` indices, and that's it.
//! The buffer allows a single producer and a single consumer, which may
//! operate concurrently.  Memory safety and thread safety are enforced at
//! compile time; the buffer is lock-free at runtime.  The buffer length
//! is required to be a power of two, and the only arithmetic operations
//! used by buffer operations are addition/subtraction and bitwise and.
//!
//! Example of threaded use:
//! ```rust
//! # const N: usize = 8;
//! # fn make_data(_: fring::Producer<N>) {}
//! # fn use_data(_: fring::Consumer<N>) {}
//! fn main() {
//!     let mut buffer = fring::Buffer::<N>::new();
//!     let (producer, consumer) = buffer.split();
//!     std::thread::scope(|s| {
//!         s.spawn(|| {
//!             make_data(producer);
//!         });
//!         use_data(consumer);
//!     });
//! }
//! ```
//!
//! Example of static (no_std) use:
//! ```rust
//! # const N: usize = 8;
//! # fn write_data(_: fring::Producer<N>) {}
//! # fn use_data(_: fring::Consumer<N>) {}
//! static BUFFER: fring::Buffer<N> = fring::Buffer::new();
//!
//! fn interrupt_handler() {
//!     // UNSAFE: this is safe because this is the only place we ever
//!     // call BUFFER.producer(), and interrupt_handler() is not reentrant
//!     let producer = unsafe { BUFFER.producer() };
//!     write_data(producer);
//! }
//!
//! fn main() {
//!     // UNSAFE: this is safe because this is the only place we ever
//!     // call BUFFER.consumer(), and main() is not reentrant
//!     let consumer = unsafe { BUFFER.consumer() };
//!     use_data(consumer);
//! }
//! ```

#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

/// A `Buffer<N>` consists of a `[u8; N]` array along with two `usize`
/// indices into the array.  `N` must be a power of two.  (If you need more
/// flexibility with sizing, consider using a `bbqueue::BBBuffer` instead.)
/// A `Buffer<N>` can hold `N` bytes of data and guarantees FIFO ordering.
/// The only way to use a `Buffer` is to split it into a [`Producer`] and a
/// [`Consumer`], which may then be passed to different threads or contexts.
pub struct Buffer<const N: usize> {
    data: core::cell::UnsafeCell<[u8; N]>,
    head: AtomicUsize, // head = next index to be read
    tail: AtomicUsize, // tail = next index to be written
}
// `head` and `tail` are allowed to increment all the way to `usize::MAX`
// and wrap around.  We maintain the invariants `0 <= tail - head <= N` and
// `0 <= N + head - tail <= N` (note that these may be *wrapping* subtractions).
// Indices into `data` are given by `head % N` and `tail % N`.  Since `N`
// is a power of 2, these are equal to `head & (N - 1)` and `tail & (N - 1)`.
// When the buffer is empty, `head == tail`.  When the buffer is full,
// `head + N == tail` (and note this may be a *wrapping* addition).

/// A `Producer` is a smart pointer to a `Buffer`, which is endowed with
/// the right to add data into the buffer.  Only one `Producer` may exist
/// at one time for any given buffer.  The methods of a `Producer` are the
/// only way to insert data into a `Buffer`.
pub struct Producer<'a, const N: usize> {
    buffer: &'a Buffer<N>,
    // The Producer is allowed to increment buffer.tail (up to a maximum
    // value of buffer.head + N), but may not modify buffer.head.
}

/// A `Consumer` is a smart pointer to a `Buffer`, which is endowed with
/// the right to remove data from the buffer.  Only one `Consumer` may exist
/// at one time for any given buffer.  The methods of a `Consumer` are the
/// only way to read data out of a `Buffer`.
pub struct Consumer<'a, const N: usize> {
    buffer: &'a Buffer<N>,
    // The Consumer is allowed to increment buffer.head (up to a maximum
    // value of buffer.tail), but may not modify buffer.tail.
}

/// A `Region` is a smart pointer to a specific region of data in a [`Buffer`].
/// The `Region` derefs to `[u8]` and may generally be used in the same way as
/// a slice (e.g. `region[i]`, `region.len()`).  When a `Region` is dropped,
/// it updates the associated `Buffer` to indicate that this section of the
/// buffer is now finished being read or written.  If a `Region` is forgotten
/// instead of dropped, the buffer will not be updated and the same region will
/// be re-issued by the next read/write.
///
/// A Region holds a mutable (i.e. exclusive) reference to its owner (of type
/// `T`), which is either a `Producer` (for writing to a buffer) or a `Consumer`
/// (for reading from a buffer). This ensures that, for a given buffer, at most
/// one region for reading and one region for writing can exist at any time.
pub struct Region<'b, T> {
    region: &'b mut [u8], // points to a subslice of Buffer.data
    index_to_increment: &'b AtomicUsize,
    _owner: &'b mut T,     // points to a Producer or Consumer
}

impl<const N: usize> Buffer<N> {
    const SIZE_CHECK: () = assert!(
        (N != 0) && ((N - 1) & N == 0),
        "buffer size must be a power of 2"
    );
    /// Return a new, empty buffer. The memory backing the buffer is zero-initialized.
    pub const fn new() -> Self {
        // Force a compile-time failure if N is not a power of 2.
        let _ = Self::SIZE_CHECK;
        Buffer {
            data: core::cell::UnsafeCell::new([0; N]),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }
    /// Split the `Buffer` into a `Producer` and a `Consumer`.  This function is the
    /// only safe way to create a `Producer` or a `Consumer`.  This function requires
    /// a mutable (i.e. exclusive) reference to the buffer, and the lifetime of that
    /// reference is equal to the lifetimes of the producer and consumer which are
    /// returned.  Therefore, for a given buffer, only one producer and one consumer
    /// can exist at one time.
    pub fn split(&mut self) -> (Producer<N>, Consumer<N>) {
        (Producer { buffer: self }, Consumer { buffer: self })
    }
    /// Return a `Producer` associated with this buffer. UNSAFE: the caller must
    /// ensure that at most one `Producer` for this buffer exists at any time.
    pub unsafe fn producer(&self) -> Producer<N> {
        Producer { buffer: self }
    }
    /// Return a `Consumer` associated with this buffer. UNSAFE: the caller must
    /// ensure that at most one `Consumer` for this buffer exists at any time.
    pub unsafe fn consumer(&self) -> Consumer<N> {
        Consumer { buffer: self }
    }
}

impl<const N: usize> Buffer<N> {
    #[inline(always)]
    fn calc_pointers(&self, indices: (usize, usize), target_len: usize) -> (*mut u8, usize, usize) {
        // length calculations which are shared between `slice()` and `split_slice()`
        let (start, end) = indices;
        (
            // points to the element of Buffer.data at position `start`
            unsafe { (self.data.get() as *mut u8).add(start & (N - 1)) },
            // maximum length from `start` which doesn't wrap around
            N - (start & (N - 1)),
            // maximum length <= `target_len` which fits between `start` and `end`
            core::cmp::min(target_len, end.wrapping_sub(start)),
        )
    }
    /// Internal use only. Return a u8 slice extending from `indices.0` to `indices.1`,
    /// except that the slice shall not be longer than `target_len`, and the slice shall
    /// not wrap around the end of the buffer.  Start and end indices are wrapped to the
    /// buffer length.  UNSAFE: caller is responsible for ensuring that overlapping
    /// slices are never created, since we return a mutable (i.e. exclusive) slice.
    unsafe fn slice(&self, indices: (usize, usize), target_len: usize) -> &mut [u8] {
        let (start_ptr, wrap_len, len) = self.calc_pointers(indices, target_len);
        core::slice::from_raw_parts_mut(start_ptr, core::cmp::min(len, wrap_len))
    }
    /// Internal use only. Return a pair of u8 slices which are logically contiguous in
    /// the buffer, extending from `indices.0` to `indices.1`, except that the total
    /// length shall not exceed `target_len`. Start and end indices are wrapped to the
    /// buffer length.  UNSAFE: caller is responsible for ensuring that overlapping
    /// slices are never created, since we return mutable (i.e. exclusive) slices.
    unsafe fn split_slice(&self, indices: (usize, usize), target_len: usize) -> [&mut [u8]; 2] {
        let (start_ptr, wrap_len, len) = self.calc_pointers(indices, target_len);
        if len <= wrap_len {
            [
                core::slice::from_raw_parts_mut(start_ptr, 0),
                core::slice::from_raw_parts_mut(start_ptr, len),
            ]
        } else {
            let data_ptr = self.data.get() as *mut u8;
            [
                core::slice::from_raw_parts_mut(start_ptr, wrap_len),
                core::slice::from_raw_parts_mut(data_ptr, len - wrap_len),
            ]
        }
    }
}

impl<'a, const N: usize> Producer<'a, N> {
    fn indices(&self) -> (usize, usize) {
        (
            self.buffer.tail.load(Relaxed),
            self.buffer.head.load(Relaxed).wrapping_add(N),
        )
    }
    /// Return a `Region` for up to `target_len` bytes to be written into
    /// the buffer. The returned region may be shorter than `target_len`.
    /// The returned region has length zero if and only if the buffer is full.
    /// To write the largest possible length, set `target_len = usize::MAX`.
    pub fn write<'b>(&'b mut self, target_len: usize) -> Region<'b, Self> {
        Region {
            region: unsafe { self.buffer.slice(self.indices(), target_len) },
            index_to_increment: &self.buffer.tail,
            _owner: self,
        }
    }
    /// If the buffer has room for `data`, write it into the buffer and return `Ok`.
    /// Otherwise, return `Err`.
    pub fn write_ref<T: ?Sized>(&mut self, data: &T) -> Result<(), ()> {
        let src = unsafe {
            core::slice::from_raw_parts(data as *const _ as *const u8, core::mem::size_of_val(data))
        };
        let dst = unsafe { self.buffer.split_slice(self.indices(), src.len()) };
        if dst[0].len() + dst[1].len() == src.len() {
            let (src0, src1) = src.split_at(dst[0].len());
            dst[0].copy_from_slice(src0);
            dst[1].copy_from_slice(src1);
            self.buffer.tail.fetch_add(src.len(), Relaxed);
            Ok(())
        } else {
            Err(())
        }
    }
    /// Return the amount of empty space currently available in the buffer.
    /// If the consumer is reading concurrently with this call, then the amount
    /// of empty space may increase, but it will not decrease below the value
    /// which is returned.
    pub fn empty_size(&self) -> usize {
        let (start, end) = self.indices();
        end.wrapping_sub(start)
    }
}

impl<'a, const N: usize> Consumer<'a, N> {
    fn indices(&self) -> (usize, usize) {
        (
            self.buffer.head.load(Relaxed),
            self.buffer.tail.load(Relaxed),
        )
    }
    /// Return a `Region` for up to `target_len` bytes to be read from
    /// the buffer. The returned region may be shorter than `target_len`.
    /// The returned region has length zero if and only if the buffer is empty.
    /// To read the largest possible length, set `target_len = usize::MAX`.
    ///
    /// Even though we are reading from a buffer, the `Region` which is returned
    /// is mutable.  Its memory is available for arbitrary use by the caller
    /// for as long as the `Region` remains in scope.
    pub fn read<'b>(&'b mut self, target_len: usize) -> Region<'b, Self> {
        Region {
            region: unsafe { self.buffer.slice(self.indices(), target_len) },
            index_to_increment: &self.buffer.head,
            _owner: self,
        }
    }
    /// If the buffer contains enough bytes to make an instance of `T`, then write
    /// them into `data` and return `Ok`. Otherwise, return `Err`.  UNSAFE: caller
    /// must guarantee that the bytes contained in the buffer constitute a valid
    /// instance of `T`.  In consequence, if `T` is an integer type or an integer
    /// array or slice type, then it is safe to call this function.
    pub unsafe fn read_ref<T: Copy + ?Sized>(&mut self, data: &mut T) -> Result<(), ()> {
        let dst = unsafe {
            core::slice::from_raw_parts_mut(data as *mut _ as *mut u8, core::mem::size_of_val(data))
        };
        let src = unsafe { self.buffer.split_slice(self.indices(), dst.len()) };
        if src[0].len() + src[1].len() == dst.len() {
            let (dst0, dst1) = dst.split_at_mut(src[0].len());
            dst0.copy_from_slice(src[0]);
            dst1.copy_from_slice(src[1]);
            self.buffer.head.fetch_add(dst.len(), Relaxed);
            Ok(())
        } else {
            Err(())
        }
    }
    /// Return the amount of data currently stored in the buffer.
    /// If the producer is writing concurrently with this call,
    /// then the amount of data may increase, but it will not
    /// decrease below the value which is returned.
    pub fn data_size(&self) -> usize {
        let (start, end) = self.indices();
        end.wrapping_sub(start)
    }
    /// Discard all data which is currently stored in the buffer.
    /// If the producer is writing concurrently with this call,
    /// then the producer's newest data may not be discarded.
    pub fn flush(&mut self) {
        self.buffer
            .head
            .store(self.buffer.tail.load(Relaxed), Relaxed);
    }
}

impl<'b, T> Region<'b, T> {
    /// Update the buffer to indicate that the first `num` bytes of this region are
    /// finished being read or written.  The start and length of this region will be
    /// updated such that the remaining `region.len() - num` bytes remain in this
    /// region for future reading or writing.
    pub fn consume(&mut self, num: usize) {
        assert!(num <= self.region.len());
        self.index_to_increment.fetch_add(num, Relaxed);
        // UNSAFE: this is safe because we are replacing self.region with a subslice
        // of self.region, and it is constrained to keep the same lifetime.
        self.region = unsafe {
            core::slice::from_raw_parts_mut(
                self.region.as_mut_ptr().add(num),
                self.region.len() - num,
            )
        }
    }
    /// Update the buffer to indicate that the first `num` bytes of this region are
    /// finished being read or written, and the remaining `region.len() - num` bytes
    /// will not be used.  `region.partial_drop(0)` is equivalent to
    /// `core::mem::forget(region)`.
    pub fn partial_drop(self, num: usize) {
        assert!(num <= self.region.len());
        self.index_to_increment.fetch_add(num, Relaxed);
        core::mem::forget(self); // don't run drop() now!
    }
}

impl<'b, T> Drop for Region<'b, T> {
    /// Update the buffer to indicate that the memory being read or written is now
    /// ready for use. Dropping a `Region` requires a single addition operation to
    /// one field of the `Buffer`.
    fn drop(&mut self) {
        self.index_to_increment.fetch_add(self.region.len(), Relaxed);
    }
}

impl<'b, T> core::ops::Deref for Region<'b, T> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.region
    }
}

impl<'b, T> core::ops::DerefMut for Region<'b, T> {
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
    for _ in 0..4 {
        assert!(p.empty_size() == 64);
        assert!(p.write(32).len() == 32);
        assert!(p.empty_size() == 32);
        assert!(p.write(usize::MAX).len() == 32);
        assert!(p.empty_size() == 0);
        assert!(p.write(usize::MAX).len() == 0);
        assert!(c.data_size() == 64);
        assert!(c.read(32).len() == 32);
        assert!(c.data_size() == 32);
        assert!(c.read(usize::MAX).len() == 32);
        assert!(c.data_size() == 0);
        assert!(c.read(usize::MAX).len() == 0);
    }
    assert!(b.head.load(Relaxed) == 128);
    assert!(b.tail.load(Relaxed) == 128);
}
