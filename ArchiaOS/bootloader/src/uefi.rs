use core::ffi::c_void;
use core::ptr;

pub type Status = usize;
pub type Handle = *mut c_void;

const EFI_ERROR_BIT: usize = 1usize << (usize::BITS - 1);
pub const EFI_SUCCESS: Status = 0;
pub const EFI_INVALID_PARAMETER: Status = EFI_ERROR_BIT | 2;
pub const EFI_LOAD_ERROR: Status = EFI_ERROR_BIT | 1;
pub const EFI_UNSUPPORTED: Status = EFI_ERROR_BIT | 3;
pub const EFI_BUFFER_TOO_SMALL: Status = EFI_ERROR_BIT | 5;
const EFI_LOADER_DATA: u32 = 2;
const EFI_FILE_MODE_READ: u64 = 1;
const FILE_INFO_BUFFER_SIZE: usize = 1024;
const MAX_KERNEL_IMAGE_SIZE: usize = 64 * 1024 * 1024;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const LOADED_IMAGE_GUID: Guid = Guid {
    data1: 0x5b1b_31a1,
    data2: 0x9562,
    data3: 0x11d2,
    data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};
const SIMPLE_FILE_SYSTEM_GUID: Guid = Guid {
    data1: 0x964e_5b22,
    data2: 0x6459,
    data3: 0x11d2,
    data4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};
