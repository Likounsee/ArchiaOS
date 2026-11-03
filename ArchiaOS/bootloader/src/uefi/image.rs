#[derive(Clone, Copy)]
pub struct BootError {
    pub status: Status,
    pub message: &'static [u8],
}

fn error(status: Status, message: &'static [u8]) -> BootError {
    BootError { status, message }
}

struct PoolAllocation {
    pointer: *mut c_void,
    boot_services: *mut BootServices,
}

impl PoolAllocation {
    unsafe fn new(
        boot_services: *mut BootServices,
        size: usize,
    ) -> Result<Self, BootError> {
        if boot_services.is_null() || size == 0 {
            return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: invalid pool allocation request.\r\n\0"));
        }
        // SAFETY: caller passes the active firmware Boot Services table.
        let allocate = unsafe { (*boot_services).allocate_pool }
            .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: AllocatePool is unavailable.\r\n\0"))?;
        let mut pointer = ptr::null_mut();
        // SAFETY: output pointer is valid and request size is bounded by the caller.
        let status = unsafe { allocate(EFI_LOADER_DATA, size, &mut pointer) };
        if status != EFI_SUCCESS {
            return Err(error(status, b"ArchiaOS: firmware pool allocation failed.\r\n\0"));
        }
        if pointer.is_null() {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware returned a null pool.\r\n\0"));
        }
        Ok(Self { pointer, boot_services })
    }

    fn take_raw(&mut self) -> *mut c_void {
        let pointer = self.pointer;
        self.pointer = ptr::null_mut();
        pointer
    }
}

impl Drop for PoolAllocation {
    fn drop(&mut self) {
        if self.pointer.is_null() || self.boot_services.is_null() {
            return;
        }
        // SAFETY: the pool was returned by AllocatePool and is freed before EBS.
        if let Some(free) = unsafe { (*self.boot_services).free_pool } {
            let _ = unsafe { free(self.pointer) };
        }
        self.pointer = ptr::null_mut();
    }
}

struct FileGuard(*mut FileProtocol);

impl Drop for FileGuard {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: this is an open EFI file handle; Drop only runs before EBS.
        if let Some(close) = unsafe { (*self.0).close } {
            let _ = unsafe { close(self.0) };
        }
        self.0 = ptr::null_mut();
    }
}

pub struct KernelImage {
    buffer: PoolAllocation,
    size: usize,
}

impl KernelImage {
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: allocation contains the exact number of bytes read from the file.
        unsafe { core::slice::from_raw_parts(self.buffer.pointer.cast::<u8>(), self.size) }
    }
}

#[cfg(target_arch = "x86_64")]
pub const EXPECTED_ELF_MACHINE: u16 = 62;
#[cfg(target_arch = "aarch64")]
pub const EXPECTED_ELF_MACHINE: u16 = 183;

const KERNEL_PATH: [u16; b"\\EFI\\ArchiaOS\\kernel.elf\0".len()] =
    ascii_to_utf16(b"\\EFI\\ArchiaOS\\kernel.elf\0");

const fn ascii_to_utf16<const N: usize>(bytes: &[u8; N]) -> [u16; N] {
    let mut result = [0u16; N];
    let mut index = 0;
    while index < N {
        result[index] = bytes[index] as u16;
        index += 1;
    }
    result
}

