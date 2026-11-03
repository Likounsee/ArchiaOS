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
