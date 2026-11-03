#![no_std]

#[cfg(test)]
extern crate std;

/// Versioned x86-64/AArch64 boot contract passed in the first argument register.
/// The memory map remains owned by the bootloader allocation and is not copied.
#[path = "bootinfo.rs"]
pub mod bootinfo;

/// Validation for a directly addressable UEFI Graphics Output Protocol framebuffer.
#[path = "framebuffer.rs"]
pub mod framebuffer;

/// Allocation-free validation and access to UEFI memory-map descriptors.
#[path = "memory_map.rs"]
pub mod memory_map;

/// Bounded RSDP validation for ACPI platform metadata.
#[path = "acpi.rs"]
pub mod acpi;

/// Minimal, allocation-free parser for the flattened Device Tree (FDT) header.
#[path = "device_tree.rs"]
pub mod device_tree;

// Defensive, allocation-free ELF64 validation for the first-party loader.
// Parsing is separate from firmware I/O so malformed-image cases can be tested
// on the host before any physical memory is allocated.
#[path = "elf.rs"]
pub mod elf;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
mod device_tree_tests {
    use std::vec;
    use std::vec::Vec;

    use super::device_tree::{parse_header, validate_tree, FdtError, FDT_MAGIC};

    fn put_be32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn valid_fdt() -> Vec<u8> {
        let mut image = vec![0u8; 96];
        put_be32(&mut image, 0, FDT_MAGIC);
        put_be32(&mut image, 4, 96);
        put_be32(&mut image, 8, 56);
        put_be32(&mut image, 12, 88);
        put_be32(&mut image, 16, 40);
        put_be32(&mut image, 20, 17);
        put_be32(&mut image, 24, 16);
        put_be32(&mut image, 28, 0);
        put_be32(&mut image, 32, 8);
        put_be32(&mut image, 36, 32);

        // Root node with one 4-byte property whose name is stored in strings.
        put_be32(&mut image, 56, 1); // FDT_BEGIN_NODE
        image[60] = 0;               // Empty root name
        put_be32(&mut image, 64, 3); // FDT_PROP
        put_be32(&mut image, 68, 4); // property length
        put_be32(&mut image, 72, 0); // name offset
        put_be32(&mut image, 76, 0x1234_5678);
        put_be32(&mut image, 80, 2); // FDT_END_NODE
        put_be32(&mut image, 84, 9); // FDT_END
        image[88..93].copy_from_slice(b"test\0");
        image
    }

    #[test]
    fn accepts_valid_fdt_header_and_tree() {
        let image = valid_fdt();
        let parsed = parse_header(&image).expect("valid FDT header");
        assert_eq!(parsed.total_size, 96);
        assert_eq!(parsed.version, 17);
        assert_eq!(parsed.structure_offset, 56);
        assert_eq!(parsed.strings_offset, 88);
        assert!(validate_tree(&image).is_ok());
    }

    #[test]
    fn rejects_truncated_header() {
        assert_eq!(parse_header(&[0u8; 16]), Err(FdtError::TruncatedHeader));
    }

    #[test]
    fn rejects_bad_magic() {
        let mut image = valid_fdt();
        put_be32(&mut image, 0, 0);
        assert_eq!(parse_header(&image), Err(FdtError::BadMagic));
    }

    #[test]
    fn rejects_total_size_larger_than_buffer() {
        let mut image = valid_fdt();
        put_be32(&mut image, 4, 129);
        assert_eq!(parse_header(&image), Err(FdtError::InvalidTotalSize));
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut image = valid_fdt();
        put_be32(&mut image, 20, 16);
        assert_eq!(parse_header(&image), Err(FdtError::UnsupportedVersion));
    }

    #[test]
    fn rejects_unaligned_structure_offset() {
        let mut image = valid_fdt();
        put_be32(&mut image, 8, 57);
        assert_eq!(parse_header(&image), Err(FdtError::InvalidAlignment));
    }