/// Read the kernel from the same filesystem volume from which this EFI application was loaded.
pub unsafe fn read_kernel_image(
    image_handle: Handle,
    system_table: *mut SystemTable,
) -> Result<KernelImage, BootError> {
    if image_handle.is_null() || system_table.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: invalid UEFI entry arguments.\r\n\0"));
    }

    // SAFETY: UEFI passes a valid system table to the EFI application.
    let boot_services = unsafe { (*system_table).boot_services };
    if boot_services.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: firmware Boot Services unavailable.\r\n\0"));
    }
    // SAFETY: HandleProtocol is called while boot services are active.
    let handle_protocol = unsafe { (*boot_services).handle_protocol }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: HandleProtocol is unavailable.\r\n\0"))?;

    let mut loaded_image_raw = ptr::null_mut();
    // SAFETY: protocol GUID and writable output pointer are valid.
    let status = unsafe { handle_protocol(image_handle, &LOADED_IMAGE_GUID, &mut loaded_image_raw) };
    if status != EFI_SUCCESS {
        return Err(error(status, b"ArchiaOS: cannot identify bootloader device.\r\n\0"));
    }
    if loaded_image_raw.is_null() {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware returned a null loaded-image interface.\r\n\0"));
    }
    let loaded_image = loaded_image_raw.cast::<LoadedImageProtocol>();
    // SAFETY: HandleProtocol returned a LoadedImageProtocol interface.
    let device_handle = unsafe { (*loaded_image).device_handle };
    if device_handle.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: bootloader has no device handle.\r\n\0"));
    }

    let mut file_system_raw = ptr::null_mut();
    // SAFETY: this protocol query uses the loaded image's device handle.
    let status = unsafe { handle_protocol(device_handle, &SIMPLE_FILE_SYSTEM_GUID, &mut file_system_raw) };
    if status != EFI_SUCCESS {
        return Err(error(status, b"ArchiaOS: boot device has no Simple File System.\r\n\0"));
    }
    if file_system_raw.is_null() {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware returned a null filesystem interface.\r\n\0"));
    }
    let file_system = file_system_raw.cast::<SimpleFileSystemProtocol>();
    // SAFETY: protocol pointer came from HandleProtocol.
    let open_volume = unsafe { (*file_system).open_volume }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: OpenVolume is unavailable.\r\n\0"))?;
    let mut root_raw = ptr::null_mut();
    // SAFETY: OpenVolume writes the root directory handle on success.
    let status = unsafe { open_volume(file_system, &mut root_raw) };
    if status != EFI_SUCCESS {
        return Err(error(status, b"ArchiaOS: cannot open the EFI filesystem root.\r\n\0"));
    }
    if root_raw.is_null() {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: filesystem returned a null root handle.\r\n\0"));
    }
    let root = FileGuard(root_raw);

    // SAFETY: root is an open directory; the path is NUL-terminated UTF-16.
    let open = unsafe { (*root.0).open }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: file Open is unavailable.\r\n\0"))?;
    let mut kernel_raw = ptr::null_mut();
    // SAFETY: file-handle output is writable and the path is an absolute EFI path.
    let status = unsafe { open(root.0, &mut kernel_raw, KERNEL_PATH.as_ptr(), EFI_FILE_MODE_READ, 0) };
    if status != EFI_SUCCESS {
        return Err(error(status, b"ArchiaOS: missing \\EFI\\ArchiaOS\\kernel.elf on boot volume.\r\n\0"));
    }
    if kernel_raw.is_null() {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: filesystem returned a null kernel file handle.\r\n\0"));
    }
    let kernel = FileGuard(kernel_raw);

    let info = unsafe { PoolAllocation::new(boot_services, FILE_INFO_BUFFER_SIZE)? };
    // SAFETY: GetInfo is a member of the file protocol interface.
    let get_info = unsafe { (*kernel.0).get_info }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: file GetInfo is unavailable.\r\n\0"))?;
    let mut info_size = FILE_INFO_BUFFER_SIZE;
    // SAFETY: info is writable and its allocated capacity is passed to firmware.
    let status = unsafe { get_info(kernel.0, &FILE_INFO_GUID, &mut info_size, info.pointer) };
    if status != EFI_SUCCESS {
        return Err(error(status, b"ArchiaOS: cannot read kernel file metadata.\r\n\0"));
    }
    if !(16..=FILE_INFO_BUFFER_SIZE).contains(&info_size) {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid kernel file metadata size.\r\n\0"));
    }
    // EFI_FILE_INFO stores FileSize as u64 at byte offset 8.
    // SAFETY: minimum metadata size was checked; use unaligned access defensively.
    let file_size_u64 = unsafe { ptr::read_unaligned(info.pointer.cast::<u8>().add(8).cast::<u64>()) };
    let file_size = usize::try_from(file_size_u64)
        .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: kernel file size overflows UINTN.\r\n\0"))?;
    if !(64..=MAX_KERNEL_IMAGE_SIZE).contains(&file_size) {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: kernel image size is outside supported bounds.\r\n\0"));
    }
    drop(info);

    let image_buffer = unsafe { PoolAllocation::new(boot_services, file_size)? };
    // SAFETY: Read is part of the file protocol interface.
    let read = unsafe { (*kernel.0).read }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: file Read is unavailable.\r\n\0"))?;
    let mut offset = 0usize;
    while offset < file_size {
        let mut requested = file_size - offset;
        // SAFETY: destination range remains inside the exact-sized allocation.
        let status = unsafe {
            read(
                kernel.0,
                &mut requested,
                image_buffer.pointer.cast::<u8>().add(offset).cast::<c_void>(),
            )
        };
        if status != EFI_SUCCESS {
            return Err(error(status, b"ArchiaOS: reading kernel image failed.\r\n\0"));
        }
        if requested == 0 || requested > file_size - offset {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid or short kernel file read.\r\n\0"));
        }
        offset += requested;
    }
    drop(kernel);
    drop(root);

    Ok(KernelImage { buffer: image_buffer, size: file_size })
}
