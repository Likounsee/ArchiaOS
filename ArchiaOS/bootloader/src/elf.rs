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
