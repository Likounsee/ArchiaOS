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