    #[test]
    fn rejects_overlapping_structure_and_strings() {
        let mut image = valid_fdt();
        put_be32(&mut image, 12, 64);
        put_be32(&mut image, 32, 8);
        assert_eq!(parse_header(&image), Err(FdtError::OverlappingBlocks));
    }

    #[test]
    fn rejects_structure_outside_total_size() {
        let mut image = valid_fdt();
        put_be32(&mut image, 8, 120);
        assert_eq!(parse_header(&image), Err(FdtError::BlockOutOfBounds));
    }

    #[test]
    fn rejects_unknown_structure_token() {
        let mut image = valid_fdt();
        put_be32(&mut image, 80, 0xdead_beef);
        assert_eq!(validate_tree(&image), Err(FdtError::InvalidStructure));
    }

    #[test]
    fn rejects_property_name_outside_strings_block() {
        let mut image = valid_fdt();
        put_be32(&mut image, 72, 100);
        assert_eq!(validate_tree(&image), Err(FdtError::InvalidStructure));
    }

    #[test]
    fn rejects_missing_fdt_end_token() {
        let mut image = valid_fdt();
        put_be32(&mut image, 84, 0);
        assert_eq!(validate_tree(&image), Err(FdtError::InvalidStructure));
    }

    #[test]
    fn rejects_non_empty_root_name() {
        let mut image = valid_fdt();
        image[60] = b'x';
        assert_eq!(validate_tree(&image), Err(FdtError::InvalidStructure));
    }

    #[test]
    fn rejects_reservation_range_overflow() {
        let mut image = valid_fdt();
        image[40..48].copy_from_slice(&(u64::MAX - 7).to_be_bytes());
        image[48..56].copy_from_slice(&16u64.to_be_bytes());
        assert_eq!(validate_tree(&image), Err(FdtError::InvalidStructure));
    }

    #[test]
    fn rejects_reservation_map_without_terminator() {
        let mut image = valid_fdt();
        image[40..48].copy_from_slice(&0x1000u64.to_be_bytes());
        image[48..56].copy_from_slice(&0x1000u64.to_be_bytes());
        assert!(validate_tree(&image).is_err());
    }

    fn append_token(bytes: &mut Vec<u8>, token: u32) {
        bytes.extend_from_slice(&token.to_be_bytes());
    }

    fn append_cstring(bytes: &mut Vec<u8>, value: &[u8]) {
        bytes.extend_from_slice(value);
        bytes.push(0);
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
    }

    fn append_property(bytes: &mut Vec<u8>, name_offset: u32, value: &[u8]) {
        append_token(bytes, 3);
        append_token(bytes, value.len() as u32);
        append_token(bytes, name_offset);
        bytes.extend_from_slice(value);
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
    }