const FILE_INFO_GUID: Guid = Guid {
    data1: 0x0957_6e92,
    data2: 0x6d3f,
    data3: 0x11d2,
    data4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

#[repr(C)]
pub struct TableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

#[repr(C)]
pub struct SimpleTextOutputProtocol {
    reset: usize,
    output_string: Option<
        unsafe extern "efiapi" fn(
            this: *mut SimpleTextOutputProtocol,
            string: *const u16,
        ) -> Status,
    >,
    test_string: usize,
    query_mode: usize,
    set_mode: usize,
    set_attribute: usize,
    clear_screen: usize,
    set_cursor_position: usize,
    enable_cursor: usize,
    mode: *mut c_void,
}

#[repr(C)]
pub struct SystemTable {
    header: TableHeader,
    firmware_vendor: *mut u16,
    firmware_revision: u32,
    _padding: u32,
    console_in_handle: Handle,
    con_in: *mut c_void,
    console_out_handle: Handle,
    pub con_out: *mut SimpleTextOutputProtocol,
    standard_error_handle: Handle,
    std_err: *mut SimpleTextOutputProtocol,
    runtime_services: *mut c_void,
    pub boot_services: *mut BootServices,
    number_of_table_entries: usize,
    configuration_table: *mut c_void,
}

type HandleProtocolFn = unsafe extern "efiapi" fn(
    handle: Handle,
    protocol: *const Guid,
    interface: *mut *mut c_void,
) -> Status;
type AllocatePoolFn = unsafe extern "efiapi" fn(
    memory_type: u32,
    size: usize,
    buffer: *mut *mut c_void,
) -> Status;
type FreePoolFn = unsafe extern "efiapi" fn(buffer: *mut c_void) -> Status;

#[repr(C)]
pub struct BootServices {
    header: TableHeader,
    raise_tpl: usize,
    restore_tpl: usize,
    allocate_pages: usize,
    free_pages: usize,
    get_memory_map: usize,
    allocate_pool: Option<AllocatePoolFn>,
    free_pool: Option<FreePoolFn>,
    create_event: usize,
    set_timer: usize,
    wait_for_event: usize,
    signal_event: usize,
    close_event: usize,
    check_event: usize,
    install_protocol_interface: usize,
    reinstall_protocol_interface: usize,
    uninstall_protocol_interface: usize,
    handle_protocol: Option<HandleProtocolFn>,
    reserved: usize,
    register_protocol_notify: usize,
    locate_handle: usize,
    locate_device_path: usize,
    install_configuration_table: usize,
    load_image: usize,
    start_image: usize,
    exit: usize,
    unload_image: usize,
    exit_boot_services: usize,
    get_next_monotonic_count: usize,
    stall: usize,
    set_watchdog_timer: usize,
    connect_controller: usize,
    disconnect_controller: usize,
    open_protocol: usize,
    close_protocol: usize,
    open_protocol_information: usize,
    protocols_per_handle: usize,
    locate_handle_buffer: usize,
    locate_protocol: usize,
    install_multiple_protocol_interfaces: usize,
    uninstall_multiple_protocol_interfaces: usize,
    calculate_crc32: usize,
    copy_mem: usize,
    set_mem: usize,
    create_event_ex: usize,
}

#[repr(C)]
pub struct LoadedImageProtocol {
    revision: u32,
    _padding0: u32,
    parent_handle: Handle,
    system_table: *mut SystemTable,
    device_handle: Handle,
    file_path: *mut c_void,
    reserved: *mut c_void,
    load_options_size: u32,
    _padding1: u32,
    load_options: *mut c_void,
    image_base: *mut c_void,
    image_size: u64,
    image_code_type: u32,
    image_data_type: u32,
    unload: usize,
}

type OpenVolumeFn = unsafe extern "efiapi" fn(
    this: *mut SimpleFileSystemProtocol,
    root: *mut *mut FileProtocol,
) -> Status;

#[repr(C)]
pub struct SimpleFileSystemProtocol {
    revision: u64,
    open_volume: Option<OpenVolumeFn>,
}

type FileOpenFn = unsafe extern "efiapi" fn(
    this: *mut FileProtocol,
    new_handle: *mut *mut FileProtocol,
    file_name: *const u16,
    open_mode: u64,
    attributes: u64,
) -> Status;
type FileCloseFn = unsafe extern "efiapi" fn(this: *mut FileProtocol) -> Status;
type FileReadFn = unsafe extern "efiapi" fn(
    this: *mut FileProtocol,
    buffer_size: *mut usize,
    buffer: *mut c_void,
) -> Status;
type FileGetInfoFn = unsafe extern "efiapi" fn(
    this: *mut FileProtocol,
    information_type: *const Guid,
    buffer_size: *mut usize,
    buffer: *mut c_void,
) -> Status;

#[repr(C)]
pub struct FileProtocol {
    revision: u64,
    open: Option<FileOpenFn>,
    close: Option<FileCloseFn>,
    delete: usize,
    read: Option<FileReadFn>,
    write: usize,
    get_position: usize,
    set_position: usize,
    get_info: Option<FileGetInfoFn>,
    set_info: usize,
    flush: usize,
    open_ex: usize,
    read_ex: usize,
    write_ex: usize,
    flush_ex: usize,
}

#[derive(Clone, Copy)]
pub struct BootError {
    pub status: Status,
    pub message: &'static [u8],
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
        // SAFETY: caller passes the firmware Boot Services table while boot services are active.
        let allocate = unsafe { (*boot_services).allocate_pool }
            .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: AllocatePool is unavailable.\r\n\0"))?;
        let mut pointer = ptr::null_mut();
        // SAFETY: output pointer is valid and the allocation size has been bounded by the caller.
        let status = unsafe { allocate(EFI_LOADER_DATA, size, &mut pointer) };
        if status != EFI_SUCCESS {
            return Err(error(status, b"ArchiaOS: firmware pool allocation failed.\r\n\0"));
        }
        if pointer.is_null() {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware returned a null pool.\r\n\0"));
        }
        Ok(Self { pointer, boot_services })
    }

    fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.pointer.cast::<T>()
    }
}

impl Drop for PoolAllocation {
    fn drop(&mut self) {
        if self.pointer.is_null() || self.boot_services.is_null() {
            return;
        }
        // SAFETY: this pointer was returned by AllocatePool and is released before ExitBootServices.
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
        // SAFETY: the file handle is valid until Close is called, and Drop only runs before EBS.
        if let Some(close) = unsafe { (*self.0).close } {
            let _ = unsafe { close(self.0) };
        }
        self.0 = ptr::null_mut();
    }
}

fn error(status: Status, message: &'static [u8]) -> BootError {
    BootError { status, message }
}

const fn ascii_to_utf16<const N: usize>(bytes: &[u8; N]) -> [u16; N] {
    let mut result = [0u16; N];
    let mut index = 0;
    while index < N {
        result[index] = bytes[index] as u16;
        index += 1;
    }
    result
}

static KERNEL_PATH: [u16; b"\\EFI\\BOOT\\ARCHIAOS.ELF\0".len()] =
    ascii_to_utf16(b"\\EFI\\BOOT\\ARCHIAOS.ELF\0");

#[cfg(target_arch = "x86_64")]
const EXPECTED_ELF_MACHINE: u16 = 62;
#[cfg(target_arch = "aarch64")]
const EXPECTED_ELF_MACHINE: u16 = 183;

