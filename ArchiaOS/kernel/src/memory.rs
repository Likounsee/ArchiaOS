use core::fmt::Write;
use core::ptr;

use archiaos_bootloader::bootinfo::{BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};
use archiaos_bootloader::memory_map::UefiMemoryMap;
use core::slice;

const PAGE_SIZE: u64 = 4096;
const ONE_MIB: u64 = 1024 * 1024;
const MAX_PHYSICAL_ADDRESS: u64 = 1u64 << 32;
const MAX_FRAMES: u64 = MAX_PHYSICAL_ADDRESS / PAGE_SIZE;
const BITMAP_WORDS: usize = (MAX_FRAMES / 64) as usize;
const MAX_MEMMAP_ENTRIES: u64 = 4096;
const PMM_SELF_TEST_PAGES: usize = 130;
// A managed bit distinguishes usable frames from reserved frames; a free bit
// distinguishes free from allocated frames. Reserved frames can never be freed.
static mut MANAGED_BITMAP: [u64; BITMAP_WORDS] = [0; BITMAP_WORDS];
static mut FREE_BITMAP: [u64; BITMAP_WORDS] = [0; BITMAP_WORDS];
static mut PMM_INITIALIZED: bool = false;
// Keep the stress-test buffer in reserved kernel BSS, not on the small boot stack.
static mut PMM_TEST_PAGES: [u64; PMM_SELF_TEST_PAGES] = [0; PMM_SELF_TEST_PAGES];

#[derive(Clone, Copy, Debug)]
pub enum MemoryError {
    InvalidBootInfoPointer,
    InvalidBootInfo,
    InvalidUefiMemoryMap,
    AlreadyInitialized,
    MissingMemoryMap,
    InvalidMemoryMapMetadata,
    InvalidMemoryRange,
    UsableMemoryTotalOverflow,
    NoFreePages,
    AllocationFailed,
    AllocationAddressInvalid,
    AllocationNotUnique,
    InvalidFreeAccepted,
    FreeFailed,
    DoubleFreeAccepted,
    FreeCountMismatch,
}

