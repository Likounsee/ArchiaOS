#[cfg(test)]
mod fdt_handoff_header_tests {
    use super::validated_fdt_prefix_length;

    fn header(magic: u32, length: u32) -> [u8; 8] {
        let mut bytes = [0; 8];
        bytes[..4].copy_from_slice(&magic.to_be_bytes());
        bytes[4..].copy_from_slice(&length.to_be_bytes());
        bytes
    }

    #[test]
    fn accepts_valid_fdt_prefix() {
        let bytes = header(
            archiaos_bootloader::device_tree::FDT_MAGIC,
            archiaos_bootloader::device_tree::HEADER_SIZE as u32,
        );
        assert_eq!(
            validated_fdt_prefix_length(&bytes),
            Some(archiaos_bootloader::device_tree::HEADER_SIZE as u64)
        );
    }

    #[test]
    fn rejects_wrong_magic_short_prefix_and_invalid_lengths() {
        assert_eq!(validated_fdt_prefix_length(&[0; 7]), None);
        assert_eq!(validated_fdt_prefix_length(&header(0, 40)), None);
        assert_eq!(
            validated_fdt_prefix_length(&header(
                archiaos_bootloader::device_tree::FDT_MAGIC,
                (archiaos_bootloader::device_tree::HEADER_SIZE - 1) as u32,
            )),
            None
        );
        assert_eq!(
            validated_fdt_prefix_length(&header(
                archiaos_bootloader::device_tree::FDT_MAGIC,
                archiaos_bootloader::device_tree::MAX_TOTAL_SIZE as u32 + 1,
            )),
            None
        );
    }
}

#[cfg(test)]
mod rsdp_handoff_signature_tests {
    use super::rsdp_signature_is_valid;

    #[test]
    fn accepts_only_the_acpi_rsdp_signature() {
        assert!(rsdp_signature_is_valid(archiaos_bootloader::acpi::RSDP_SIGNATURE));
        assert!(!rsdp_signature_is_valid(b"BAD PTR "));
        assert!(!rsdp_signature_is_valid(b"RSD PTR"));
        assert!(!rsdp_signature_is_valid(b""));
    }
}

#[cfg(test)]
mod rsdp_handoff_checksum_tests {
    use super::rsdp_checksums_are_valid;

    #[test]
    fn accepts_zero_sum_legacy_and_extended_rsdp_tables() {
        assert!(rsdp_checksums_are_valid(&[0; 20], 0, 20));
        assert!(rsdp_checksums_are_valid(&[0; 36], 2, 36));
    }

    #[test]
    fn rejects_bad_legacy_or_extended_checksums() {
        let mut legacy = [0; 20];
        legacy[19] = 1;
        assert!(!rsdp_checksums_are_valid(&legacy, 0, 20));

        let mut extended_legacy_bad = [0; 36];
        extended_legacy_bad[0] = 1;
        assert!(!rsdp_checksums_are_valid(&extended_legacy_bad, 2, 36));

        let mut extended_full_bad = [0; 36];
        extended_full_bad[35] = 1;
        assert!(!rsdp_checksums_are_valid(&extended_full_bad, 2, 36));
    }

    #[test]
    fn rejects_mismatched_lengths_and_truncated_tables() {
        assert!(!rsdp_checksums_are_valid(&[0; 19], 0, 19));
        assert!(!rsdp_checksums_are_valid(&[0; 20], 2, 20));
        assert!(!rsdp_checksums_are_valid(&[0; 36], 2, 37));
    }
}

#[cfg(test)]
mod rsdp_handoff_length_tests {
    use super::rsdp_length_is_valid;

    #[test]
    fn accepts_legacy_and_complete_extended_rsdp_lengths() {
        assert!(rsdp_length_is_valid(0, 20));
        assert!(rsdp_length_is_valid(2, 36));
        assert!(rsdp_length_is_valid(
            2,
            archiaos_bootloader::acpi::MAX_RSDP_SIZE as u64
        ));
    }

    #[test]
    fn rejects_truncated_extended_rsdp_and_oversized_lengths() {
        assert!(!rsdp_length_is_valid(1, 20));
        assert!(!rsdp_length_is_valid(1, 36));
        assert!(!rsdp_length_is_valid(2, 20));
        assert!(!rsdp_length_is_valid(2, 35));
        assert!(!rsdp_length_is_valid(
            2,
            archiaos_bootloader::acpi::MAX_RSDP_SIZE as u64 + 1
        ));
        assert!(!rsdp_length_is_valid(0, 19));
    }
}