    fn fdt_with_pl011() -> Vec<u8> {
        let mut image = vec![0u8; 56]; // Header + terminating reservation entry.
        let structure_offset = image.len();
        let strings = b"#address-cells\0#size-cells\0compatible\0reg\0";
        let offset_address = 0u32;
        let offset_size = b"#address-cells\0".len() as u32;
        let offset_compatible = (b"#address-cells\0#size-cells\0").len() as u32;
        let offset_reg = (b"#address-cells\0#size-cells\0compatible\0").len() as u32;

        append_token(&mut image, 1); // Root begin
        append_cstring(&mut image, b"");
        append_property(&mut image, offset_address, &2u32.to_be_bytes());
        append_property(&mut image, offset_size, &2u32.to_be_bytes());

        append_token(&mut image, 1); // soc node
        append_cstring(&mut image, b"soc");
        append_property(&mut image, offset_address, &2u32.to_be_bytes());
        append_property(&mut image, offset_size, &2u32.to_be_bytes());

        append_token(&mut image, 1); // UART node
        append_cstring(&mut image, b"pl011@9000000");
        let compatible_value = b"arm,pl011\0arm,primecell\0".to_vec();
        append_property(&mut image, offset_compatible, &compatible_value);
        let mut reg = Vec::new();
        for cell in [0u32, 0x0900_0000, 0, 0x1000] {
            reg.extend_from_slice(&cell.to_be_bytes());
        }
        append_property(&mut image, offset_reg, &reg);
        append_token(&mut image, 2); // UART end
        append_token(&mut image, 2); // soc end
        append_token(&mut image, 2); // Root end
        append_token(&mut image, 9); // FDT_END

        let structure_size = image.len() - structure_offset;
        while image.len() % 4 != 0 {
            image.push(0);
        }
        let strings_offset = image.len();
        image.extend_from_slice(strings);
        while image.len() % 4 != 0 {
            image.push(0);
        }
        let total_size = image.len();

        put_be32(&mut image, 0, FDT_MAGIC);
        put_be32(&mut image, 4, total_size as u32);
        put_be32(&mut image, 8, structure_offset as u32);
        put_be32(&mut image, 12, strings_offset as u32);
        put_be32(&mut image, 16, 40);
        put_be32(&mut image, 20, 17);
        put_be32(&mut image, 24, 16);
        put_be32(&mut image, 28, 0);
        put_be32(&mut image, 32, strings.len() as u32);
        put_be32(&mut image, 36, structure_size as u32);
        image
    }

    #[test]
    fn finds_uart_reg_from_compatible_string_and_parent_cells() {
        let image = fdt_with_pl011();
        let reg = super::device_tree::find_compatible_reg(&image, b"arm,pl011")
            .expect("valid FDT")
            .expect("PL011 node should exist");
        assert_eq!(reg.address, 0x0900_0000);
        assert_eq!(reg.size, 0x1000);
    }

    #[test]
    fn returns_none_when_compatible_device_is_absent() {
        let image = fdt_with_pl011();
        assert_eq!(
            super::device_tree::find_compatible_reg(&image, b"arm,gic-v3"),
            Ok(None)
        );
    }

    #[test]
    fn accepts_empty_strings_block_without_false_overlap() {
        let mut image = valid_fdt();
        put_be32(&mut image, 12, 60);
        put_be32(&mut image, 32, 0);
        assert!(parse_header(&image).is_ok());
    }
}

#[cfg(test)]
mod acpi_tests {
    use std::vec;
    use std::vec::Vec;

    use super::acpi::{parse_rsdp, RsdpError};