#[derive(Clone, Copy, Debug)]
pub struct MemoryStats {
    pub map_entries: u64,
    pub usable_ranges: u64,
    pub usable_bytes: u64,
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

fn frame_range(base: u64, length: u64) -> Option<(u64, u64)> {
    let end = base.checked_add(length)?;
    let mut first = base / PAGE_SIZE;
    if base % PAGE_SIZE != 0 {
        first = first.checked_add(1)?;
    }
    Some((first.min(MAX_FRAMES), (end / PAGE_SIZE).min(MAX_FRAMES)))
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

fn apply_range(base: u64, length: u64, managed: bool, free: bool) {
    if let Some((first, last)) = frame_range(base, length) {
        unsafe {
            set_bit_range(managed_bitmap_ptr(), first, last, managed);
            set_bit_range(free_bitmap_ptr(), first, last, free);
        }
    }
}

/// Initialize the physical-page allocator from the ArchiaOS UEFI handoff.
pub unsafe fn initialize_from_bootinfo<W: Write>(
    log: &mut W,
    boot_info_ptr: *const BootInfo,
) -> Result<MemoryStats, MemoryError> {
    if unsafe { ptr::read_volatile(ptr::addr_of!(PMM_INITIALIZED)) } {
        return Err(MemoryError::AlreadyInitialized);
    }
    if boot_info_ptr.is_null()
        || (boot_info_ptr as usize) % core::mem::align_of::<BootInfo>() != 0
    {
        return Err(MemoryError::InvalidBootInfoPointer);
    }

    // The UEFI loader owns this contract. Once the pointer is non-null and
    // aligned, validate the complete versioned header before using any fields.
    let info = unsafe { &*boot_info_ptr };
    if info.magic != BOOT_INFO_MAGIC
        || info.version != BOOT_INFO_VERSION
        || (info.struct_size as usize) < core::mem::size_of::<BootInfo>()
        || info.memory_map.is_null()
        || info.memory_map_size == 0
        || info.memory_map_size > 1024 * 1024
        || info.memory_map_descriptor_version == 0
        || info.memory_map_descriptor_size == 0
    {
        return Err(MemoryError::InvalidBootInfo);
    }

    let map_size = usize::try_from(info.memory_map_size)
        .map_err(|_| MemoryError::InvalidBootInfo)?;
    let descriptor_size = usize::try_from(info.memory_map_descriptor_size)
        .map_err(|_| MemoryError::InvalidBootInfo)?;
    if (info.memory_map as usize) % core::mem::align_of::<u64>() != 0 {
        return Err(MemoryError::InvalidBootInfo);
    }

    // SAFETY: BootInfo is supplied by our bootloader; bounds and alignment are
    // checked above, and the loader keeps the map allocation reserved through
    // the handoff. UefiMemoryMap then validates stride and every physical range.
    let bytes = unsafe { slice::from_raw_parts(info.memory_map, map_size) };
    let memory_map = UefiMemoryMap::parse(bytes, descriptor_size)
        .map_err(|_| MemoryError::InvalidUefiMemoryMap)?;
    if memory_map.is_empty() || memory_map.len() > 65_536 {
        return Err(MemoryError::InvalidUefiMemoryMap);
    }

    let mut usable_ranges = 0u64;
    let mut usable_bytes = 0u64;
    // Preflight every descriptor before touching the global allocator state.
    // This keeps malformed reserved descriptors from leaving a half-initialized PMM.
    for index in 0..memory_map.len() {
        let descriptor = memory_map.entry(index)
            .map_err(|_| MemoryError::InvalidUefiMemoryMap)?;
        let length = descriptor.byte_length()
            .map_err(|_| MemoryError::InvalidMemoryRange)?;
        if descriptor.is_conventional() {
            // Report only complete pages the current bitmap can actually manage:
            // frames below 1 MiB are reserved, and physical addresses at/above
            // 4 GiB are outside this PMM's current supported window.
            let (first, last) = frame_range(descriptor.physical_start, length)
                .ok_or(MemoryError::InvalidMemoryRange)?;
            let first = first.max(ONE_MIB / PAGE_SIZE);
            if first < last {
                usable_ranges = usable_ranges.checked_add(1)
                    .ok_or(MemoryError::UsableMemoryTotalOverflow)?;
                let managed_bytes = (last - first).checked_mul(PAGE_SIZE)
                    .ok_or(MemoryError::UsableMemoryTotalOverflow)?;
                usable_bytes = usable_bytes.checked_add(managed_bytes)
                    .ok_or(MemoryError::UsableMemoryTotalOverflow)?;
            }
        }
    }
    if usable_ranges == 0 {
        return Err(MemoryError::NoFreePages);
    }

    // Only UEFI EfiConventionalMemory ranges become allocatable; loader, runtime,
    // ACPI, MMIO and all other descriptor types remain reserved. Do not publish
    // the allocator as initialized until every bitmap is populated and verified.
    let _ = writeln!(log, "PMM: validated ArchiaOS UEFI BootInfo v{}", info.version);
    let _ = writeln!(log, "PMM: UEFI memory descriptors validated: {}", memory_map.len());

    for index in 0..memory_map.len() {
        let descriptor = memory_map.entry(index)
            .map_err(|_| MemoryError::InvalidUefiMemoryMap)?;
        if descriptor.is_conventional() {
            let length = descriptor.byte_length()
                .map_err(|_| MemoryError::InvalidMemoryRange)?;
            apply_range(descriptor.physical_start, length, true, true);
        }
    }

    // Reserved descriptors always override usable bits if a non-conforming map
    // ever changes during handoff. The parser rejects overlap, but this also
    // makes the intended precedence explicit and keeps the two boot paths aligned.
    for index in 0..memory_map.len() {
        let descriptor = memory_map.entry(index)
            .map_err(|_| MemoryError::InvalidUefiMemoryMap)?;
        if !descriptor.is_conventional() {
            let length = descriptor.byte_length()
                .map_err(|_| MemoryError::InvalidMemoryRange)?;
            apply_range(descriptor.physical_start, length, false, false);
        }
    }

    unsafe {
        set_bit_range(managed_bitmap_ptr(), 0, ONE_MIB / PAGE_SIZE, false);
        set_bit_range(free_bitmap_ptr(), 0, ONE_MIB / PAGE_SIZE, false);
    }
    let free_pages = count_free_bitmap_pages();
    if free_pages == 0 {
        return Err(MemoryError::NoFreePages);
    }
    unsafe { ptr::write_volatile(ptr::addr_of_mut!(PMM_INITIALIZED), true) };
    let _ = writeln!(log, "PMM: conventional UEFI ranges: {}, bytes: {}", usable_ranges, usable_bytes);
    let _ = writeln!(log, "PMM: initial free page count {}", free_pages);
    Ok(MemoryStats {
        map_entries: memory_map.len() as u64,
        usable_ranges,
        usable_bytes,
        free_pages,
    })
}

pub fn alloc_page() -> Option<u64> {
    if !unsafe { ptr::read_volatile(ptr::addr_of!(PMM_INITIALIZED)) } {
        return None;
    }
    let free = free_bitmap_ptr() as *const u64;
    let managed = managed_bitmap_ptr() as *const u64;
    let first_word = (ONE_MIB / PAGE_SIZE / 64) as usize;

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
            return Some(frame * PAGE_SIZE);
        }
    }
    None
}