#[cfg(test)]
mod configuration_table_bounds_tests {
    use super::configuration_table_range_is_bounded;

    #[test]
    fn accepts_small_bounded_configuration_table_arrays() {
        assert!(configuration_table_range_is_bounded(0x1000, 1));
        assert!(configuration_table_range_is_bounded(0x1000, 4096));
    }

    #[test]
    fn rejects_missing_oversized_and_wrapping_configuration_table_arrays() {
        assert!(!configuration_table_range_is_bounded(0, 1));
        assert!(!configuration_table_range_is_bounded(0x1000, 0));
        assert!(!configuration_table_range_is_bounded(0x1000, 4097));
        assert!(!configuration_table_range_is_bounded(usize::MAX - 7, 1));
    }
}

#[cfg(test)]
mod page_rounded_physical_range_tests {
    use super::page_rounded_physical_range;

    #[test]
    fn rounds_unaligned_ranges_out_to_all_touched_pages() {
        assert_eq!(page_rounded_physical_range(0x1234, 0x20), Some((0x1000, 0x2000)));
        assert_eq!(page_rounded_physical_range(0x1ff0, 0x30), Some((0x1000, 0x3000)));
        assert_eq!(page_rounded_physical_range(0x2000, 0x1000), Some((0x2000, 0x3000)));
    }

    #[test]
    fn rejects_empty_and_overflowing_ranges() {
        assert_eq!(page_rounded_physical_range(0x1000, 0), None);
        assert_eq!(page_rounded_physical_range(u64::MAX, 1), None);
        assert_eq!(page_rounded_physical_range(u64::MAX - 0xfff, 0x1000), None);
    }
}

#[cfg(test)]
mod platform_range_handoff_tests {
    use super::platform_range_overlaps_handoff_storage;

    const HANDOFF_RANGES: [(u64, u64); 6] = [
        (0x1000, 0x1000), // BootInfo
        (0x3000, 0x1000), // final memory map
        (0, 0),           // optional loader image
        (0x8000, 0x1000), // page-table arena
        (0xa000, 0x2000), // page-rounded kernel text
        (0xc000, 0x1000), // page-rounded kernel data
    ];

    #[test]
    fn rejects_platform_tables_that_alias_handoff_storage() {
        assert!(platform_range_overlaps_handoff_storage(
            0x1000, 0x1000, &HANDOFF_RANGES
        ));
        assert!(platform_range_overlaps_handoff_storage(
            0x1800, 0x100, &HANDOFF_RANGES
        ));
        assert!(platform_range_overlaps_handoff_storage(
            0x0ff0, 0x20, &HANDOFF_RANGES
        ));
        assert!(platform_range_overlaps_handoff_storage(
            0x8000, 0x1000, &HANDOFF_RANGES
        ));
        assert!(platform_range_overlaps_handoff_storage(
            0xaff0, 0x40, &HANDOFF_RANGES
        ));
        assert!(platform_range_overlaps_handoff_storage(
            0xc800, 0x100, &HANDOFF_RANGES
        ));
    }

    #[test]
    fn allows_disjoint_adjacent_and_empty_optional_ranges() {
        assert!(!platform_range_overlaps_handoff_storage(
            0x2000, 0x1000, &HANDOFF_RANGES
        ));
        assert!(!platform_range_overlaps_handoff_storage(
            0x4000, 0x1000, &HANDOFF_RANGES
        ));
        assert!(!platform_range_overlaps_handoff_storage(
            0x9000, 0x1000, &HANDOFF_RANGES
        ));
        assert!(!platform_range_overlaps_handoff_storage(
            0xd000, 0x1000, &HANDOFF_RANGES
        ));
        assert!(!platform_range_overlaps_handoff_storage(
            0x1000, 0, &HANDOFF_RANGES
        ));
    }
}

#[cfg(test)]
mod physical_range_overlap_tests {
    use super::physical_ranges_overlap;

    #[test]
    fn detects_partial_contained_and_exact_overlaps() {
        assert!(physical_ranges_overlap(0x1000, 0x3000, 0x2000, 0x1000));
        assert!(physical_ranges_overlap(0x2000, 0x1000, 0x1000, 0x3000));
        assert!(physical_ranges_overlap(0x1000, 0x1000, 0x1000, 0x1000));
        assert!(physical_ranges_overlap(0x1000, 0x3000, 0x1000, 0x3000));
    }

