fn rollback_pages(
    boot_services: *mut BootServices,
    starts: &[u64; MAX_LOAD_SEGMENTS],
    page_counts: &[usize; MAX_LOAD_SEGMENTS],
    allocated_count: usize,
) {
    // SAFETY: these page addresses/counts were returned by AllocatePages in this invocation.
    let Some(free_pages) = (unsafe { (*boot_services).free_pages }) else {
        return;
    };
    for index in (0..allocated_count).rev() {
        let _ = unsafe { free_pages(starts[index], page_counts[index]) };
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn flush_instruction_cache(start: u64, length: u64) {
    if length == 0 {
        return;
    }
    let ctr: u64;
    // SAFETY: CTR_EL0 is readable at the kernel's privileged execution level.
    unsafe { core::arch::asm!("mrs {value}, ctr_el0", value = out(reg) ctr, options(nomem, nostack, preserves_flags)) };
    let dcache_line = 4usize << (((ctr >> 16) & 0xf) as usize);
    let icache_line = 4usize << ((ctr & 0xf) as usize);
    let start = start as usize;
    let end = start.saturating_add(length as usize);
    let mut address = start & !(dcache_line - 1);
    while address < end {
        // SAFETY: address lies in the physical range just written by this loader.
        unsafe { core::arch::asm!("dc cvau, {address}", address = in(reg) address, options(nostack, preserves_flags)) };
        address = address.saturating_add(dcache_line);
    }
    // SAFETY: required ordering between data clean and instruction-cache invalidation.
    unsafe { core::arch::asm!("dsb ish", options(nostack, preserves_flags)) };
    address = start & !(icache_line - 1);
    while address < end {
        // SAFETY: address lies in the executable segment just written by this loader.
        unsafe { core::arch::asm!("ic ivau, {address}", address = in(reg) address, options(nostack, preserves_flags)) };
        address = address.saturating_add(icache_line);
    }
    // SAFETY: complete cache maintenance before transferring execution.
    unsafe { core::arch::asm!("dsb ish", "isb", options(nostack, preserves_flags)) };
}

/// Allocate and copy ELF64 PT_LOAD segments. The x86-64 bring-up path accepts
/// identity or bounded high-half images; AArch64 remains identity-only.
pub unsafe fn load_kernel_segments(
    boot_services: *mut BootServices,
    image: &[u8],
    elf: &archiaos_bootloader::elf::ElfFile,
) -> Result<u64, BootError> {
    if boot_services.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: Boot Services unavailable while loading segments.\r\n\0"));
    }
    // SAFETY: AllocatePages is called while boot services are active.
    let allocate_pages = unsafe { (*boot_services).allocate_pages }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: AllocatePages is unavailable.\r\n\0"))?;
    // SAFETY: FreePages is used to roll back successful prior allocations on failure.
    let free_pages = unsafe { (*boot_services).free_pages }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: FreePages is unavailable.\r\n\0"))?;

    let high_half_image = image_uses_highhalf(elf);
    let mut segment_indices = [0u16; MAX_LOAD_SEGMENTS];
    let mut page_starts = [0u64; MAX_LOAD_SEGMENTS];
    let mut page_ends = [0u64; MAX_LOAD_SEGMENTS];
    let mut virtual_page_starts = [0u64; MAX_LOAD_SEGMENTS];
    let mut virtual_page_ends = [0u64; MAX_LOAD_SEGMENTS];
    let mut page_counts = [0usize; MAX_LOAD_SEGMENTS];
    let mut total_memory_size = 0u64;
    let mut segment_count = 0usize;

    for index in 0..elf.program_header_count() {
        let segment = match archiaos_bootloader::elf::load_segment(image, elf, index) {
            Ok(segment) => segment,
            Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
            Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF load segment.\r\n\0")),
        };

        // Enforce W^X across both architectures: no segment may be writable and
        // executable at the same time. The x86-64 page-table builder applies
        // these permissions with NX; AArch64 permission tables will do likewise
        // when its MMU transition is introduced.
        if segment.flags & (PF_W | PF_X) == (PF_W | PF_X) {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: writable-executable ELF segments are rejected by the W^X policy.\r\n\0"));
        }

        let virtual_end = segment.virtual_address
            .checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF virtual address overflow.\r\n\0"))?;
        if segment.virtual_address % EFI_PAGE_SIZE != 0
            || segment.physical_address % EFI_PAGE_SIZE != 0
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ELF load segments must be page-aligned.\r\n\0"));
        }
        if high_half_image {
            #[cfg(target_arch = "x86_64")]
            if segment.virtual_address == segment.physical_address
                || segment.virtual_address < X86_64_HIGH_HALF_BASE
                || virtual_end > X86_64_HIGH_HALF_END
            {
                return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: high-half ELF segments must remain inside the first 128 MiB of the ArchiaOS high-half window.\r\n\0"));
            }
            #[cfg(target_arch = "aarch64")]
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: AArch64 high-half loading is not implemented.\r\n\0"));
        } else if segment.virtual_address != segment.physical_address {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: this ELF image requires virtual mappings not supported by its entry mode.\r\n\0"));
        }
        if segment.memory_size == 0 || segment.memory_size < segment.file_size {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF segment memory size.\r\n\0"));
        }
        total_memory_size = total_memory_size.checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: total ELF memory size overflow.\r\n\0"))?;
        if total_memory_size > MAX_TOTAL_KERNEL_MEMORY_SIZE {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: total ELF segment memory exceeds 128 MiB.\r\n\0"));
        }
        if segment_count >= MAX_LOAD_SEGMENTS {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: too many ELF load segments.\r\n\0"));
        }

        let segment_end = segment.physical_address
            .checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment address overflow.\r\n\0"))?;
        let rounded_end = segment_end
            .checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment page-range overflow.\r\n\0"))?;
        if segment.physical_address < MIN_KERNEL_PHYSICAL_ADDRESS
            || segment_end > MAX_KERNEL_PHYSICAL_ADDRESS
            || rounded_end > MAX_KERNEL_PHYSICAL_ADDRESS
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ELF segment is outside the supported 1 MiB..4 GiB physical load window.\r\n\0"));
        }
        let rounded_length = rounded_end
            .checked_sub(segment.physical_address)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF page range.\r\n\0"))?;
        let rounded_virtual_end = virtual_end
            .checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF virtual page-range overflow.\r\n\0"))?;
        #[cfg(target_arch = "x86_64")]
        if high_half_image && rounded_virtual_end > X86_64_HIGH_HALF_END {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: rounded high-half segment exceeds the page-table window.\r\n\0"));
        }
        let pages = usize::try_from(rounded_length / EFI_PAGE_SIZE)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF allocation size overflows UINTN.\r\n\0"))?;
        if pages == 0 {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: empty ELF page allocation.\r\n\0"));
        }
        let virtual_page_start = segment.virtual_address;
        for prior in 0..segment_count {
            if segment.physical_address < page_ends[prior]
                && page_starts[prior] < rounded_end
            {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segments overlap physically after page rounding.\r\n\0"));
            }
            if virtual_page_start < virtual_page_ends[prior]
                && virtual_page_starts[prior] < rounded_virtual_end
            {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segments overlap virtually after page rounding.\r\n\0"));
            }
        }
        segment_indices[segment_count] = index;
        page_starts[segment_count] = segment.physical_address;
        page_ends[segment_count] = rounded_end;
        virtual_page_starts[segment_count] = virtual_page_start;
        virtual_page_ends[segment_count] = rounded_virtual_end;
        page_counts[segment_count] = pages;
        segment_count += 1;
    }

    if segment_count == 0 {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF has no loadable segments.\r\n\0"));
    }

    let mut allocated_starts = [0u64; MAX_LOAD_SEGMENTS];
    let mut allocated_pages = [0usize; MAX_LOAD_SEGMENTS];
    let mut allocated_count = 0usize;
    let mut found_entry = false;

    for item in 0..segment_count {
        let segment = match archiaos_bootloader::elf::load_segment(image, elf, segment_indices[item]) {
            Ok(segment) => segment,
            Err(_) => {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment changed after validation.\r\n\0"));
            }
        };
        let mut address = page_starts[item];
        let memory_type = if segment.flags & PF_X != 0 { EFI_LOADER_CODE } else { EFI_LOADER_DATA };
        // SAFETY: address and page count were checked; AllocateAddress requests the exact physical range.
        let status = unsafe { allocate_pages(EFI_ALLOCATE_ADDRESS, memory_type, page_counts[item], &mut address) };
        if status != EFI_SUCCESS {
            rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
            return Err(error(status, b"ArchiaOS: firmware could not reserve an ELF physical range.\r\n\0"));
        }
        if address != page_starts[item] {
            let _ = unsafe { free_pages(address, page_counts[item]) };
            rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware allocated ELF pages at an unexpected address.\r\n\0"));
        }
        allocated_starts[allocated_count] = address;
        allocated_pages[allocated_count] = page_counts[item];
        allocated_count += 1;

        let allocation_length = page_counts[item]
            .checked_mul(EFI_PAGE_SIZE as usize)
            .ok_or_else(|| {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                error(EFI_LOAD_ERROR, b"ArchiaOS: segment allocation length overflow.\r\n\0")
            })?;
        // SAFETY: UEFI pages are addressable during boot services and the allocated range is writable.
        let destination = address as *mut u8;
        unsafe { ptr::write_bytes(destination, 0, allocation_length) };

        let file_offset = usize::try_from(segment.file_offset)
            .map_err(|_| {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                error(EFI_LOAD_ERROR, b"ArchiaOS: ELF file offset overflows UINTN.\r\n\0")
            })?;
        let file_size = usize::try_from(segment.file_size)
            .map_err(|_| {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment file size overflows UINTN.\r\n\0")
            })?;
        let source_end = file_offset.checked_add(file_size)
            .ok_or_else(|| {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                error(EFI_LOAD_ERROR, b"ArchiaOS: ELF source range overflows.\r\n\0")
            })?;
        let source = image.get(file_offset..source_end).ok_or_else(|| {
            rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
            error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment lies outside the kernel file.\r\n\0")
        })?;
        // SAFETY: file range is validated; destination is a distinct newly allocated range.
        unsafe { ptr::copy_nonoverlapping(source.as_ptr(), destination, source.len()) };

        if segment.flags & PF_X != 0 {
            let segment_end = segment.virtual_address + segment.memory_size;
            found_entry |= elf.entry() >= segment.virtual_address && elf.entry() < segment_end;
            #[cfg(target_arch = "aarch64")]
            // SAFETY: executable bytes have just been copied to this allocated range.
            unsafe { flush_instruction_cache(segment.physical_address, segment.memory_size) };
        }
    }

    if !found_entry {
        rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF entry is not in an executable loaded segment.\r\n\0"));
    }
    Ok(elf.entry())
}


