/// Capture platform metadata, validate the final firmware memory map, terminate
/// boot services, and transfer control using the versioned BootInfo contract.
pub unsafe fn transfer_to_kernel(
    image_handle: Handle,
    system_table: *mut SystemTable,
    kernel_entry: u64,
    _kernel_image: &[u8],
    _elf: &archiaos_bootloader::elf::ElfFile,
) -> Result<Infallible, BootError> {
    if image_handle.is_null() || system_table.is_null() || kernel_entry == 0 {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: invalid kernel handoff arguments.\r\n\0"));
    }
    // SAFETY: system table is the pointer provided by UEFI to efi_main.
    let boot_services = unsafe { (*system_table).boot_services };
    if boot_services.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: Boot Services unavailable for handoff.\r\n\0"));
    }
    let get_memory_map = unsafe { (*boot_services).get_memory_map }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: GetMemoryMap is unavailable.\r\n\0"))?;
    let exit_boot_services = unsafe { (*boot_services).exit_boot_services }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: ExitBootServices is unavailable.\r\n\0"))?;

    #[cfg(target_arch = "x86_64")]
    if image_uses_highhalf(_elf) && !cpu_supports_nx() {
        return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: high-half paging requires CPU NX support.\r\n\0"));
    }

    let mut info_allocation = unsafe {
        PoolAllocation::new(boot_services, core::mem::size_of::<BootInfo>())?
    };
    let mut map_allocation = unsafe {
        PoolAllocation::new(boot_services, MAX_MEMORY_MAP_SIZE)?
    };
    let info_address = info_allocation.pointer.cast::<BootInfo>();
    let map_address = map_allocation.pointer;
    #[cfg(target_arch = "x86_64")]
    if image_uses_highhalf(_elf) {
        let info_start = info_address as u64;
        let info_end = info_start
            .checked_add(core::mem::size_of::<BootInfo>() as u64)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: BootInfo physical range overflow.\r\n\0"))?;
        let map_start = map_address as u64;
        let map_end = map_start
            .checked_add(MAX_MEMORY_MAP_SIZE as u64)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: memory-map buffer physical range overflow.\r\n\0"))?;
        // The current x86-64 high-half page tables deliberately identity-map
        // RAM only below 4 GiB. Fail explicitly instead of transferring pointers
        // that the production kernel could not dereference after the CR3 switch.
        if info_start < MIN_KERNEL_PHYSICAL_ADDRESS
            || info_end > MAX_KERNEL_PHYSICAL_ADDRESS
            || map_start < MIN_KERNEL_PHYSICAL_ADDRESS
            || map_end > MAX_KERNEL_PHYSICAL_ADDRESS
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: BootInfo or final memory-map buffer lies outside the identity-mapped 1 MiB..4 GiB window.\r\n\0"));
        }
    }
    #[cfg(target_arch = "x86_64")]
    let loader_image_range = if image_uses_highhalf(_elf) {
        Some(unsafe { loaded_image_physical_range(image_handle, system_table)? })
    } else {
        None
    };
    #[cfg(target_arch = "x86_64")]
    let mut page_table_arena = if image_uses_highhalf(_elf) {
        Some(unsafe { PageTableArena::allocate(boot_services)? })
    } else {
        None
    };
    // Query GOP before the final memory-map snapshot. A missing or malformed GOP
    // is a supported headless mode; a discovered range is audited below as reserved.
    let (framebuffer_base, framebuffer_size, framebuffer_width, framebuffer_height, framebuffer_stride, framebuffer_pixel_format) =
        unsafe { discover_framebuffer(boot_services) };
    let mut leaked_info = ptr::null_mut::<BootInfo>();
    let mut leaked_map = ptr::null_mut::<u8>();

    for attempt in 0..MAX_EXIT_BOOT_SERVICES_ATTEMPTS {
        let mut map_size = MAX_MEMORY_MAP_SIZE;
        let mut map_key = 0usize;
        let mut descriptor_size = 0usize;
        let mut descriptor_version = 0u32;
        let destination = if leaked_map.is_null() {
            map_address
        } else {
            leaked_map.cast::<c_void>()
        };
        // SAFETY: map buffer is allocated to MAX_MEMORY_MAP_SIZE bytes; all outputs are valid.
        let status = unsafe {
            get_memory_map(
                &mut map_size,
                destination,
                &mut map_key,
                &mut descriptor_size,
                &mut descriptor_version,
            )
        };
        if status != EFI_SUCCESS {
            return Err(error(status, b"ArchiaOS: GetMemoryMap failed or exceeded the reserved buffer.\r\n\0"));
        }
        if map_size == 0 || map_size > MAX_MEMORY_MAP_SIZE || descriptor_size == 0
            || map_size % descriptor_size != 0
            || descriptor_version != UEFI_MEMORY_DESCRIPTOR_VERSION
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware returned an invalid or unsupported memory map.\r\n\0"));
        }

        // This parser is allocation-free and checks every range for overflow and overlap.
        // SAFETY: GetMemoryMap filled exactly map_size bytes in this buffer.
        let map_bytes = unsafe { core::slice::from_raw_parts(destination.cast::<u8>(), map_size) };
        let parsed_map = UefiMemoryMap::parse(map_bytes, descriptor_size)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: UEFI memory-map validation failed.\r\n\0"))?;
        if parsed_map.is_empty() {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: UEFI memory map is empty.\r\n\0"));
        }

        let (acpi_rsdp, device_tree, configuration_table_range) = platform_tables(
            system_table,
            &parsed_map,
            image_uses_highhalf(_elf),
            framebuffer_base,
            framebuffer_size,
        )?;
        #[cfg(target_arch = "aarch64")]
        if device_tree == 0 && acpi_rsdp == 0 {
            return Err(error(EFI_NOT_FOUND, b"ArchiaOS: neither a valid UEFI Device Tree nor ACPI RSDP table was found.\r\n\0"));
        }

        // The final map must continue to classify loader-owned handoff storage as
        // reserved, never conventional RAM. Otherwise the PMM could later allocate
        // memory still holding BootInfo or the map it is actively reading.
        let info_storage = if leaked_info.is_null() {
            info_address.cast::<u8>()
        } else {
            leaked_info.cast::<u8>()
        };
        let info_storage_start = info_storage as u64;
        if !parsed_map.range_is_fully_reserved(
            info_storage_start,
            core::mem::size_of::<BootInfo>() as u64,
        ).map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid BootInfo storage range in final memory map.\r\n\0"))? {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: BootInfo storage is incorrectly marked conventional RAM.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if image_uses_highhalf(_elf) {
            require_identity_mapped_ram(
                &parsed_map,
                info_storage_start,
                core::mem::size_of::<BootInfo>() as u64,
                b"ArchiaOS: BootInfo storage is not covered by the active high-half identity map.\r\n\0",
            )?;
        }
        let map_storage_start = destination as u64;
        if !parsed_map.range_is_fully_reserved(
            map_storage_start,
            MAX_MEMORY_MAP_SIZE as u64,
        ).map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid memory-map buffer range in final map.\r\n\0"))? {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: memory-map buffer is incorrectly marked conventional RAM.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if image_uses_highhalf(_elf) {
            require_identity_mapped_ram(
                &parsed_map,
                map_storage_start,
                MAX_MEMORY_MAP_SIZE as u64,
                b"ArchiaOS: final memory-map buffer is not covered by the active high-half identity map.\r\n\0",
            )?;
        }

        // Every PT_LOAD segment was reserved with AllocateAddress before this
        // final snapshot. Confirm firmware still marks each rounded physical range
        // as loader-owned/reserved instead of conventional RAM.
        for segment_index in 0.._elf.program_header_count() {
            let segment = match archiaos_bootloader::elf::load_segment(
                _kernel_image,
                _elf,
                segment_index,
            ) {
                Ok(segment) => segment,
                Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
                Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF segment while auditing final memory map.\r\n\0")),
            };
            let segment_end = segment.physical_address
                .checked_add(segment.memory_size)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF range overflow during final memory-map audit.\r\n\0"))?;
            let rounded_end = segment_end
                .checked_add(EFI_PAGE_SIZE - 1)
                .map(|value| value & !(EFI_PAGE_SIZE - 1))
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF rounded range overflow during final memory-map audit.\r\n\0"))?;
            let segment_length = rounded_end.checked_sub(segment.physical_address)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF range during final memory-map audit.\r\n\0"))?;
            if !parsed_map.range_is_fully_reserved(
                segment.physical_address,
                segment_length,
            ).map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed ELF range in final memory map.\r\n\0"))? {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: kernel segment is incorrectly marked conventional RAM.\r\n\0"));
            }
            #[cfg(target_arch = "x86_64")]
            if image_uses_highhalf(_elf) {
                require_identity_mapped_ram(
                    &parsed_map,
                    segment.physical_address,
                    segment_length,
                    b"ArchiaOS: kernel segment is not covered by the active identity map.\r\n\0",
                )?;
            }
        }

        #[cfg(target_arch = "x86_64")]
        if let Some((loader_start, loader_end)) = loader_image_range {
            if !parsed_map.range_is_fully_reserved(
                loader_start,
                loader_end.checked_sub(loader_start)
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: inverted loader-image range in final map.\r\n\0"))?,
            ).map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid loader-image range in final map.\r\n\0"))? {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: loader image is incorrectly marked conventional RAM.\r\n\0"));
            }
        }
        #[cfg(target_arch = "x86_64")]
        if let Some(arena) = page_table_arena.as_ref() {
            let arena_bytes = (PAGE_TABLE_ARENA_PAGES as u64)
                .checked_mul(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena size overflow.\r\n\0"))?;
            if !parsed_map.range_is_fully_reserved(arena.physical_base, arena_bytes)
                .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid page-table arena range in final map.\r\n\0"))? {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena is incorrectly marked conventional RAM.\r\n\0"));
            }
            require_identity_mapped_ram(
                &parsed_map,
                arena.physical_base,
                arena_bytes,
                b"ArchiaOS: page-table arena is not covered by the active identity map.\r\n\0",
            )?;
        }

        #[cfg(target_arch = "x86_64")]
        let mut kernel_page_table_root = 0u64;
        #[cfg(target_arch = "x86_64")]
        if let Some(arena) = page_table_arena.as_mut() {
            // The arena and all handoff buffers were allocated before this snapshot.
            // Building tables does not call firmware and therefore preserves the map key.
            let (loader_image_start, loader_image_end) = loader_image_range
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half page tables require a loader image range.\r\n\0"))?;
            kernel_page_table_root = unsafe {
                build_x86_64_page_tables(
                    arena,
                    &parsed_map,
                    _kernel_image,
                    _elf,
                    loader_image_start,
                    loader_image_end,
                    framebuffer_base,
                    framebuffer_size,
                )?
            };
        }

        let info_pointer = if leaked_info.is_null() { info_address } else { leaked_info };
        // SAFETY: the structure is held in a pre-allocated pool that remains reserved after EBS.
        unsafe {
            ptr::write(
                info_pointer,
                BootInfo {
                    // Dedicated CI-only feature corrupts the ABI magic after all normal
                    // loader checks, so the kernel rejection path is exercised end-to-end.
                    magic: if cfg!(feature = "test-invalid-bootinfo") { 0 } else { BOOT_INFO_MAGIC },
                    version: BOOT_INFO_VERSION,
                    struct_size: core::mem::size_of::<BootInfo>() as u32,
                    kernel_entry,
                    memory_map: destination.cast::<u8>(),
                    memory_map_size: map_size as u64,
                    memory_map_descriptor_size: descriptor_size as u64,
                    memory_map_descriptor_version: descriptor_version,
                    flags: 0,
                    acpi_rsdp,
                    device_tree,
                    framebuffer_base,
                    framebuffer_size,
                    framebuffer_width,
                    framebuffer_height,
                    framebuffer_stride,
                    framebuffer_pixel_format,
                },
            );
        }

        // Re-read framebuffer metadata from the completed BootInfo before auditing
        // its physical range. GOP discovery has already run above; keep this check
        // next to the handoff validation so incomplete metadata cannot slip through.
        let framebuffer_base = unsafe { (*info_pointer).framebuffer_base };
        let framebuffer_size = unsafe { (*info_pointer).framebuffer_size };
        if (framebuffer_base == 0) != (framebuffer_size == 0) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: incomplete framebuffer range in BootInfo.\r\n\0"));
        }
        // GOP framebuffer apertures are device MMIO and may legitimately be absent
        // from the UEFI memory descriptors. Validate arithmetic here; the page-table
        // builder maps the aperture explicitly as uncached and excludes it from RAM.
        if framebuffer_base != 0 {
            framebuffer_base.checked_add(framebuffer_size)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer range overflow in BootInfo.\r\n\0"))?;
        }

        // The framebuffer is an MMIO aperture, not ordinary memory. Even if its
        // arithmetic is valid, it must not alias memory that the loader or kernel
        // will use for executable code, BootInfo, the final map, or page tables.
        if framebuffer_base != 0 {
            let framebuffer_conflicts_with = |start: u64, length: u64| {
                physical_ranges_overlap(framebuffer_base, framebuffer_size, start, length)
            };
            if framebuffer_conflicts_with(
                info_storage_start,
                core::mem::size_of::<BootInfo>() as u64,
            ) || framebuffer_conflicts_with(map_storage_start, MAX_MEMORY_MAP_SIZE as u64) {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer overlaps BootInfo or memory-map storage.\r\n\0"));
            }

            for segment_index in 0.._elf.program_header_count() {
                let segment = match archiaos_bootloader::elf::load_segment(
                    _kernel_image,
                    _elf,
                    segment_index,
                ) {
                    Ok(segment) => segment,
                    Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
                    Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF segment during framebuffer overlap audit.\r\n\0")),
                };
                let (rounded_start, rounded_end) =
                    page_rounded_physical_range(segment.physical_address, segment.memory_size)
                        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid or overflowing ELF page range during framebuffer overlap audit.\r\n\0"))?;
                if framebuffer_conflicts_with(rounded_start, rounded_end - rounded_start) {
                    return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer overlaps a loaded kernel segment.\r\n\0"));
                }
            }

            #[cfg(target_arch = "x86_64")]
            if let Some((loader_start, loader_end)) = loader_image_range {
                if framebuffer_conflicts_with(
                    loader_start,
                    loader_end.checked_sub(loader_start)
                        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: inverted loader-image range during framebuffer overlap audit.\r\n\0"))?,
                ) {
                    return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer overlaps the UEFI loader image.\r\n\0"));
                }
            }
            #[cfg(target_arch = "x86_64")]
            if let Some(arena) = page_table_arena.as_ref() {
                let arena_bytes = (PAGE_TABLE_ARENA_PAGES as u64)
                    .checked_mul(EFI_PAGE_SIZE)
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena size overflow during framebuffer overlap audit.\r\n\0"))?;
                if framebuffer_conflicts_with(arena.physical_base, arena_bytes) {
                    return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer overlaps page-table storage.\r\n\0"));
                }
            }
        }
        // Some UEFI implementations expose GOP framebuffer MMIO inside a
        // descriptor labelled EfiConventionalMemory. Do not reject the entire boot
        // on that firmware defect: the x86-64 page-table builder excludes every
        // framebuffer-overlapping page from its ordinary RAM identity map, and the
        // kernel PMM independently reserves those frames before publishing alloc_page().

        // Keep every retained handoff object out of firmware platform-table
        // storage. Four fixed slots cover BootInfo, the map buffer, loader image,
        // and page-table arena; the remaining slots cover each allocated PT_LOAD
        // segment, including the complete pages reserved by AllocateAddress.
        let mut handoff_ranges = [(0u64, 0u64); MAX_LOAD_SEGMENTS + 4];
        handoff_ranges[0] = (
            info_storage_start,
            core::mem::size_of::<BootInfo>() as u64,
        );
        handoff_ranges[1] = (map_storage_start, MAX_MEMORY_MAP_SIZE as u64);
        let mut handoff_range_count = 4usize;
        #[cfg(target_arch = "x86_64")]
        if let Some((loader_start, loader_end)) = loader_image_range {
            handoff_ranges[2] = (
                loader_start,
                loader_end.checked_sub(loader_start)
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: inverted loader range during platform-table audit.\r\n\0"))?,
            );
        }
        #[cfg(target_arch = "x86_64")]
        if let Some(arena) = page_table_arena.as_ref() {
            let arena_bytes = (PAGE_TABLE_ARENA_PAGES as u64)
                .checked_mul(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena size overflow during platform-table audit.\r\n\0"))?;
            handoff_ranges[3] = (arena.physical_base, arena_bytes);
        }
        // Platform tables must not alias any page backing the loaded kernel.
        // Use the page-rounded allocation ranges rather than only ELF byte ranges.
        for segment_index in 0.._elf.program_header_count() {
            let segment = match archiaos_bootloader::elf::load_segment(
                _kernel_image,
                _elf,
                segment_index,
            ) {
                Ok(segment) => segment,
                Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
                Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF segment during platform-table overlap audit.\r\n\0")),
            };
            let (rounded_start, rounded_end) =
                page_rounded_physical_range(segment.physical_address, segment.memory_size)
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid or overflowing ELF page range during platform-table overlap audit.\r\n\0"))?;
            if handoff_range_count >= handoff_ranges.len() {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid or excessive ELF ranges during platform-table overlap audit.\r\n\0"));
            }
            handoff_ranges[handoff_range_count] = (rounded_start, rounded_end - rounded_start);
            handoff_range_count += 1;
        }
        audit_platform_table_ranges(
            &parsed_map,
            acpi_rsdp,
            device_tree,
            image_uses_highhalf(_elf),
            framebuffer_base,
            framebuffer_size,
            &handoff_ranges[..handoff_range_count],
            configuration_table_range,
        )?;

        // Stop tracking the two pool buffers before the first EBS call, so no
        // destructor can call FreePool after boot services have been terminated.
        if leaked_info.is_null() {
            leaked_map = map_allocation.take_raw().cast::<u8>();
            leaked_info = info_allocation.take_raw().cast::<BootInfo>();
        }

        // Once EBS is attempted, never run a page-arena destructor that might call FreePages.
        // A stale map key retries with the same preallocated arena and no other UEFI services.
        #[cfg(target_arch = "x86_64")]
        if let Some(arena) = page_table_arena.as_mut() {
            arena.disarm();
        }
        // SAFETY: map_key is from the immediately preceding successful GetMemoryMap call.
        let status = unsafe { exit_boot_services(image_handle, map_key) };
        if status == EFI_SUCCESS {
            // Boot services are no longer available: do not call firmware or drop guards here.
            #[cfg(target_arch = "x86_64")]
            unsafe { jump_to_kernel(kernel_entry, leaked_info, kernel_page_table_root) };
            #[cfg(target_arch = "aarch64")]
            unsafe { jump_to_kernel(kernel_entry, leaked_info) };
        }
        if status != EFI_INVALID_PARAMETER {
            return Err(error(status, b"ArchiaOS: ExitBootServices failed.\r\n\0"));
        }
        if attempt + 1 == MAX_EXIT_BOOT_SERVICES_ATTEMPTS {
            return Err(error(status, b"ArchiaOS: ExitBootServices could not obtain a stable memory-map key.\r\n\0"));
        }
        // Retry GetMemoryMap immediately with the same buffer and no intervening service calls.
    }

    Err(error(EFI_LOAD_ERROR, b"ArchiaOS: boot handoff ended unexpectedly.\r\n\0"))
}
