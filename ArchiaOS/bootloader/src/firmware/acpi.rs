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