#[cfg(target_arch = "x86_64")]
pub fn entry_uses_highhalf(entry: u64) -> bool {
    entry >= X86_64_HIGH_HALF_BASE && entry < X86_64_HIGH_HALF_END
}

#[cfg(target_arch = "aarch64")]
pub fn entry_uses_highhalf(_entry: u64) -> bool {
    false
}

fn image_uses_highhalf(elf: &archiaos_bootloader::elf::ElfFile) -> bool {
    entry_uses_highhalf(elf.entry())
}

#[cfg(target_arch = "x86_64")]
fn cpu_supports_nx() -> bool {
    // Extended CPUID leaf 0x80000001 EDX bit 20 advertises the architectural
    // NX bit. Refuse high-half paging before constructing NX-marked PTEs when
    // the CPU cannot enable EFER.NXE.
    let maximum_leaf = unsafe { core::arch::x86_64::__cpuid(0x8000_0000) }.eax;
    if maximum_leaf < 0x8000_0001 {
        return false;
    }
    let features = unsafe { core::arch::x86_64::__cpuid(0x8000_0001) };
    features.edx & (1 << 20) != 0
}

#[cfg(target_arch = "x86_64")]
unsafe fn loaded_image_physical_range(
    image_handle: Handle,
    system_table: *mut SystemTable,
) -> Result<(u64, u64), BootError> {
    if image_handle.is_null() || system_table.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: invalid handle while locating loader image.\r\n\0"));
    }
    // SAFETY: SystemTable comes from the active UEFI application entry point.
    let boot_services = unsafe { (*system_table).boot_services };
    if boot_services.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: Boot Services unavailable while locating loader image.\r\n\0"));
    }
    let handle_protocol = unsafe { (*boot_services).handle_protocol }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: HandleProtocol is unavailable for loader-image mapping.\r\n\0"))?;
    let mut loaded_image_raw = ptr::null_mut();
    // SAFETY: the protocol GUID and output pointer are valid during boot services.
    let status = unsafe { handle_protocol(image_handle, &LOADED_IMAGE_GUID, &mut loaded_image_raw) };
    if status != EFI_SUCCESS || loaded_image_raw.is_null() {
        return Err(error(
            if status == EFI_SUCCESS { EFI_LOAD_ERROR } else { status },
            b"ArchiaOS: cannot query loaded-image range for the page-table transition.\r\n\0",
        ));
    }
    let loaded = loaded_image_raw.cast::<LoadedImageProtocol>();
    // SAFETY: HandleProtocol returned the LoadedImageProtocol interface.
    let (image_base, image_size) = unsafe {
        ((*loaded).image_base as u64, (*loaded).image_size)
    };
    rounded_loader_image_range(image_base, image_size)
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: loader image has an invalid range or is outside the low-canonical identity-map range (512 GiB).\r\n\0"))
}

