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
fn rsdp_signature_is_valid(signature: &[u8]) -> bool {
    signature == archiaos_bootloader::acpi::RSDP_SIGNATURE
}

/// Enforce the distinct ACPI 1.0 and extended RSDP minimum lengths during handoff.
/// Extended RSDPs include fields through byte 35; accepting a 20-byte prefix for
/// revision >= 2 would let mutated firmware metadata pass the final-map audit.
fn rsdp_length_is_valid(revision: u8, length: u64) -> bool {
    // ACPI revision 1 is reserved. Only revision 0 uses the 20-byte legacy
    // structure; revision 2 and later must expose the complete extended prefix.
    let minimum = match revision {
        0 => 20,
        1 => return false,
        _ => 36,
    };
    (minimum..=archiaos_bootloader::acpi::MAX_RSDP_SIZE as u64).contains(&length)
}

/// Recheck ACPI RSDP checksums at handoff, not only during initial discovery.
/// ACPI 2.0+ requires both the legacy first-20-byte checksum and the full-table checksum.
fn rsdp_checksums_are_valid(bytes: &[u8], revision: u8, length: u64) -> bool {
    if !rsdp_length_is_valid(revision, length) || bytes.len() as u64 != length {
        return false;
    }
    if bytes[..20].iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
        return false;
    }
    revision < 2 || bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) == 0
}

/// Parse only the fixed FDT header prefix without trusting firmware-provided lengths.
fn validated_fdt_prefix_length(header: &[u8]) -> Option<u64> {
    if header.len() < 8
        || u32::from_be_bytes([header[0], header[1], header[2], header[3]])
            != archiaos_bootloader::device_tree::FDT_MAGIC
    {
        return None;
    }
    let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as u64;
    if !(archiaos_bootloader::device_tree::HEADER_SIZE as u64
        ..=archiaos_bootloader::device_tree::MAX_TOTAL_SIZE as u64).contains(&length)
    {
        return None;
    }
    Some(length)
}

/// Convert a non-empty physical byte range to the complete page allocation it touches.
/// Returns a half-open (start, end) pair and rejects every arithmetic overflow.
fn page_rounded_physical_range(start: u64, length: u64) -> Option<(u64, u64)> {
    if length == 0 {
        return None;
    }
    let end = start.checked_add(length)?;
    let rounded_start = start & !(EFI_PAGE_SIZE - 1);
    let rounded_end = end.checked_add(EFI_PAGE_SIZE - 1)? & !(EFI_PAGE_SIZE - 1);
    (rounded_end > rounded_start).then_some((rounded_start, rounded_end))
}

/// Conservatively reject overlapping handoff ranges; arithmetic overflow fails closed.
fn physical_ranges_overlap(first_start: u64, first_length: u64, second_start: u64, second_length: u64) -> bool {
    if first_length == 0 || second_length == 0 {
        return false;
    }
    let Some(first_end) = first_start.checked_add(first_length) else {
        return true;
    };
    let Some(second_end) = second_start.checked_add(second_length) else {
        return true;
    };
    first_start < second_end && second_start < first_end
}

/// Reject platform tables that alias any loader-owned range retained across the handoff.
fn platform_range_overlaps_handoff_storage(
    start: u64,
    length: u64,
    handoff_ranges: &[(u64, u64)],
) -> bool {
    handoff_ranges.iter().any(|&(reserved_start, reserved_length)| {
        physical_ranges_overlap(start, length, reserved_start, reserved_length)
    })
}

