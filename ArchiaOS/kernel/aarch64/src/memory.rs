use core::ptr;

use archiaos_bootloader::memory_map::{
    MemoryMapError, UefiMemoryMap, EFI_CONVENTIONAL_MEMORY, EFI_PAGE_SIZE,
};

const MAX_PHYSICAL_ADDRESS: u64 = 1u64 << 32;
const MAX_FRAMES: u64 = MAX_PHYSICAL_ADDRESS / EFI_PAGE_SIZE;
const BITMAP_WORDS: usize = (MAX_FRAMES / 64) as usize;
const ONE_MIB: u64 = 1024 * 1024;
const SELF_TEST_PAGES: usize = 130;
const FIRST_ALLOCATABLE_FRAME: u64 = ONE_MIB / EFI_PAGE_SIZE;

static mut MANAGED_BITMAP: [u64; BITMAP_WORDS] = [0; BITMAP_WORDS];
static mut FREE_BITMAP: [u64; BITMAP_WORDS] = [0; BITMAP_WORDS];
static mut INITIALIZED: bool = false;
// Keep the self-test addresses in reserved kernel BSS, not on the early stack.
static mut SELF_TEST_ADDRESSES: [u64; SELF_TEST_PAGES] = [0; SELF_TEST_PAGES];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryError {
    AlreadyInitialized,
    InvalidMemoryMap,
    NoFreePages,
    AllocationFailed,
    DuplicateAllocation,
    FreeCountMismatch,
    FreeFailed,
    InvalidFreeAccepted,
    DoubleFreeAccepted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryStats {
    pub descriptors: usize,
    pub conventional_pages_below_4g: u64,
    pub free_pages: u64,
}

fn managed_bitmap_ptr() -> *mut u64 {
    ptr::addr_of_mut!(MANAGED_BITMAP).cast::<u64>()
}

fn free_bitmap_ptr() -> *mut u64 {
    ptr::addr_of_mut!(FREE_BITMAP).cast::<u64>()
}

unsafe fn bit_is_set(bitmap: *const u64, frame: u64) -> bool {
    let word_ptr = unsafe { bitmap.add((frame / 64) as usize) };
    let word = unsafe { ptr::read(word_ptr) };
    (word & (1u64 << (frame % 64))) != 0
}

unsafe fn set_bit(bitmap: *mut u64, frame: u64, value: bool) {
    let word_ptr = unsafe { bitmap.add((frame / 64) as usize) };
    let mask = 1u64 << (frame % 64);
    let current = unsafe { ptr::read(word_ptr) };
    let updated = if value { current | mask } else { current & !mask };
    unsafe { ptr::write(word_ptr, updated) };
}

unsafe fn set_bit_range(bitmap: *mut u64, first: u64, last: u64, value: bool) {
    let mut frame = first;
    while frame < last {
        let word_index = (frame / 64) as usize;
        let bit_offset = (frame % 64) as u32;
        let chunk = (64 - bit_offset as u64).min(last - frame) as u32;
        let mask = if chunk == 64 {
            u64::MAX
        } else {
            ((1u64 << chunk) - 1) << bit_offset
        };
        let word_ptr = unsafe { bitmap.add(word_index) };
        let current = unsafe { ptr::read(word_ptr) };
        let updated = if value { current | mask } else { current & !mask };
        unsafe { ptr::write(word_ptr, updated) };
        frame += chunk as u64;
    }
}

fn free_page_count_inner() -> u64 {
    let free = free_bitmap_ptr() as *const u64;
    let mut count = 0u64;
    for index in 0..BITMAP_WORDS {
        count += unsafe { ptr::read(free.add(index)) }.count_ones() as u64;
    }
    count
}

/// Initialize the early physical-page allocator from a validated UEFI memory map.
///
/// Only EFI conventional-memory descriptors are made allocatable. Firmware,
/// loader, kernel, BootInfo and memory-map pages remain reserved by their UEFI
/// memory type. The initial allocator is restricted to the first 4 GiB and is
/// single-core only; it is not an SMP allocator.
pub fn initialize(map: &UefiMemoryMap<'_>) -> Result<MemoryStats, MemoryError> {
    if unsafe { ptr::read_volatile(ptr::addr_of!(INITIALIZED)) } {
        return Err(MemoryError::AlreadyInitialized);
    }
    if map.is_empty() {
        return Err(MemoryError::InvalidMemoryMap);
    }

    // Mark the singleton before mutating BSS bitmaps. On any later error, callers
    // must fail-stop instead of retrying over partially initialized state.
    unsafe { ptr::write_volatile(ptr::addr_of_mut!(INITIALIZED), true) };

    let mut conventional_pages_below_4g = 0u64;
    for index in 0..map.len() {
        let entry = map.entry(index).map_err(|_| MemoryError::InvalidMemoryMap)?;
        if entry.memory_type != EFI_CONVENTIONAL_MEMORY {
            continue;
        }

        let start = entry.physical_start / EFI_PAGE_SIZE;
        let end = entry.physical_end().map_err(|_| MemoryError::InvalidMemoryMap)?
            / EFI_PAGE_SIZE;
        let first = start.min(MAX_FRAMES);
        let last = end.min(MAX_FRAMES);
        if first >= last {
            continue;
        }

        let pages = last - first;
        conventional_pages_below_4g = conventional_pages_below_4g
            .checked_add(pages)
            .ok_or(MemoryError::InvalidMemoryMap)?;
        unsafe {
            set_bit_range(managed_bitmap_ptr(), first, last, true);
            set_bit_range(free_bitmap_ptr(), first, last, true);
        }
    }

    // Never allocate the low 1 MiB, which contains legacy and firmware data.
    unsafe {
        set_bit_range(
            managed_bitmap_ptr(),
            0,
            FIRST_ALLOCATABLE_FRAME,
            false,
        );
        set_bit_range(
            free_bitmap_ptr(),
            0,
            FIRST_ALLOCATABLE_FRAME,
            false,
        );
    }

    let free_pages = free_page_count_inner();
    if free_pages == 0 {
        return Err(MemoryError::NoFreePages);
    }

    Ok(MemoryStats {
        descriptors: map.len(),
        conventional_pages_below_4g,
        free_pages,
    })
}

pub fn alloc_page() -> Option<u64> {
    if !unsafe { ptr::read_volatile(ptr::addr_of!(INITIALIZED)) } {
        return None;
    }

    let free = free_bitmap_ptr() as *const u64;
    let managed = managed_bitmap_ptr() as *const u64;
    let first_word = (FIRST_ALLOCATABLE_FRAME / 64) as usize;

    for word_index in first_word..BITMAP_WORDS {
        let word_ptr = unsafe { free.add(word_index) };
        let mut available = unsafe { ptr::read(word_ptr) };
        while available != 0 {
            let bit = available.trailing_zeros() as u64;
            available &= available - 1;
            let frame = (word_index as u64) * 64 + bit;
            if frame >= MAX_FRAMES {
                continue;
            }
            if !unsafe { bit_is_set(managed, frame) } {
                unsafe { set_bit(free_bitmap_ptr(), frame, false) };
                continue;
            }
            unsafe { set_bit(free_bitmap_ptr(), frame, false) };
            return Some(frame * EFI_PAGE_SIZE);
        }
    }
    None
}

/// Return a page to the early allocator. Rejects unaligned, reserved, outside-limit,
/// unmanaged and already-free addresses; synchronization is not implemented yet.
pub fn free_page(physical_address: u64) -> bool {
    if !unsafe { ptr::read_volatile(ptr::addr_of!(INITIALIZED)) } {
        return false;
    }
    if physical_address == 0
        || physical_address % EFI_PAGE_SIZE != 0
        || physical_address >= MAX_PHYSICAL_ADDRESS
        || physical_address < ONE_MIB
    {
        return false;
    }

    let frame = physical_address / EFI_PAGE_SIZE;
    let managed = managed_bitmap_ptr() as *const u64;
    if !unsafe { bit_is_set(managed, frame) }
        || unsafe { bit_is_set(free_bitmap_ptr(), frame) }
    {
        return false;
    }

    unsafe { set_bit(free_bitmap_ptr(), frame, true) };
    true
}

pub fn free_page_count() -> u64 {
    if !unsafe { ptr::read_volatile(ptr::addr_of!(INITIALIZED)) } {
        return 0;
    }
    free_page_count_inner()
}

/// Validate multi-word allocation/free behavior without writing into any allocated page.
pub fn self_test() -> Result<(), MemoryError> {
    let before = free_page_count();
    if before < SELF_TEST_PAGES as u64 {
        return Err(MemoryError::NoFreePages);
    }

    if free_page(0)
        || free_page(1)
        || free_page(ONE_MIB - EFI_PAGE_SIZE)
        || free_page(MAX_PHYSICAL_ADDRESS)
    {
        return Err(MemoryError::InvalidFreeAccepted);
    }

    let addresses = ptr::addr_of_mut!(SELF_TEST_ADDRESSES).cast::<u64>();
    for index in 0..SELF_TEST_PAGES {
        let page = alloc_page().ok_or(MemoryError::AllocationFailed)?;
        if page < ONE_MIB || page % EFI_PAGE_SIZE != 0 || page >= MAX_PHYSICAL_ADDRESS {
            return Err(MemoryError::AllocationFailed);
        }
        for previous in 0..index {
            if unsafe { ptr::read(addresses.add(previous)) } == page {
                return Err(MemoryError::DuplicateAllocation);
            }
        }
        unsafe { ptr::write(addresses.add(index), page) };
    }

    if free_page_count().checked_add(SELF_TEST_PAGES as u64) != Some(before) {
        return Err(MemoryError::FreeCountMismatch);
    }
    for index in (0..SELF_TEST_PAGES).rev() {
        let page = unsafe { ptr::read(addresses.add(index)) };
        if !free_page(page) {
            return Err(MemoryError::FreeFailed);
        }
    }
    if free_page_count() != before {
        return Err(MemoryError::FreeCountMismatch);
    }
    let first_page = unsafe { ptr::read(addresses) };
    if free_page(first_page) {
        return Err(MemoryError::DoubleFreeAccepted);
    }
    Ok(())
}
