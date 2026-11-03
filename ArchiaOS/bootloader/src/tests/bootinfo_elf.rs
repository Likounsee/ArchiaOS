// Bootloader library unit tests. Kept separate from the ABI/parser implementation.
use std::vec;
    use std::vec::Vec;

    use super::bootinfo::{validate_header, BootInfo, BootInfoError, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};
    use super::elf::{parse_elf64, load_segment, ElfError};

    fn valid_boot_info() -> BootInfo {
        BootInfo {
            magic: BOOT_INFO_MAGIC,
            version: BOOT_INFO_VERSION,
            struct_size: core::mem::size_of::<BootInfo>() as u32,
            kernel_entry: 0x100000,
            memory_map: 0x1000usize as *const u8,
            memory_map_size: 40,
            memory_map_descriptor_size: 40,
            memory_map_descriptor_version: 1,
            flags: 0,
            acpi_rsdp: 0,
            device_tree: 0,
            framebuffer_base: 0,
            framebuffer_size: 0,
            framebuffer_width: 0,
            framebuffer_height: 0,
            framebuffer_stride: 0,
            framebuffer_pixel_format: 0,
        }
    }

    #[test]
    fn boot_info_header_accepts_valid_contract() {
        assert_eq!(validate_header(&valid_boot_info()), Ok(()));
    }

    #[test]
    fn boot_info_header_rejects_zero_kernel_entry() {
        let mut info = valid_boot_info();
        info.kernel_entry = 0;
        assert_eq!(validate_header(&info), Err(BootInfoError::InvalidKernelEntry));
    }

    #[test]
    fn boot_info_header_rejects_inconsistent_or_overflowing_framebuffer() {
        let mut info = valid_boot_info();
        info.framebuffer_base = 0x8000_0000;
        assert_eq!(validate_header(&info), Err(BootInfoError::InvalidFramebufferRange));

        let mut info = valid_boot_info();
        info.framebuffer_base = u64::MAX - 0xFFF;
        info.framebuffer_size = 0x2000;
        assert_eq!(validate_header(&info), Err(BootInfoError::InvalidFramebufferRange));

        let mut info = valid_boot_info();
        info.framebuffer_base = 0x8000_0000;
        info.framebuffer_width = 640;
        info.framebuffer_height = 480;
        info.framebuffer_stride = 640;
        info.framebuffer_pixel_format = 0;
        info.framebuffer_size = 640 * 480 * 4;
        assert_eq!(validate_header(&info), Ok(()));
    }

    #[test]
    fn boot_info_header_rejects_unaligned_descriptor_stride() {
        let mut info = valid_boot_info();
        info.memory_map_descriptor_size = 49;
        info.memory_map_size = 49;
        assert_eq!(
            validate_header(&info),
            Err(BootInfoError::InvalidDescriptorMetadata)
        );
    }

    #[test]
    fn boot_info_header_rejects_invalid_descriptor_stride() {
        let mut info = valid_boot_info();
        info.memory_map_descriptor_size = 8;
        assert_eq!(validate_header(&info), Err(BootInfoError::InvalidDescriptorMetadata));

        let mut info = valid_boot_info();
        info.memory_map_size = 41;
        assert_eq!(validate_header(&info), Err(BootInfoError::InvalidMemoryMapSize));
    }

    #[test]
    fn boot_info_header_rejects_memory_map_address_wraparound() {
        let mut info = valid_boot_info();
        info.memory_map = (usize::MAX - 7) as *const u8;
        assert_eq!(
            validate_header(&info),
            Err(BootInfoError::InvalidMemoryMapAddressRange)
        );
    }

    #[test]
    fn boot_info_header_rejects_undefined_v1_flags() {
        let mut info = valid_boot_info();
        info.flags = 1;
        assert_eq!(validate_header(&info), Err(BootInfoError::UnsupportedFlags));

        let mut info = valid_boot_info();
        info.flags = u32::MAX;
        assert_eq!(validate_header(&info), Err(BootInfoError::UnsupportedFlags));
    }

    #[test]
    fn boot_info_header_rejects_bad_magic_and_version() {
        let mut info = valid_boot_info();
        info.magic = 0;
        assert_eq!(validate_header(&info), Err(BootInfoError::BadMagic));
        let mut info = valid_boot_info();
        info.version += 1;
        assert_eq!(validate_header(&info), Err(BootInfoError::UnsupportedVersion));
    }

    #[test]
    fn boot_info_header_rejects_short_struct_and_missing_map() {
        let mut info = valid_boot_info();
        info.struct_size = 1;
        assert_eq!(validate_header(&info), Err(BootInfoError::StructureTooSmall));
        let mut info = valid_boot_info();
        info.memory_map = core::ptr::null();
        assert_eq!(validate_header(&info), Err(BootInfoError::MissingMemoryMap));
    }

    #[test]
    fn boot_info_header_rejects_unsupported_descriptor_version() {
        let mut info = valid_boot_info();
        info.memory_map_descriptor_version = 2;
        assert_eq!(
            validate_header(&info),
            Err(BootInfoError::InvalidDescriptorMetadata)
        );
    }

    #[test]
    fn boot_info_header_rejects_invalid_map_bounds_and_metadata() {
        let mut info = valid_boot_info();
        info.memory_map_size = 0;
        assert_eq!(validate_header(&info), Err(BootInfoError::InvalidMemoryMapSize));
        let mut info = valid_boot_info();
        info.memory_map_size = super::bootinfo::MAX_MEMORY_MAP_SIZE + 1;
        assert_eq!(validate_header(&info), Err(BootInfoError::InvalidMemoryMapSize));
        let mut info = valid_boot_info();
        info.memory_map_descriptor_version = 0;
        assert_eq!(validate_header(&info), Err(BootInfoError::InvalidDescriptorMetadata));
        let mut info = valid_boot_info();
        info.memory_map = 0x1001usize as *const u8;
        assert_eq!(validate_header(&info), Err(BootInfoError::UnalignedMemoryMap));
    }

    const X86_64: u16 = 62;
    const AARCH64: u16 = 183;

    fn put_u16(image: &mut [u8], offset: usize, value: u16) {
        image[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32(image: &mut [u8], offset: usize, value: u32) {
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(image: &mut [u8], offset: usize, value: u64) {
        image[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn valid_elf(machine: u16) -> Vec<u8> {
        let mut image = vec![0u8; 0x104];
        image[0..4].copy_from_slice(b"\x7fELF");
        image[4] = 2;
        image[5] = 1;
        image[6] = 1;
        put_u16(&mut image, 16, 2);
        put_u16(&mut image, 18, machine);
        put_u32(&mut image, 20, 1);
        put_u64(&mut image, 24, 0x4008_0100);
        put_u64(&mut image, 32, 64);
        put_u16(&mut image, 52, 64);
        put_u16(&mut image, 54, 56);
        put_u16(&mut image, 56, 1);
        put_u32(&mut image, 64, 1);
        put_u32(&mut image, 68, 5);
        put_u64(&mut image, 72, 0x100);
        put_u64(&mut image, 80, 0x4008_0100);
        put_u64(&mut image, 88, 0x4008_0100);
        put_u64(&mut image, 96, 4);
        put_u64(&mut image, 104, 16);
        put_u64(&mut image, 112, 0x1000);
        image[0x100..0x104].copy_from_slice(b"CODE");
        image
    }

    #[test]
    fn boot_info_layout_is_stable() {
        // Version 2 adds four u32 GOP geometry/format fields to the v1 ABI.
        assert_eq!(core::mem::size_of::<super::bootinfo::BootInfo>(), 104);
    }

    #[test]
    fn accepts_valid_x86_64_image() {
        let image = valid_elf(X86_64);
        let parsed = parse_elf64(&image, X86_64).expect("valid ELF should parse");
        assert_eq!(parsed.entry(), 0x4008_0100);
        let segment = load_segment(&image, &parsed, 0).expect("PT_LOAD should be exposed");
        assert_eq!(segment.file_size, 4);
        assert_eq!(segment.physical_address, 0x4008_0100);
        assert_eq!(segment.memory_size, 16);
        assert_eq!(segment.flags & 1, 1);
    }

    #[test]
    fn accepts_valid_aarch64_image() {
        assert!(parse_elf64(&valid_elf(AARCH64), AARCH64).is_ok());
    }

    #[test]
    fn rejects_wrong_machine() {
        assert_eq!(
            parse_elf64(&valid_elf(AARCH64), X86_64),
            Err(ElfError::WrongMachine)
        );
    }

    #[test]
    fn rejects_truncated_program_table() {
        let mut image = valid_elf(X86_64);
        image.truncate(100);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::SegmentOutOfBounds)
        );
    }

    #[test]
    fn rejects_segment_memory_smaller_than_file() {
        let mut image = valid_elf(X86_64);
        put_u64(&mut image, 104, 2);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::SegmentSizeMismatch)
        );
    }

    #[test]
    fn rejects_physical_address_overflow() {
        let mut image = valid_elf(X86_64);
        put_u64(&mut image, 88, u64::MAX - 7);
        put_u64(&mut image, 104, 16);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::IntegerOverflow)
        );
    }

    #[test]
    fn rejects_virtual_address_overflow() {
        let mut image = valid_elf(X86_64);
        put_u64(&mut image, 80, u64::MAX - 7);
        put_u64(&mut image, 88, u64::MAX - 7);
        put_u64(&mut image, 104, 16);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::IntegerOverflow)
        );
    }

    #[test]
    fn rejects_writable_executable_load_segment() {
        let mut image = valid_elf(X86_64);
        // ELF64 program header p_flags sits at offset 68; PF_W | PF_X = 7.
        put_u32(&mut image, 68, 7);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::WritableExecutableSegment)
        );
    }

    #[test]
    fn rejects_non_executable_entry() {
        let mut image = valid_elf(X86_64);
        put_u32(&mut image, 68, 4);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::EntryNotExecutable)
        );
    }

    #[test]
    fn rejects_non_power_of_two_alignment() {
        let mut image = valid_elf(X86_64);
        put_u64(&mut image, 112, 3);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::InvalidAlignment)
        );
    }

    #[test]
    fn rejects_file_segment_outside_image() {
        let mut image = valid_elf(X86_64);
        put_u64(&mut image, 72, 0x1ff);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::SegmentOutOfBounds)
        );
    }

    #[test]
    fn rejects_overlapping_physical_load_segments() {
        let mut image = valid_elf(X86_64);
        image.resize(0x2104, 0);
        put_u16(&mut image, 56, 2);
        // Distinct virtual ranges, but both segments would claim the same physical page.
        put_u32(&mut image, 120, 1);
        put_u32(&mut image, 124, 4);
        put_u64(&mut image, 128, 0x2100);
        put_u64(&mut image, 136, 0x4008_2100);
        put_u64(&mut image, 144, 0x4008_0100);
        put_u64(&mut image, 152, 4);
        put_u64(&mut image, 160, 16);
        put_u64(&mut image, 168, 0x1000);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::OverlappingPhysicalSegments)
        );
    }

    #[test]
    fn rejects_overlapping_load_segments() {
        let mut image = valid_elf(X86_64);
        image.resize(0x1104, 0);
        put_u16(&mut image, 56, 2);

        // The second PT_LOAD has a matching file/virtual alignment, but maps
        // the same virtual range as the first one. Such an image is ambiguous.
        put_u32(&mut image, 120, 1);
        put_u32(&mut image, 124, 4);
        put_u64(&mut image, 128, 0x1100);
        put_u64(&mut image, 136, 0x4008_0100);
        put_u64(&mut image, 144, 0x4008_0100);
        put_u64(&mut image, 152, 4);
        put_u64(&mut image, 160, 16);
        put_u64(&mut image, 168, 0x1000);
        image[0x1100..0x1104].copy_from_slice(b"DATA");

        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::OverlappingSegments)
        );
    }

    #[test]
    fn rejects_entry_outside_loadable_executable_range() {
        let mut image = valid_elf(X86_64);
        put_u64(&mut image, 24, 0xffff_ffff_9000_0000);
        assert_eq!(
            parse_elf64(&image, X86_64),
            Err(ElfError::EntryNotExecutable)
        );
    }