#[cfg(target_arch = "x86_64")]
fn rounded_loader_image_range(image_base: u64, image_size: u64) -> Option<(u64, u64)> {
    // The x86-64 four-level page tables can identity-map the loader only while
    // it remains in the low canonical 512 GiB region (PML4[0]). Firmware may
    // still place the image above 4 GiB, so its own range is mapped explicitly.
    const MAX_LOW_CANONICAL_IDENTITY_ADDRESS: u64 = 1u64 << 39;
    if image_base == 0 || image_size == 0 {
        return None;
    }
    let image_end = image_base.checked_add(image_size)?;
    let start = image_base & !(EFI_PAGE_SIZE - 1);
    let end = image_end.checked_add(EFI_PAGE_SIZE - 1)? & !(EFI_PAGE_SIZE - 1);
    if start >= end || end > MAX_LOW_CANONICAL_IDENTITY_ADDRESS {
        return None;
    }
    Some((start, end))
}

#[cfg(target_arch = "x86_64")]
struct PageTableArena {
    physical_base: u64,
    base: *mut u64,
    used_pages: usize,
    boot_services: *mut BootServices,
    owns_allocation: bool,
}

#[cfg(target_arch = "x86_64")]
impl PageTableArena {
    unsafe fn allocate(boot_services: *mut BootServices) -> Result<Self, BootError> {
        let allocate_pages = unsafe { (*boot_services).allocate_pages }
            .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: AllocatePages is unavailable for page tables.\r\n\0"))?;
        let mut physical_base = MAX_KERNEL_PHYSICAL_ADDRESS - 1;
        let status = unsafe {
            allocate_pages(
                1, // EFI_ALLOCATE_MAX_ADDRESS
                EFI_LOADER_DATA,
                PAGE_TABLE_ARENA_PAGES,
                &mut physical_base,
            )
        };
        if status != EFI_SUCCESS {
            return Err(error(status, b"ArchiaOS: could not reserve the x86-64 page-table arena below 4 GiB.\r\n\0"));
        }
        let bytes = (PAGE_TABLE_ARENA_PAGES as u64)
            .checked_mul(EFI_PAGE_SIZE)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena size overflow.\r\n\0"))?;
        let end = physical_base.checked_add(bytes)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena address overflow.\r\n\0"))?;
        if physical_base % EFI_PAGE_SIZE != 0 || end > MAX_KERNEL_PHYSICAL_ADDRESS {
            if let Some(free_pages) = unsafe { (*boot_services).free_pages } {
                let _ = unsafe { free_pages(physical_base, PAGE_TABLE_ARENA_PAGES) };
            }
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware returned an invalid page-table arena.\r\n\0"));
        }
        let arena = Self {
            physical_base,
            base: physical_base as *mut u64,
            used_pages: 0,
            boot_services,
            owns_allocation: true,
        };
        // The arena allocation is reserved loader data; clear once before table construction.
        unsafe {
            ptr::write_bytes(
                arena.base,
                0,
                PAGE_TABLE_ARENA_PAGES * (EFI_PAGE_SIZE as usize / core::mem::size_of::<u64>()),
            );
        }
        Ok(arena)
    }

