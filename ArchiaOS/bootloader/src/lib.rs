#![no_std]

#[cfg(test)]
extern crate std;

/// Versioned x86-64/AArch64 boot contract passed in the first argument register.
/// The memory map remains owned by the bootloader allocation and is not copied.
pub mod bootinfo {
    pub const BOOT_INFO_MAGIC: u64 = 0x4942_4149_4843_5241; // "ARCHIABI"
    pub const BOOT_INFO_VERSION: u32 = 1;
    /// UEFI currently defines EFI_MEMORY_DESCRIPTOR_VERSION as 1.
    pub const UEFI_MEMORY_DESCRIPTOR_VERSION: u32 = 1;
    pub const MAX_MEMORY_MAP_SIZE: u64 = 1024 * 1024;

    #[repr(C)]
    pub struct BootInfo {
        pub magic: u64,
        pub version: u32,
        pub struct_size: u32,
        pub kernel_entry: u64,
        pub memory_map: *const u8,
        pub memory_map_size: u64,
        pub memory_map_descriptor_size: u64,
        pub memory_map_descriptor_version: u32,
        pub flags: u32,
        pub acpi_rsdp: u64,
        pub device_tree: u64,
        pub framebuffer_base: u64,
        pub framebuffer_size: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum BootInfoError {
        BadMagic,
        UnsupportedVersion,
        StructureTooSmall,
        MissingMemoryMap,
        InvalidMemoryMapSize,
        InvalidDescriptorMetadata,
        UnalignedMemoryMap,
        InvalidMemoryMapAddressRange,
        InvalidKernelEntry,
        InvalidFramebufferRange,
    }

    /// Validate scalar ABI fields before the caller dereferences the memory map.
    /// This does not prove the pointers are mapped or readable in the active address space.
    pub fn validate_header(info: &BootInfo) -> Result<(), BootInfoError> {
        if info.magic != BOOT_INFO_MAGIC {
            return Err(BootInfoError::BadMagic);
        }
        if info.kernel_entry == 0 {
            return Err(BootInfoError::InvalidKernelEntry);
        }
        if info.version != BOOT_INFO_VERSION {
            return Err(BootInfoError::UnsupportedVersion);
        }
        if (info.struct_size as usize) < core::mem::size_of::<BootInfo>() {
            return Err(BootInfoError::StructureTooSmall);
        }
        if info.memory_map.is_null() {
            return Err(BootInfoError::MissingMemoryMap);
        }
        if info.memory_map_size == 0 || info.memory_map_size > MAX_MEMORY_MAP_SIZE {
            return Err(BootInfoError::InvalidMemoryMapSize);
        }
        if info.memory_map_descriptor_version != UEFI_MEMORY_DESCRIPTOR_VERSION
            || info.memory_map_descriptor_size < super::memory_map::MIN_DESCRIPTOR_SIZE as u64
            || info.memory_map_descriptor_size > super::memory_map::MAX_DESCRIPTOR_SIZE as u64
            || info.memory_map_descriptor_size % core::mem::align_of::<u64>() as u64 != 0
        {
            return Err(BootInfoError::InvalidDescriptorMetadata);
        }
        if info.memory_map_size % info.memory_map_descriptor_size != 0 {
            return Err(BootInfoError::InvalidMemoryMapSize);
        }
        let map_start = info.memory_map as usize;
        if map_start % core::mem::align_of::<u64>() != 0 {
            return Err(BootInfoError::UnalignedMemoryMap);
        }
        // Rust slices require the complete byte range to fit in the address space
        // and within isize::MAX, not merely a non-null aligned starting pointer.
        let map_length = usize::try_from(info.memory_map_size)
            .map_err(|_| BootInfoError::InvalidMemoryMapSize)?;
        if map_length > isize::MAX as usize || map_start.checked_add(map_length).is_none() {
            return Err(BootInfoError::InvalidMemoryMapAddressRange);
        }
        // A disabled framebuffer is represented by two zero fields. If enabled,
        // its physical range must be non-empty and must not wrap the address space.
        if (info.framebuffer_base == 0) != (info.framebuffer_size == 0)
            || (info.framebuffer_size != 0
                && info.framebuffer_base.checked_add(info.framebuffer_size).is_none())
        {
            return Err(BootInfoError::InvalidFramebufferRange);
        }
        Ok(())
    }
}

/// Allocation-free validation and access to UEFI memory-map descriptors.
pub mod memory_map {
    pub const EFI_PAGE_SIZE: u64 = 4096;
    pub const MIN_DESCRIPTOR_SIZE: usize = 40;
    pub const MAX_DESCRIPTOR_SIZE: usize = 4096;
    pub const MAX_DESCRIPTOR_COUNT: usize = 65_536;
    pub const MAX_UNSORTED_DESCRIPTOR_COUNT: usize = 4096;
    pub const EFI_CONVENTIONAL_MEMORY: u32 = 7;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum MemoryMapError {
        EmptyMap,
        InvalidDescriptorSize,
        SizeNotMultipleOfStride,
        TooManyDescriptors,
        UnsupportedOrdering,
        OverlappingRanges,
        EntryOutOfBounds,
        TruncatedEntry,
        UnalignedPhysicalAddress,
        EmptyRange,
        RangeOverflow,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct MemoryDescriptor {
        pub memory_type: u32,
        pub physical_start: u64,
        pub virtual_start: u64,
        pub number_of_pages: u64,
        pub attributes: u64,
    }

    impl MemoryDescriptor {
        pub fn byte_length(&self) -> Result<u64, MemoryMapError> {
            self.number_of_pages
                .checked_mul(EFI_PAGE_SIZE)
                .ok_or(MemoryMapError::RangeOverflow)
        }

        pub fn physical_end(&self) -> Result<u64, MemoryMapError> {
            self.physical_start
                .checked_add(self.byte_length()?)
                .ok_or(MemoryMapError::RangeOverflow)
        }

        pub const fn is_conventional(&self) -> bool {
            self.memory_type == EFI_CONVENTIONAL_MEMORY
        }
    }

    pub struct UefiMemoryMap<'a> {
        bytes: &'a [u8],
        descriptor_size: usize,
        count: usize,
        sorted: bool,
    }

    fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, MemoryMapError> {
        let end = offset.checked_add(4).ok_or(MemoryMapError::TruncatedEntry)?;
        let field = bytes.get(offset..end).ok_or(MemoryMapError::TruncatedEntry)?;
        Ok(u32::from_le_bytes([field[0], field[1], field[2], field[3]]))
    }

    fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, MemoryMapError> {
        let end = offset.checked_add(8).ok_or(MemoryMapError::TruncatedEntry)?;
        let field = bytes.get(offset..end).ok_or(MemoryMapError::TruncatedEntry)?;
        Ok(u64::from_le_bytes([
            field[0], field[1], field[2], field[3],
            field[4], field[5], field[6], field[7],
        ]))
    }

    impl<'a> UefiMemoryMap<'a> {
        /// Validate the descriptor array and every physical range without allocating.
        pub fn parse(bytes: &'a [u8], descriptor_size: usize) -> Result<Self, MemoryMapError> {
            if descriptor_size < MIN_DESCRIPTOR_SIZE
                || descriptor_size > MAX_DESCRIPTOR_SIZE
                || descriptor_size % core::mem::align_of::<u64>() != 0
            {
                return Err(MemoryMapError::InvalidDescriptorSize);
            }
            if bytes.is_empty() {
                return Err(MemoryMapError::EmptyMap);
            }
            if bytes.len() % descriptor_size != 0 {
                return Err(MemoryMapError::SizeNotMultipleOfStride);
            }
            let count = bytes.len() / descriptor_size;
            if count == 0 {
                return Err(MemoryMapError::EmptyMap);
            }
            if count > MAX_DESCRIPTOR_COUNT {
                return Err(MemoryMapError::TooManyDescriptors);
            }

            let mut map = Self { bytes, descriptor_size, count, sorted: true };
            for index in 0..count {
                map.entry(index)?;
            }

            // Fast path for the common sorted map: validate each pair of adjacent
            // ranges. GetMemoryMap ordering is not assumed for other implementations,
            // so bounded-size unsorted maps receive a complete pairwise check instead.
            let mut previous: Option<MemoryDescriptor> = None;
            let mut sorted = true;
            for index in 0..count {
                let current = map.entry(index)?;
                if let Some(prior) = previous {
                    if current.physical_start < prior.physical_start {
                        sorted = false;
                        break;
                    }
                    if current.physical_start < prior.physical_end()? {
                        return Err(MemoryMapError::OverlappingRanges);
                    }
                }
                previous = Some(current);
            }

            if !sorted {
                if count > MAX_UNSORTED_DESCRIPTOR_COUNT {
                    return Err(MemoryMapError::UnsupportedOrdering);
                }
                for left_index in 0..count {
                    let left = map.entry(left_index)?;
                    let left_end = left.physical_end()?;
                    for right_index in (left_index + 1)..count {
                        let right = map.entry(right_index)?;
                        let right_end = right.physical_end()?;
                        if left.physical_start < right_end
                            && right.physical_start < left_end
                        {
                            return Err(MemoryMapError::OverlappingRanges);
                        }
                    }
                }
            }
            // Keep the validated ordering fact so reserved-range checks can use
            // binary search + a linear walk on normal firmware maps.
            map.sorted = sorted;
            Ok(map)
        }

        pub const fn len(&self) -> usize {
            self.count
        }

        pub const fn is_empty(&self) -> bool {
            self.count == 0
        }

        pub const fn descriptor_size(&self) -> usize {
            self.descriptor_size
        }

        /// Return whether any conventional-memory descriptor intersects a physical range.
        /// Callers use this to ensure loader-owned handoff buffers remain reserved in the
        /// final map passed to the kernel. The map parser has already validated all ranges.
        pub fn range_overlaps_conventional(
            &self,
            physical_start: u64,
            byte_length: u64,
        ) -> Result<bool, MemoryMapError> {
            if byte_length == 0 {
                return Err(MemoryMapError::EmptyRange);
            }
            let end = physical_start
                .checked_add(byte_length)
                .ok_or(MemoryMapError::RangeOverflow)?;
            for index in 0..self.count {
                let descriptor = self.entry(index)?;
                if !descriptor.is_conventional() {
                    continue;
                }
                let descriptor_end = descriptor.physical_end()?;
                if physical_start < descriptor_end && descriptor.physical_start < end {
                    return Ok(true);
                }
            }
            Ok(false)
        }

        /// Return true only when every byte of a non-empty range is covered by
        /// UEFI descriptors, regardless of memory type. This is useful for device
        /// ranges such as GOP framebuffers, which some firmware reports as conventional.
        pub fn range_is_fully_covered(
            &self,
            physical_start: u64,
            byte_length: u64,
        ) -> Result<bool, MemoryMapError> {
            if byte_length == 0 {
                return Err(MemoryMapError::EmptyRange);
            }
            let target_end = physical_start
                .checked_add(byte_length)
                .ok_or(MemoryMapError::RangeOverflow)?;
            let mut cursor = physical_start;
            while cursor < target_end {
                let mut covered_until = cursor;
                if self.sorted {
                    let mut low = 0usize;
                    let mut high = self.count;
                    while low < high {
                        let middle = low + (high - low) / 2;
                        if self.entry(middle)?.physical_start <= cursor {
                            low = middle + 1;
                        } else {
                            high = middle;
                        }
                    }
                    if low != 0 {
                        let descriptor = self.entry(low - 1)?;
                        let descriptor_end = descriptor.physical_end()?;
                        if descriptor.physical_start <= cursor && cursor < descriptor_end {
                            covered_until = descriptor_end.min(target_end);
                        }
                    }
                } else {
                    for index in 0..self.count {
                        let descriptor = self.entry(index)?;
                        let descriptor_end = descriptor.physical_end()?;
                        if descriptor.physical_start <= cursor && cursor < descriptor_end {
                            covered_until = descriptor_end.min(target_end);
                            break;
                        }
                    }
                }
                if covered_until == cursor {
                    return Ok(false);
                }
                cursor = covered_until;
            }
            Ok(true)
        }

