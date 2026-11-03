// Bootloader parser tests for the memory_map module.
mod memory_map_tests {
    use std::vec;
    use std::vec::Vec;

    use super::memory_map::{MemoryMapError, UefiMemoryMap, EFI_CONVENTIONAL_MEMORY};

    fn put_le32(image: &mut [u8], offset: usize, value: u32) {
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_le64(image: &mut [u8], offset: usize, value: u64) {
        image[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn descriptor(memory_type: u32, physical_start: u64, pages: u64) -> Vec<u8> {
        let mut bytes = vec![0u8; 48];
        put_le32(&mut bytes, 0, memory_type);
        put_le64(&mut bytes, 8, physical_start);
        put_le64(&mut bytes, 16, 0);
        put_le64(&mut bytes, 24, pages);
        put_le64(&mut bytes, 32, 0);
        bytes
    }

    #[test]
    fn parses_descriptor_with_extended_stride() {
        let mut bytes = descriptor(EFI_CONVENTIONAL_MEMORY, 0x1000, 8);
        bytes.extend_from_slice(&descriptor(4, 0x9000, 3));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid EFI memory map");
        assert_eq!(map.len(), 2);
        assert_eq!(map.descriptor_size(), 48);

        let first = map.entry(0).expect("first descriptor");
        assert_eq!(first.memory_type, EFI_CONVENTIONAL_MEMORY);
        assert_eq!(first.byte_length(), Ok(8 * 4096));
        assert_eq!(first.physical_end(), Ok(0x9000));
        assert!(first.is_conventional());

        let second = map.entry(1).expect("second descriptor");
        assert_eq!(second.memory_type, 4);
        assert_eq!(second.physical_end(), Ok(0xc000));
        assert!(!second.is_conventional());
    }

    #[test]
    fn sums_only_conventional_pages() {
        let mut bytes = descriptor(EFI_CONVENTIONAL_MEMORY, 0x1000, 8);
        bytes.extend_from_slice(&descriptor(4, 0x9000, 3));
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0xc000, 12));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map");
        assert_eq!(map.conventional_page_count(), Ok(20));
    }

    #[test]
    fn rejects_overlapping_memory_ranges() {
        let mut bytes = descriptor(EFI_CONVENTIONAL_MEMORY, 0x1000, 8);
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0x5000, 2));
        assert_eq!(
            UefiMemoryMap::parse(&bytes, 48).err(),
            Some(MemoryMapError::OverlappingRanges)
        );
    }

    #[test]
    fn accepts_unsorted_non_overlapping_memory_ranges() {
        let mut bytes = descriptor(EFI_CONVENTIONAL_MEMORY, 0x9000, 3);
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0x1000, 8));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid unsorted map");
        assert_eq!(map.conventional_page_count(), Ok(11));
    }

    #[test]
    fn rejects_overlapping_unsorted_memory_ranges() {
        let mut bytes = descriptor(EFI_CONVENTIONAL_MEMORY, 0x5000, 4);
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0x1000, 8));
        assert_eq!(
            UefiMemoryMap::parse(&bytes, 48).err(),
            Some(MemoryMapError::OverlappingRanges)
        );
    }

    #[test]
    fn reserved_range_audit_rejects_conventional_pages_inside_mixed_ranges() {
        // ACPI/DTB/firmware objects must not be considered reserved if even one
        // page in the requested range is available as conventional memory.
        let mut bytes = descriptor(2, 0x1000, 1);
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0x2000, 1));
        bytes.extend_from_slice(&descriptor(4, 0x3000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map");

        assert_eq!(map.range_is_fully_reserved(0x1000, 0x1000), Ok(true));
        assert_eq!(map.range_is_fully_reserved(0x1000, 0x3000), Ok(false));
        assert_eq!(map.range_is_fully_reserved(0x2000, 0x1000), Ok(false));
        assert_eq!(map.range_is_fully_reserved(0x3000, 0x1000), Ok(true));
    }

    #[test]
    fn reserved_range_audit_rejects_unreported_gaps_and_range_overflow() {
        let mut bytes = descriptor(2, 0x1000, 1);
        // The next descriptor is adjacent to the first; the following one leaves
        // an unreported page, which must never be assumed to be reserved.
        bytes.extend_from_slice(&descriptor(4, 0x2000, 1));
        bytes.extend_from_slice(&descriptor(2, 0x4000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map");

        assert_eq!(map.range_is_fully_reserved(0x1000, 0x2000), Ok(true));
        assert_eq!(map.range_is_fully_reserved(0x1000, 0x4000), Ok(false));
        assert_eq!(
            map.range_is_fully_reserved(u64::MAX - 7, 16),
            Err(MemoryMapError::RangeOverflow)
        );
        assert_eq!(
            map.range_is_fully_reserved(0x1000, 0),
            Err(MemoryMapError::EmptyRange)
        );
    }

    #[test]
    fn rejects_descriptor_stride_smaller_than_uefi_layout() {
        assert_eq!(
            UefiMemoryMap::parse(&[0u8; 32], 32).err(),
            Some(MemoryMapError::InvalidDescriptorSize)
        );
    }

    #[test]
    fn rejects_descriptor_stride_not_aligned_to_u64() {
        let mut bytes = descriptor(2, 0x1000, 1);
        bytes.push(0);
        assert_eq!(
            UefiMemoryMap::parse(&bytes, 49).err(),
            Some(MemoryMapError::InvalidDescriptorSize)
        );
    }

    #[test]
    fn rejects_descriptor_stride_larger_than_limit() {
        assert_eq!(
            UefiMemoryMap::parse(&[0u8; 8192], 8192).err(),
            Some(MemoryMapError::InvalidDescriptorSize)
        );
    }

    #[test]
    fn rejects_non_multiple_map_size() {
        assert_eq!(
            UefiMemoryMap::parse(&[0u8; 41], 40).err(),
            Some(MemoryMapError::SizeNotMultipleOfStride)
        );
    }

    #[test]
    fn rejects_empty_map() {
        assert_eq!(
            UefiMemoryMap::parse(&[], 40).err(),
            Some(MemoryMapError::EmptyMap)
        );
    }

    #[test]
    fn rejects_unaligned_physical_start() {
        let bytes = descriptor(EFI_CONVENTIONAL_MEMORY, 0x1001, 1);
        assert_eq!(
            UefiMemoryMap::parse(&bytes, 48).err(),
            Some(MemoryMapError::UnalignedPhysicalAddress)
        );
    }

    #[test]
    fn rejects_zero_page_descriptor() {
        let bytes = descriptor(EFI_CONVENTIONAL_MEMORY, 0x1000, 0);
        assert_eq!(
            UefiMemoryMap::parse(&bytes, 48).err(),
            Some(MemoryMapError::EmptyRange)
        );
    }

    #[test]
    fn rejects_physical_range_overflow() {
        let bytes = descriptor(EFI_CONVENTIONAL_MEMORY, u64::MAX - 4095, 1);
        assert_eq!(
            UefiMemoryMap::parse(&bytes, 48).err(),
            Some(MemoryMapError::RangeOverflow)
        );
    }

    #[test]
    fn rejects_out_of_bounds_descriptor_index() {
        let bytes = descriptor(EFI_CONVENTIONAL_MEMORY, 0x1000, 1);
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid EFI memory map");
        assert_eq!(map.entry(1), Err(MemoryMapError::EntryOutOfBounds));
    }

    #[test]
    fn detects_ranges_intersecting_conventional_memory() {
        let mut bytes = descriptor(2, 0x1000, 2);
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0x3000, 4));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map");
        assert_eq!(map.range_overlaps_conventional(0x1000, 0x2000), Ok(false));
        assert_eq!(map.range_overlaps_conventional(0x2000, 0x2000), Ok(true));
        assert_eq!(map.range_overlaps_conventional(0x5000, 0x1000), Ok(true));
        assert_eq!(map.range_overlaps_conventional(0x7000, 0x1000), Ok(false));
        assert_eq!(map.range_overlaps_conventional(0x1000, 0), Err(MemoryMapError::EmptyRange));
        assert_eq!(map.range_overlaps_conventional(u64::MAX - 1, 8), Err(MemoryMapError::RangeOverflow));
    }

    #[test]
    fn requires_reserved_ranges_to_be_fully_covered_without_gaps() {
        let mut bytes = descriptor(2, 0x1000, 1);
        bytes.extend_from_slice(&descriptor(4, 0x2000, 1));
        bytes.extend_from_slice(&descriptor(2, 0x3000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map");
        assert_eq!(map.range_is_fully_reserved(0x1000, 0x3000), Ok(true));
        assert_eq!(map.range_is_fully_reserved(0x1000, 0x1000), Ok(true));
        assert_eq!(map.range_is_fully_reserved(0x1000, 0x2000), Ok(true));
        // A range extending beyond the final descriptor crosses an unknown gap.
        assert_eq!(map.range_is_fully_reserved(0x3000, 0x2000), Ok(false));
        // Unaligned subranges are valid when every byte remains covered.
        assert_eq!(map.range_is_fully_reserved(0x1800, 0x2000), Ok(true));
        assert_eq!(map.range_is_fully_reserved(0x4000, 0x1000), Ok(false));
        assert_eq!(map.range_is_fully_reserved(0x1000, 0), Err(MemoryMapError::EmptyRange));
        assert_eq!(map.range_is_fully_reserved(u64::MAX - 3, 8), Err(MemoryMapError::RangeOverflow));
    }

    #[test]
    fn generic_coverage_accepts_conventional_and_reserved_ranges_but_rejects_gaps() {
        let mut bytes = descriptor(2, 0x1000, 1);
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0x2000, 2));
        bytes.extend_from_slice(&descriptor(2, 0x4000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map");
        assert_eq!(map.range_is_fully_covered(0x1000, 0x4000), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x1800, 0x1800), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x1000, 0x5000), Ok(false));
        assert_eq!(map.range_is_fully_covered(0x5000, 0x1000), Ok(false));
        assert_eq!(map.range_is_fully_covered(0x1000, 0), Err(MemoryMapError::EmptyRange));
        assert_eq!(map.range_is_fully_covered(u64::MAX - 3, 8), Err(MemoryMapError::RangeOverflow));
    }

    #[test]
    fn generic_coverage_handles_unaligned_ranges_across_adjacent_mixed_types() {
        let mut bytes = descriptor(2, 0x1000, 1);
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0x2000, 2));
        bytes.extend_from_slice(&descriptor(4, 0x4000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid adjacent map");

        // The range starts and ends inside descriptors and crosses two descriptor
        // boundaries; coverage depends on physical continuity, not memory type.
        assert_eq!(map.range_is_fully_covered(0x1fff, 2), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x2fff, 0x1002), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x3fff, 2), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x4fff, 1), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x4fff, 2), Ok(false));
    }

    #[test]
    fn generic_coverage_does_not_bridge_a_page_gap() {
        let mut bytes = descriptor(2, 0x1000, 1);
        bytes.extend_from_slice(&descriptor(4, 0x3000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map with a one-page gap");
        assert_eq!(map.range_is_fully_covered(0x1000, 0x1000), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x1000, 0x2000), Ok(false));
        assert_eq!(map.range_is_fully_covered(0x3000, 0x1000), Ok(true));
    }

    #[test]
    fn generic_coverage_rejects_a_gap_after_an_initially_covered_prefix() {
        let mut bytes = descriptor(2, 0x1000, 2);
        bytes.extend_from_slice(&descriptor(4, 0x4000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map with a gap");
        assert_eq!(map.range_is_fully_covered(0x1800, 0x100), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x1800, 0x3000), Ok(false));
        assert_eq!(map.range_is_fully_covered(0x4000, 0x1000), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x4fff, 2), Ok(false));
    }

    #[test]
    fn generic_coverage_rejects_gaps_in_unsorted_maps() {
        let mut bytes = descriptor(2, 0x5000, 1);
        bytes.extend_from_slice(&descriptor(4, 0x1000, 1));
        bytes.extend_from_slice(&descriptor(2, 0x3000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid unsorted map");
        assert_eq!(map.range_is_fully_covered(0x1000, 0x1000), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x1000, 0x5000), Ok(false));
        assert_eq!(map.range_is_fully_covered(0x5000, 0x1000), Ok(true));
    }

    #[test]
    fn generic_coverage_handles_unsorted_descriptors() {
        let mut bytes = descriptor(2, 0x3000, 1);
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0x1000, 1));
        bytes.extend_from_slice(&descriptor(4, 0x2000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid unsorted map");
        assert_eq!(map.range_is_fully_covered(0x1000, 0x3000), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x0fff, 0x3001), Ok(false));
    }

    #[test]
    fn reserved_coverage_scales_to_large_sorted_maps() {
        let mut bytes = Vec::new();
        // 5,000 adjacent reserved descriptors exercise the sorted-map fast path
        // above the limit where unsorted maps are accepted.
        for index in 0..5_000u64 {
            bytes.extend_from_slice(&descriptor(2, 0x1000 + index * 0x1000, 1));
        }
        let map = UefiMemoryMap::parse(&bytes, 48).expect("large sorted map");
        assert_eq!(
            map.range_is_fully_reserved(0x1000, 5_000 * 0x1000),
            Ok(true)
        );
        assert_eq!(map.range_is_fully_reserved(0x1000, 5_001 * 0x1000), Ok(false));
    }

    #[test]
    fn reserved_coverage_rejects_conventional_memory() {
        let mut bytes = descriptor(2, 0x1000, 1);
        bytes.extend_from_slice(&descriptor(EFI_CONVENTIONAL_MEMORY, 0x2000, 1));
        bytes.extend_from_slice(&descriptor(2, 0x3000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map");
        assert_eq!(map.range_is_fully_reserved(0x1000, 0x1000), Ok(true));
        assert_eq!(map.range_is_fully_reserved(0x2000, 0x1000), Ok(false));
        assert_eq!(map.range_is_fully_reserved(0x1000, 0x3000), Ok(false));
    }

    #[test]
    fn reserved_coverage_handles_unsorted_descriptors() {
        // The parser accepts non-overlapping unsorted maps within its bounded
        // limit; coverage must therefore search all descriptors at each step.
        let mut bytes = descriptor(2, 0x3000, 1);
        bytes.extend_from_slice(&descriptor(2, 0x1000, 1));
        bytes.extend_from_slice(&descriptor(4, 0x2000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid unsorted map");
        assert_eq!(map.range_is_fully_reserved(0x1000, 0x3000), Ok(true));
        assert_eq!(map.range_is_fully_reserved(0x1800, 0x1800), Ok(true));
        assert_eq!(map.range_is_fully_reserved(0x0fff, 0x3001), Ok(false));
    }

    #[test]
    fn accepts_adjacent_ranges_without_treating_them_as_overlap() {
        let mut bytes = descriptor(EFI_CONVENTIONAL_MEMORY, 0x1000, 1);
        bytes.extend_from_slice(&descriptor(4, 0x2000, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("adjacent ranges are valid");
        assert_eq!(map.len(), 2);
        assert_eq!(map.entry(0).unwrap().physical_end(), Ok(0x2000));
        assert_eq!(map.entry(1).unwrap().physical_start, 0x2000);
    }

    #[test]
    fn rejects_large_unsorted_maps_without_quadratic_scan() {
        use super::memory_map::MAX_UNSORTED_DESCRIPTOR_COUNT;

        let mut bytes = Vec::with_capacity((MAX_UNSORTED_DESCRIPTOR_COUNT + 1) * 48);
        for index in (0..=MAX_UNSORTED_DESCRIPTOR_COUNT).rev() {
            bytes.extend_from_slice(&descriptor(
                EFI_CONVENTIONAL_MEMORY,
                0x1000 + (index as u64) * 0x1000,
                1,
            ));
        }
        assert_eq!(
            UefiMemoryMap::parse(&bytes, 48).err(),
            Some(MemoryMapError::UnsupportedOrdering)
        );
    }
}
