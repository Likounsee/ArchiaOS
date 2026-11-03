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