        /// Return true only when the entire non-empty physical range is covered
        /// continuously by descriptors that are not conventional RAM. A range that
        /// crosses an unreported gap is not considered reserved.
        pub fn range_is_fully_reserved(
            &self,
            physical_start: u64,
            byte_length: u64,
        ) -> Result<bool, MemoryMapError> {
            if byte_length == 0 {
                return Err(MemoryMapError::EmptyRange);
            }
            let target_end = physical_start
                .checked_add(byte_length)
                .ok_or(MemoryMapError::RangeOverflow)?;
            let mut cursor = physical_start;
            if self.sorted {
                // Find the descriptor at or immediately before the requested start.
                // A sequential walk then proves continuous reserved coverage in O(log n + k).
                let mut low = 0usize;
                let mut high = self.count;
                while low < high {
                    let middle = low + (high - low) / 2;
                    if self.entry(middle)?.physical_start <= cursor {
                        low = middle + 1;
                    } else {
                        high = middle;
                    }
                }
                if low == 0 {
                    return Ok(false);
                }
                let mut index = low - 1;
                while cursor < target_end {
                    if index >= self.count {
                        return Ok(false);
                    }
                    let descriptor = self.entry(index)?;
                    let descriptor_end = descriptor.physical_end()?;
                    if descriptor.physical_start > cursor
                        || cursor >= descriptor_end
                        || descriptor.is_conventional()
                    {
                        return Ok(false);
                    }
                    cursor = descriptor_end.min(target_end);
                    index += 1;
                }
                return Ok(true);
            }

            // Unsorted maps are deliberately capped at MAX_UNSORTED_DESCRIPTOR_COUNT
            // during parsing; retain a bounded scan for this uncommon compatibility path.
            while cursor < target_end {
                let mut covered_until = cursor;
                for index in 0..self.count {
                    let descriptor = self.entry(index)?;
                    let descriptor_end = descriptor.physical_end()?;
                    if descriptor.physical_start <= cursor && cursor < descriptor_end {
                        if descriptor.is_conventional() {
                            return Ok(false);
                        }
                        covered_until = descriptor_end.min(target_end);
                        break;
                    }
                }
                if covered_until == cursor {
                    return Ok(false);
                }
                cursor = covered_until;
            }
            Ok(true)
        }

        /// Sum pages explicitly classified by UEFI as conventional and available
        /// for general allocation. This does not itself reserve kernel or DMA ranges.
        pub fn conventional_page_count(&self) -> Result<u64, MemoryMapError> {
            let mut total = 0u64;
            for index in 0..self.count {
                let descriptor = self.entry(index)?;
                if descriptor.is_conventional() {
                    total = total
                        .checked_add(descriptor.number_of_pages)
                        .ok_or(MemoryMapError::RangeOverflow)?;
                }
            }
            Ok(total)
        }

        pub fn entry(&self, index: usize) -> Result<MemoryDescriptor, MemoryMapError> {
            if index >= self.count {
                return Err(MemoryMapError::EntryOutOfBounds);
            }
            let start = index
                .checked_mul(self.descriptor_size)
                .ok_or(MemoryMapError::TruncatedEntry)?;
            let end = start
                .checked_add(self.descriptor_size)
                .ok_or(MemoryMapError::TruncatedEntry)?;
            let bytes = self.bytes.get(start..end).ok_or(MemoryMapError::TruncatedEntry)?;

            let entry = MemoryDescriptor {
                memory_type: le_u32(bytes, 0)?,
                // EFI_MEMORY_DESCRIPTOR's 4-byte padding at offset 4 is ignored.
                physical_start: le_u64(bytes, 8)?,
                virtual_start: le_u64(bytes, 16)?,
                number_of_pages: le_u64(bytes, 24)?,
                attributes: le_u64(bytes, 32)?,
            };
            if entry.physical_start % EFI_PAGE_SIZE != 0 {
                return Err(MemoryMapError::UnalignedPhysicalAddress);
            }
            if entry.number_of_pages == 0 {
                return Err(MemoryMapError::EmptyRange);
            }
            entry.physical_end()?;
            Ok(entry)
        }
    }
}

/// Bounded RSDP validation for ACPI platform metadata.
pub mod acpi {
    pub const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";
    pub const RSDP_V1_SIZE: usize = 20;
    pub const RSDP_V2_MIN_SIZE: usize = 36;
    pub const MAX_RSDP_SIZE: usize = 4096;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum RsdpError {
        Truncated,
        InvalidSignature,
        InvalidChecksum,
        InvalidLength,
        InvalidRevision,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct RsdpInfo {
        pub revision: u8,
        pub length: usize,
        pub rsdt_address: u32,
        pub xsdt_address: u64,
    }

    fn checksum_valid(bytes: &[u8]) -> bool {
        bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) == 0
    }

    fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, RsdpError> {
        let end = offset.checked_add(4).ok_or(RsdpError::InvalidLength)?;
        let value = bytes.get(offset..end).ok_or(RsdpError::Truncated)?;
        Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
    }

    fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, RsdpError> {
        let end = offset.checked_add(8).ok_or(RsdpError::InvalidLength)?;
        let value = bytes.get(offset..end).ok_or(RsdpError::Truncated)?;
        Ok(u64::from_le_bytes([
            value[0], value[1], value[2], value[3],
            value[4], value[5], value[6], value[7],
        ]))
    }

    /// Validate a complete RSDP image. For revision 2 and later, the byte slice
    /// must contain the advertised extended length; both legacy and extended
    /// checksums are verified before the pointer is put into BootInfo.
    pub fn parse_rsdp(image: &[u8]) -> Result<RsdpInfo, RsdpError> {
        if image.len() < RSDP_V1_SIZE {
            return Err(RsdpError::Truncated);
        }
        if image.get(0..8) != Some(&RSDP_SIGNATURE[..]) {
            return Err(RsdpError::InvalidSignature);
        }
        if !checksum_valid(&image[..RSDP_V1_SIZE]) {
            return Err(RsdpError::InvalidChecksum);
        }

        let revision = image[15];
        if revision == 1 {
            return Err(RsdpError::InvalidRevision);
        }
        let rsdt_address = le_u32(image, 16)?;
        if revision == 0 {
            return Ok(RsdpInfo {
                revision,
                length: RSDP_V1_SIZE,
                rsdt_address,
                xsdt_address: 0,
            });
        }

        if image.len() < RSDP_V2_MIN_SIZE {
            return Err(RsdpError::Truncated);
        }
        let length = le_u32(image, 20)? as usize;
        if length < RSDP_V2_MIN_SIZE
            || length > MAX_RSDP_SIZE
            || length > image.len()
        {
            return Err(RsdpError::InvalidLength);
        }
        if !checksum_valid(&image[..length]) {
            return Err(RsdpError::InvalidChecksum);
        }

        Ok(RsdpInfo {
            revision,
            length,
            rsdt_address,
            xsdt_address: le_u64(image, 24)?,
        })
    }