    fn reset(&mut self) {
        self.used_pages = 0;
    }

    fn allocate_page(&mut self) -> Result<*mut u64, BootError> {
        if self.used_pages >= PAGE_TABLE_ARENA_PAGES {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: x86-64 page-table arena exhausted.\r\n\0"));
        }
        let offset = self.used_pages * (EFI_PAGE_SIZE as usize / core::mem::size_of::<u64>());
        let page = unsafe { self.base.add(offset) };
        unsafe { ptr::write_bytes(page, 0, EFI_PAGE_SIZE as usize / core::mem::size_of::<u64>()) };
        self.used_pages += 1;
        Ok(page)
    }

    fn physical_address(&self, page: *mut u64) -> u64 {
        self.physical_base + (page as u64 - self.base as u64)
    }

    fn disarm(&mut self) {
        self.owns_allocation = false;
    }
}

#[cfg(target_arch = "x86_64")]
impl Drop for PageTableArena {
    fn drop(&mut self) {
        if !self.owns_allocation || self.boot_services.is_null() {
            return;
        }
        // SAFETY: the arena came from AllocatePages and this destructor runs before EBS.
        if let Some(free_pages) = unsafe { (*self.boot_services).free_pages } {
            let _ = unsafe { free_pages(self.physical_base, PAGE_TABLE_ARENA_PAGES) };
        }
        self.owns_allocation = false;
    }
}