pub unsafe fn inspect_kernel(
    image_handle: Handle,
    system_table: *mut SystemTable,
) -> Result<u64, BootError> {
    if image_handle.is_null() || system_table.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: invalid UEFI entry arguments.\r\n\0"));
    }

    // SAFETY: UEFI passes a valid system table to an EFI application.
    let boot_services = unsafe { (*system_table).boot_services };
    if boot_services.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: firmware Boot Services unavailable.\r\n\0"));
    }
    // SAFETY: we call HandleProtocol while this EFI application is running in boot services.
    let handle_protocol = unsafe { (*boot_services).handle_protocol }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: HandleProtocol is unavailable.\r\n\0"))?;

    let mut loaded_image_raw = ptr::null_mut();
    // SAFETY: output pointer is writable and the GUID identifies the loaded-image protocol.
    let status = unsafe {
        handle_protocol(image_handle, &LOADED_IMAGE_GUID, &mut loaded_image_raw)
    };
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
    // SAFETY: the device handle belongs to the loaded image; query its Simple File System protocol.
    let status = unsafe {
        handle_protocol(device_handle, &SIMPLE_FILE_SYSTEM_GUID, &mut file_system_raw)
    };
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
    // SAFETY: the output handle is valid and the file is opened read-only.
    let status = unsafe { open(root.0, &mut kernel_raw, KERNEL_PATH.as_ptr(), EFI_FILE_MODE_READ, 0) };
    if status != EFI_SUCCESS {
        return Err(error(status, b"ArchiaOS: missing \\EFI\\BOOT\\ARCHIAOS.ELF on boot volume.\r\n\0"));
    }
    if kernel_raw.is_null() {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: filesystem returned a null kernel file handle.\r\n\0"));
    }
    let kernel = FileGuard(kernel_raw);

    // EFI_FILE_INFO begins with Size and FileSize, both u64 fields. A fixed,
    // bounded buffer avoids a size-query race while the file handle is open.
    let mut info = unsafe { PoolAllocation::new(boot_services, FILE_INFO_BUFFER_SIZE)? };
    let get_info = unsafe { (*kernel.0).get_info }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: file GetInfo is unavailable.\r\n\0"))?;
    let mut info_size = FILE_INFO_BUFFER_SIZE;
    // SAFETY: info is a properly allocated buffer of the size passed to firmware.
    let status = unsafe {
        get_info(kernel.0, &FILE_INFO_GUID, &mut info_size, info.pointer)
    };
    if status != EFI_SUCCESS {
        return Err(error(status, b"ArchiaOS: cannot read kernel file metadata.\r\n\0"));
    }
    if info_size < 16 {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: truncated kernel file metadata.\r\n\0"));
    }
    // SAFETY: EFI_FILE_INFO contains FileSize at byte offset 8; read unaligned defensively.
    let file_size_u64 = unsafe { ptr::read_unaligned(info.pointer.cast::<u8>().add(8).cast::<u64>()) };
    let file_size = usize::try_from(file_size_u64)
        .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: kernel file is too large for this platform.\r\n\0"))?;
    if file_size < 64 || file_size > MAX_KERNEL_IMAGE_SIZE {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: kernel image size is outside the supported bounds.\r\n\0"));
    }
    drop(info);

    let mut image_buffer = unsafe { PoolAllocation::new(boot_services, file_size)? };
    let read = unsafe { (*kernel.0).read }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: file Read is unavailable.\r\n\0"))?;
    let mut bytes_read = file_size;
    // SAFETY: buffer has file_size bytes and the file remains open for this call.
    let status = unsafe { read(kernel.0, &mut bytes_read, image_buffer.pointer) };
    if status != EFI_SUCCESS {
        return Err(error(status, b"ArchiaOS: reading kernel image failed.\r\n\0"));
    }
    if bytes_read != file_size {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: short read while loading kernel image.\r\n\0"));
    }

    // SAFETY: firmware supplied exactly file_size bytes into this live allocation.
    let image = unsafe { core::slice::from_raw_parts(image_buffer.as_mut_ptr::<u8>(), file_size) };
    let parsed = archiaos_bootloader::elf::parse_elf64(image, EXPECTED_ELF_MACHINE)
        .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: kernel ELF validation failed.\r\n\0"))?;
    Ok(parsed.entry())
}
