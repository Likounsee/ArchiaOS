//! Page-backed kernel heap for the bootstrap kernel.
//!
//! A first-fit free-list allocator maps physical frames on demand into a fixed
//! kernel-only virtual range. It is intentionally single-CPU/interrupt-context
//! only until the kernel gains IRQ-safe and SMP-safe locking.

use core::alloc::{GlobalAlloc, Layout};
use core::fmt::Write;
use core::ptr;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::{memory, vmm};

const PAGE_SIZE: u64 = 4096;
const HEAP_BASE: u64 = 0xffff_a000_0000_0000;
const HEAP_LIMIT: u64 = HEAP_BASE + (1u64 << 30);
const ALLOCATION_MAGIC: u64 = 0x4152_4348_4845_4150;

#[repr(C)]
struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}

#[repr(C)]
struct AllocationHeader {
    block_start: *mut u8,
    block_size: usize,
    magic: u64,
}

struct KernelHeap;
unsafe impl Sync for KernelHeap {}

#[global_allocator]
static GLOBAL_ALLOCATOR: KernelHeap = KernelHeap;

static HEAP_LOCKED: AtomicBool = AtomicBool::new(false);
static HEAP_INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut FREE_LIST: *mut FreeBlock = ptr::null_mut();
static mut HEAP_END: u64 = HEAP_BASE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeapError {
    AlreadyInitialized,
    OutOfMemory,
    Mapping(vmm::VmmError),
    AllocationTestFailed,
}

fn lock() {
    while HEAP_LOCKED.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        spin_loop();
    }
}

fn unlock() {
    HEAP_LOCKED.store(false, Ordering::Release);
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    let mask = alignment.checked_sub(1)?;
    value.checked_add(mask).map(|value| value & !mask)
}

/// Insert a whole free region in address order and merge adjacent neighbors.
/// The caller must hold HEAP_LOCKED and pass a disjoint region inside the heap.
unsafe fn insert_free_block(start: u64, size: usize) {
    let node = start as *mut FreeBlock;
    unsafe { ptr::write(node, FreeBlock { size, next: ptr::null_mut() }) };

    let mut previous: *mut FreeBlock = ptr::null_mut();
    let mut current = unsafe { ptr::read(ptr::addr_of!(FREE_LIST)) };
    while !current.is_null() && (current as u64) < start {
        previous = current;
        current = unsafe { ptr::read(ptr::addr_of!((*current).next)) };
    }

    unsafe { ptr::write(ptr::addr_of_mut!((*node).next), current) };
    if previous.is_null() {
        unsafe { ptr::write(ptr::addr_of_mut!(FREE_LIST), node) };
    } else {
        unsafe { ptr::write(ptr::addr_of_mut!((*previous).next), node) };
    }

    if !current.is_null() {
        let node_end = start.saturating_add(unsafe { ptr::read(ptr::addr_of!((*node).size)) } as u64);
        if node_end == current as u64 {
            let merged_size = unsafe { ptr::read(ptr::addr_of!((*node).size)) }
                .saturating_add(unsafe { ptr::read(ptr::addr_of!((*current).size)) });
            let next = unsafe { ptr::read(ptr::addr_of!((*current).next)) };
            unsafe {
                ptr::write(ptr::addr_of_mut!((*node).size), merged_size);
                ptr::write(ptr::addr_of_mut!((*node).next), next);
            }
        }
    }

    if !previous.is_null() {
        let previous_end = (previous as u64).saturating_add(
            unsafe { ptr::read(ptr::addr_of!((*previous).size)) } as u64,
        );
        if previous_end == node as u64 {
            let node_size = unsafe { ptr::read(ptr::addr_of!((*node).size)) };
            let node_next = unsafe { ptr::read(ptr::addr_of!((*node).next)) };
            let previous_size = unsafe { ptr::read(ptr::addr_of!((*previous).size)) };
            unsafe {
                ptr::write(ptr::addr_of_mut!((*previous).size), previous_size.saturating_add(node_size));
                ptr::write(ptr::addr_of_mut!((*previous).next), node_next);
            }
        }
    }
}

