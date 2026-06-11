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
use core::ptr;

/// A bump allocator.
///
/// A pointer always goes upwards and never returns `ptr::null()`
///
/// ### Safety
/// - The caller is expected to have only one thread or hold the lock
/// - The caller is responsible for not hitting reserved memory by setting safe memory range
pub struct BumpAllocator {
    ptr: Cell<Option<usize>>,
    size: Cell<usize>,
}

unsafe impl Sync for BumpAllocator {}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();
        let available_size = self.size.get();

        if let Some(ptr) = self.ptr.get() {
            let aligned_ptr = align_up(ptr, align);

            let padding = aligned_ptr - ptr;

            if let Some(total) = padding.checked_add(size) {
                if available_size >= total {
                    self.ptr.set(Some(aligned_ptr + size));
                    self.size.set(available_size - total);

                    return aligned_ptr as *mut u8;
                }
            }
        }

        ptr::null_mut()
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
    pub const fn new(ptr: usize, size: usize) -> Self {
        Self {
            ptr: Cell::new(Some(ptr)),
            size: Cell::new(size),
        }
    }

    /// Create a new bump allocator with empty data.
    ///
    /// Useful for runtime memory address setting
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            ptr: Cell::new(None),
            size: Cell::new(0),
        }
    }

    /// Initialize allocator with given address and `size`
    pub const fn init(&mut self, ptr: usize, size: usize) {
        self.ptr.replace(Some(ptr));
        self.size.replace(size);
    }
}

/// Align `x` up to the next multiple of `align`. `align` must be a power of two.
const fn align_up(x: usize, align: usize) -> usize {
    (x + align - 1) & !(align - 1)
}