/// Validate one ACPI SDT range before reading its advertised size or checksum.
fn audit_acpi_sdt_range(
    memory_map: &UefiMemoryMap<'_>,
    address: u64,
    high_half_image: bool,
    framebuffer_base: u64,
    framebuffer_size: u64,
    protected_ranges: &[(u64, u64)],
    handoff_ranges: &[(u64, u64)],
    max_table_length: u64,
) -> Result<([u8; 4], u64), BootError> {
    const HEADER_SIZE: u64 = archiaos_bootloader::acpi::SDT_HEADER_SIZE as u64;
    const MAX_TABLE_SIZE: u64 = archiaos_bootloader::acpi::MAX_SDT_SIZE as u64;
    #[cfg(target_arch = "aarch64")]
    let _ = high_half_image;

    let overlaps_protected_storage = |length: u64| {
        physical_ranges_overlap(address, length, framebuffer_base, framebuffer_size)
            || protected_ranges.iter().any(|&(start, size)| {
                physical_ranges_overlap(address, length, start, size)
            })
            || platform_range_overlaps_handoff_storage(address, length, handoff_ranges)
    };

    if address == 0 {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI table has a null physical address.\r\n\0"));
    }
    if !memory_map.range_is_fully_reserved(address, HEADER_SIZE)
        .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed ACPI table header range in final memory map.\r\n\0"))?
    {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI table header is not fully reserved in the final memory map.\r\n\0"));
    }
    #[cfg(target_arch = "x86_64")]
    if high_half_image && !range_is_identity_mapped_ram(memory_map, address, HEADER_SIZE)? {
        return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ACPI table header is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
    }
    if overlaps_protected_storage(HEADER_SIZE) {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI table header overlaps framebuffer, platform metadata, or handoff storage.\r\n\0"));
    }

    // SAFETY: the fixed SDT header is reserved, mapped where required, and
    // disjoint from every range that must remain independently readable.
    let header = unsafe {
        core::slice::from_raw_parts(address as *const u8, HEADER_SIZE as usize)
    };
    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
    if !(HEADER_SIZE..=MAX_TABLE_SIZE).contains(&length) || length > max_table_length {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI table length is invalid or exceeds the bounded handoff audit budget.\r\n\0"));
    }
    if !memory_map.range_is_fully_reserved(address, length)
        .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed ACPI table range in final memory map.\r\n\0"))?
    {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI table is not fully reserved in the final memory map.\r\n\0"));
    }
    #[cfg(target_arch = "x86_64")]
    if high_half_image && !range_is_identity_mapped_ram(memory_map, address, length)? {
        return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ACPI table is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
    }
    if overlaps_protected_storage(length) {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI table overlaps framebuffer, platform metadata, or handoff storage.\r\n\0"));
    }

    // SAFETY: the complete advertised table range is bounded, reserved, mapped
    // where required, and disjoint from protected storage as checked above.
    let complete = unsafe { core::slice::from_raw_parts(address as *const u8, length as usize) };
    let info = archiaos_bootloader::acpi::parse_sdt_header(complete)
        .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ACPI table header or checksum during final memory-map audit.\r\n\0"))?;
    if info.length as u64 != length {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: inconsistent ACPI table length during final memory-map audit.\r\n\0"));
    }
    Ok((info.signature, length))
}

/// Read the FADT's indirect pointers after its SDT checksum and length were validated.
fn acpi_fadt_indirect_pointers(table: &[u8]) -> Result<(u64, u64), ()> {
    if table.len() < 44 || table.get(0..4) != Some(&b"FACP"[..]) {
        return Err(());
    }
    let legacy_facs = u32::from_le_bytes([table[36], table[37], table[38], table[39]]) as u64;
    let legacy_dsdt = u32::from_le_bytes([table[40], table[41], table[42], table[43]]) as u64;
    let extended_facs = if table.len() >= 140 {
        u64::from_le_bytes([
            table[132], table[133], table[134], table[135],
            table[136], table[137], table[138], table[139],
        ])
    } else {
        0
    };
    let extended_dsdt = if table.len() >= 148 {
        u64::from_le_bytes([
            table[140], table[141], table[142], table[143],
            table[144], table[145], table[146], table[147],
        ])
    } else {
        0
    };
    let facs = if extended_facs != 0 { extended_facs } else { legacy_facs };
    let dsdt = if extended_dsdt != 0 { extended_dsdt } else { legacy_dsdt };
    if dsdt == 0 {
        return Err(());
    }
    Ok((facs, dsdt))
}