/// Map one more heap page and make it available to the free list.
/// A partial expansion remains valid if a later page allocation fails.
unsafe fn grow_one_page() -> bool {
    let start = unsafe { ptr::read(ptr::addr_of!(HEAP_END)) };
    let Some(end) = start.checked_add(PAGE_SIZE) else { return false; };
    if end > HEAP_LIMIT { return false; }
    let Some(physical) = memory::alloc_page() else { return false; };
    if let Err(_) = vmm::map_kernel_page(start, physical, true, false) {
        let _ = memory::free_page(physical);
        return false;
    }
    unsafe {
        insert_free_block(start, PAGE_SIZE as usize);
        ptr::write(ptr::addr_of_mut!(HEAP_END), end);
    }
    true
}

impl KernelHeap {
    fn allocate(&self, layout: Layout) -> *mut u8 {
        if !HEAP_INITIALIZED.load(Ordering::Acquire) {
            return ptr::null_mut();
        }
        if layout.size() == 0 {
            return layout.align() as *mut u8;
        }
        let minimum = match layout.size()
            .checked_add(core::mem::size_of::<AllocationHeader>())
            .and_then(|size| size.checked_add(layout.align().saturating_sub(1)))
        {
            Some(size) => size,
            None => return ptr::null_mut(),
        };
        if minimum as u64 > HEAP_LIMIT - HEAP_BASE {
            return ptr::null_mut();
        }

        lock();
        loop {
            let mut previous: *mut FreeBlock = ptr::null_mut();
            let mut current = unsafe { ptr::read(ptr::addr_of!(FREE_LIST)) };
            while !current.is_null() {
                let start = current as usize;
                let block_size = unsafe { ptr::read(ptr::addr_of!((*current).size)) };
                let block_end = start.checked_add(block_size);
                let payload = start
                    .checked_add(core::mem::size_of::<AllocationHeader>())
                    .and_then(|value| align_up(value, layout.align()));
                let allocation_end = payload.and_then(|value| value.checked_add(layout.size()));
                if let (Some(block_end), Some(payload), Some(allocation_end)) =
                    (block_end, payload, allocation_end)
                {
                    if allocation_end <= block_end {
                        let next = unsafe { ptr::read(ptr::addr_of!((*current).next)) };
                        if previous.is_null() {
                            unsafe { ptr::write(ptr::addr_of_mut!(FREE_LIST), next) };
                        } else {
                            unsafe { ptr::write(ptr::addr_of_mut!((*previous).next), next) };
                        }
                        let header = (payload - core::mem::size_of::<AllocationHeader>())
                            as *mut AllocationHeader;
                        unsafe {
                            ptr::write(header, AllocationHeader {
                                block_start: start as *mut u8,
                                block_size,
                                magic: ALLOCATION_MAGIC,
                            });
                        }
                        unlock();
                        return payload as *mut u8;
                    }
                }
                previous = current;
                current = unsafe { ptr::read(ptr::addr_of!((*current).next)) };
            }

            if !unsafe { grow_one_page() } {
                unlock();
                return ptr::null_mut();
            }
        }
    }

    unsafe fn deallocate(&self, allocation: *mut u8) {
        if allocation.is_null() || !HEAP_INITIALIZED.load(Ordering::Acquire) {
            return;
        }
        lock();
        let heap_end = unsafe { ptr::read(ptr::addr_of!(HEAP_END)) };
        let address = allocation as u64;
        if address < HEAP_BASE + core::mem::size_of::<AllocationHeader>() as u64
            || address >= heap_end
        {
            unlock();
            return;
        }
        let header = unsafe {
            allocation.sub(core::mem::size_of::<AllocationHeader>()) as *mut AllocationHeader
        };
        let magic = unsafe { ptr::read(ptr::addr_of!((*header).magic)) };
        let start = unsafe { ptr::read(ptr::addr_of!((*header).block_start)) } as u64;
        let size = unsafe { ptr::read(ptr::addr_of!((*header).block_size)) };
        let Some(end) = start.checked_add(size as u64) else {
            unlock();
            return;
        };
        if magic != ALLOCATION_MAGIC || start < HEAP_BASE || end > heap_end || size == 0 {
            unlock();
            return;
        }
        unsafe {
            ptr::write(ptr::addr_of_mut!((*header).magic), 0);
            insert_free_block(start, size);
        }
        unlock();
    }
}

unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
        unsafe { self.deallocate(pointer) };
    }
}

/// Initialize the heap after PMM and VMM startup, before enabling allocations.
pub fn initialize<W: Write>(log: &mut W) -> Result<(), HeapError> {
    lock();
    if HEAP_INITIALIZED.load(Ordering::Acquire) {
        unlock();
        return Err(HeapError::AlreadyInitialized);
    }
    let Some(physical) = memory::alloc_page() else {
        unlock();
        return Err(HeapError::OutOfMemory);
    };
    if let Err(error) = vmm::map_kernel_page(HEAP_BASE, physical, true, false) {
        let _ = memory::free_page(physical);
        unlock();
        return Err(HeapError::Mapping(error));
    }
    unsafe {
        ptr::write(ptr::addr_of_mut!(FREE_LIST), HEAP_BASE as *mut FreeBlock);
        ptr::write(
            HEAP_BASE as *mut FreeBlock,
            FreeBlock { size: PAGE_SIZE as usize, next: ptr::null_mut() },
        );
        ptr::write(ptr::addr_of_mut!(HEAP_END), HEAP_BASE + PAGE_SIZE);
    }
    HEAP_INITIALIZED.store(true, Ordering::Release);
    unlock();
    let _ = writeln!(log, "Heap: initialized page-backed kernel heap at {:#x}", HEAP_BASE);
    Ok(())
}

/// Exercise alignment, page growth, read/write access, freeing and reuse.
pub fn self_test<W: Write>(log: &mut W) -> Result<(), HeapError> {
    let small = Layout::from_size_align(73, 64).map_err(|_| HeapError::AllocationTestFailed)?;
    let large = Layout::from_size_align(PAGE_SIZE as usize + 17, PAGE_SIZE as usize)
        .map_err(|_| HeapError::AllocationTestFailed)?;
    let first = unsafe { GLOBAL_ALLOCATOR.alloc(small) };
    if first.is_null() || (first as usize) % 64 != 0 {
        return Err(HeapError::AllocationTestFailed);
    }
    unsafe {
        ptr::write_bytes(first, 0x5a, small.size());
        if ptr::read(first) != 0x5a || ptr::read(first.add(small.size() - 1)) != 0x5a {
            GLOBAL_ALLOCATOR.dealloc(first, small);
            return Err(HeapError::AllocationTestFailed);
        }
        GLOBAL_ALLOCATOR.dealloc(first, small);
    }

    let second = unsafe { GLOBAL_ALLOCATOR.alloc(large) };
    if second.is_null() || (second as usize) % PAGE_SIZE as usize != 0 {
        return Err(HeapError::AllocationTestFailed);
    }
    unsafe {
        ptr::write_volatile(second, 0x37);
        ptr::write_volatile(second.add(large.size() - 1), 0x73);
        if ptr::read_volatile(second) != 0x37
            || ptr::read_volatile(second.add(large.size() - 1)) != 0x73
        {
            GLOBAL_ALLOCATOR.dealloc(second, large);
            return Err(HeapError::AllocationTestFailed);
        }
        GLOBAL_ALLOCATOR.dealloc(second, large);
    }

    let reused = unsafe { GLOBAL_ALLOCATOR.alloc(small) };
    if reused.is_null() || (reused as usize) % 64 != 0 {
        return Err(HeapError::AllocationTestFailed);
    }
    unsafe { GLOBAL_ALLOCATOR.dealloc(reused, small) };
    let _ = writeln!(log, "Heap: alignment, page growth, access, free and reuse tests passed");
    Ok(())
}
