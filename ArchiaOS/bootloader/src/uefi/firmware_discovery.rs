fn configuration_table_range_is_bounded(base: usize, count: usize) -> bool {
    if base == 0 || count == 0 || count > 4096 {
        return false;
    }
    let Some(bytes) = count.checked_mul(core::mem::size_of::<ConfigurationTable>()) else {
        return false;
    };
    bytes <= isize::MAX as usize && base.checked_add(bytes).is_some()
}

fn configuration_table(
    tables: *mut ConfigurationTable,
    count: usize,
    wanted: &Guid,
) -> *mut c_void {
    if !configuration_table_range_is_bounded(tables as usize, count) {
        return ptr::null_mut();
    }
    for index in 0..count {
        // Firmware metadata is external input: read unaligned so malformed alignment
        // cannot trigger undefined behavior before the table's GUID is checked.
        // SAFETY: callers validate the full array range before lookup; the bounds
        // helper proves multiplication and address arithmetic cannot overflow.
        let offset = index * core::mem::size_of::<ConfigurationTable>();
        let candidate = unsafe {
            ptr::read_unaligned(tables.cast::<u8>().add(offset).cast::<ConfigurationTable>())
        };
        if candidate.vendor_guid == *wanted && !candidate.vendor_table.is_null() {
            return candidate.vendor_table;
        }
    }
    ptr::null_mut()
}

/// Capture a firmware table address without dereferencing it before the final memory map
/// proves that the referenced physical range is reserved, mapped, and disjoint from handoff data.
fn platform_table_address(pointer: *mut c_void) -> u64 {
    pointer as u64
}

/// Return the active GOP framebuffer range when firmware exposes a usable mode.
/// This is metadata discovery only: it does not write pixels or assume a pixel format.
unsafe fn discover_framebuffer(boot_services: *mut BootServices) -> (u64, u64, u32, u32, u32, u32) {
    if boot_services.is_null() {
        return (0, 0, 0, 0, 0, 0);
    }
    let Some(locate_protocol) = (unsafe { (*boot_services).locate_protocol }) else {
        return (0, 0, 0, 0, 0, 0);
    };
    let mut raw = ptr::null_mut();
    // SAFETY: GOP is queried while UEFI boot services are active; the output is writable.
    let status = unsafe {
        locate_protocol(
            &GRAPHICS_OUTPUT_PROTOCOL_GUID,
            ptr::null_mut(),
            &mut raw,
        )
    };
    if status != EFI_SUCCESS || raw.is_null() {
        return (0, 0, 0, 0, 0, 0);
    }
    let gop = raw.cast::<GraphicsOutputProtocol>();
    // SAFETY: LocateProtocol returned the GOP interface for the active firmware mode.
    let mode = unsafe { (*gop).mode };
    if mode.is_null() {
        return (0, 0, 0, 0, 0, 0);
    }
    // Reject inconsistent mode metadata before interpreting the firmware-provided
    // GraphicsOutputModeInformation pointer as a complete structure.
    let max_mode = unsafe { (*mode).max_mode };
    let current_mode = unsafe { (*mode).mode };
    let info_size = unsafe { (*mode).size_of_info };
    if !gop_mode_metadata_is_valid(max_mode, current_mode, info_size) {
        return (0, 0, 0, 0, 0, 0);
    }
    // SAFETY: the current GOP mode reports enough bytes for the full information structure.
    let mode_info = unsafe { (*mode).info };
    let base = unsafe { (*mode).framebuffer_base };
    let size = unsafe { (*mode).framebuffer_size } as u64;
    if mode_info.is_null() || base == 0 || size == 0 || base.checked_add(size).is_none() {
        return (0, 0, 0, 0, 0, 0);
    }
    // Do not publish a partial or blit-only mode as a linear framebuffer. The
    // kernel receives only the physical range, so the byte capacity must cover
    // every scan line and the mode must expose a directly addressable pixel format.
    let info = unsafe { &*mode_info };
    // The structure layout is defined by GOP information version 0. Do not
    // interpret an unknown future layout using the current field offsets.
    if !gop_mode_information_is_supported(info.version) {
        return (0, 0, 0, 0, 0, 0);
    }
    if !archiaos_bootloader::framebuffer::is_usable(
        base,
        size,
        info.horizontal_resolution,
        info.vertical_resolution,
        info.pixel_format,
        info.pixels_per_scan_line,
    ) {
        return (0, 0, 0, 0, 0, 0);
    }
    (base, size, info.horizontal_resolution, info.vertical_resolution, info.pixels_per_scan_line, info.pixel_format)
}

fn gop_mode_metadata_is_valid(max_mode: u32, current_mode: u32, info_size: usize) -> bool {
    max_mode != 0
        && current_mode < max_mode
        && info_size >= core::mem::size_of::<GraphicsOutputModeInformation>()
}

fn gop_mode_information_is_supported(version: u32) -> bool {
    // UEFI currently defines only version 0 of this fixed-layout structure.
    version == 0
}

#[cfg(target_arch = "x86_64")]
fn range_is_identity_mapped_ram(
    memory_map: &UefiMemoryMap<'_>,
    start: u64,
    length: u64,
) -> Result<bool, BootError> {
    let end = start.checked_add(length)
        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: platform-table range overflow.\r\n\0"))?;
    if start == 0 || end > MAX_KERNEL_PHYSICAL_ADDRESS {
        return Ok(false);
    }
    let mut cursor = start;
    while cursor < end {
        let mut next = cursor;
        for index in 0..memory_map.len() {
            let descriptor = memory_map.entry(index)
                .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid descriptor while checking platform-table mappings.\r\n\0"))?;
            let descriptor_end = descriptor.physical_end()
                .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: descriptor overflow while checking platform-table mappings.\r\n\0"))?;
            if descriptor.physical_start <= cursor && cursor < descriptor_end {
                // Match the x86-64 page-table builder: MMIO and generic reserved
                // descriptors are deliberately not identity-mapped as normal RAM.
                if !uefi_memory_type_is_ram(descriptor.memory_type) {
                    return Ok(false);
                }
                next = descriptor_end.min(end);
                break;
            }
        }
        if next == cursor {
            return Ok(false);
        }
        cursor = next;
    }
    Ok(true)
}

/// Validate the ACPI signature before interpreting firmware-controlled RSDP fields.