    pub const SDT_HEADER_SIZE: usize = 36;
    pub const MAX_SDT_SIZE: usize = 16 * 1024 * 1024;
    pub const SPCR_INTERFACE_ARM_PL011: u8 = 0x03;
    pub const SPCR_MIN_SIZE: usize = 52;
    pub const UART_REGISTER_WINDOW_SIZE: u64 = 0x48;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SdtError {
        Truncated,
        InvalidLength,
        InvalidChecksum,
        WrongSignature,
        UnsupportedInterface,
        UnsupportedAddressSpace,
        InvalidAddress,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SdtInfo {
        pub signature: [u8; 4],
        pub length: usize,
        pub revision: u8,
    }

    /// Validate the common ACPI System Description Table header, advertised
    /// length, and checksum before interpreting any table-specific fields.
    pub fn parse_sdt_header(image: &[u8]) -> Result<SdtInfo, SdtError> {
        if image.len() < SDT_HEADER_SIZE {
            return Err(SdtError::Truncated);
        }
        let length = u32::from_le_bytes([image[4], image[5], image[6], image[7]]) as usize;
        if !(SDT_HEADER_SIZE..=MAX_SDT_SIZE).contains(&length) || length > image.len() {
            return Err(SdtError::InvalidLength);
        }
        if !checksum_valid(&image[..length]) {
            return Err(SdtError::InvalidChecksum);
        }
        Ok(SdtInfo {
            signature: [image[0], image[1], image[2], image[3]],
            length,
            revision: image[8],
        })
    }

    /// Extract the memory-mapped PL011 base address from an ACPI SPCR table.
    /// The input may contain trailing bytes, but only the validated table length
    /// participates in checksum and field validation.
    pub fn parse_spcr_uart_base(image: &[u8]) -> Result<u64, SdtError> {
        let header = parse_sdt_header(image)?;
        if header.signature != *b"SPCR" {
            return Err(SdtError::WrongSignature);
        }
        if header.length < SPCR_MIN_SIZE {
            return Err(SdtError::InvalidLength);
        }
        // ACPI SPCR InterfaceType=3 denotes ARM PL011 UART; the Generic
        // Address Structure begins at byte 40 and its Address is at byte 44.
        if image[36] != SPCR_INTERFACE_ARM_PL011 {
            return Err(SdtError::UnsupportedInterface);
        }
        // AddressSpaceId 0 is System Memory. System I/O ports are not used for
        // the ARM PL011 MMIO console.
        if image[40] != 0 {
            return Err(SdtError::UnsupportedAddressSpace);
        }
        let address = u64::from_le_bytes([
            image[44], image[45], image[46], image[47],
            image[48], image[49], image[50], image[51],
        ]);
        if address == 0
            || address % 4 != 0
            || address.checked_add(UART_REGISTER_WINDOW_SIZE).is_none()
        {
            return Err(SdtError::InvalidAddress);
        }
        Ok(address)
    }
}

/// Minimal, allocation-free parser for the flattened Device Tree (FDT) header.
pub mod device_tree {
    pub const FDT_MAGIC: u32 = 0xd00d_feed;
    pub const HEADER_SIZE: usize = 40;
    pub const MAX_TOTAL_SIZE: usize = 16 * 1024 * 1024;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FdtError {
        TruncatedHeader,
        BadMagic,
        InvalidTotalSize,
        UnsupportedVersion,
        InvalidOffset,
        InvalidAlignment,
        BlockOutOfBounds,
        OverlappingBlocks,
        InvalidStructure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct FdtHeader {
        pub total_size: u32,
        pub structure_offset: u32,
        pub strings_offset: u32,
        pub memory_reservation_offset: u32,
        pub version: u32,
        pub last_compatible_version: u32,
        pub strings_size: u32,
        pub structure_size: u32,
    }

    fn be32(bytes: &[u8], offset: usize) -> Result<u32, FdtError> {
        let end = offset.checked_add(4).ok_or(FdtError::BlockOutOfBounds)?;
        let value = bytes.get(offset..end).ok_or(FdtError::TruncatedHeader)?;
        Ok(u32::from_be_bytes([value[0], value[1], value[2], value[3]]))
    }

    fn bounded_end(start: u32, size: u32, total: u32) -> Result<u32, FdtError> {
        let end = start.checked_add(size).ok_or(FdtError::BlockOutOfBounds)?;
        if end > total {
            return Err(FdtError::BlockOutOfBounds);
        }
        Ok(end)
    }

    fn be64(bytes: &[u8], offset: usize) -> Result<u64, FdtError> {
        let end = offset.checked_add(8).ok_or(FdtError::BlockOutOfBounds)?;
        let value = bytes.get(offset..end).ok_or(FdtError::BlockOutOfBounds)?;
        Ok(u64::from_be_bytes([
            value[0], value[1], value[2], value[3],
            value[4], value[5], value[6], value[7],
        ]))
    }

    fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
        a_start < b_end && b_start < a_end
    }

    fn validate_reservation_map(image: &[u8], header: &FdtHeader) -> Result<(), FdtError> {
        const MAX_RESERVATION_ENTRIES: usize = 65_536;
        let total = header.total_size as usize;
        let structure_start = header.structure_offset as usize;
        let structure_end = structure_start
            .checked_add(header.structure_size as usize)
            .ok_or(FdtError::BlockOutOfBounds)?;
        let strings_start = header.strings_offset as usize;
        let strings_end = strings_start
            .checked_add(header.strings_size as usize)
            .ok_or(FdtError::BlockOutOfBounds)?;
        let mut offset = header.memory_reservation_offset as usize;

        for _ in 0..MAX_RESERVATION_ENTRIES {
            let end = offset.checked_add(16).ok_or(FdtError::BlockOutOfBounds)?;
            if end > total {
                return Err(FdtError::BlockOutOfBounds);
            }
            if ranges_overlap(offset, end, structure_start, structure_end)
                || (header.strings_size != 0
                    && ranges_overlap(offset, end, strings_start, strings_end))
            {
                return Err(FdtError::OverlappingBlocks);
            }

            let address = be64(image, offset)?;
            let size = be64(image, offset + 8)?;
            offset = end;
            if address == 0 && size == 0 {
                return Ok(());
            }
            if size == 0 || address.checked_add(size).is_none() {
                return Err(FdtError::InvalidStructure);
            }
        }
        Err(FdtError::InvalidStructure)
    }