    fn put_le32(image: &mut [u8], offset: usize, value: u32) {
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_le64(image: &mut [u8], offset: usize, value: u64) {
        image[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn set_checksum(image: &mut [u8], checksum_offset: usize, range_end: usize) {
        image[checksum_offset] = 0;
        let sum = image[..range_end]
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_add(*byte));
        image[checksum_offset] = 0u8.wrapping_sub(sum);
    }

    fn valid_v1() -> Vec<u8> {
        let mut image = vec![0u8; 20];
        image[0..8].copy_from_slice(b"RSD PTR ");
        image[9..15].copy_from_slice(b"ARCHIA");
        image[15] = 0;
        put_le32(&mut image, 16, 0x1234_5000);
        set_checksum(&mut image, 8, 20);
        image
    }

    fn valid_v2() -> Vec<u8> {
        let mut image = vec![0u8; 36];
        image[0..8].copy_from_slice(b"RSD PTR ");
        image[9..15].copy_from_slice(b"ARCHIA");
        image[15] = 2;
        put_le32(&mut image, 16, 0x1234_5000);
        put_le32(&mut image, 20, 36);
        put_le64(&mut image, 24, 0x0000_0001_2345_6000);
        set_checksum(&mut image, 8, 20);
        set_checksum(&mut image, 32, 36);
        image
    }

    #[test]
    fn accepts_legacy_rsdp() {
        let parsed = parse_rsdp(&valid_v1()).expect("valid revision-0 RSDP");
        assert_eq!(parsed.revision, 0);
        assert_eq!(parsed.length, 20);
        assert_eq!(parsed.rsdt_address, 0x1234_5000);
        assert_eq!(parsed.xsdt_address, 0);
    }

    #[test]
    fn accepts_extended_rsdp() {
        let parsed = parse_rsdp(&valid_v2()).expect("valid revision-2 RSDP");
        assert_eq!(parsed.revision, 2);
        assert_eq!(parsed.length, 36);
        assert_eq!(parsed.xsdt_address, 0x0000_0001_2345_6000);
    }

    #[test]
    fn rejects_bad_signature() {
        let mut image = valid_v1();
        image[0] = b'X';
        assert_eq!(parse_rsdp(&image), Err(RsdpError::InvalidSignature));
    }

    #[test]
    fn rejects_reserved_revision_one() {
        let mut image = valid_v1();
        image[15] = 1;
        set_checksum(&mut image, 8, 20);
        assert_eq!(parse_rsdp(&image), Err(RsdpError::InvalidRevision));
    }

    #[test]
    fn rejects_legacy_checksum() {
        let mut image = valid_v1();
        image[10] ^= 1;
        assert_eq!(parse_rsdp(&image), Err(RsdpError::InvalidChecksum));
    }

    #[test]
    fn rejects_bad_extended_checksum() {
        let mut image = valid_v2();
        image[35] ^= 1;
        assert_eq!(parse_rsdp(&image), Err(RsdpError::InvalidChecksum));
    }

    #[test]
    fn rejects_extended_length_beyond_image() {
        let mut image = valid_v2();
        put_le32(&mut image, 20, 37);
        // Keep the legacy checksum valid so the error specifically covers length.
        assert_eq!(parse_rsdp(&image), Err(RsdpError::InvalidLength));
    }

    #[test]
    fn rejects_truncated_extended_revision() {
        let mut image = valid_v2();
        image.truncate(24);
        assert_eq!(parse_rsdp(&image), Err(RsdpError::Truncated));
    }

    fn valid_spcr() -> Vec<u8> {
        let mut image = vec![0u8; 56];
        image[0..4].copy_from_slice(b"SPCR");
        image[4..8].copy_from_slice(&(56u32).to_le_bytes());
        image[8] = 2;
        image[36] = super::acpi::SPCR_INTERFACE_ARM_PL011;
        image[40] = 0; // System Memory address space
        image[41] = 32; // Register bit width
        image[44..52].copy_from_slice(&0x0900_0000u64.to_le_bytes());
        set_checksum(&mut image, 9, 56);
        image
    }

    #[test]
    fn accepts_spcr_pl011_uart() {
        let image = valid_spcr();
        assert_eq!(
            super::acpi::parse_spcr_uart_base(&image),
            Ok(0x0900_0000)
        );
    }

    #[test]
    fn rejects_spcr_bad_checksum() {
        let mut image = valid_spcr();
        image[48] ^= 1;
        assert_eq!(
            super::acpi::parse_spcr_uart_base(&image),
            Err(super::acpi::SdtError::InvalidChecksum)
        );
    }

    #[test]
    fn rejects_spcr_non_pl011_interface() {
        let mut image = valid_spcr();
        image[36] = 0;
        set_checksum(&mut image, 9, 56);
        assert_eq!(
            super::acpi::parse_spcr_uart_base(&image),
            Err(super::acpi::SdtError::UnsupportedInterface)
        );
    }

    #[test]
    fn rejects_spcr_non_memory_address_space() {
        let mut image = valid_spcr();
        image[40] = 1;
        set_checksum(&mut image, 9, 56);
        assert_eq!(
            super::acpi::parse_spcr_uart_base(&image),
            Err(super::acpi::SdtError::UnsupportedAddressSpace)
        );
    }
}

#[cfg(test)]
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
