use core::fmt::Write;
use core::ptr;

const PAGE_SIZE: u64 = 4096;
const ONE_MIB: u64 = 1024 * 1024;
const MAX_PHYSICAL_ADDRESS: u64 = 1u64 << 32;
const MAX_FRAMES: u64 = MAX_PHYSICAL_ADDRESS / PAGE_SIZE;
const BITMAP_WORDS: usize = (MAX_FRAMES / 64) as usize;
const MAX_MEMMAP_ENTRIES: u64 = 4096;
const PMM_SELF_TEST_PAGES: usize = 130;
const LIMINE_MEMMAP_USABLE: u64 = 0;

#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_BASE_REVISION: [u64; 3] = [
    0xf956_2b2d_5c95_a6c8,
    0x6a7b_3849_4453_6bdc,
    6,
];

#[repr(C)]
struct LimineMemmapEntry {
    base: u64,
    length: u64,
    kind: u64,
}

#[repr(C)]
struct LimineMemmapResponse {
    revision: u64,
    entry_count: u64,
    entries: *mut *mut LimineMemmapEntry,
}

#[repr(C)]
struct LimineMemmapRequest {
    id: [u64; 4],
    revision: u64,
    response: *mut LimineMemmapResponse,
}

#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_MEMMAP_REQUEST: LimineMemmapRequest = LimineMemmapRequest {
    id: [
        0xc7b1_dd30_df4c_8b88,
        0x0a82_e883_a194_f07b,
        0x67cf_3d9d_378a_806f,
        0xe304_acdf_c50c_3c62,
    ],
    revision: 0,
    response: ptr::null_mut(),
};

// A managed bit distinguishes usable frames from reserved frames; a free bit
// distinguishes free from allocated frames. Reserved frames can never be freed.
static mut MANAGED_BITMAP: [u64; BITMAP_WORDS] = [0; BITMAP_WORDS];
static mut FREE_BITMAP: [u64; BITMAP_WORDS] = [0; BITMAP_WORDS];
static mut PMM_INITIALIZED: bool = false;
// Keep the stress-test buffer in reserved kernel BSS, not on the small boot stack.
static mut PMM_TEST_PAGES: [u64; PMM_SELF_TEST_PAGES] = [0; PMM_SELF_TEST_PAGES];