    #[test]
    fn allows_disjoint_adjacent_and_empty_ranges() {
        assert!(!physical_ranges_overlap(0x1000, 0x1000, 0x2000, 0x1000));
        assert!(!physical_ranges_overlap(0x3000, 0x1000, 0x1000, 0x1000));
        assert!(!physical_ranges_overlap(0x1000, 0, 0x1000, 0x1000));
        assert!(!physical_ranges_overlap(0x1000, 0x1000, 0x1000, 0));
    }

    #[test]
    fn treats_wrapping_ranges_as_conflicting() {
        assert!(physical_ranges_overlap(u64::MAX - 3, 8, 0x1000, 0x1000));
        assert!(physical_ranges_overlap(0x1000, 0x1000, u64::MAX - 3, 8));
    }
}

#[cfg(test)]
mod acpi_fadt_indirect_pointer_tests {
    use super::acpi_fadt_indirect_pointers;

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn reads_legacy_facs_and_dsdt_pointers() {
        let mut fadt = [0u8; 44];
        fadt[0..4].copy_from_slice(b"FACP");
        put_u32(&mut fadt, 36, 0x1234_0000);
        put_u32(&mut fadt, 40, 0x1234_1000);
        assert_eq!(acpi_fadt_indirect_pointers(&fadt), Ok((0x1234_0000, 0x1234_1000)));
    }

    #[test]
    fn prefers_nonzero_extended_pointers_and_falls_back_individually() {
        let mut fadt = [0u8; 148];
        fadt[0..4].copy_from_slice(b"FACP");
        put_u32(&mut fadt, 36, 0x1000);
        put_u32(&mut fadt, 40, 0x2000);
        put_u64(&mut fadt, 132, 0x1_0000_1000);
        put_u64(&mut fadt, 140, 0);
        assert_eq!(acpi_fadt_indirect_pointers(&fadt), Ok((0x1_0000_1000, 0x2000)));
        put_u64(&mut fadt, 140, 0x1_0000_2000);
        assert_eq!(acpi_fadt_indirect_pointers(&fadt), Ok((0x1_0000_1000, 0x1_0000_2000)));
    }

    #[test]
    fn rejects_truncated_fadt_wrong_signature_and_missing_dsdt() {
        assert!(acpi_fadt_indirect_pointers(&[0u8; 43]).is_err());
        let mut fadt = [0u8; 44];
        assert!(acpi_fadt_indirect_pointers(&fadt).is_err());
        fadt[0..4].copy_from_slice(b"FACP");
        assert!(acpi_fadt_indirect_pointers(&fadt).is_err());
        put_u32(&mut fadt, 40, 0x2000);
        assert_eq!(acpi_fadt_indirect_pointers(&fadt), Ok((0, 0x2000)));
    }
}


#[cfg(test)]
mod configuration_table_lookup_tests {
    use super::{configuration_table, ConfigurationTable, Guid};
    use core::ffi::c_void;

    const FIRST_GUID: Guid = Guid {
        data1: 0x1234,
        data2: 0x5678,
        data3: 0x9abc,
        data4: [1, 2, 3, 4, 5, 6, 7, 8],
    };
    const SECOND_GUID: Guid = Guid {
        data1: 0x4321,
        data2: 0x8765,
        data3: 0xcba9,
        data4: [8, 7, 6, 5, 4, 3, 2, 1],
    };

    #[test]
    fn finds_requested_guid_in_validated_snapshot() {
        let first = 0x1234usize as *mut c_void;
        let second = 0x5678usize as *mut c_void;
        let entries = [
            ConfigurationTable { vendor_guid: FIRST_GUID, vendor_table: first },
            ConfigurationTable { vendor_guid: SECOND_GUID, vendor_table: second },
        ];
        assert_eq!(configuration_table(entries.as_ptr() as *mut _, entries.len(), &SECOND_GUID), second);
    }

    #[test]
    fn missing_or_null_entries_are_not_returned() {
        let entries = [
            ConfigurationTable { vendor_guid: FIRST_GUID, vendor_table: core::ptr::null_mut() },
        ];
        assert!(configuration_table(entries.as_ptr() as *mut _, entries.len(), &FIRST_GUID).is_null());
        assert!(configuration_table(entries.as_ptr() as *mut _, entries.len(), &SECOND_GUID).is_null());
        assert!(configuration_table(core::ptr::null_mut(), 1, &FIRST_GUID).is_null());
        assert!(configuration_table(entries.as_ptr() as *mut _, 0, &FIRST_GUID).is_null());
    }

