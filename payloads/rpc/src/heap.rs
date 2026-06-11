use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, NonNull};

use acid_alloc::{Raw, Slab};

pub struct LockedSlab {
    inner: Option<Slab<Raw>>,
}

impl LockedSlab {
    pub const fn empty() -> Self {
        Self { inner: None }
    }

    // A runtime method to inject your dynamic memory range
    pub unsafe fn init(&mut self, start: *mut u8, block_size: usize, num_blocks: usize) {
        if self.inner.is_none() {
            let alloc = unsafe { Slab::new_raw(NonNull::new_unchecked(start), block_size, num_blocks).expect("Failed to initialize underlying slab allocator") };
            self.inner = Some(alloc);
        }
    }
}

// 2. Implement GlobalAlloc for your wrapper type
unsafe impl GlobalAlloc for LockedSlab {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if let Some(ref mut slab) = self.inner {
            slab.allocate(layout).map(|p| p.as_mut_ptr()).unwrap_or(ptr::null_mut())
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(ref mut slab) = *self.inner.lock() {
            slab.dealloc(ptr, layout);
        }
    }
}

// 3. Register your static allocator in its uninitialized state
#[global_allocator]
static ALLOCATOR: LockedSlab = LockedSlab::empty();

// 4. Call this early in your setup execution phase
pub fn init_global_allocator() {
    let range = get_params().find_unused_range(0x100 * 256 * 2).expect("failed getting range for the allocator");

    get_params_mut().blacklist_reloc(range).expect("failed blacklisting range");

    // Wire the runtime address range into the global allocator
    unsafe {
        ALLOCATOR.init(range.start as *mut u8, 0x100, 256);
    }
}
