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