    #[test]
    fn reads_unaligned_configuration_table_entries_safely() {
        use core::{mem, ptr};

        let first = 0x1234usize as *mut c_void;
        let second = 0x5678usize as *mut c_void;
        let entry_count = 2;
        let mut storage = [0u8; 1 + 2 * mem::size_of::<ConfigurationTable>()];
        // Deliberately offset the table by one byte to exercise unaligned firmware data.
        let table = unsafe { storage.as_mut_ptr().add(1).cast::<ConfigurationTable>() };
        unsafe {
            ptr::write_unaligned(
                table,
                ConfigurationTable { vendor_guid: FIRST_GUID, vendor_table: first },
            );
            ptr::write_unaligned(
                table.cast::<u8>().add(mem::size_of::<ConfigurationTable>()).cast::<ConfigurationTable>(),
                ConfigurationTable { vendor_guid: SECOND_GUID, vendor_table: second },
            );
        }

        assert_eq!(configuration_table(table, entry_count, &FIRST_GUID), first);
        assert_eq!(configuration_table(table, entry_count, &SECOND_GUID), second);
    }

    #[test]
    fn rejects_unbounded_configuration_table_counts() {
        let table = 0x1000usize as *mut ConfigurationTable;
        assert!(configuration_table(table, 4097, &FIRST_GUID).is_null());
        assert!(configuration_table(table, usize::MAX, &FIRST_GUID).is_null());
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod loaded_image_range_tests {
    use super::rounded_loader_image_range;

    #[test]
    fn rounds_valid_loader_image_to_page_boundaries() {
        assert_eq!(rounded_loader_image_range(0x1234, 0x100), Some((0x1000, 0x2000)));
        assert_eq!(rounded_loader_image_range(0x1fff, 2), Some((0x1000, 0x3000)));
        // Firmware may place the loader above 4 GiB, within the low-canonical window.
        assert_eq!(
            rounded_loader_image_range(0x1_0000_1234, 0x100),
            Some((0x1_0000_1000, 0x1_0000_2000))
        );
    }

    #[test]
    fn rejects_empty_overflowing_and_noncanonical_loader_ranges() {
        assert_eq!(rounded_loader_image_range(0, 0x1000), None);
        assert_eq!(rounded_loader_image_range(0x1000, 0), None);
        assert_eq!(rounded_loader_image_range(u64::MAX - 0x10, 0x20), None);
        assert_eq!(rounded_loader_image_range((1u64 << 39) - 0x1000, 0x1001), None);
    }
}

#[cfg(test)]
mod gop_mode_metadata_tests {
    use super::{
        gop_mode_information_is_supported, gop_mode_metadata_is_valid,
        GraphicsOutputModeInformation,
    };

    #[test]
    fn accepts_complete_current_gop_mode_information() {
        assert!(gop_mode_metadata_is_valid(
            1,
            0,
            core::mem::size_of::<GraphicsOutputModeInformation>()
        ));
        assert!(gop_mode_metadata_is_valid(
            4,
            3,
            core::mem::size_of::<GraphicsOutputModeInformation>() + 8
        ));
    }

    #[test]
    fn rejects_invalid_mode_indices_and_truncated_information() {
        assert!(!gop_mode_metadata_is_valid(0, 0, core::mem::size_of::<GraphicsOutputModeInformation>()));
        assert!(!gop_mode_metadata_is_valid(2, 2, core::mem::size_of::<GraphicsOutputModeInformation>()));
        assert!(!gop_mode_metadata_is_valid(1, 0, core::mem::size_of::<GraphicsOutputModeInformation>() - 1));
    }

    #[test]
    fn accepts_only_the_known_gop_information_layout() {
        assert!(gop_mode_information_is_supported(0));
        assert!(!gop_mode_information_is_supported(1));
        assert!(!gop_mode_information_is_supported(u32::MAX));
    }
}


#[cfg(all(test, target_arch = "x86_64"))]
mod uefi_memory_type_classification_tests {
    use super::uefi_memory_type_is_ram;

    #[test]
    fn maps_only_memory_types_explicitly_treated_as_ram() {
        for memory_type in [1, 2, 3, 4, 5, 6, 7, 9, 10, 14] {
            assert!(
                uefi_memory_type_is_ram(memory_type),
                "UEFI memory type {memory_type} should be mapped as RAM"
            );
        }
    }

    #[test]
    fn excludes_reserved_unusable_mmio_and_unknown_memory_types() {
        for memory_type in [0, 8, 11, 12, 13, 15, u32::MAX] {
            assert!(
                !uefi_memory_type_is_ram(memory_type),
                "UEFI memory type {memory_type} must not be mapped as RAM"
            );
        }
    }
}
