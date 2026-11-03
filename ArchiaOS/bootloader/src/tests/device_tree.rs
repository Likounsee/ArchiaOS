// Unit tests for the device_tree parser.
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