    /// Validate the complete FDT image header and bounds of header-described blocks.
    pub fn parse_header(image: &[u8]) -> Result<FdtHeader, FdtError> {
        if image.len() < HEADER_SIZE {
            return Err(FdtError::TruncatedHeader);
        }
        if be32(image, 0)? != FDT_MAGIC {
            return Err(FdtError::BadMagic);
        }

        let total_size = be32(image, 4)?;
        let structure_offset = be32(image, 8)?;
        let strings_offset = be32(image, 12)?;
        let memory_reservation_offset = be32(image, 16)?;
        let version = be32(image, 20)?;
        let last_compatible_version = be32(image, 24)?;
        let strings_size = be32(image, 32)?;
        let structure_size = be32(image, 36)?;

        if total_size < HEADER_SIZE as u32
            || total_size as usize > MAX_TOTAL_SIZE
            || total_size as usize > image.len()
        {
            return Err(FdtError::InvalidTotalSize);
        }
        // size_dt_struct is present from FDT version 17 onward. Our parser only
        // relies on that defined header layout, and supports v17 compatibility.
        if version < 17 || last_compatible_version > version || last_compatible_version > 17 {
            return Err(FdtError::UnsupportedVersion);
        }

        if structure_offset < HEADER_SIZE as u32
            || strings_offset < HEADER_SIZE as u32
            || memory_reservation_offset < HEADER_SIZE as u32
        {
            return Err(FdtError::InvalidOffset);
        }
        if structure_offset % 4 != 0 || memory_reservation_offset % 8 != 0 {
            return Err(FdtError::InvalidAlignment);
        }
        if structure_size == 0 {
            return Err(FdtError::BlockOutOfBounds);
        }

        let structure_end = bounded_end(structure_offset, structure_size, total_size)?;
        let strings_end = bounded_end(strings_offset, strings_size, total_size)?;
        // A reservation-map entry is 16 bytes. Require at least its first entry
        // to fit; scanning for the terminating zero pair is a later milestone.
        bounded_end(memory_reservation_offset, 16, total_size)?;

        if strings_size != 0 && structure_offset < strings_end && strings_offset < structure_end {
            return Err(FdtError::OverlappingBlocks);
        }

        Ok(FdtHeader {
            total_size,
            structure_offset,
            strings_offset,
            memory_reservation_offset,
            version,
            last_compatible_version,
            strings_size,
            structure_size,
        })
    }

    fn nul_terminated(bytes: &[u8], offset: usize) -> Result<(&[u8], usize), FdtError> {
        let suffix = bytes.get(offset..).ok_or(FdtError::InvalidStructure)?;
        let length = suffix.iter().position(|byte| *byte == 0).ok_or(FdtError::InvalidStructure)?;
        let next = offset.checked_add(length).and_then(|value| value.checked_add(1))
            .ok_or(FdtError::InvalidStructure)?;
        Ok((&suffix[..length], next))
    }

    fn align4(value: usize) -> Result<usize, FdtError> {
        value.checked_add(3).map(|aligned| aligned & !3).ok_or(FdtError::InvalidStructure)
    }

    fn validate_structure_block(block: &[u8], strings: &[u8]) -> Result<(), FdtError> {
        const FDT_BEGIN_NODE: u32 = 1;
        const FDT_END_NODE: u32 = 2;
        const FDT_PROP: u32 = 3;
        const FDT_NOP: u32 = 4;
        const FDT_END: u32 = 9;
        const MAX_TREE_DEPTH: usize = 256;

        let mut offset = 0usize;
        let mut depth = 0usize;
        let mut root_seen = false;
        let mut root_closed = false;
        let mut saw_end = false;

        while offset < block.len() {
            let token = be32(block, offset).map_err(|_| FdtError::InvalidStructure)?;
            offset = offset.checked_add(4).ok_or(FdtError::InvalidStructure)?;
            match token {
                FDT_BEGIN_NODE => {
                    if root_closed || saw_end || depth >= MAX_TREE_DEPTH {
                        return Err(FdtError::InvalidStructure);
                    }
                    let (name, name_end) = nul_terminated(block, offset)?;
                    if depth == 0 {
                        if root_seen || !name.is_empty() {
                            return Err(FdtError::InvalidStructure);
                        }
                        root_seen = true;
                    }
                    offset = align4(name_end)?;
                    if offset > block.len() {
                        return Err(FdtError::InvalidStructure);
                    }
                    depth += 1;
                }
                FDT_END_NODE => {
                    if depth == 0 || saw_end {
                        return Err(FdtError::InvalidStructure);
                    }
                    depth -= 1;
                    if depth == 0 {
                        root_closed = true;
                    }
                }
                FDT_PROP => {
                    if depth == 0 || root_closed || saw_end {
                        return Err(FdtError::InvalidStructure);
                    }
                    let length = be32(block, offset).map_err(|_| FdtError::InvalidStructure)? as usize;
                    let name_offset = be32(
                        block,
                        offset.checked_add(4).ok_or(FdtError::InvalidStructure)?,
                    ).map_err(|_| FdtError::InvalidStructure)? as usize;
                    offset = offset.checked_add(8).ok_or(FdtError::InvalidStructure)?;
                    let (name, _) = nul_terminated(strings, name_offset)?;
                    if name.is_empty() {
                        return Err(FdtError::InvalidStructure);
                    }
                    let data_end = offset.checked_add(length).ok_or(FdtError::InvalidStructure)?;
                    if data_end > block.len() {
                        return Err(FdtError::InvalidStructure);
                    }
                    offset = align4(data_end)?;
                    if offset > block.len() {
                        return Err(FdtError::InvalidStructure);
                    }
                }
                FDT_NOP => {}
                FDT_END => {
                    if !root_seen || !root_closed || depth != 0 {
                        return Err(FdtError::InvalidStructure);
                    }
                    saw_end = true;
                    // FDT_END terminates the token stream; a few producers may
                    // leave zero padding in size_dt_struct, but no extra tokens.
                    if block[offset..].iter().any(|byte| *byte != 0) {
                        return Err(FdtError::InvalidStructure);
                    }
                    break;
                }
                _ => return Err(FdtError::InvalidStructure),
            }
        }
        if !saw_end || !root_seen || !root_closed || depth != 0 {
            return Err(FdtError::InvalidStructure);
        }
        Ok(())
    }

