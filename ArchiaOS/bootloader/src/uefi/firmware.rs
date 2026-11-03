pub type Status = usize;
pub type Handle = *mut c_void;

const EFI_ERROR_BIT: usize = 1usize << (usize::BITS - 1);
pub const EFI_SUCCESS: Status = 0;
pub const EFI_INVALID_PARAMETER: Status = EFI_ERROR_BIT | 2;
pub const EFI_LOAD_ERROR: Status = EFI_ERROR_BIT | 1;
pub const EFI_UNSUPPORTED: Status = EFI_ERROR_BIT | 3;
#[cfg(target_arch = "aarch64")]
const EFI_NOT_FOUND: Status = EFI_ERROR_BIT | 14;
const EFI_LOADER_CODE: u32 = 1;
const EFI_LOADER_DATA: u32 = 2;
const EFI_ALLOCATE_ADDRESS: u32 = 2;
const EFI_FILE_MODE_READ: u64 = 1;
const EFI_PAGE_SIZE: u64 = 4096;
const MIN_KERNEL_PHYSICAL_ADDRESS: u64 = 1024 * 1024;
const MAX_KERNEL_PHYSICAL_ADDRESS: u64 = 1u64 << 32;
const FILE_INFO_BUFFER_SIZE: usize = 1024;
const MAX_KERNEL_IMAGE_SIZE: usize = 64 * 1024 * 1024;
const MAX_MEMORY_MAP_SIZE: usize = 1024 * 1024;
const MAX_LOAD_SEGMENTS: usize = 1024;
const MAX_TOTAL_KERNEL_MEMORY_SIZE: u64 = 128 * 1024 * 1024;
#[cfg(target_arch = "x86_64")]
const X86_64_HIGH_HALF_BASE: u64 = 0xffff_ffff_8000_0000;
#[cfg(target_arch = "x86_64")]
const X86_64_HIGH_HALF_END: u64 = X86_64_HIGH_HALF_BASE + MAX_TOTAL_KERNEL_MEMORY_SIZE;
#[cfg(target_arch = "x86_64")]
const X86_64_IDENTITY_PT_SPANS: usize = (MAX_KERNEL_PHYSICAL_ADDRESS / (2 * 1024 * 1024)) as usize;
#[cfg(target_arch = "x86_64")]
const X86_64_HIGH_PT_SPANS: usize = (MAX_TOTAL_KERNEL_MEMORY_SIZE / (2 * 1024 * 1024)) as usize;
#[cfg(target_arch = "x86_64")]
const PAGE_TABLE_ARENA_PAGES: usize = 2200;
const MAX_EXIT_BOOT_SERVICES_ATTEMPTS: usize = 3;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
#[cfg(target_arch = "x86_64")]
const X86_64_PTE_NX: u64 = 1u64 << 63;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
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
const ACPI_20_TABLE_GUID: Guid = Guid {
    data1: 0x8868_e871,
    data2: 0xe4f1,
    data3: 0x11d3,
    data4: [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
};
const ACPI_10_TABLE_GUID: Guid = Guid {
    data1: 0xeb9d_2d30,
    data2: 0x2d88,
    data3: 0x11d3,
    data4: [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
};
const EFI_DTB_TABLE_GUID: Guid = Guid {
    data1: 0xb1b6_21d5,
    data2: 0xf19c,
    data3: 0x41a5,
    data4: [0x83, 0x0b, 0xd9, 0x15, 0x2c, 0x69, 0xaa, 0xe0],
};
const GRAPHICS_OUTPUT_PROTOCOL_GUID: Guid = Guid {
    data1: 0x9042_a9de,
    data2: 0x23dc,
    data3: 0x4a38,
    data4: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
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
    pub output_string: Option<
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
    configuration_table: *mut ConfigurationTable,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ConfigurationTable {
    vendor_guid: Guid,
    vendor_table: *mut c_void,
}

#[repr(C)]
struct GraphicsOutputProtocol {
    query_mode: usize,
    set_mode: usize,
    blt: usize,
    mode: *mut GraphicsOutputProtocolMode,
}

#[repr(C)]
struct GraphicsOutputProtocolMode {
    max_mode: u32,
    mode: u32,
    info: *mut GraphicsOutputModeInformation,
    size_of_info: usize,
    framebuffer_base: u64,
    framebuffer_size: usize,
}

#[repr(C)]
struct GraphicsOutputModeInformation {
    version: u32,
    horizontal_resolution: u32,
    vertical_resolution: u32,
    pixel_format: u32,
    pixel_information: [u32; 4],
    pixels_per_scan_line: u32,
}

type HandleProtocolFn = unsafe extern "efiapi" fn(
    handle: Handle,
    protocol: *const Guid,
    interface: *mut *mut c_void,
) -> Status;
type LocateProtocolFn = unsafe extern "efiapi" fn(
    protocol: *const Guid,
    registration: *mut c_void,
    interface: *mut *mut c_void,
) -> Status;
type AllocatePoolFn = unsafe extern "efiapi" fn(
    memory_type: u32,
    size: usize,
    buffer: *mut *mut c_void,
) -> Status;
type FreePoolFn = unsafe extern "efiapi" fn(buffer: *mut c_void) -> Status;
type AllocatePagesFn = unsafe extern "efiapi" fn(
    allocation_type: u32,
    memory_type: u32,
    pages: usize,
    memory: *mut u64,
) -> Status;
type FreePagesFn = unsafe extern "efiapi" fn(memory: u64, pages: usize) -> Status;
type GetMemoryMapFn = unsafe extern "efiapi" fn(
    memory_map_size: *mut usize,
    memory_map: *mut c_void,
    map_key: *mut usize,
    descriptor_size: *mut usize,
    descriptor_version: *mut u32,
) -> Status;
type ExitBootServicesFn = unsafe extern "efiapi" fn(image_handle: Handle, map_key: usize) -> Status;

#[repr(C)]
pub struct BootServices {
    header: TableHeader,
    raise_tpl: usize,
    restore_tpl: usize,
    allocate_pages: Option<AllocatePagesFn>,
    free_pages: Option<FreePagesFn>,
    get_memory_map: Option<GetMemoryMapFn>,
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
    exit_boot_services: Option<ExitBootServicesFn>,
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
    locate_protocol: Option<LocateProtocolFn>,
    install_multiple_protocol_interfaces: usize,
    uninstall_multiple_protocol_interfaces: usize,
    calculate_crc32: usize,
    copy_mem: usize,
    set_mem: usize,
    create_event_ex: usize,
}

#[repr(C)]
struct LoadedImageProtocol {
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
struct SimpleFileSystemProtocol {
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
struct FileProtocol {
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
