#![no_std]

//! A minimal bump allocator.
//!
//! Example usage:
//! ```
//! #[global_allocator]
//! static ALLOCATOR: BumpAllocator = BumpAllocator::new(0xa0000000);
//! ```

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;

/// A bump allocator.
///
/// A pointer always goes upwards and never returns `ptr::null()`
///
/// ### Safety
/// - The caller is expected to have only one thread or hold the lock
/// - The caller is responsible for not hitting reserved memory by setting safe memory range
pub struct BumpAllocator {
    ptr: Cell<usize>,
}

unsafe impl Sync for BumpAllocator {}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();

        let ptr = self.ptr.get();
        let aligned = align_up(ptr, align);

        self.ptr.set(aligned + size);

        aligned as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

impl BumpAllocator {
    /// Create a new bump allocator starting at the given raw memory address.
    ///
    /// ### Safety
    /// The caller must guarantee that the provided `ptr` is the start of a
    /// valid and sufficiently large memory region that will not be used for
    /// any other purpose.
    #[must_use]
    pub const fn new(ptr: usize) -> Self {
        Self {
            ptr: Cell::new(ptr),
        }
    }
}

/// Align `x` up to the next multiple of `align`. `align` must be a power of two.
const fn align_up(x: usize, align: usize) -> usize {
    (x + align - 1) & !(align - 1)
}