#[derive(Clone, Copy, Debug)]
pub enum MemoryError {
    UnsupportedLimineBaseRevision,
    AlreadyInitialized,
    MissingMemoryMap,
    InvalidMemoryMapMetadata,
    NullMemoryMapEntry,
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

fn frame_range(entry: &LimineMemmapEntry) -> Option<(u64, u64)> {
    let end = entry.base.checked_add(entry.length)?;
    let mut first = entry.base / PAGE_SIZE;
    if entry.base % PAGE_SIZE != 0 {
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

fn apply_range(entry: &LimineMemmapEntry, managed: bool, free: bool) {
    if let Some((first, last)) = frame_range(entry) {
        unsafe {
            set_bit_range(managed_bitmap_ptr(), first, last, managed);
            set_bit_range(free_bitmap_ptr(), first, last, free);
        }
    }
}

fn read_entry(response: &LimineMemmapResponse, index: u64) -> Result<LimineMemmapEntry, MemoryError> {
    // SAFETY: caller bounds index by the validated entry_count.
    let entry_ptr = unsafe { ptr::read_volatile(response.entries.add(index as usize)) };
    if entry_ptr.is_null() {
        return Err(MemoryError::NullMemoryMapEntry);
    }
    // SAFETY: Limine supplies the pointer; it is checked for null and its range is validated.
    let entry = unsafe { ptr::read_volatile(entry_ptr) };
    if entry.length == 0 || entry.base.checked_add(entry.length).is_none() {
        return Err(MemoryError::InvalidMemoryRange);
    }
    Ok(entry)
}

pub fn initialize<W: Write>(log: &mut W) -> Result<MemoryStats, MemoryError> {
    // The allocator is an early-boot singleton and must not be reinitialized.
    if unsafe { ptr::read_volatile(ptr::addr_of!(PMM_INITIALIZED)) } {
        return Err(MemoryError::AlreadyInitialized);
    }
    let _ = writeln!(log, "PMM: checking Limine base revision");
    let base_revision_ptr = ptr::addr_of!(LIMINE_BASE_REVISION).cast::<u64>();
    // SAFETY: the pointer addresses the third u64 in the Limine base-revision request.
    let supported_revision = unsafe { ptr::read_volatile(base_revision_ptr.add(2)) };
    if supported_revision != 0 {
        return Err(MemoryError::UnsupportedLimineBaseRevision);
    }

    // SAFETY: Limine writes the response pointer before transferring control.
    let _ = writeln!(log, "PMM: base revision supported");
    let response_ptr = unsafe { ptr::read_volatile(ptr::addr_of!(LIMINE_MEMMAP_REQUEST.response)) };
    if response_ptr.is_null() {
        return Err(MemoryError::MissingMemoryMap);
    }
    // SAFETY: pointer supplied by Limine; validate metadata before reading entries.
    let response = unsafe { &*response_ptr };
    if response.entry_count == 0
        || response.entry_count > MAX_MEMMAP_ENTRIES
        || response.entries.is_null()
    {
        return Err(MemoryError::InvalidMemoryMapMetadata);
    }

    let _ = writeln!(log, "PMM: memory map metadata valid: {} entries", response.entry_count);
    let mut usable_ranges = 0u64;
    let mut usable_bytes = 0u64;
    for index in 0..response.entry_count {
        let entry = read_entry(response, index)?;
        if entry.kind == LIMINE_MEMMAP_USABLE {
            usable_ranges += 1;
            usable_bytes = usable_bytes
                .checked_add(entry.length)
                .ok_or(MemoryError::UsableMemoryTotalOverflow)?;
        }
    }
    if usable_ranges == 0 {
        return Err(MemoryError::NoFreePages);
    }
    let _ = writeln!(log, "PMM: map entries validated, {} usable ranges", usable_ranges);

    // Set the guard before touching bitmap state. Any later error is fatal to the
    // caller and must not allow a second initialization over partially changed bits.
    unsafe { ptr::write_volatile(ptr::addr_of_mut!(PMM_INITIALIZED), true) };

    // Both bitmap statics live in ELF BSS, which Limine zero-initializes.
    // Initialization is single-shot; do not redundantly clear 256 KiB here.
    let _ = writeln!(log, "PMM: using zero-initialized BSS bitmaps");

    // Mark only full pages from usable regions as managed and free.
    for index in 0..response.entry_count {
        let entry = read_entry(response, index)?;
        if entry.kind == LIMINE_MEMMAP_USABLE {
            apply_range(&entry, true, true);
        }
    }

    let _ = writeln!(log, "PMM: usable ranges marked");

    // Reserved regions take precedence if the supplied map ever overlaps.
    for index in 0..response.entry_count {
        let entry = read_entry(response, index)?;
        if entry.kind != LIMINE_MEMMAP_USABLE {
            apply_range(&entry, false, false);
        }
    }

    let _ = writeln!(log, "PMM: reserved ranges marked");

    // Never allocate the low 1 MiB, which contains legacy and boot structures.
    unsafe {
        set_bit_range(managed_bitmap_ptr(), 0, ONE_MIB / PAGE_SIZE, false);
        set_bit_range(free_bitmap_ptr(), 0, ONE_MIB / PAGE_SIZE, false);
    }

    let _ = writeln!(log, "PMM: low memory reserved");
    let free_pages = free_page_count();
    if free_pages == 0 {
        return Err(MemoryError::NoFreePages);
    }
    let _ = writeln!(log, "PMM: initial free page count {}", free_pages);
    Ok(MemoryStats { map_entries: response.entry_count, usable_ranges, usable_bytes, free_pages })
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

pub fn free_page_count() -> u64 {
    if !unsafe { ptr::read_volatile(ptr::addr_of!(PMM_INITIALIZED)) } {
        return 0;
    }
    let free = free_bitmap_ptr() as *const u64;
    let mut count = 0u64;
    for index in 0..BITMAP_WORDS {
        count += unsafe { ptr::read(free.add(index)) }.count_ones() as u64;
    }
    count
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