pub fn free_page(physical_address: u64) -> bool {
    if !unsafe { ptr::read_volatile(ptr::addr_of!(PMM_INITIALIZED)) } {
        return false;
    }
    if physical_address == 0
        || physical_address % PAGE_SIZE != 0
        || physical_address >= MAX_PHYSICAL_ADDRESS
    {
        return false;
    }
    let frame = physical_address / PAGE_SIZE;
    if frame < ONE_MIB / PAGE_SIZE {
        return false;
    }

    let managed = managed_bitmap_ptr() as *const u64;
    if !unsafe { bit_is_set(managed, frame) } || unsafe { bit_is_set(free_bitmap_ptr(), frame) } {
        return false;
    }
    unsafe { set_bit(free_bitmap_ptr(), frame, true) };
    true
}

fn count_free_bitmap_pages() -> u64 {
    let free = free_bitmap_ptr() as *const u64;
    let mut count = 0u64;
    for index in 0..BITMAP_WORDS {
        count += unsafe { ptr::read(free.add(index)) }.count_ones() as u64;
    }
    count
}

pub fn free_page_count() -> u64 {
    if !unsafe { ptr::read_volatile(ptr::addr_of!(PMM_INITIALIZED)) } {
        return 0;
    }
    count_free_bitmap_pages()
}

pub fn self_test<W: Write>(log: &mut W) -> Result<(), MemoryError> {
    let before = free_page_count();
    if before < PMM_SELF_TEST_PAGES as u64 {
        return Err(MemoryError::NoFreePages);
    }

    let _ = writeln!(log, "PMM test: rejecting invalid free addresses");
    // Reserved, unaligned, zero, and exclusive-limit addresses must never free a frame.
    if free_page(0)
        || free_page(1)
        || free_page(ONE_MIB - PAGE_SIZE)
        || free_page(MAX_PHYSICAL_ADDRESS)
    {
        return Err(MemoryError::InvalidFreeAccepted);
    }
    let _ = writeln!(log, "PMM test: invalid frees rejected");

    // Cross multiple 64-bit bitmap words to exercise allocation and fragmented frees.
    // Keep addresses in kernel BSS rather than consuming the limited boot stack.
    let pages = ptr::addr_of_mut!(PMM_TEST_PAGES).cast::<u64>();
    for index in 0..PMM_SELF_TEST_PAGES {
        let page = alloc_page().ok_or(MemoryError::AllocationFailed)?;
        if page < ONE_MIB || page % PAGE_SIZE != 0 || page >= MAX_PHYSICAL_ADDRESS {
            return Err(MemoryError::AllocationAddressInvalid);
        }
        for previous in 0..index {
            if unsafe { ptr::read(pages.add(previous)) } == page {
                return Err(MemoryError::AllocationNotUnique);
            }
        }
        unsafe { ptr::write(pages.add(index), page) };
        if index == 0 || (index + 1) % 64 == 0 || index + 1 == PMM_SELF_TEST_PAGES {
            let _ = writeln!(log, "PMM test: allocated {} pages", index + 1);
        }
    }

    if free_page_count().checked_add(PMM_SELF_TEST_PAGES as u64) != Some(before) {
        return Err(MemoryError::FreeCountMismatch);
    }
    let _ = writeln!(log, "PMM test: allocation count verified");
    for index in (0..PMM_SELF_TEST_PAGES).rev() {
        let page = unsafe { ptr::read(pages.add(index)) };
        if !free_page(page) {
            return Err(MemoryError::FreeFailed);
        }
        let freed = PMM_SELF_TEST_PAGES - index;
        if freed == 1 || freed % 64 == 0 || freed == PMM_SELF_TEST_PAGES {
            let _ = writeln!(log, "PMM test: freed {} pages", freed);
        }
    }
    if free_page_count() != before {
        return Err(MemoryError::FreeCountMismatch);
    }
    let _ = writeln!(log, "PMM test: free count restored");
    let first_page = unsafe { ptr::read(pages) };
    if free_page(first_page) {
        return Err(MemoryError::DoubleFreeAccepted);
    }
    let _ = writeln!(log, "PMM test: double-free rejected");
    Ok(())
}
