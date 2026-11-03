// Bootloader parser tests for the acpi module.
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