#[cfg(target_arch = "x86_64")]
fn uefi_memory_type_is_ram(memory_type: u32) -> bool {
    // Include loader/services/conventional RAM and ACPI reclaim/NVS/persistent
    // RAM. Exclude MMIO and reserved descriptors to avoid caching device ranges.
    matches!(memory_type, 1..=7 | 9 | 10 | 14)
}

#[cfg(target_arch = "x86_64")]
unsafe fn build_x86_64_page_tables(
    arena: &mut PageTableArena,
    memory_map: &UefiMemoryMap<'_>,
    image: &[u8],
    elf: &archiaos_bootloader::elf::ElfFile,
    loader_image_start: u64,
    loader_image_end: u64,
    framebuffer_base: u64,
    framebuffer_size: u64,
) -> Result<u64, BootError> {
    arena.reset();
    let pml4 = arena.allocate_page()?;
    let identity_pdpt = arena.allocate_page()?;
    let mut identity_pd = [ptr::null_mut::<u64>(); 4];
    let high_pdpt = arena.allocate_page()?;
    let high_pd = arena.allocate_page()?;

    for index in 0..4 {
        let page = arena.allocate_page()?;
        identity_pd[index] = page;
        // P=1, RW=1, physical addresses in the identity region use 4 KiB leaves.
        unsafe { ptr::write(identity_pdpt.add(index), arena.physical_address(page) | 0x3) };
    }
    unsafe {
        ptr::write(pml4, arena.physical_address(identity_pdpt) | 0x3);
        ptr::write(pml4.add(511), arena.physical_address(high_pdpt) | 0x3);
        ptr::write(high_pdpt.add(510), arena.physical_address(high_pd) | 0x3);
    }

    // Index the identity PTs by their 2 MiB physical span. The fixed arena can
    // map all RAM below 4 GiB with 4 KiB leaves, while leaving reserved/MMIO
    // descriptors unmapped instead of giving devices ordinary write-back aliases.
    let mut identity_tables = [0u64; X86_64_IDENTITY_PT_SPANS];
    for index in 0..memory_map.len() {
        let descriptor = memory_map.entry(index)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid descriptor during x86-64 page-table construction.\r\n\0"))?;
        if !uefi_memory_type_is_ram(descriptor.memory_type) {
            continue;
        }
        let mut address = descriptor.physical_start;
        let end = descriptor.physical_end()
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: memory descriptor overflow during page-table construction.\r\n\0"))?
            .min(MAX_KERNEL_PHYSICAL_ADDRESS);
        // GOP memory is not ordinary RAM even if buggy firmware reports it
        // inside an EfiConventionalMemory descriptor. Leave its identity PTE absent
        // until the graphics subsystem installs an explicitly uncached MMIO mapping.
        let framebuffer_end = framebuffer_base.checked_add(framebuffer_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer range overflow during page-table construction.\r\n\0"))?;
        while address < end {
            let in_framebuffer = framebuffer_size != 0
                && address < framebuffer_end
                && framebuffer_base < address + EFI_PAGE_SIZE;
            if address != 0 && !in_framebuffer {
                let span = (address / (2 * 1024 * 1024)) as usize;
                if span >= identity_tables.len() {
                    return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: physical memory span exceeds the identity map.\r\n\0"));
                }
                let mut table_address = identity_tables[span];
                if table_address == 0 {
                    let table = arena.allocate_page()?;
                    table_address = arena.physical_address(table);
                    identity_tables[span] = table_address;
                    let pdpt_index = (address >> 30) as usize;
                    let pd_index = ((address >> 21) & 0x1ff) as usize;
                    unsafe {
                        ptr::write(identity_pd[pdpt_index].add(pd_index), table_address | 0x3);
                    }
                }
                let pte_index = ((address >> 12) & 0x1ff) as usize;
                unsafe {
                    ptr::write(
                        (table_address as *mut u64).add(pte_index),
                        address | 0x3 | X86_64_PTE_NX,
                    );
                }
            }
            address = address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: identity-map page iteration overflow.\r\n\0"))?;
        }
    }

    // Map the GOP framebuffer explicitly as uncached device memory. This also
    // supports GOP apertures above 4 GiB, which some physical UEFI firmware uses.
    if framebuffer_size != 0 {
        const MAX_LOW_CANONICAL_IDENTITY_ADDRESS: u64 = 1u64 << 39;
        let framebuffer_end = framebuffer_base.checked_add(framebuffer_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer range overflow while mapping GOP memory.\r\n\0"))?;
        if framebuffer_base == 0 || framebuffer_end > MAX_LOW_CANONICAL_IDENTITY_ADDRESS {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: GOP framebuffer is outside the low-canonical 512 GiB identity map.\r\n\0"));
        }
        let mut address = framebuffer_base & !(EFI_PAGE_SIZE - 1);
        let rounded_end = framebuffer_end.checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer page range overflow.\r\n\0"))?;
        while address < rounded_end {
            let table_address = if address < MAX_KERNEL_PHYSICAL_ADDRESS {
                // Reuse the low-memory page tables that also map ordinary RAM.
                let span = (address / (2 * 1024 * 1024)) as usize;
                if span >= identity_tables.len() {
                    return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: framebuffer lies outside the supported identity map.\r\n\0"));
                }
                let mut table_address = identity_tables[span];
                if table_address == 0 {
                    let table = arena.allocate_page()?;
                    table_address = arena.physical_address(table);
                    identity_tables[span] = table_address;
                    let pdpt_index = (address >> 30) as usize;
                    let pd_index = ((address >> 21) & 0x1ff) as usize;
                    unsafe { ptr::write(identity_pd[pdpt_index].add(pd_index), table_address | 0x3) };
                }
                table_address
            } else {
                // Add only the framebuffer's pages above 4 GiB; never create a
                // write-back RAM alias for the surrounding device aperture.
                let pdpt_index = (address >> 30) as usize;
                let pdpt_entry = unsafe { ptr::read(identity_pdpt.add(pdpt_index)) };
                let pd = if pdpt_entry & 1 == 0 {
                    let page = arena.allocate_page()?;
                    let physical = arena.physical_address(page);
                    unsafe { ptr::write(identity_pdpt.add(pdpt_index), physical | 0x3) };
                    page
                } else {
                    (pdpt_entry & 0x000f_ffff_ffff_f000) as *mut u64
                };
                let pd_index = ((address >> 21) & 0x1ff) as usize;
                let pd_entry = unsafe { ptr::read(pd.add(pd_index)) };
                let pt = if pd_entry & 1 == 0 {
                    let page = arena.allocate_page()?;
                    let physical = arena.physical_address(page);
                    unsafe { ptr::write(pd.add(pd_index), physical | 0x3) };
                    page
                } else {
                    (pd_entry & 0x000f_ffff_ffff_f000) as *mut u64
                };
                arena.physical_address(pt)
            };
            let pte_index = ((address >> 12) & 0x1ff) as usize;
            // P=1, RW=1, PWT=1, PCD=1, NX=1: uncached writable framebuffer.
            unsafe {
                ptr::write(
                    (table_address as *mut u64).add(pte_index),
                    address | 0x1 | 0x2 | 0x8 | 0x10 | X86_64_PTE_NX,
                );
            }
            address = address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer mapping iteration overflow.\r\n\0"))?;
        }
    }

    // The default identity map is RW+NX. Override the kernel PT_LOAD pages so
    // their identity aliases cannot bypass high-half W^X: kernel code and rodata
    // are read-only/NX, while data is writable/NX.
    for index in 0..elf.program_header_count() {
        let segment = match archiaos_bootloader::elf::load_segment(image, elf, index) {
            Ok(segment) => segment,
            Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
            Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF segment while protecting identity aliases.\r\n\0")),
        };
        let segment_end = segment.physical_address.checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: physical segment range overflow while protecting identity aliases.\r\n\0"))?;
        let rounded_end = segment_end.checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: rounded physical segment overflow while protecting identity aliases.\r\n\0"))?;
        let flags = 0x1
            | if segment.flags & PF_W != 0 { 0x2 } else { 0 }
            | X86_64_PTE_NX;
        let mut address = segment.physical_address;
        while address < rounded_end {
            let span = (address / (2 * 1024 * 1024)) as usize;
            if span >= identity_tables.len() || identity_tables[span] == 0 {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: kernel identity alias lacks a page table.\r\n\0"));
            }
            let table = identity_tables[span] as *mut u64;
            let pte_index = ((address >> 12) & 0x1ff) as usize;
            unsafe { ptr::write(table.add(pte_index), address | flags) };
            address = address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: kernel identity-map iteration overflow.\r\n\0"))?;
        }
    }

    // The CR3 transition instructions execute from the UEFI loader image after
    // the switch. Keep that image read-only/executable in the identity map,
    // while all kernel physical aliases remain NX.
    let mut loader_address = loader_image_start;
    while loader_address < loader_image_end {
        let pte_index = ((loader_address >> 12) & 0x1ff) as usize;
        if loader_address < MAX_KERNEL_PHYSICAL_ADDRESS {
            // Under 4 GiB, reuse the page tables created from RAM descriptors.
            let span = (loader_address / (2 * 1024 * 1024)) as usize;
            if span >= identity_tables.len() || identity_tables[span] == 0 {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: loader image is outside mapped RAM below 4 GiB.\r\n\0"));
            }
            let table = identity_tables[span] as *mut u64;
            unsafe { ptr::write(table.add(pte_index), loader_address | 0x1) };
        } else {
            // Firmware can load this EFI application above 4 GiB. Map only
            // its own pages there, preserving the bounded <4 GiB RAM map used
            // by the PMM. All paging structures themselves remain below 4 GiB.
            let pdpt_index = (loader_address >> 30) as usize;
            if pdpt_index >= 512 {
                return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: loader image exceeds the low-canonical identity map.\r\n\0"));
            }
            let pdpt_entry = unsafe { ptr::read(identity_pdpt.add(pdpt_index)) };
            let pd = if pdpt_entry & 1 == 0 {
                let page = arena.allocate_page()?;
                let physical = arena.physical_address(page);
                unsafe { ptr::write(identity_pdpt.add(pdpt_index), physical | 0x3) };
                page
            } else {
                (pdpt_entry & 0x000f_ffff_ffff_f000) as *mut u64
            };
            let pd_index = ((loader_address >> 21) & 0x1ff) as usize;
            let pd_entry = unsafe { ptr::read(pd.add(pd_index)) };
            let pt = if pd_entry & 1 == 0 {
                let page = arena.allocate_page()?;
                let physical = arena.physical_address(page);
                unsafe { ptr::write(pd.add(pd_index), physical | 0x3) };
                page
            } else {
                (pd_entry & 0x000f_ffff_ffff_f000) as *mut u64
            };
            unsafe { ptr::write(pt.add(pte_index), loader_address | 0x1) };
        }
        loader_address = loader_address.checked_add(EFI_PAGE_SIZE)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: loader-image identity-map iteration overflow.\r\n\0"))?;
    }

    // Map each validated ELF segment at its high-half virtual address, using
    // its physical load address. The supported virtual window is 128 MiB.
    let mut high_tables = [0u64; X86_64_HIGH_PT_SPANS];
    let mut entry_is_executable = false;
    for index in 0..elf.program_header_count() {
        let segment = match archiaos_bootloader::elf::load_segment(image, elf, index) {
            Ok(segment) => segment,
            Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
            Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid high-half ELF segment.\r\n\0")),
        };
        let segment_end = segment.virtual_address.checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half virtual range overflow.\r\n\0"))?;
        if segment.virtual_address < X86_64_HIGH_HALF_BASE
            || segment_end > X86_64_HIGH_HALF_END
            || segment.virtual_address % EFI_PAGE_SIZE != 0
            || segment.physical_address % EFI_PAGE_SIZE != 0
            || segment.virtual_address == segment.physical_address
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ELF segment does not fit the supported high-half window.\r\n\0"));
        }
        entry_is_executable |= segment.flags & PF_X != 0
            && elf.entry() >= segment.virtual_address
            && elf.entry() < segment_end;

        let mut virtual_address = segment.virtual_address;
        let mut physical_address = segment.physical_address;
        let rounded_end = segment_end.checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half page-range overflow.\r\n\0"))?;
        // High-half leaves mirror ELF permissions: code is read-only/executable,
        // rodata read-only/NX, and data writable/NX. RWX segments were rejected
        // before allocation by the common W^X policy.
        let pte_flags = 0x1
            | if segment.flags & PF_W != 0 { 0x2 } else { 0 }
            | if segment.flags & PF_X == 0 { X86_64_PTE_NX } else { 0 };
        while virtual_address < rounded_end {
            let span = ((virtual_address - X86_64_HIGH_HALF_BASE) / (2 * 1024 * 1024)) as usize;
            if span >= high_tables.len() {
                return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: high-half page-table span exceeds 128 MiB.\r\n\0"));
            }
            let mut table_address = high_tables[span];
            if table_address == 0 {
                let table = arena.allocate_page()?;
                table_address = arena.physical_address(table);
                high_tables[span] = table_address;
                let pd_index = ((virtual_address >> 21) & 0x1ff) as usize;
                unsafe { ptr::write(high_pd.add(pd_index), table_address | 0x3) };
            }
            let pte_index = ((virtual_address >> 12) & 0x1ff) as usize;
            unsafe {
                ptr::write(
                    (table_address as *mut u64).add(pte_index),
                    physical_address | pte_flags,
                );
            }
            virtual_address = virtual_address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half VA iteration overflow.\r\n\0"))?;
            physical_address = physical_address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half PA iteration overflow.\r\n\0"))?;
        }
    }
    if !entry_is_executable {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF entry is not in an executable high-half segment.\r\n\0"));
    }

    Ok(arena.physical_address(pml4))
}