/// Validate an ACPI FACS range. Unlike an SDT, FACS has no checksum or 36-byte header.
fn audit_acpi_facs_range(
    memory_map: &UefiMemoryMap<'_>,
    address: u64,
    high_half_image: bool,
    framebuffer_base: u64,
    framebuffer_size: u64,
    protected_ranges: &[(u64, u64)],
    handoff_ranges: &[(u64, u64)],
    max_table_length: u64,
) -> Result<u64, BootError> {
    const FACS_HEADER_SIZE: u64 = 8;
    const FACS_MIN_SIZE: u64 = 64;
    const FACS_MAX_SIZE: u64 = archiaos_bootloader::acpi::MAX_SDT_SIZE as u64;
    let overlaps_protected_storage = |length: u64| {
        physical_ranges_overlap(address, length, framebuffer_base, framebuffer_size)
            || protected_ranges.iter().any(|&(start, size)| {
                physical_ranges_overlap(address, length, start, size)
            })
            || platform_range_overlaps_handoff_storage(address, length, handoff_ranges)
    };

    if address == 0 {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: FADT references a null FACS address.\r\n\0"));
    }
    if !memory_map.range_is_fully_reserved(address, FACS_HEADER_SIZE)
        .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed FACS header range in final memory map.\r\n\0"))?
    {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: FACS header is not fully reserved in the final memory map.\r\n\0"));
    }
    #[cfg(target_arch = "x86_64")]
    if high_half_image && !range_is_identity_mapped_ram(memory_map, address, FACS_HEADER_SIZE)? {
        return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: FACS header is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
    }
    if overlaps_protected_storage(FACS_HEADER_SIZE) {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: FACS header overlaps framebuffer, platform metadata, or handoff storage.\r\n\0"));
    }

    // SAFETY: the prefix is reserved, mapped where required, and disjoint from protected ranges.
    let header = unsafe { core::slice::from_raw_parts(address as *const u8, FACS_HEADER_SIZE as usize) };
    if header.get(0..4) != Some(&b"FACS"[..]) {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: FADT FACS pointer has an invalid signature.\r\n\0"));
    }
    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
    if !(FACS_MIN_SIZE..=FACS_MAX_SIZE).contains(&length) || length > max_table_length {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: FACS length is invalid or exceeds the bounded handoff audit budget.\r\n\0"));
    }
    if !memory_map.range_is_fully_reserved(address, length)
        .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed FACS range in final memory map.\r\n\0"))?
    {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: FACS is not fully reserved in the final memory map.\r\n\0"));
    }
    #[cfg(target_arch = "x86_64")]
    if high_half_image && !range_is_identity_mapped_ram(memory_map, address, length)? {
        return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: FACS is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
    }
    if overlaps_protected_storage(length) {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: FACS overlaps framebuffer, platform metadata, or another ACPI table.\r\n\0"));
    }
    Ok(length)
}

