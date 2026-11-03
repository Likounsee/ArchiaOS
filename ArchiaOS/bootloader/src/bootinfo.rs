pub const BOOT_INFO_MAGIC: u64 = 0x4942_4149_4843_5241; // "ARCHIABI"
pub const BOOT_INFO_VERSION: u32 = 2;
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
    /// GOP horizontal pixel resolution.
    pub framebuffer_width: u32,
    /// GOP vertical pixel resolution.
    pub framebuffer_height: u32,
    /// GOP pixel stride in pixels (may exceed visible width).
    pub framebuffer_stride: u32,
    /// EFI_GRAPHICS_PIXEL_FORMAT: 0=RGB, 1=BGR, 2=bitmask.
    pub framebuffer_pixel_format: u32,
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
    UnsupportedFlags,
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
    // Version 1 defines no flag bits; reject unknown claims rather than
    // silently handing a contract the kernel does not understand.
    if info.flags != 0 {
        return Err(BootInfoError::UnsupportedFlags);
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
    let geometry_is_zero = info.framebuffer_width == 0
        && info.framebuffer_height == 0
        && info.framebuffer_stride == 0
        && info.framebuffer_pixel_format == 0;
    if info.framebuffer_base == 0 {
        if !geometry_is_zero {
            return Err(BootInfoError::InvalidFramebufferRange);
        }
    } else {
        let required_pixels = u64::from(info.framebuffer_stride)
            .checked_mul(u64::from(info.framebuffer_height));
        let required_bytes = required_pixels.and_then(|pixels| pixels.checked_mul(4));
        if info.framebuffer_width == 0
            || info.framebuffer_height == 0
            || info.framebuffer_stride < info.framebuffer_width
            || info.framebuffer_pixel_format > 2
            || required_bytes.map_or(true, |bytes| bytes > info.framebuffer_size)
        {
            return Err(BootInfoError::InvalidFramebufferRange);
        }
    }
    Ok(())
}
