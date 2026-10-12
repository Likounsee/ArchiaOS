use core::fmt::Write;
use core::ptr;

use archiaos_bootloader::bootinfo::BootInfo;
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
// Scratch snapshot lets the boot-time self-test exercise the exhausted allocator
// deterministically without consuming every usable page or relying on firmware RAM size.

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
    BitmapInvariantViolation,
    AllocationAddressInvalid,
    AllocationNotUnique,
    InvalidFreeAccepted,
    ReservedFreeAccepted,
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

/// Return every allocator frame touched by a reserved byte range, including
/// partially covered first/last pages. This is used for framebuffer MMIO.
fn reserved_frame_range(base: u64, length: u64) -> Option<(u64, u64)> {
    if length == 0 {
        return Some((0, 0));
    }
    let end = base.checked_add(length)?;
    let first = (base / PAGE_SIZE).min(MAX_FRAMES);
    let rounded_end = end.checked_add(PAGE_SIZE - 1)? / PAGE_SIZE;
    Some((first, rounded_end.min(MAX_FRAMES)))
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
    archiaos_bootloader::bootinfo::validate_header(info)
        .map_err(|_| MemoryError::InvalidBootInfo)?;

    let map_size = usize::try_from(info.memory_map_size)
        .map_err(|_| MemoryError::InvalidBootInfo)?;
    let descriptor_size = usize::try_from(info.memory_map_descriptor_size)
        .map_err(|_| MemoryError::InvalidBootInfo)?;

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
            let (fb_first, fb_last) = reserved_frame_range(info.framebuffer_base, info.framebuffer_size)
                .ok_or(MemoryError::InvalidMemoryRange)?;
            let framebuffer_overlap = last.min(fb_last).saturating_sub(first.max(fb_first));
            let usable_pages = (last - first).saturating_sub(framebuffer_overlap);
            if usable_pages != 0 {
                usable_ranges = usable_ranges.checked_add(1)
                    .ok_or(MemoryError::UsableMemoryTotalOverflow)?;
                let managed_bytes = usable_pages.checked_mul(PAGE_SIZE)
                    .ok_or(MemoryError::UsableMemoryTotalOverflow)?;
                usable_bytes = usable_bytes.checked_add(managed_bytes)
                    .ok_or(MemoryError::UsableMemoryTotalOverflow)?;
            }
        }
    }
    if usable_ranges == 0 {
        return Err(MemoryError::NoFreePages);
    }

    // A previous failed initialization attempt must not leak stale managed/free
    // bits into a retry. Reset the complete allocator state only after all BootInfo
    // and descriptor preflight checks have passed, while PMM_INITIALIZED is false.
    // This also makes the failure path deterministic if the handoff map changes
    // unexpectedly between validation and population.
    unsafe {
        set_bit_range(managed_bitmap_ptr(), 0, MAX_FRAMES, false);
        set_bit_range(free_bitmap_ptr(), 0, MAX_FRAMES, false);
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

    // Reserve framebuffer pages even if firmware incorrectly labelled them
    // conventional. Round outward so no partial page can be handed to the PMM.
    let (fb_first, fb_last) = reserved_frame_range(info.framebuffer_base, info.framebuffer_size)
        .ok_or(MemoryError::InvalidMemoryRange)?;
    unsafe {
        set_bit_range(managed_bitmap_ptr(), fb_first, fb_last, false);
        set_bit_range(free_bitmap_ptr(), fb_first, fb_last, false);
        set_bit_range(managed_bitmap_ptr(), 0, ONE_MIB / PAGE_SIZE, false);
        set_bit_range(free_bitmap_ptr(), 0, ONE_MIB / PAGE_SIZE, false);
    }
    let free_pages = count_free_bitmap_pages();
    if free_pages == 0 {
        return Err(MemoryError::NoFreePages);
    }
    // The published count must match the preflighted usable-page total exactly.
    // Check this before making the allocator visible to the rest of the kernel.
    let expected_free_pages = usable_bytes / PAGE_SIZE;
    if free_pages != expected_free_pages {
        return Err(MemoryError::FreeCountMismatch);
    }
    if !validate_bitmap_invariants() {
        return Err(MemoryError::BitmapInvariantViolation);
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

/// Find the first frame marked both managed and free in a bounded bitmap view.
/// Kept independent of global allocator state so exhaustion can be simulated safely.
fn first_free_frame(
    free: &[u64],
    managed: &[u64],
    first_frame: u64,
    frame_limit: u64,
) -> Option<u64> {
    let limit = frame_limit.min((free.len().min(managed.len()) as u64).saturating_mul(64));
    if first_frame >= limit {
        return None;
    }
    let first_word = (first_frame / 64) as usize;
    let last_word = ((limit - 1) / 64) as usize;
    for word_index in first_word..=last_word {
        let mut available = free[word_index] & managed[word_index];
        if word_index == first_word {
            available &= u64::MAX << (first_frame % 64);
        }
        if word_index == last_word && limit % 64 != 0 {
            available &= (1u64 << (limit % 64)) - 1;
        }
        if available != 0 {
            return Some((word_index as u64) * 64 + available.trailing_zeros() as u64);
        }
    }
    None
}

pub fn alloc_page() -> Option<u64> {
    if !unsafe { ptr::read_volatile(ptr::addr_of!(PMM_INITIALIZED)) } {
        return None;
    }
    let free_ptr = free_bitmap_ptr() as *const u64;
    let managed_ptr = managed_bitmap_ptr() as *const u64;
    // SAFETY: both global bitmaps have exactly BITMAP_WORDS elements.
    let free = unsafe { slice::from_raw_parts(free_ptr, BITMAP_WORDS) };
    let managed = unsafe { slice::from_raw_parts(managed_ptr, BITMAP_WORDS) };
    let frame = first_free_frame(free, managed, ONE_MIB / PAGE_SIZE, MAX_FRAMES)?;
    // Clear the free bit only after selecting a frame that is also managed.
    unsafe { set_bit(free_bitmap_ptr(), frame, false) };
    Some(frame * PAGE_SIZE)
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

/// Verify that every free frame is managed and that low memory stays reserved.
fn validate_bitmap_invariants() -> bool {
    let managed = managed_bitmap_ptr() as *const u64;
    let free = free_bitmap_ptr() as *const u64;
    for index in 0..BITMAP_WORDS {
        let managed_word = unsafe { ptr::read(managed.add(index)) };
        let free_word = unsafe { ptr::read(free.add(index)) };
        if free_word & !managed_word != 0 {
            return false;
        }
    }
    let low_frames = ONE_MIB / PAGE_SIZE;
    for frame in 0..low_frames {
        if unsafe { bit_is_set(managed, frame) } || unsafe { bit_is_set(free, frame) } {
            return false;
        }
    }
    true
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
    if !validate_bitmap_invariants() {
        return Err(MemoryError::BitmapInvariantViolation);
    }
    let _ = writeln!(log, "PMM test: free bitmap is a subset of managed memory; low memory reserved");

    // Deterministic exhaustion test on tiny local bitmaps: no firmware-dependent
    // allocation loop, no global PMM mutation, and no risk of stalling QEMU.
    let mut simulated_managed = [0b111u64, 0];
    let mut simulated_free = [0b111u64, 0];
    for expected in 0..3u64 {
        let frame = first_free_frame(&simulated_free, &simulated_managed, 0, 3)
            .ok_or(MemoryError::AllocationFailed)?;
        if frame != expected {
            return Err(MemoryError::AllocationAddressInvalid);
        }
        simulated_free[0] &= !(1u64 << frame);
    }
    if first_free_frame(&simulated_free, &simulated_managed, 0, 3).is_some() {
        return Err(MemoryError::BitmapInvariantViolation);
    }
    // A stale free bit on an unmanaged frame must never be returned.
    simulated_free[0] = 1u64 << 3;
    simulated_managed[0] &= !(1u64 << 3);
    if first_free_frame(&simulated_free, &simulated_managed, 0, 4).is_some() {
        return Err(MemoryError::BitmapInvariantViolation);
    }
    // Releasing a managed frame makes it selectable again.
    simulated_managed[0] |= 1;
    simulated_free[0] = 1;
    if first_free_frame(&simulated_free, &simulated_managed, 0, 1) != Some(0) {
        return Err(MemoryError::AllocationFailed);
    }
    let _ = writeln!(log, "PMM test: deterministic exhaustion, reserved-frame filtering, and reuse passed");

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

    // Also select a reserved frame inside the allocator's managed address window.
    // This covers firmware-reserved holes above 1 MiB, not only the fixed low-memory
    // reservation tested above. Never require a specific firmware map layout.
    let managed = managed_bitmap_ptr() as *const u64;
    let mut reserved_frame = None;
    for frame in (ONE_MIB / PAGE_SIZE)..MAX_FRAMES {
        if !unsafe { bit_is_set(managed, frame) } {
            reserved_frame = Some(frame);
            break;
        }
    }
    if let Some(frame) = reserved_frame {
        if free_page(frame * PAGE_SIZE) {
            return Err(MemoryError::ReservedFreeAccepted);
        }
        let _ = writeln!(log, "PMM test: firmware-reserved frame cannot be freed");
    } else {
        let _ = writeln!(log, "PMM test: no additional reserved frame in 1 MiB..4 GiB window");
    }

    // Cross multiple 64-bit bitmap words to exercise allocation and fragmented frees.
    // Keep addresses in kernel BSS rather than consuming the limited boot stack.
    let pages = ptr::addr_of_mut!(PMM_TEST_PAGES).cast::<u64>();
    for index in 0..PMM_SELF_TEST_PAGES {
        let page = alloc_page().ok_or(MemoryError::AllocationFailed)?;
        if page < ONE_MIB || page % PAGE_SIZE != 0 || page >= MAX_PHYSICAL_ADDRESS {
            return Err(MemoryError::AllocationAddressInvalid);
        }
        let frame = page / PAGE_SIZE;
        if !unsafe { bit_is_set(managed_bitmap_ptr(), frame) }
            || unsafe { bit_is_set(free_bitmap_ptr(), frame) }
        {
            return Err(MemoryError::BitmapInvariantViolation);
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
    if !validate_bitmap_invariants() {
        return Err(MemoryError::BitmapInvariantViolation);
    }
    let _ = writeln!(log, "PMM test: free count restored");
    let _ = writeln!(log, "PMM test: bitmap invariants preserved after allocation/free cycle");
    let first_page = unsafe { ptr::read(pages) };
    if free_page(first_page) {
        return Err(MemoryError::DoubleFreeAccepted);
    }
    let _ = writeln!(log, "PMM test: double-free rejected");

    // Prove that a correctly freed frame remains reusable and that recycling
    // does not change the allocator's total free-page accounting.
    let recycled = alloc_page().ok_or(MemoryError::AllocationFailed)?;
    let recycled_frame = recycled / PAGE_SIZE;
    if !unsafe { bit_is_set(managed_bitmap_ptr(), recycled_frame) }
        || unsafe { bit_is_set(free_bitmap_ptr(), recycled_frame) }
    {
        return Err(MemoryError::BitmapInvariantViolation);
    }
    if !free_page(recycled) {
        return Err(MemoryError::FreeFailed);
    }
    if free_page_count() != before || !validate_bitmap_invariants() {
        return Err(MemoryError::FreeCountMismatch);
    }
    let _ = writeln!(log, "PMM test: freed pages are reusable and accounting is stable");
    Ok(())
}