/// Ensure firmware platform-table storage remains reserved in the final handoff map.
/// These pointers are consumed after ExitBootServices, so they must never be reclassified
/// as conventional RAM that the physical allocator could hand to another subsystem.
fn audit_platform_table_ranges(
    memory_map: &UefiMemoryMap<'_>,
    acpi_rsdp: u64,
    device_tree: u64,
    high_half_image: bool,
    framebuffer_base: u64,
    framebuffer_size: u64,
    handoff_ranges: &[(u64, u64)],
    configuration_table_range: Option<(u64, u64)>,
) -> Result<(), BootError> {
    #[cfg(target_arch = "aarch64")]
    let _ = high_half_image;
    // The kernel may follow the ACPI root table after ExitBootServices, so retain
    // and audit its address separately from the RSDP that points to it.
    let mut validated_acpi_root: Option<(u64, bool)> = None;
    let mut validated_rsdp_range: Option<(u64, u64)> = None;
    let mut validated_device_tree_range: Option<(u64, u64)> = None;
    if let Some((start, length)) = configuration_table_range {
        if platform_range_overlaps_handoff_storage(start, length, handoff_ranges)
            || physical_ranges_overlap(start, length, framebuffer_base, framebuffer_size)
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: configuration-table array overlaps handoff storage or framebuffer.\r\n\0"));
        }
    }
    if acpi_rsdp != 0 {
        // ACPI 1.0 RSDP is only 20 bytes. Validate that prefix first so a valid
        // legacy table at a descriptor boundary is not forced to reserve 24 bytes.
        const RSDP_V1_PREFIX: u64 = 20;
        if !memory_map.range_is_fully_reserved(acpi_rsdp, RSDP_V1_PREFIX)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed RSDP prefix in final memory map.\r\n\0"))?
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP prefix is not fully reserved in the final memory map.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if high_half_image
            && !range_is_identity_mapped_ram(memory_map, acpi_rsdp, RSDP_V1_PREFIX)?
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ACPI RSDP prefix is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
        }
        if physical_ranges_overlap(acpi_rsdp, RSDP_V1_PREFIX, framebuffer_base, framebuffer_size) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP overlaps the GOP framebuffer.\r\n\0"));
        }
        if platform_range_overlaps_handoff_storage(acpi_rsdp, RSDP_V1_PREFIX, handoff_ranges) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP prefix overlaps handoff storage.\r\n\0"));
        }

        let rsdp = acpi_rsdp as *const u8;
        // SAFETY: the 20-byte legacy prefix is reserved and mapped, as checked above.
        let signature = unsafe { core::slice::from_raw_parts(rsdp, 8) };
        if !rsdp_signature_is_valid(signature) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ACPI RSDP signature during final memory-map audit.\r\n\0"));
        }
        let revision = unsafe { ptr::read(rsdp.add(15)) };
        let length = if revision < 2 {
            RSDP_V1_PREFIX
        } else {
            // The ACPI 2.0+ length field occupies bytes 20..24. Prove this
            // extension is safe before reading it; do not assume v1 tables have it.
            let length_field_start = acpi_rsdp.checked_add(RSDP_V1_PREFIX)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: RSDP length-field address overflow.\r\n\0"))?;
            if !memory_map.range_is_fully_reserved(length_field_start, 4)
                .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed RSDP length field in final memory map.\r\n\0"))?
            {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP length field is not fully reserved in the final memory map.\r\n\0"));
            }
            #[cfg(target_arch = "x86_64")]
            if high_half_image
                && !range_is_identity_mapped_ram(memory_map, length_field_start, 4)?
            {
                return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ACPI RSDP length field is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
            }
            if physical_ranges_overlap(length_field_start, 4, framebuffer_base, framebuffer_size) {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP length field overlaps the GOP framebuffer.\r\n\0"));
            }
            if platform_range_overlaps_handoff_storage(length_field_start, 4, handoff_ranges) {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP length field overlaps handoff storage.\r\n\0"));
            }
            // SAFETY: bytes 20..24 are separately proven reserved and mapped above.
            let field = unsafe { core::slice::from_raw_parts(rsdp.add(20), 4) };
            u32::from_le_bytes([field[0], field[1], field[2], field[3]]) as u64
        };
        if !rsdp_length_is_valid(revision, length) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid RSDP size during final memory-map audit.\r\n\0"));
        }
        if !memory_map.range_is_fully_reserved(acpi_rsdp, length)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed RSDP range in final memory map.\r\n\0"))?
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP storage is not fully reserved in the final memory map.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if high_half_image
            && !range_is_identity_mapped_ram(memory_map, acpi_rsdp, length)?
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ACPI RSDP is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
        }
        if physical_ranges_overlap(acpi_rsdp, length, framebuffer_base, framebuffer_size) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP overlaps the GOP framebuffer.\r\n\0"));
        }
        if platform_range_overlaps_handoff_storage(acpi_rsdp, length, handoff_ranges) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP overlaps handoff storage.\r\n\0"));
        }
        // SAFETY: the complete length is bounded, reserved, and identity-mapped
        // where required, as checked above.
        let complete = unsafe { core::slice::from_raw_parts(rsdp, length as usize) };
        if !rsdp_checksums_are_valid(complete, revision, length) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ACPI RSDP checksum during final memory-map audit.\r\n\0"));
        }
        let rsdp_info = archiaos_bootloader::acpi::parse_rsdp(complete)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ACPI root pointer metadata during final memory-map audit.\r\n\0"))?;
        let (root_address, root_is_xsdt) = if rsdp_info.xsdt_address != 0 {
            (rsdp_info.xsdt_address, true)
        } else {
            (rsdp_info.rsdt_address as u64, false)
        };
        if root_address == 0 {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP does not reference an RSDT or XSDT.
 "));
        }
        validated_acpi_root = Some((root_address, root_is_xsdt));
        validated_rsdp_range = Some((acpi_rsdp, length));
    }

    if device_tree != 0 {
        const FDT_FIXED_PREFIX: u64 = 8;
        if !memory_map.range_is_fully_reserved(device_tree, FDT_FIXED_PREFIX)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed Device Tree prefix in final memory map.\r\n\0"))?
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree prefix is not fully reserved in the final memory map.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if high_half_image
            && !range_is_identity_mapped_ram(memory_map, device_tree, FDT_FIXED_PREFIX)?
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: Device Tree prefix is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
        }
        if physical_ranges_overlap(device_tree, FDT_FIXED_PREFIX, framebuffer_base, framebuffer_size) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree header overlaps the GOP framebuffer.\r\n\0"));
        }
        if platform_range_overlaps_handoff_storage(device_tree, FDT_FIXED_PREFIX, handoff_ranges) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree header overlaps handoff storage.\r\n\0"));
        }

        let dtb = device_tree as *const u8;
        // SAFETY: the fixed prefix is reserved and mapped, as checked above.
        let header = unsafe { core::slice::from_raw_parts(dtb, FDT_FIXED_PREFIX as usize) };
        let length = validated_fdt_prefix_length(header)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid Device Tree header during final memory-map audit.\r\n\0"))?;
        if !memory_map.range_is_fully_reserved(device_tree, length)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed Device Tree range in final memory map.\r\n\0"))?
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree storage is not fully reserved in the final memory map.\r\n\0"));
        }
        if let Some((rsdp_start, rsdp_length)) = validated_rsdp_range {
            if physical_ranges_overlap(device_tree, length, rsdp_start, rsdp_length) {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP overlaps the Device Tree blob.\r\n\0"));
            }
        }
        #[cfg(target_arch = "x86_64")]
        if high_half_image
            && !range_is_identity_mapped_ram(memory_map, device_tree, length)?
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: Device Tree is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
        }
        if physical_ranges_overlap(device_tree, length, framebuffer_base, framebuffer_size) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree overlaps the GOP framebuffer.\r\n\0"));
        }
        if platform_range_overlaps_handoff_storage(device_tree, length, handoff_ranges) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree overlaps handoff storage.\r\n\0"));
        }
        // Firmware data may have changed since initial discovery. Re-parse the
        // entire bounded tree while its full range is still proven readable.
        // SAFETY: length was validated, the complete range is reserved, and
        // high-half x86-64 mapping coverage was checked above.
        let complete_tree = unsafe {
            core::slice::from_raw_parts(device_tree as *const u8, length as usize)
        };
        if archiaos_bootloader::device_tree::validate_tree(complete_tree).is_err() {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid Device Tree contents during final memory-map audit.\r\n\0"));
        }
        validated_device_tree_range = Some((device_tree, length));
    }

    // ACPI consumers can follow the RSDT/XSDT and its child SDTs long after
    // firmware services end. Audit every directly referenced table so none can
    // be reclaimed as conventional RAM or alias another retained handoff object.
    if let Some((root_address, root_is_xsdt)) = validated_acpi_root {
        const MAX_ACPI_ROOT_ENTRIES: usize = 256;
        const MAX_ACPI_AUDITED_BYTES: u64 = 64 * 1024 * 1024;
        let mut protected = [(0u64, 0u64); 3];
        let mut protected_count = 0usize;
        if let Some(range) = validated_rsdp_range {
            protected[protected_count] = range;
            protected_count += 1;
        }
        if let Some(range) = validated_device_tree_range {
            protected[protected_count] = range;
            protected_count += 1;
        }
        if let Some(range) = configuration_table_range {
            protected[protected_count] = range;
            protected_count += 1;
        }

        let (root_signature, root_length) = audit_acpi_sdt_range(
            memory_map,
            root_address,
            high_half_image,
            framebuffer_base,
            framebuffer_size,
            &protected[..protected_count],
            handoff_ranges,
            MAX_ACPI_AUDITED_BYTES,
        )?;
        let expected_signature = if root_is_xsdt { *b"XSDT" } else { *b"RSDT" };
        if root_signature != expected_signature {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI root table signature does not match the RSDP pointer.
 "));
        }

        // SAFETY: audit_acpi_sdt_range validated the complete root table range
        // and checksum before any entry is read.
        let root_table = unsafe {
            core::slice::from_raw_parts(root_address as *const u8, root_length as usize)
        };
        let entry_size = if root_is_xsdt { 8usize } else { 4usize };
        let entries_bytes = root_table.len() - archiaos_bootloader::acpi::SDT_HEADER_SIZE;
        if entries_bytes % entry_size != 0 {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI root table has a truncated entry array.
 "));
        }
        let entry_count = entries_bytes / entry_size;
        if entry_count > MAX_ACPI_ROOT_ENTRIES {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ACPI root table contains too many entries for a bounded handoff audit.
 "));
        }

        let mut audited_bytes = root_length;
        // Protect already-audited child tables too, so duplicate pointers and
        // partial table aliases are rejected before dereferencing the next one.
        // Each root entry can add one SDT, and a FADT can add a DSDT plus a FACS.
        let mut child_protected = [(0u64, 0u64); MAX_ACPI_ROOT_ENTRIES * 3 + 4];
        child_protected[..protected_count].copy_from_slice(&protected[..protected_count]);
        child_protected[protected_count] = (root_address, root_length);
        let mut child_protected_count = protected_count + 1;
        for entry_index in 0..entry_count {
            let offset = archiaos_bootloader::acpi::SDT_HEADER_SIZE + entry_index * entry_size;
            let table_address = if root_is_xsdt {
                u64::from_le_bytes([
                    root_table[offset], root_table[offset + 1],
                    root_table[offset + 2], root_table[offset + 3],
                    root_table[offset + 4], root_table[offset + 5],
                    root_table[offset + 6], root_table[offset + 7],
                ])
            } else {
                u32::from_le_bytes([
                    root_table[offset], root_table[offset + 1],
                    root_table[offset + 2], root_table[offset + 3],
                ]) as u64
            };
            let remaining_budget = MAX_ACPI_AUDITED_BYTES.checked_sub(audited_bytes)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI audit byte budget underflow.\r\n\0"))?;
            let (child_signature, child_length) = audit_acpi_sdt_range(
                memory_map,
                table_address,
                high_half_image,
                framebuffer_base,
                framebuffer_size,
                &child_protected[..child_protected_count],
                handoff_ranges,
                remaining_budget,
            )?;
            // Protect the FADT itself before following any of its firmware-controlled pointers.
            child_protected[child_protected_count] = (table_address, child_length);
            child_protected_count += 1;
            audited_bytes = audited_bytes.checked_add(child_length)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI audited table size overflow.\r\n\0"))?;

            if child_signature == *b"FACP" {
                // SAFETY: the FADT SDT has already passed reservation, mapping,
                // bounds, checksum, and overlap validation.
                let fadt = unsafe { core::slice::from_raw_parts(table_address as *const u8, child_length as usize) };
                let (facs_address, dsdt_address) = acpi_fadt_indirect_pointers(fadt)
                    .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: FADT has no valid DSDT pointer or a truncated pointer field.\r\n\0"))?;

                let remaining_budget = MAX_ACPI_AUDITED_BYTES.checked_sub(audited_bytes)
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI audit byte budget underflow before DSDT.\r\n\0"))?;
                let (dsdt_signature, dsdt_length) = audit_acpi_sdt_range(
                    memory_map,
                    dsdt_address,
                    high_half_image,
                    framebuffer_base,
                    framebuffer_size,
                    &child_protected[..child_protected_count],
                    handoff_ranges,
                    remaining_budget,
                )?;
                if dsdt_signature != *b"DSDT" {
                    return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: FADT DSDT pointer does not reference a DSDT.\r\n\0"));
                }
                child_protected[child_protected_count] = (dsdt_address, dsdt_length);
                child_protected_count += 1;
                audited_bytes = audited_bytes.checked_add(dsdt_length)
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI audited table size overflow after DSDT.\r\n\0"))?;

                if facs_address != 0 {
                    let remaining_budget = MAX_ACPI_AUDITED_BYTES.checked_sub(audited_bytes)
                        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI audit byte budget underflow before FACS.\r\n\0"))?;
                    let facs_length = audit_acpi_facs_range(
                        memory_map,
                        facs_address,
                        high_half_image,
                        framebuffer_base,
                        framebuffer_size,
                        &child_protected[..child_protected_count],
                        handoff_ranges,
                        remaining_budget,
                    )?;
                    child_protected[child_protected_count] = (facs_address, facs_length);
                    child_protected_count += 1;
                    audited_bytes = audited_bytes.checked_add(facs_length)
                        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI audited table size overflow after FACS.\r\n\0"))?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(target_arch = "x86_64")]
fn require_identity_mapped_ram(
    memory_map: &UefiMemoryMap<'_>,
    start: u64,
    length: u64,
    message: &'static [u8],
) -> Result<(), BootError> {
    if !range_is_identity_mapped_ram(memory_map, start, length)? {
        return Err(error(EFI_UNSUPPORTED, message));
    }
    Ok(())
}

fn platform_tables(
    system_table: *mut SystemTable,
    memory_map: &UefiMemoryMap<'_>,
    high_half_image: bool,
    framebuffer_base: u64,
    framebuffer_size: u64,
) -> Result<(u64, u64, Option<(u64, u64)>), BootError> {
    if system_table.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: missing system table during platform-table discovery.\r\n\0"));
    }
    // Snapshot the configuration-table array metadata, then prove the complete
    // array is reserved in the final map before reading any firmware entry.
    let (count, tables) = unsafe {
        ((*system_table).number_of_table_entries, (*system_table).configuration_table)
    };
    if count == 0 {
        return Ok((0, 0, None));
    }
    if !configuration_table_range_is_bounded(tables as usize, count) {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid UEFI configuration-table array bounds.\r\n\0"));
    }
    let bytes = (count as u64)
        .checked_mul(core::mem::size_of::<ConfigurationTable>() as u64)
        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: configuration-table array size overflow.\r\n\0"))?;
    let address = tables as u64;
    if !memory_map.range_is_fully_reserved(address, bytes)
        .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed configuration-table array in final memory map.\r\n\0"))?
    {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: configuration-table array is not fully reserved in the final memory map.\r\n\0"));
    }
    #[cfg(target_arch = "x86_64")]
    if high_half_image && !range_is_identity_mapped_ram(memory_map, address, bytes)? {
        return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: configuration-table array is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
    }
    if physical_ranges_overlap(address, bytes, framebuffer_base, framebuffer_size) {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: configuration-table array overlaps the GOP framebuffer.\r\n\0"));
    }
    let _ = high_half_image;
    // Use the exact metadata snapshot whose full range was just validated. Do not
    // reread count or pointer from SystemTable between validation and lookup.
    let acpi20 = configuration_table(tables, count, &ACPI_20_TABLE_GUID);
    let acpi10 = configuration_table(tables, count, &ACPI_10_TABLE_GUID);
    // Do not dereference firmware-controlled table pointers here: the final memory
    // map is not available yet. The handoff audit validates every prefix and complete
    // range before parsing the RSDP or Device Tree.
    let rsdp = if !acpi20.is_null() {
        platform_table_address(acpi20)
    } else {
        platform_table_address(acpi10)
    };

    let dtb = configuration_table(tables, count, &EFI_DTB_TABLE_GUID);
    let dtb = platform_table_address(dtb);
    Ok((rsdp, dtb, Some((address, bytes))))
}

/// Capture platform metadata, validate the final firmware memory map, terminate
/// boot services, and transfer control using the versioned BootInfo contract.