    /// Validate the header, reservation map, node/property tokens and property-name references.
    /// Semantic device properties are not interpreted yet.
    pub fn validate_tree(image: &[u8]) -> Result<FdtHeader, FdtError> {
        let header = parse_header(image)?;
        validate_reservation_map(image, &header)?;
        let structure_start = header.structure_offset as usize;
        let structure_end = structure_start
            .checked_add(header.structure_size as usize)
            .ok_or(FdtError::BlockOutOfBounds)?;
        let strings_start = header.strings_offset as usize;
        let strings_end = strings_start
            .checked_add(header.strings_size as usize)
            .ok_or(FdtError::BlockOutOfBounds)?;
        let structure = image.get(structure_start..structure_end).ok_or(FdtError::BlockOutOfBounds)?;
        let strings = image.get(strings_start..strings_end).ok_or(FdtError::BlockOutOfBounds)?;
        validate_structure_block(structure, strings)?;
        Ok(header)
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct RegRange {
        pub address: u64,
        pub size: u64,
    }

    fn compatible_list_contains(value: &[u8], compatible: &[u8]) -> Result<bool, FdtError> {
        let mut offset = 0usize;
        while offset < value.len() {
            let (entry, next) = nul_terminated(value, offset)?;
            if entry.is_empty() {
                return Err(FdtError::InvalidStructure);
            }
            if entry == compatible {
                return Ok(true);
            }
            offset = next;
        }
        Ok(false)
    }

    fn decode_cells(bytes: &[u8], cell_count: usize) -> Result<u64, FdtError> {
        if cell_count > 2 || bytes.len() < cell_count * 4 {
            return Err(FdtError::InvalidStructure);
        }
        let mut value = 0u64;
        for index in 0..cell_count {
            value = (value << 32) | (be32(bytes, index * 4)? as u64);
        }
        Ok(value)
    }

    fn decode_reg(
        value: &[u8],
        address_cells: u32,
        size_cells: u32,
    ) -> Result<RegRange, FdtError> {
        if address_cells == 0 || address_cells > 2 || size_cells > 2 {
            return Err(FdtError::InvalidStructure);
        }
        let address_bytes = (address_cells as usize)
            .checked_mul(4)
            .ok_or(FdtError::InvalidStructure)?;
        let size_bytes = (size_cells as usize)
            .checked_mul(4)
            .ok_or(FdtError::InvalidStructure)?;
        let tuple_size = address_bytes
            .checked_add(size_bytes)
            .filter(|size| *size != 0)
            .ok_or(FdtError::InvalidStructure)?;
        if value.len() < tuple_size || value.len() % tuple_size != 0 {
            return Err(FdtError::InvalidStructure);
        }
        let address = decode_cells(&value[..address_bytes], address_cells as usize)?;
        let size = decode_cells(&value[address_bytes..tuple_size], size_cells as usize)?;
        if size == 0 || address.checked_add(size).is_none() {
            return Err(FdtError::InvalidStructure);
        }
        Ok(RegRange { address, size })
    }

    /// Find the first node declaring a compatible string and decode its first reg tuple.
    /// This does not translate addresses across non-identity parent bus ranges.
    pub fn find_compatible_reg(
        image: &[u8],
        compatible: &[u8],
    ) -> Result<Option<RegRange>, FdtError> {
        const MAX_TREE_DEPTH: usize = 256;
        const FDT_BEGIN_NODE: u32 = 1;
        const FDT_END_NODE: u32 = 2;
        const FDT_PROP: u32 = 3;
        const FDT_NOP: u32 = 4;
        const FDT_END: u32 = 9;

        let header = validate_tree(image)?;
        let structure_start = header.structure_offset as usize;
        let structure_end = structure_start
            .checked_add(header.structure_size as usize)
            .ok_or(FdtError::BlockOutOfBounds)?;
        let strings_start = header.strings_offset as usize;
        let strings_end = strings_start
            .checked_add(header.strings_size as usize)
            .ok_or(FdtError::BlockOutOfBounds)?;
        let block = image.get(structure_start..structure_end).ok_or(FdtError::BlockOutOfBounds)?;
        let strings = image.get(strings_start..strings_end).ok_or(FdtError::BlockOutOfBounds)?;

        let mut address_cells = [1u32; MAX_TREE_DEPTH];
        let mut size_cells = [1u32; MAX_TREE_DEPTH];
        let mut compatible_match = [false; MAX_TREE_DEPTH];
        let mut reg_values: [Option<&[u8]>; MAX_TREE_DEPTH] = [None; MAX_TREE_DEPTH];
        let mut depth = 0usize;
        let mut offset = 0usize;

        while offset < block.len() {
            let token = be32(block, offset).map_err(|_| FdtError::InvalidStructure)?;
            offset = offset.checked_add(4).ok_or(FdtError::InvalidStructure)?;
            match token {
                FDT_BEGIN_NODE => {
                    if depth >= MAX_TREE_DEPTH {
                        return Err(FdtError::InvalidStructure);
                    }
                    let (_, name_end) = nul_terminated(block, offset)?;
                    offset = align4(name_end)?;
                    if offset > block.len() {
                        return Err(FdtError::InvalidStructure);
                    }
                    address_cells[depth] = 2;
                    size_cells[depth] = 1;
                    compatible_match[depth] = false;
                    reg_values[depth] = None;
                    depth += 1;
                }
                FDT_END_NODE => {
                    if depth == 0 {
                        return Err(FdtError::InvalidStructure);
                    }
                    let index = depth - 1;
                    if compatible_match[index] {
                        if let Some(reg) = reg_values[index] {
                            if index == 0 {
                                return Err(FdtError::InvalidStructure);
                            }
                            return decode_reg(reg, address_cells[index - 1], size_cells[index - 1])
                                .map(Some);
                        }
                    }
                    depth -= 1;
                }
                FDT_PROP => {
                    if depth == 0 {
                        return Err(FdtError::InvalidStructure);
                    }
                    let length = be32(block, offset).map_err(|_| FdtError::InvalidStructure)? as usize;
                    let name_offset = be32(
                        block,
                        offset.checked_add(4).ok_or(FdtError::InvalidStructure)?,
                    ).map_err(|_| FdtError::InvalidStructure)? as usize;
                    offset = offset.checked_add(8).ok_or(FdtError::InvalidStructure)?;
                    let (name, _) = nul_terminated(strings, name_offset)?;
                    let data_end = offset.checked_add(length).ok_or(FdtError::InvalidStructure)?;
                    let value = block.get(offset..data_end).ok_or(FdtError::InvalidStructure)?;
                    let index = depth - 1;
                    if name == &b"#address-cells"[..] {
                        if value.len() != 4 {
                            return Err(FdtError::InvalidStructure);
                        }
                        address_cells[index] = be32(value, 0)?;
                    } else if name == &b"#size-cells"[..] {
                        if value.len() != 4 {
                            return Err(FdtError::InvalidStructure);
                        }
                        size_cells[index] = be32(value, 0)?;
                    } else if name == &b"compatible"[..] {
                        compatible_match[index] |= compatible_list_contains(value, compatible)?;
                    } else if name == &b"reg"[..] {
                        reg_values[index] = Some(value);
                    }
                    offset = align4(data_end)?;
                    if offset > block.len() {
                        return Err(FdtError::InvalidStructure);
                    }
                }
                FDT_NOP => {}
                FDT_END => break,
                _ => return Err(FdtError::InvalidStructure),
            }
        }
        Ok(None)
    }
}

// Defensive, allocation-free ELF64 validation for the first-party loader.
// Parsing is separate from firmware I/O so malformed-image cases can be tested
// on the host before any physical memory is allocated.
pub mod elf {
    const ELF_HEADER_SIZE: usize = 64;
    const ELF_PROGRAM_HEADER_SIZE: usize = 56;
    const MAX_PROGRAM_HEADERS: u16 = 1024;
    const ET_EXEC: u16 = 2;
    const PT_LOAD: u32 = 1;
    const PF_X: u32 = 1;
    const PF_W: u32 = 2;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ElfError {
        Truncated,
        BadMagic,
        UnsupportedClass,
        UnsupportedEndian,
        UnsupportedVersion,
        UnsupportedType,
        WrongMachine,
        InvalidHeader,
        InvalidProgramHeader,
        IntegerOverflow,
        SegmentOutOfBounds,
        SegmentSizeMismatch,
        InvalidAlignment,
        OverlappingSegments,
        OverlappingPhysicalSegments,
        MissingLoadSegment,
        EntryNotExecutable,
        WritableExecutableSegment,
        NotLoadableSegment,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ElfFile {
        entry: u64,
        program_header_offset: u64,
        program_header_count: u16,
    }

    impl ElfFile {
        pub const fn entry(&self) -> u64 {
            self.entry
        }

        pub const fn program_header_count(&self) -> u16 {
            self.program_header_count
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct LoadSegment {
        pub flags: u32,
        pub file_offset: u64,
        pub virtual_address: u64,
        pub physical_address: u64,
        pub file_size: u64,
        pub memory_size: u64,
        pub alignment: u64,
    }

    #[derive(Clone, Copy)]
    struct ProgramHeader {
        kind: u32,
        flags: u32,
        offset: u64,
        virtual_address: u64,
        physical_address: u64,
        file_size: u64,
        memory_size: u64,
        alignment: u64,
    }

    fn range(image: &[u8], offset: u64, size: u64) -> Result<&[u8], ElfError> {
        let end = offset.checked_add(size).ok_or(ElfError::IntegerOverflow)?;
        let start = usize::try_from(offset).map_err(|_| ElfError::SegmentOutOfBounds)?;
        let end = usize::try_from(end).map_err(|_| ElfError::SegmentOutOfBounds)?;
        image.get(start..end).ok_or(ElfError::SegmentOutOfBounds)
    }

    fn u16_at(image: &[u8], offset: usize) -> Result<u16, ElfError> {
        let end = offset.checked_add(2).ok_or(ElfError::IntegerOverflow)?;
        let bytes = image.get(offset..end).ok_or(ElfError::Truncated)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32_at(image: &[u8], offset: usize) -> Result<u32, ElfError> {
        let end = offset.checked_add(4).ok_or(ElfError::IntegerOverflow)?;
        let bytes = image.get(offset..end).ok_or(ElfError::Truncated)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64_at(image: &[u8], offset: usize) -> Result<u64, ElfError> {
        let end = offset.checked_add(8).ok_or(ElfError::IntegerOverflow)?;
        let bytes = image.get(offset..end).ok_or(ElfError::Truncated)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_program_header(image: &[u8], offset: u64) -> Result<ProgramHeader, ElfError> {
        let offset = usize::try_from(offset).map_err(|_| ElfError::IntegerOverflow)?;
        Ok(ProgramHeader {
            kind: u32_at(image, offset)?,
            flags: u32_at(image, offset.checked_add(4).ok_or(ElfError::IntegerOverflow)?)?,
            offset: u64_at(image, offset.checked_add(8).ok_or(ElfError::IntegerOverflow)?)?,
            virtual_address: u64_at(image, offset.checked_add(16).ok_or(ElfError::IntegerOverflow)?)?,
            physical_address: u64_at(image, offset.checked_add(24).ok_or(ElfError::IntegerOverflow)?)?,
            file_size: u64_at(image, offset.checked_add(32).ok_or(ElfError::IntegerOverflow)?)?,
            memory_size: u64_at(image, offset.checked_add(40).ok_or(ElfError::IntegerOverflow)?)?,
            alignment: u64_at(image, offset.checked_add(48).ok_or(ElfError::IntegerOverflow)?)?,
        })
    }

    fn validated_load_segment(image: &[u8], header: ProgramHeader) -> Result<LoadSegment, ElfError> {
        if header.kind != PT_LOAD {
            return Err(ElfError::NotLoadableSegment);
        }
        if header.memory_size == 0 || header.memory_size < header.file_size {
            return Err(ElfError::SegmentSizeMismatch);
        }
        if header.flags & (PF_W | PF_X) == (PF_W | PF_X) {
            return Err(ElfError::WritableExecutableSegment);
        }
        range(image, header.offset, header.file_size)?;
        header.virtual_address
            .checked_add(header.memory_size)
            .ok_or(ElfError::IntegerOverflow)?;
        // The loader copies PT_LOAD data to p_paddr; reject wrapping physical
        // ranges during parsing rather than after allocation has begun.
        header.physical_address
            .checked_add(header.memory_size)
            .ok_or(ElfError::IntegerOverflow)?;

        if header.alignment > 1 {
            if !header.alignment.is_power_of_two()
                || header.offset % header.alignment != header.virtual_address % header.alignment
            {
                return Err(ElfError::InvalidAlignment);
            }
        }

        Ok(LoadSegment {
            flags: header.flags,
            file_offset: header.offset,
            virtual_address: header.virtual_address,
            physical_address: header.physical_address,
            file_size: header.file_size,
            memory_size: header.memory_size,
            alignment: header.alignment,
        })
    }

    /// Validate an ELF64 little-endian executable for the requested ELF machine ID.
    ///
    /// Known machine IDs: x86-64 = 62, AArch64 = 183. The caller must still
    /// validate platform-specific address limits before allocating or mapping memory.
    pub fn parse_elf64(image: &[u8], expected_machine: u16) -> Result<ElfFile, ElfError> {
        if image.len() < ELF_HEADER_SIZE {
            return Err(ElfError::Truncated);
        }
        if image.get(0..4) != Some(&b"\x7fELF"[..]) {
            return Err(ElfError::BadMagic);
        }
        if image[4] != 2 {
            return Err(ElfError::UnsupportedClass);
        }
        if image[5] != 1 {
            return Err(ElfError::UnsupportedEndian);
        }
        if image[6] != 1 || u32_at(image, 20)? != 1 {
            return Err(ElfError::UnsupportedVersion);
        }
        if u16_at(image, 16)? != ET_EXEC {
            return Err(ElfError::UnsupportedType);
        }
        if u16_at(image, 18)? != expected_machine {
            return Err(ElfError::WrongMachine);
        }

        let entry = u64_at(image, 24)?;
        let phoff = u64_at(image, 32)?;
        let ehsize = u16_at(image, 52)? as usize;
        let phentsize = u16_at(image, 54)? as usize;
        let phnum = u16_at(image, 56)?;

        if ehsize != ELF_HEADER_SIZE
            || phentsize != ELF_PROGRAM_HEADER_SIZE
            || phnum == 0
            || phnum > MAX_PROGRAM_HEADERS
            || phoff < ELF_HEADER_SIZE as u64
        {
            return Err(ElfError::InvalidHeader);
        }
        let table_size = (phentsize as u64)
            .checked_mul(phnum as u64)
            .ok_or(ElfError::IntegerOverflow)?;
        range(image, phoff, table_size)?;

        let file = ElfFile {
            entry,
            program_header_offset: phoff,
            program_header_count: phnum,
        };
        let mut load_count = 0u16;
        let mut entry_is_executable = false;

        for index in 0..phnum {
            let current_offset = phoff
                .checked_add((index as u64).checked_mul(ELF_PROGRAM_HEADER_SIZE as u64)
                    .ok_or(ElfError::IntegerOverflow)?)
                .ok_or(ElfError::IntegerOverflow)?;
            let current = read_program_header(image, current_offset)?;
            if current.kind != PT_LOAD {
                continue;
            }
            let current = validated_load_segment(image, current)?;
            load_count += 1;

            let current_end = current.virtual_address
                .checked_add(current.memory_size)
                .ok_or(ElfError::IntegerOverflow)?;
            if current.flags & PF_X != 0
                && entry >= current.virtual_address
                && entry < current_end
            {
                entry_is_executable = true;
            }

            for other_index in (index + 1)..phnum {
                let other_offset = phoff
                    .checked_add((other_index as u64).checked_mul(ELF_PROGRAM_HEADER_SIZE as u64)
                        .ok_or(ElfError::IntegerOverflow)?)
                    .ok_or(ElfError::IntegerOverflow)?;
                let other = read_program_header(image, other_offset)?;
                if other.kind != PT_LOAD {
                    continue;
                }
                let other = validated_load_segment(image, other)?;
                let other_end = other.virtual_address
                    .checked_add(other.memory_size)
                    .ok_or(ElfError::IntegerOverflow)?;
                if current.virtual_address < other_end && other.virtual_address < current_end {
                    return Err(ElfError::OverlappingSegments);
                }
                let current_physical_end = current.physical_address
                    .checked_add(current.memory_size)
                    .ok_or(ElfError::IntegerOverflow)?;
                let other_physical_end = other.physical_address
                    .checked_add(other.memory_size)
                    .ok_or(ElfError::IntegerOverflow)?;
                if current.physical_address < other_physical_end
                    && other.physical_address < current_physical_end
                {
                    return Err(ElfError::OverlappingPhysicalSegments);
                }
            }
        }

        if load_count == 0 {
            return Err(ElfError::MissingLoadSegment);
        }
        if !entry_is_executable {
            return Err(ElfError::EntryNotExecutable);
        }
        Ok(file)
    }

    /// Return a validated PT_LOAD segment from an already validated image.
    pub fn load_segment(
        image: &[u8],
        file: &ElfFile,
        index: u16,
    ) -> Result<LoadSegment, ElfError> {
        if index >= file.program_header_count {
            return Err(ElfError::InvalidProgramHeader);
        }
        let offset = file.program_header_offset
            .checked_add((index as u64).checked_mul(ELF_PROGRAM_HEADER_SIZE as u64)
                .ok_or(ElfError::IntegerOverflow)?)
            .ok_or(ElfError::IntegerOverflow)?;
        let header = read_program_header(image, offset)?;
        validated_load_segment(image, header)
    }
}

#[cfg(test)]
mod tests {
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
        info.framebuffer_size = 0x1000;
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
        assert_eq!(core::mem::size_of::<super::bootinfo::BootInfo>(), 88);
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
}

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
    fn generic_coverage_does_not_bridge_a_one_byte_gap() {
        let mut bytes = descriptor(2, 0x1000, 1);
        bytes.extend_from_slice(&descriptor(4, 0x2001, 1));
        let map = UefiMemoryMap::parse(&bytes, 48).expect("valid map with one-byte gap");
        assert_eq!(map.range_is_fully_covered(0x1000, 0x1000), Ok(true));
        assert_eq!(map.range_is_fully_covered(0x1000, 0x1001), Ok(false));
        assert_eq!(map.range_is_fully_covered(0x2001, 0x1000), Ok(true));
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
