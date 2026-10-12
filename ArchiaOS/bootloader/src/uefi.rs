use core::convert::Infallible;
use core::ffi::c_void;
use core::ptr;

use archiaos_bootloader::bootinfo::{
    BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION, UEFI_MEMORY_DESCRIPTOR_VERSION,
};
use archiaos_bootloader::memory_map::UefiMemoryMap;

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

fn rollback_pages(
    boot_services: *mut BootServices,
    starts: &[u64; MAX_LOAD_SEGMENTS],
    page_counts: &[usize; MAX_LOAD_SEGMENTS],
    allocated_count: usize,
) {
    // SAFETY: these page addresses/counts were returned by AllocatePages in this invocation.
    let Some(free_pages) = (unsafe { (*boot_services).free_pages }) else {
        return;
    };
    for index in (0..allocated_count).rev() {
        let _ = unsafe { free_pages(starts[index], page_counts[index]) };
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn flush_instruction_cache(start: u64, length: u64) {
    if length == 0 {
        return;
    }
    let ctr: u64;
    // SAFETY: CTR_EL0 is readable at the kernel's privileged execution level.
    unsafe { core::arch::asm!("mrs {value}, ctr_el0", value = out(reg) ctr, options(nomem, nostack, preserves_flags)) };
    let dcache_line = 4usize << (((ctr >> 16) & 0xf) as usize);
    let icache_line = 4usize << ((ctr & 0xf) as usize);
    let start = start as usize;
    let end = start.saturating_add(length as usize);
    let mut address = start & !(dcache_line - 1);
    while address < end {
        // SAFETY: address lies in the physical range just written by this loader.
        unsafe { core::arch::asm!("dc cvau, {address}", address = in(reg) address, options(nostack, preserves_flags)) };
        address = address.saturating_add(dcache_line);
    }
    // SAFETY: required ordering between data clean and instruction-cache invalidation.
    unsafe { core::arch::asm!("dsb ish", options(nostack, preserves_flags)) };
    address = start & !(icache_line - 1);
    while address < end {
        // SAFETY: address lies in the executable segment just written by this loader.
        unsafe { core::arch::asm!("ic ivau, {address}", address = in(reg) address, options(nostack, preserves_flags)) };
        address = address.saturating_add(icache_line);
    }
    // SAFETY: complete cache maintenance before transferring execution.
    unsafe { core::arch::asm!("dsb ish", "isb", options(nostack, preserves_flags)) };
}

/// Allocate and copy ELF64 PT_LOAD segments. The x86-64 bring-up path accepts
/// identity or bounded high-half images; AArch64 remains identity-only.
pub unsafe fn load_kernel_segments(
    boot_services: *mut BootServices,
    image: &[u8],
    elf: &archiaos_bootloader::elf::ElfFile,
) -> Result<u64, BootError> {
    if boot_services.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: Boot Services unavailable while loading segments.\r\n\0"));
    }
    // SAFETY: AllocatePages is called while boot services are active.
    let allocate_pages = unsafe { (*boot_services).allocate_pages }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: AllocatePages is unavailable.\r\n\0"))?;
    // SAFETY: FreePages is used to roll back successful prior allocations on failure.
    let free_pages = unsafe { (*boot_services).free_pages }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: FreePages is unavailable.\r\n\0"))?;

    let high_half_image = image_uses_highhalf(elf);
    let mut segment_indices = [0u16; MAX_LOAD_SEGMENTS];
    let mut page_starts = [0u64; MAX_LOAD_SEGMENTS];
    let mut page_ends = [0u64; MAX_LOAD_SEGMENTS];
    let mut virtual_page_starts = [0u64; MAX_LOAD_SEGMENTS];
    let mut virtual_page_ends = [0u64; MAX_LOAD_SEGMENTS];
    let mut page_counts = [0usize; MAX_LOAD_SEGMENTS];
    let mut total_memory_size = 0u64;
    let mut segment_count = 0usize;

    for index in 0..elf.program_header_count() {
        let segment = match archiaos_bootloader::elf::load_segment(image, elf, index) {
            Ok(segment) => segment,
            Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
            Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF load segment.\r\n\0")),
        };

        // Enforce W^X across both architectures: no segment may be writable and
        // executable at the same time. The x86-64 page-table builder applies
        // these permissions with NX; AArch64 permission tables will do likewise
        // when its MMU transition is introduced.
        if segment.flags & (PF_W | PF_X) == (PF_W | PF_X) {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: writable-executable ELF segments are rejected by the W^X policy.\r\n\0"));
        }

        let virtual_end = segment.virtual_address
            .checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF virtual address overflow.\r\n\0"))?;
        if segment.virtual_address % EFI_PAGE_SIZE != 0
            || segment.physical_address % EFI_PAGE_SIZE != 0
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ELF load segments must be page-aligned.\r\n\0"));
        }
        if high_half_image {
            #[cfg(target_arch = "x86_64")]
            if segment.virtual_address == segment.physical_address
                || segment.virtual_address < X86_64_HIGH_HALF_BASE
                || virtual_end > X86_64_HIGH_HALF_END
            {
                return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: high-half ELF segments must remain inside the first 128 MiB of the ArchiaOS high-half window.\r\n\0"));
            }
            #[cfg(target_arch = "aarch64")]
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: AArch64 high-half loading is not implemented.\r\n\0"));
        } else if segment.virtual_address != segment.physical_address {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: this ELF image requires virtual mappings not supported by its entry mode.\r\n\0"));
        }
        if segment.memory_size == 0 || segment.memory_size < segment.file_size {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF segment memory size.\r\n\0"));
        }
        total_memory_size = total_memory_size.checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: total ELF memory size overflow.\r\n\0"))?;
        if total_memory_size > MAX_TOTAL_KERNEL_MEMORY_SIZE {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: total ELF segment memory exceeds 128 MiB.\r\n\0"));
        }
        if segment_count >= MAX_LOAD_SEGMENTS {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: too many ELF load segments.\r\n\0"));
        }

        let segment_end = segment.physical_address
            .checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment address overflow.\r\n\0"))?;
        let rounded_end = segment_end
            .checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment page-range overflow.\r\n\0"))?;
        if segment.physical_address < MIN_KERNEL_PHYSICAL_ADDRESS
            || segment_end > MAX_KERNEL_PHYSICAL_ADDRESS
            || rounded_end > MAX_KERNEL_PHYSICAL_ADDRESS
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ELF segment is outside the supported 1 MiB..4 GiB physical load window.\r\n\0"));
        }
        let rounded_length = rounded_end
            .checked_sub(segment.physical_address)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF page range.\r\n\0"))?;
        let rounded_virtual_end = virtual_end
            .checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF virtual page-range overflow.\r\n\0"))?;
        #[cfg(target_arch = "x86_64")]
        if high_half_image && rounded_virtual_end > X86_64_HIGH_HALF_END {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: rounded high-half segment exceeds the page-table window.\r\n\0"));
        }
        let pages = usize::try_from(rounded_length / EFI_PAGE_SIZE)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF allocation size overflows UINTN.\r\n\0"))?;
        if pages == 0 {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: empty ELF page allocation.\r\n\0"));
        }
        let virtual_page_start = segment.virtual_address;
        for prior in 0..segment_count {
            if segment.physical_address < page_ends[prior]
                && page_starts[prior] < rounded_end
            {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segments overlap physically after page rounding.\r\n\0"));
            }
            if virtual_page_start < virtual_page_ends[prior]
                && virtual_page_starts[prior] < rounded_virtual_end
            {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segments overlap virtually after page rounding.\r\n\0"));
            }
        }
        segment_indices[segment_count] = index;
        page_starts[segment_count] = segment.physical_address;
        page_ends[segment_count] = rounded_end;
        virtual_page_starts[segment_count] = virtual_page_start;
        virtual_page_ends[segment_count] = rounded_virtual_end;
        page_counts[segment_count] = pages;
        segment_count += 1;
    }

    if segment_count == 0 {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF has no loadable segments.\r\n\0"));
    }

    let mut allocated_starts = [0u64; MAX_LOAD_SEGMENTS];
    let mut allocated_pages = [0usize; MAX_LOAD_SEGMENTS];
    let mut allocated_count = 0usize;
    let mut found_entry = false;

    for item in 0..segment_count {
        let segment = match archiaos_bootloader::elf::load_segment(image, elf, segment_indices[item]) {
            Ok(segment) => segment,
            Err(_) => {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment changed after validation.\r\n\0"));
            }
        };
        let mut address = page_starts[item];
        let memory_type = if segment.flags & PF_X != 0 { EFI_LOADER_CODE } else { EFI_LOADER_DATA };
        // SAFETY: address and page count were checked; AllocateAddress requests the exact physical range.
        let status = unsafe { allocate_pages(EFI_ALLOCATE_ADDRESS, memory_type, page_counts[item], &mut address) };
        if status != EFI_SUCCESS {
            rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
            return Err(error(status, b"ArchiaOS: firmware could not reserve an ELF physical range.\r\n\0"));
        }
        if address != page_starts[item] {
            let _ = unsafe { free_pages(address, page_counts[item]) };
            rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware allocated ELF pages at an unexpected address.\r\n\0"));
        }
        allocated_starts[allocated_count] = address;
        allocated_pages[allocated_count] = page_counts[item];
        allocated_count += 1;

        let allocation_length = page_counts[item]
            .checked_mul(EFI_PAGE_SIZE as usize)
            .ok_or_else(|| {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                error(EFI_LOAD_ERROR, b"ArchiaOS: segment allocation length overflow.\r\n\0")
            })?;
        // SAFETY: UEFI pages are addressable during boot services and the allocated range is writable.
        let destination = address as *mut u8;
        unsafe { ptr::write_bytes(destination, 0, allocation_length) };

        let file_offset = usize::try_from(segment.file_offset)
            .map_err(|_| {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                error(EFI_LOAD_ERROR, b"ArchiaOS: ELF file offset overflows UINTN.\r\n\0")
            })?;
        let file_size = usize::try_from(segment.file_size)
            .map_err(|_| {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment file size overflows UINTN.\r\n\0")
            })?;
        let source_end = file_offset.checked_add(file_size)
            .ok_or_else(|| {
                rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
                error(EFI_LOAD_ERROR, b"ArchiaOS: ELF source range overflows.\r\n\0")
            })?;
        let source = image.get(file_offset..source_end).ok_or_else(|| {
            rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
            error(EFI_LOAD_ERROR, b"ArchiaOS: ELF segment lies outside the kernel file.\r\n\0")
        })?;
        // SAFETY: file range is validated; destination is a distinct newly allocated range.
        unsafe { ptr::copy_nonoverlapping(source.as_ptr(), destination, source.len()) };

        if segment.flags & PF_X != 0 {
            let segment_end = segment.virtual_address + segment.memory_size;
            found_entry |= elf.entry() >= segment.virtual_address && elf.entry() < segment_end;
            #[cfg(target_arch = "aarch64")]
            // SAFETY: executable bytes have just been copied to this allocated range.
            unsafe { flush_instruction_cache(segment.physical_address, segment.memory_size) };
        }
    }

    if !found_entry {
        rollback_pages(boot_services, &allocated_starts, &allocated_pages, allocated_count);
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF entry is not in an executable loaded segment.\r\n\0"));
    }
    Ok(elf.entry())
}


#[cfg(target_arch = "x86_64")]
pub fn entry_uses_highhalf(entry: u64) -> bool {
    entry >= X86_64_HIGH_HALF_BASE && entry < X86_64_HIGH_HALF_END
}

#[cfg(target_arch = "aarch64")]
pub fn entry_uses_highhalf(_entry: u64) -> bool {
    false
}

fn image_uses_highhalf(elf: &archiaos_bootloader::elf::ElfFile) -> bool {
    entry_uses_highhalf(elf.entry())
}

#[cfg(target_arch = "x86_64")]
fn cpu_supports_nx() -> bool {
    // Extended CPUID leaf 0x80000001 EDX bit 20 advertises the architectural
    // NX bit. Refuse high-half paging before constructing NX-marked PTEs when
    // the CPU cannot enable EFER.NXE.
    let maximum_leaf = unsafe { core::arch::x86_64::__cpuid(0x8000_0000) }.eax;
    if maximum_leaf < 0x8000_0001 {
        return false;
    }
    let features = unsafe { core::arch::x86_64::__cpuid(0x8000_0001) };
    features.edx & (1 << 20) != 0
}

#[cfg(target_arch = "x86_64")]
unsafe fn loaded_image_physical_range(
    image_handle: Handle,
    system_table: *mut SystemTable,
) -> Result<(u64, u64), BootError> {
    if image_handle.is_null() || system_table.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: invalid handle while locating loader image.\r\n\0"));
    }
    // SAFETY: SystemTable comes from the active UEFI application entry point.
    let boot_services = unsafe { (*system_table).boot_services };
    if boot_services.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: Boot Services unavailable while locating loader image.\r\n\0"));
    }
    let handle_protocol = unsafe { (*boot_services).handle_protocol }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: HandleProtocol is unavailable for loader-image mapping.\r\n\0"))?;
    let mut loaded_image_raw = ptr::null_mut();
    // SAFETY: the protocol GUID and output pointer are valid during boot services.
    let status = unsafe { handle_protocol(image_handle, &LOADED_IMAGE_GUID, &mut loaded_image_raw) };
    if status != EFI_SUCCESS || loaded_image_raw.is_null() {
        return Err(error(
            if status == EFI_SUCCESS { EFI_LOAD_ERROR } else { status },
            b"ArchiaOS: cannot query loaded-image range for the page-table transition.\r\n\0",
        ));
    }
    let loaded = loaded_image_raw.cast::<LoadedImageProtocol>();
    // SAFETY: HandleProtocol returned the LoadedImageProtocol interface.
    let (image_base, image_size) = unsafe {
        ((*loaded).image_base as u64, (*loaded).image_size)
    };
    let image_end = image_base.checked_add(image_size)
        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: loader-image range overflow.\r\n\0"))?;
    let start = image_base & !(EFI_PAGE_SIZE - 1);
    let end = image_end.checked_add(EFI_PAGE_SIZE - 1)
        .map(|value| value & !(EFI_PAGE_SIZE - 1))
        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: rounded loader-image range overflow.\r\n\0"))?;
    // The loader may be placed above 4 GiB by firmware on machines with
    // large memory. The x86-64 four-level page tables can identity-map it as
    // long as it remains in the low canonical 512 GiB region (PML4[0]).
    // RAM below 4 GiB is mapped from the UEFI memory map; the loader's own
    // pages above 4 GiB are added explicitly during page-table construction.
    const MAX_LOW_CANONICAL_IDENTITY_ADDRESS: u64 = 1u64 << 39;
    if image_base == 0
        || image_size == 0
        || start >= end
        || end > MAX_LOW_CANONICAL_IDENTITY_ADDRESS
    {
        return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: loader image is outside the low-canonical identity-map range (512 GiB).\r\n\0"));
    }
    Ok((start, end))
}

#[cfg(target_arch = "x86_64")]
struct PageTableArena {
    physical_base: u64,
    base: *mut u64,
    used_pages: usize,
    boot_services: *mut BootServices,
    owns_allocation: bool,
}

#[cfg(target_arch = "x86_64")]
impl PageTableArena {
    unsafe fn allocate(boot_services: *mut BootServices) -> Result<Self, BootError> {
        let allocate_pages = unsafe { (*boot_services).allocate_pages }
            .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: AllocatePages is unavailable for page tables.\r\n\0"))?;
        let mut physical_base = MAX_KERNEL_PHYSICAL_ADDRESS - 1;
        let status = unsafe {
            allocate_pages(
                1, // EFI_ALLOCATE_MAX_ADDRESS
                EFI_LOADER_DATA,
                PAGE_TABLE_ARENA_PAGES,
                &mut physical_base,
            )
        };
        if status != EFI_SUCCESS {
            return Err(error(status, b"ArchiaOS: could not reserve the x86-64 page-table arena below 4 GiB.\r\n\0"));
        }
        let bytes = (PAGE_TABLE_ARENA_PAGES as u64)
            .checked_mul(EFI_PAGE_SIZE)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena size overflow.\r\n\0"))?;
        let end = physical_base.checked_add(bytes)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena address overflow.\r\n\0"))?;
        if physical_base % EFI_PAGE_SIZE != 0 || end > MAX_KERNEL_PHYSICAL_ADDRESS {
            if let Some(free_pages) = unsafe { (*boot_services).free_pages } {
                let _ = unsafe { free_pages(physical_base, PAGE_TABLE_ARENA_PAGES) };
            }
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware returned an invalid page-table arena.\r\n\0"));
        }
        let arena = Self {
            physical_base,
            base: physical_base as *mut u64,
            used_pages: 0,
            boot_services,
            owns_allocation: true,
        };
        // The arena allocation is reserved loader data; clear once before table construction.
        unsafe {
            ptr::write_bytes(
                arena.base,
                0,
                PAGE_TABLE_ARENA_PAGES * (EFI_PAGE_SIZE as usize / core::mem::size_of::<u64>()),
            );
        }
        Ok(arena)
    }

    fn reset(&mut self) {
        self.used_pages = 0;
    }

    fn allocate_page(&mut self) -> Result<*mut u64, BootError> {
        if self.used_pages >= PAGE_TABLE_ARENA_PAGES {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: x86-64 page-table arena exhausted.\r\n\0"));
        }
        let offset = self.used_pages * (EFI_PAGE_SIZE as usize / core::mem::size_of::<u64>());
        let page = unsafe { self.base.add(offset) };
        unsafe { ptr::write_bytes(page, 0, EFI_PAGE_SIZE as usize / core::mem::size_of::<u64>()) };
        self.used_pages += 1;
        Ok(page)
    }

    fn physical_address(&self, page: *mut u64) -> u64 {
        self.physical_base + (page as u64 - self.base as u64)
    }

    fn disarm(&mut self) {
        self.owns_allocation = false;
    }
}

#[cfg(target_arch = "x86_64")]
impl Drop for PageTableArena {
    fn drop(&mut self) {
        if !self.owns_allocation || self.boot_services.is_null() {
            return;
        }
        // SAFETY: the arena came from AllocatePages and this destructor runs before EBS.
        if let Some(free_pages) = unsafe { (*self.boot_services).free_pages } {
            let _ = unsafe { free_pages(self.physical_base, PAGE_TABLE_ARENA_PAGES) };
        }
        self.owns_allocation = false;
    }
}

#[cfg(target_arch = "x86_64")]
fn uefi_memory_type_is_ram(memory_type: u32) -> bool {
    // Include loader/services/conventional RAM and ACPI reclaim/NVS/persistent
    // RAM. Exclude MMIO and reserved descriptors to avoid caching device ranges.
    matches!(memory_type, 1..=7 | 9 | 10 | 14)
}

#[cfg(target_arch = "x86_64")]
unsafe fn build_x86_64_page_tables(
    arena: &mut PageTableArena,
    memory_map: &UefiMemoryMap<'_>,
    image: &[u8],
    elf: &archiaos_bootloader::elf::ElfFile,
    loader_image_start: u64,
    loader_image_end: u64,
    framebuffer_base: u64,
    framebuffer_size: u64,
) -> Result<u64, BootError> {
    arena.reset();
    let pml4 = arena.allocate_page()?;
    let identity_pdpt = arena.allocate_page()?;
    let mut identity_pd = [ptr::null_mut::<u64>(); 4];
    let high_pdpt = arena.allocate_page()?;
    let high_pd = arena.allocate_page()?;

    for index in 0..4 {
        let page = arena.allocate_page()?;
        identity_pd[index] = page;
        // P=1, RW=1, physical addresses in the identity region use 4 KiB leaves.
        unsafe { ptr::write(identity_pdpt.add(index), arena.physical_address(page) | 0x3) };
    }
    unsafe {
        ptr::write(pml4, arena.physical_address(identity_pdpt) | 0x3);
        ptr::write(pml4.add(511), arena.physical_address(high_pdpt) | 0x3);
        ptr::write(high_pdpt.add(510), arena.physical_address(high_pd) | 0x3);
    }

    // Index the identity PTs by their 2 MiB physical span. The fixed arena can
    // map all RAM below 4 GiB with 4 KiB leaves, while leaving reserved/MMIO
    // descriptors unmapped instead of giving devices ordinary write-back aliases.
    let mut identity_tables = [0u64; X86_64_IDENTITY_PT_SPANS];
    for index in 0..memory_map.len() {
        let descriptor = memory_map.entry(index)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid descriptor during x86-64 page-table construction.\r\n\0"))?;
        if !uefi_memory_type_is_ram(descriptor.memory_type) {
            continue;
        }
        let mut address = descriptor.physical_start;
        let end = descriptor.physical_end()
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: memory descriptor overflow during page-table construction.\r\n\0"))?
            .min(MAX_KERNEL_PHYSICAL_ADDRESS);
        // GOP memory is not ordinary RAM even if buggy firmware reports it
        // inside an EfiConventionalMemory descriptor. Leave its identity PTE absent
        // until the graphics subsystem installs an explicitly uncached MMIO mapping.
        let framebuffer_end = framebuffer_base.checked_add(framebuffer_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer range overflow during page-table construction.\r\n\0"))?;
        while address < end {
            let in_framebuffer = framebuffer_size != 0
                && address < framebuffer_end
                && framebuffer_base < address + EFI_PAGE_SIZE;
            if address != 0 && !in_framebuffer {
                let span = (address / (2 * 1024 * 1024)) as usize;
                if span >= identity_tables.len() {
                    return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: physical memory span exceeds the identity map.\r\n\0"));
                }
                let mut table_address = identity_tables[span];
                if table_address == 0 {
                    let table = arena.allocate_page()?;
                    table_address = arena.physical_address(table);
                    identity_tables[span] = table_address;
                    let pdpt_index = (address >> 30) as usize;
                    let pd_index = ((address >> 21) & 0x1ff) as usize;
                    unsafe {
                        ptr::write(identity_pd[pdpt_index].add(pd_index), table_address | 0x3);
                    }
                }
                let pte_index = ((address >> 12) & 0x1ff) as usize;
                unsafe {
                    ptr::write(
                        (table_address as *mut u64).add(pte_index),
                        address | 0x3 | X86_64_PTE_NX,
                    );
                }
            }
            address = address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: identity-map page iteration overflow.\r\n\0"))?;
        }
    }

    // Map the GOP framebuffer explicitly as uncached device memory. This also
    // supports GOP apertures above 4 GiB, which some physical UEFI firmware uses.
    if framebuffer_size != 0 {
        const MAX_LOW_CANONICAL_IDENTITY_ADDRESS: u64 = 1u64 << 39;
        let framebuffer_end = framebuffer_base.checked_add(framebuffer_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer range overflow while mapping GOP memory.\r\n\0"))?;
        if framebuffer_base == 0 || framebuffer_end > MAX_LOW_CANONICAL_IDENTITY_ADDRESS {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: GOP framebuffer is outside the low-canonical 512 GiB identity map.\r\n\0"));
        }
        let mut address = framebuffer_base & !(EFI_PAGE_SIZE - 1);
        let rounded_end = framebuffer_end.checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer page range overflow.\r\n\0"))?;
        while address < rounded_end {
            let table_address = if address < MAX_KERNEL_PHYSICAL_ADDRESS {
                // Reuse the low-memory page tables that also map ordinary RAM.
                let span = (address / (2 * 1024 * 1024)) as usize;
                if span >= identity_tables.len() {
                    return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: framebuffer lies outside the supported identity map.\r\n\0"));
                }
                let mut table_address = identity_tables[span];
                if table_address == 0 {
                    let table = arena.allocate_page()?;
                    table_address = arena.physical_address(table);
                    identity_tables[span] = table_address;
                    let pdpt_index = (address >> 30) as usize;
                    let pd_index = ((address >> 21) & 0x1ff) as usize;
                    unsafe { ptr::write(identity_pd[pdpt_index].add(pd_index), table_address | 0x3) };
                }
                table_address
            } else {
                // Add only the framebuffer's pages above 4 GiB; never create a
                // write-back RAM alias for the surrounding device aperture.
                let pdpt_index = (address >> 30) as usize;
                let pdpt_entry = unsafe { ptr::read(identity_pdpt.add(pdpt_index)) };
                let pd = if pdpt_entry & 1 == 0 {
                    let page = arena.allocate_page()?;
                    let physical = arena.physical_address(page);
                    unsafe { ptr::write(identity_pdpt.add(pdpt_index), physical | 0x3) };
                    page
                } else {
                    (pdpt_entry & 0x000f_ffff_ffff_f000) as *mut u64
                };
                let pd_index = ((address >> 21) & 0x1ff) as usize;
                let pd_entry = unsafe { ptr::read(pd.add(pd_index)) };
                let pt = if pd_entry & 1 == 0 {
                    let page = arena.allocate_page()?;
                    let physical = arena.physical_address(page);
                    unsafe { ptr::write(pd.add(pd_index), physical | 0x3) };
                    page
                } else {
                    (pd_entry & 0x000f_ffff_ffff_f000) as *mut u64
                };
                arena.physical_address(pt)
            };
            let pte_index = ((address >> 12) & 0x1ff) as usize;
            // P=1, RW=1, PWT=1, PCD=1, NX=1: uncached writable framebuffer.
            unsafe {
                ptr::write(
                    (table_address as *mut u64).add(pte_index),
                    address | 0x1 | 0x2 | 0x8 | 0x10 | X86_64_PTE_NX,
                );
            }
            address = address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer mapping iteration overflow.\r\n\0"))?;
        }
    }

    // The default identity map is RW+NX. Override the kernel PT_LOAD pages so
    // their identity aliases cannot bypass high-half W^X: kernel code and rodata
    // are read-only/NX, while data is writable/NX.
    for index in 0..elf.program_header_count() {
        let segment = match archiaos_bootloader::elf::load_segment(image, elf, index) {
            Ok(segment) => segment,
            Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
            Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF segment while protecting identity aliases.\r\n\0")),
        };
        let segment_end = segment.physical_address.checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: physical segment range overflow while protecting identity aliases.\r\n\0"))?;
        let rounded_end = segment_end.checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: rounded physical segment overflow while protecting identity aliases.\r\n\0"))?;
        let flags = 0x1
            | if segment.flags & PF_W != 0 { 0x2 } else { 0 }
            | X86_64_PTE_NX;
        let mut address = segment.physical_address;
        while address < rounded_end {
            let span = (address / (2 * 1024 * 1024)) as usize;
            if span >= identity_tables.len() || identity_tables[span] == 0 {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: kernel identity alias lacks a page table.\r\n\0"));
            }
            let table = identity_tables[span] as *mut u64;
            let pte_index = ((address >> 12) & 0x1ff) as usize;
            unsafe { ptr::write(table.add(pte_index), address | flags) };
            address = address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: kernel identity-map iteration overflow.\r\n\0"))?;
        }
    }

    // The CR3 transition instructions execute from the UEFI loader image after
    // the switch. Keep that image read-only/executable in the identity map,
    // while all kernel physical aliases remain NX.
    let mut loader_address = loader_image_start;
    while loader_address < loader_image_end {
        let pte_index = ((loader_address >> 12) & 0x1ff) as usize;
        if loader_address < MAX_KERNEL_PHYSICAL_ADDRESS {
            // Under 4 GiB, reuse the page tables created from RAM descriptors.
            let span = (loader_address / (2 * 1024 * 1024)) as usize;
            if span >= identity_tables.len() || identity_tables[span] == 0 {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: loader image is outside mapped RAM below 4 GiB.\r\n\0"));
            }
            let table = identity_tables[span] as *mut u64;
            unsafe { ptr::write(table.add(pte_index), loader_address | 0x1) };
        } else {
            // Firmware can load this EFI application above 4 GiB. Map only
            // its own pages there, preserving the bounded <4 GiB RAM map used
            // by the PMM. All paging structures themselves remain below 4 GiB.
            let pdpt_index = (loader_address >> 30) as usize;
            if pdpt_index >= 512 {
                return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: loader image exceeds the low-canonical identity map.\r\n\0"));
            }
            let pdpt_entry = unsafe { ptr::read(identity_pdpt.add(pdpt_index)) };
            let pd = if pdpt_entry & 1 == 0 {
                let page = arena.allocate_page()?;
                let physical = arena.physical_address(page);
                unsafe { ptr::write(identity_pdpt.add(pdpt_index), physical | 0x3) };
                page
            } else {
                (pdpt_entry & 0x000f_ffff_ffff_f000) as *mut u64
            };
            let pd_index = ((loader_address >> 21) & 0x1ff) as usize;
            let pd_entry = unsafe { ptr::read(pd.add(pd_index)) };
            let pt = if pd_entry & 1 == 0 {
                let page = arena.allocate_page()?;
                let physical = arena.physical_address(page);
                unsafe { ptr::write(pd.add(pd_index), physical | 0x3) };
                page
            } else {
                (pd_entry & 0x000f_ffff_ffff_f000) as *mut u64
            };
            unsafe { ptr::write(pt.add(pte_index), loader_address | 0x1) };
        }
        loader_address = loader_address.checked_add(EFI_PAGE_SIZE)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: loader-image identity-map iteration overflow.\r\n\0"))?;
    }

    // Map each validated ELF segment at its high-half virtual address, using
    // its physical load address. The supported virtual window is 128 MiB.
    let mut high_tables = [0u64; X86_64_HIGH_PT_SPANS];
    let mut entry_is_executable = false;
    for index in 0..elf.program_header_count() {
        let segment = match archiaos_bootloader::elf::load_segment(image, elf, index) {
            Ok(segment) => segment,
            Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
            Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid high-half ELF segment.\r\n\0")),
        };
        let segment_end = segment.virtual_address.checked_add(segment.memory_size)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half virtual range overflow.\r\n\0"))?;
        if segment.virtual_address < X86_64_HIGH_HALF_BASE
            || segment_end > X86_64_HIGH_HALF_END
            || segment.virtual_address % EFI_PAGE_SIZE != 0
            || segment.physical_address % EFI_PAGE_SIZE != 0
            || segment.virtual_address == segment.physical_address
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ELF segment does not fit the supported high-half window.\r\n\0"));
        }
        entry_is_executable |= segment.flags & PF_X != 0
            && elf.entry() >= segment.virtual_address
            && elf.entry() < segment_end;

        let mut virtual_address = segment.virtual_address;
        let mut physical_address = segment.physical_address;
        let rounded_end = segment_end.checked_add(EFI_PAGE_SIZE - 1)
            .map(|value| value & !(EFI_PAGE_SIZE - 1))
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half page-range overflow.\r\n\0"))?;
        // High-half leaves mirror ELF permissions: code is read-only/executable,
        // rodata read-only/NX, and data writable/NX. RWX segments were rejected
        // before allocation by the common W^X policy.
        let pte_flags = 0x1
            | if segment.flags & PF_W != 0 { 0x2 } else { 0 }
            | if segment.flags & PF_X == 0 { X86_64_PTE_NX } else { 0 };
        while virtual_address < rounded_end {
            let span = ((virtual_address - X86_64_HIGH_HALF_BASE) / (2 * 1024 * 1024)) as usize;
            if span >= high_tables.len() {
                return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: high-half page-table span exceeds 128 MiB.\r\n\0"));
            }
            let mut table_address = high_tables[span];
            if table_address == 0 {
                let table = arena.allocate_page()?;
                table_address = arena.physical_address(table);
                high_tables[span] = table_address;
                let pd_index = ((virtual_address >> 21) & 0x1ff) as usize;
                unsafe { ptr::write(high_pd.add(pd_index), table_address | 0x3) };
            }
            let pte_index = ((virtual_address >> 12) & 0x1ff) as usize;
            unsafe {
                ptr::write(
                    (table_address as *mut u64).add(pte_index),
                    physical_address | pte_flags,
                );
            }
            virtual_address = virtual_address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half VA iteration overflow.\r\n\0"))?;
            physical_address = physical_address.checked_add(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half PA iteration overflow.\r\n\0"))?;
        }
    }
    if !entry_is_executable {
        return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ELF entry is not in an executable high-half segment.\r\n\0"));
    }

    Ok(arena.physical_address(pml4))
}

fn configuration_table_range_is_bounded(base: usize, count: usize) -> bool {
    if base == 0 || count == 0 || count > 4096 {
        return false;
    }
    let Some(bytes) = count.checked_mul(core::mem::size_of::<ConfigurationTable>()) else {
        return false;
    };
    bytes <= isize::MAX as usize && base.checked_add(bytes).is_some()
}

fn configuration_table(system_table: *mut SystemTable, wanted: &Guid) -> *mut c_void {
    if system_table.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: firmware provides the configuration-table array and count.
    let (count, tables) = unsafe {
        ((*system_table).number_of_table_entries, (*system_table).configuration_table)
    };
    if !configuration_table_range_is_bounded(tables as usize, count) {
        return ptr::null_mut();
    }
    for index in 0..count {
        // Firmware metadata is external input: read unaligned so malformed alignment
        // cannot trigger undefined behavior before the table's GUID is checked.
        // SAFETY: the index is bounded and the byte-range arithmetic was checked above.
        let candidate = unsafe { ptr::read_unaligned(tables.cast::<u8>().add(
            index * core::mem::size_of::<ConfigurationTable>()
        ).cast::<ConfigurationTable>()) };
        if candidate.vendor_guid == *wanted && !candidate.vendor_table.is_null() {
            return candidate.vendor_table;
        }
    }
    ptr::null_mut()
}

unsafe fn validated_rsdp(pointer: *mut c_void) -> u64 {
    if pointer.is_null() {
        return 0;
    }
    let address = pointer.cast::<u8>();
    // Check the fixed signature before interpreting revision or length fields.
    // SAFETY: pointer is supplied by the firmware ACPI configuration table.
    let signature = unsafe { core::slice::from_raw_parts(address, 8) };
    if !rsdp_signature_is_valid(signature) {
        return 0;
    }
    // SAFETY: the ACPI RSDP contains a fixed legacy prefix of 20 bytes.
    let legacy = unsafe { core::slice::from_raw_parts(address, 20) };
    if legacy.get(15).copied() == Some(0) {
        return if archiaos_bootloader::acpi::parse_rsdp(legacy).is_ok() {
            pointer as u64
        } else {
            0
        };
    }
    // Extended RSDP structures contain at least 36 bytes including their length.
    // SAFETY: the ACPI 2.0 firmware table entry points to an extended RSDP.
    let fixed = unsafe { core::slice::from_raw_parts(address, 36) };
    if fixed.get(15).copied().unwrap_or(0) < 2 {
        return 0;
    }
    let length = u32::from_le_bytes([fixed[20], fixed[21], fixed[22], fixed[23]]) as usize;
    if !(36..=archiaos_bootloader::acpi::MAX_RSDP_SIZE).contains(&length) {
        return 0;
    }
    // SAFETY: advertised length is bounded and belongs to the firmware table.
    let complete = unsafe { core::slice::from_raw_parts(address, length) };
    if archiaos_bootloader::acpi::parse_rsdp(complete).is_ok() {
        pointer as u64
    } else {
        0
    }
}

unsafe fn validated_device_tree(pointer: *mut c_void) -> u64 {
    if pointer.is_null() {
        return 0;
    }
    let address = pointer.cast::<u8>();
    // SAFETY: pointer is returned by EFI_DTB_TABLE_GUID; the fixed prefix is read
    // before its advertised total size is trusted.
    let prefix = unsafe { core::slice::from_raw_parts(address, 8) };
    let Some(total) = validated_fdt_prefix_length(prefix) else {
        return 0;
    };
    let total = total as usize;
    // SAFETY: total is bounded by the parser limit and firmware supplied the DTB address.
    let tree = unsafe { core::slice::from_raw_parts(address, total) };
    if archiaos_bootloader::device_tree::validate_tree(tree).is_ok() {
        pointer as u64
    } else {
        0
    }
}

/// Return the active GOP framebuffer range when firmware exposes a usable mode.
/// This is metadata discovery only: it does not write pixels or assume a pixel format.
unsafe fn discover_framebuffer(boot_services: *mut BootServices) -> (u64, u64) {
    if boot_services.is_null() {
        return (0, 0);
    }
    let Some(locate_protocol) = (unsafe { (*boot_services).locate_protocol }) else {
        return (0, 0);
    };
    let mut raw = ptr::null_mut();
    // SAFETY: GOP is queried while UEFI boot services are active; the output is writable.
    let status = unsafe {
        locate_protocol(
            &GRAPHICS_OUTPUT_PROTOCOL_GUID,
            ptr::null_mut(),
            &mut raw,
        )
    };
    if status != EFI_SUCCESS || raw.is_null() {
        return (0, 0);
    }
    let gop = raw.cast::<GraphicsOutputProtocol>();
    // SAFETY: LocateProtocol returned the GOP interface for the active firmware mode.
    let mode = unsafe { (*gop).mode };
    if mode.is_null() {
        return (0, 0);
    }
    // SAFETY: a valid GOP mode carries its mode information and framebuffer range.
    let mode_info = unsafe { (*mode).info };
    let base = unsafe { (*mode).framebuffer_base };
    let size = unsafe { (*mode).framebuffer_size } as u64;
    if mode_info.is_null() || base == 0 || size == 0 || base.checked_add(size).is_none() {
        return (0, 0);
    }
    // Do not publish a partial or blit-only mode as a linear framebuffer. The
    // kernel receives only the physical range, so the byte capacity must cover
    // every scan line and the mode must expose a directly addressable pixel format.
    let info = unsafe { &*mode_info };
    if !archiaos_bootloader::framebuffer::is_usable(
        base,
        size,
        info.horizontal_resolution,
        info.vertical_resolution,
        info.pixel_format,
        info.pixels_per_scan_line,
    ) {
        return (0, 0);
    }
    (base, size)
}

#[cfg(target_arch = "x86_64")]
fn range_is_identity_mapped_ram(
    memory_map: &UefiMemoryMap<'_>,
    start: u64,
    length: u64,
) -> Result<bool, BootError> {
    let end = start.checked_add(length)
        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: platform-table range overflow.\r\n\0"))?;
    if start == 0 || end > MAX_KERNEL_PHYSICAL_ADDRESS {
        return Ok(false);
    }
    let mut cursor = start;
    while cursor < end {
        let mut next = cursor;
        for index in 0..memory_map.len() {
            let descriptor = memory_map.entry(index)
                .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid descriptor while checking platform-table mappings.\r\n\0"))?;
            let descriptor_end = descriptor.physical_end()
                .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: descriptor overflow while checking platform-table mappings.\r\n\0"))?;
            if descriptor.physical_start <= cursor && cursor < descriptor_end {
                // Match the x86-64 page-table builder: MMIO and generic reserved
                // descriptors are deliberately not identity-mapped as normal RAM.
                if !uefi_memory_type_is_ram(descriptor.memory_type) {
                    return Ok(false);
                }
                next = descriptor_end.min(end);
                break;
            }
        }
        if next == cursor {
            return Ok(false);
        }
        cursor = next;
    }
    Ok(true)
}

/// Validate the ACPI signature before interpreting firmware-controlled RSDP fields.
fn rsdp_signature_is_valid(signature: &[u8]) -> bool {
    signature == archiaos_bootloader::acpi::RSDP_SIGNATURE
}

/// Enforce the distinct ACPI 1.0 and extended RSDP minimum lengths during handoff.
/// Extended RSDPs include fields through byte 35; accepting a 20-byte prefix for
/// revision >= 2 would let mutated firmware metadata pass the final-map audit.
fn rsdp_length_is_valid(revision: u8, length: u64) -> bool {
    // ACPI revision 1 is reserved. Only revision 0 uses the 20-byte legacy
    // structure; revision 2 and later must expose the complete extended prefix.
    let minimum = match revision {
        0 => 20,
        1 => return false,
        _ => 36,
    };
    (minimum..=archiaos_bootloader::acpi::MAX_RSDP_SIZE as u64).contains(&length)
}

/// Recheck ACPI RSDP checksums at handoff, not only during initial discovery.
/// ACPI 2.0+ requires both the legacy first-20-byte checksum and the full-table checksum.
fn rsdp_checksums_are_valid(bytes: &[u8], revision: u8, length: u64) -> bool {
    if !rsdp_length_is_valid(revision, length) || bytes.len() as u64 != length {
        return false;
    }
    if bytes[..20].iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
        return false;
    }
    revision < 2 || bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) == 0
}

/// Parse only the fixed FDT header prefix without trusting firmware-provided lengths.
fn validated_fdt_prefix_length(header: &[u8]) -> Option<u64> {
    if header.len() < 8
        || u32::from_be_bytes([header[0], header[1], header[2], header[3]])
            != archiaos_bootloader::device_tree::FDT_MAGIC
    {
        return None;
    }
    let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as u64;
    if !(archiaos_bootloader::device_tree::HEADER_SIZE as u64
        ..=archiaos_bootloader::device_tree::MAX_TOTAL_SIZE as u64).contains(&length)
    {
        return None;
    }
    Some(length)
}

/// Conservatively reject overlapping handoff ranges; arithmetic overflow fails closed.
fn physical_ranges_overlap(first_start: u64, first_length: u64, second_start: u64, second_length: u64) -> bool {
    if first_length == 0 || second_length == 0 {
        return false;
    }
    let Some(first_end) = first_start.checked_add(first_length) else {
        return true;
    };
    let Some(second_end) = second_start.checked_add(second_length) else {
        return true;
    };
    first_start < second_end && second_start < first_end
}

/// Ensure firmware platform-table storage remains reserved in the final handoff map.
/// These pointers are consumed after ExitBootServices, so they must never be reclassified
/// as conventional RAM that the physical allocator could hand to another subsystem.
fn audit_platform_table_ranges(
    memory_map: &UefiMemoryMap<'_>,
    acpi_rsdp: u64,
    device_tree: u64,
    high_half_image: bool,
    framebuffer_base: u64,
    framebuffer_size: u64,
) -> Result<(), BootError> {
    #[cfg(target_arch = "aarch64")]
    let _ = high_half_image;
    if acpi_rsdp != 0 {
        // Prove the fixed prefix is retained and readable in the kernel's active
        // mapping before dereferencing firmware-controlled length/revision fields.
        const RSDP_FIXED_PREFIX: u64 = 24;
        if !memory_map.range_is_fully_reserved(acpi_rsdp, RSDP_FIXED_PREFIX)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed RSDP prefix in final memory map.\r\n\0"))?
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP prefix is not fully reserved in the final memory map.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if high_half_image
            && !range_is_identity_mapped_ram(memory_map, acpi_rsdp, RSDP_FIXED_PREFIX)?
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ACPI RSDP prefix is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
        }
        if physical_ranges_overlap(acpi_rsdp, RSDP_FIXED_PREFIX, framebuffer_base, framebuffer_size) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP overlaps the GOP framebuffer.\r\n\0"));
        }

        let rsdp = acpi_rsdp as *const u8;
        // SAFETY: the fixed prefix is covered by reserved RAM and (for high-half
        // x86-64) the identity map, as checked above.
        let signature = unsafe { core::slice::from_raw_parts(rsdp, 8) };
        if !rsdp_signature_is_valid(signature) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ACPI RSDP signature during final memory-map audit.\r\n\0"));
        }
        let revision = unsafe { ptr::read(rsdp.add(15)) };
        let length = if revision < 2 {
            20
        } else {
            // SAFETY: revision >= 2 means the validated fixed prefix includes length.
            let field = unsafe { core::slice::from_raw_parts(rsdp.add(20), 4) };
            u32::from_le_bytes([field[0], field[1], field[2], field[3]]) as u64
        };
        if !rsdp_length_is_valid(revision, length) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid RSDP size during final memory-map audit.\r\n\0"));
        }
        if !memory_map.range_is_fully_reserved(acpi_rsdp, length)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed RSDP range in final memory map.\r\n\0"))?
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP storage is not fully reserved in the final memory map.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if high_half_image
            && !range_is_identity_mapped_ram(memory_map, acpi_rsdp, length)?
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: ACPI RSDP is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
        }
        if physical_ranges_overlap(acpi_rsdp, length, framebuffer_base, framebuffer_size) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: ACPI RSDP overlaps the GOP framebuffer.\r\n\0"));
        }
        // SAFETY: the complete length is bounded, reserved, and identity-mapped
        // where required, as checked above.
        let complete = unsafe { core::slice::from_raw_parts(rsdp, length as usize) };
        if !rsdp_checksums_are_valid(complete, revision, length) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ACPI RSDP checksum during final memory-map audit.\r\n\0"));
        }
    }

    if device_tree != 0 {
        const FDT_FIXED_PREFIX: u64 = 8;
        if !memory_map.range_is_fully_reserved(device_tree, FDT_FIXED_PREFIX)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed Device Tree prefix in final memory map.\r\n\0"))?
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree prefix is not fully reserved in the final memory map.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if high_half_image
            && !range_is_identity_mapped_ram(memory_map, device_tree, FDT_FIXED_PREFIX)?
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: Device Tree prefix is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
        }
        if physical_ranges_overlap(device_tree, FDT_FIXED_PREFIX, framebuffer_base, framebuffer_size) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree header overlaps the GOP framebuffer.\r\n\0"));
        }

        let dtb = device_tree as *const u8;
        // SAFETY: the fixed prefix is reserved and mapped, as checked above.
        let header = unsafe { core::slice::from_raw_parts(dtb, FDT_FIXED_PREFIX as usize) };
        let length = validated_fdt_prefix_length(header)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid Device Tree header during final memory-map audit.\r\n\0"))?;
        if !memory_map.range_is_fully_reserved(device_tree, length)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed Device Tree range in final memory map.\r\n\0"))?
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree storage is not fully reserved in the final memory map.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if high_half_image
            && !range_is_identity_mapped_ram(memory_map, device_tree, length)?
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: Device Tree is outside identity-mapped RAM for the high-half kernel.\r\n\0"));
        }
        if physical_ranges_overlap(device_tree, length, framebuffer_base, framebuffer_size) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: Device Tree overlaps the GOP framebuffer.\r\n\0"));
        }
        // Firmware data may have changed since initial discovery. Re-parse the
        // entire bounded tree while its full range is still proven readable.
        // SAFETY: length was validated, the complete range is reserved, and
        // high-half x86-64 mapping coverage was checked above.
        let complete_tree = unsafe {
            core::slice::from_raw_parts(device_tree as *const u8, length as usize)
        };
        if archiaos_bootloader::device_tree::validate_tree(complete_tree).is_err() {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid Device Tree contents during final memory-map audit.\r\n\0"));
        }
    }
    Ok(())
}

#[cfg(target_arch = "x86_64")]
fn require_identity_mapped_ram(
    memory_map: &UefiMemoryMap<'_>,
    start: u64,
    length: u64,
    message: &'static [u8],
) -> Result<(), BootError> {
    if !range_is_identity_mapped_ram(memory_map, start, length)? {
        return Err(error(EFI_UNSUPPORTED, message));
    }
    Ok(())
}

fn platform_tables(system_table: *mut SystemTable) -> (u64, u64) {
    let acpi20 = configuration_table(system_table, &ACPI_20_TABLE_GUID);
    let acpi10 = configuration_table(system_table, &ACPI_10_TABLE_GUID);
    // SAFETY: firmware configuration-table pointers are validated before use.
    let rsdp20 = unsafe { validated_rsdp(acpi20) };
    let rsdp10 = unsafe { validated_rsdp(acpi10) };
    let rsdp = if rsdp20 != 0 { rsdp20 } else { rsdp10 };

    let dtb = configuration_table(system_table, &EFI_DTB_TABLE_GUID);
    // SAFETY: pointer comes from firmware and is fully parsed before being exposed.
    let dtb = unsafe { validated_device_tree(dtb) };
    (rsdp, dtb)
}

/// Capture platform metadata, validate the final firmware memory map, terminate
/// boot services, and transfer control using the versioned BootInfo contract.
pub unsafe fn transfer_to_kernel(
    image_handle: Handle,
    system_table: *mut SystemTable,
    kernel_entry: u64,
    _kernel_image: &[u8],
    _elf: &archiaos_bootloader::elf::ElfFile,
) -> Result<Infallible, BootError> {
    if image_handle.is_null() || system_table.is_null() || kernel_entry == 0 {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: invalid kernel handoff arguments.\r\n\0"));
    }
    // SAFETY: system table is the pointer provided by UEFI to efi_main.
    let boot_services = unsafe { (*system_table).boot_services };
    if boot_services.is_null() {
        return Err(error(EFI_INVALID_PARAMETER, b"ArchiaOS: Boot Services unavailable for handoff.\r\n\0"));
    }
    let get_memory_map = unsafe { (*boot_services).get_memory_map }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: GetMemoryMap is unavailable.\r\n\0"))?;
    let exit_boot_services = unsafe { (*boot_services).exit_boot_services }
        .ok_or_else(|| error(EFI_UNSUPPORTED, b"ArchiaOS: ExitBootServices is unavailable.\r\n\0"))?;

    #[cfg(target_arch = "x86_64")]
    if image_uses_highhalf(_elf) && !cpu_supports_nx() {
        return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: high-half paging requires CPU NX support.\r\n\0"));
    }

    let (acpi_rsdp, device_tree) = platform_tables(system_table);
    #[cfg(target_arch = "aarch64")]
    if device_tree == 0 && acpi_rsdp == 0 {
        return Err(error(EFI_NOT_FOUND, b"ArchiaOS: neither a valid UEFI Device Tree nor ACPI RSDP table was found.\r\n\0"));
    }

    let mut info_allocation = unsafe {
        PoolAllocation::new(boot_services, core::mem::size_of::<BootInfo>())?
    };
    let mut map_allocation = unsafe {
        PoolAllocation::new(boot_services, MAX_MEMORY_MAP_SIZE)?
    };
    let info_address = info_allocation.pointer.cast::<BootInfo>();
    let map_address = map_allocation.pointer;
    #[cfg(target_arch = "x86_64")]
    if image_uses_highhalf(_elf) {
        let info_start = info_address as u64;
        let info_end = info_start
            .checked_add(core::mem::size_of::<BootInfo>() as u64)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: BootInfo physical range overflow.\r\n\0"))?;
        let map_start = map_address as u64;
        let map_end = map_start
            .checked_add(MAX_MEMORY_MAP_SIZE as u64)
            .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: memory-map buffer physical range overflow.\r\n\0"))?;
        // The current x86-64 high-half page tables deliberately identity-map
        // RAM only below 4 GiB. Fail explicitly instead of transferring pointers
        // that the production kernel could not dereference after the CR3 switch.
        if info_start < MIN_KERNEL_PHYSICAL_ADDRESS
            || info_end > MAX_KERNEL_PHYSICAL_ADDRESS
            || map_start < MIN_KERNEL_PHYSICAL_ADDRESS
            || map_end > MAX_KERNEL_PHYSICAL_ADDRESS
        {
            return Err(error(EFI_UNSUPPORTED, b"ArchiaOS: BootInfo or final memory-map buffer lies outside the identity-mapped 1 MiB..4 GiB window.\r\n\0"));
        }
    }
    #[cfg(target_arch = "x86_64")]
    let loader_image_range = if image_uses_highhalf(_elf) {
        Some(unsafe { loaded_image_physical_range(image_handle, system_table)? })
    } else {
        None
    };
    #[cfg(target_arch = "x86_64")]
    let mut page_table_arena = if image_uses_highhalf(_elf) {
        Some(unsafe { PageTableArena::allocate(boot_services)? })
    } else {
        None
    };
    // Query GOP before the final memory-map snapshot. A missing or malformed GOP
    // is a supported headless mode; a discovered range is audited below as reserved.
    let (framebuffer_base, framebuffer_size) = unsafe { discover_framebuffer(boot_services) };
    let mut leaked_info = ptr::null_mut::<BootInfo>();
    let mut leaked_map = ptr::null_mut::<u8>();

    for attempt in 0..MAX_EXIT_BOOT_SERVICES_ATTEMPTS {
        let mut map_size = MAX_MEMORY_MAP_SIZE;
        let mut map_key = 0usize;
        let mut descriptor_size = 0usize;
        let mut descriptor_version = 0u32;
        let destination = if leaked_map.is_null() {
            map_address
        } else {
            leaked_map.cast::<c_void>()
        };
        // SAFETY: map buffer is allocated to MAX_MEMORY_MAP_SIZE bytes; all outputs are valid.
        let status = unsafe {
            get_memory_map(
                &mut map_size,
                destination,
                &mut map_key,
                &mut descriptor_size,
                &mut descriptor_version,
            )
        };
        if status != EFI_SUCCESS {
            return Err(error(status, b"ArchiaOS: GetMemoryMap failed or exceeded the reserved buffer.\r\n\0"));
        }
        if map_size == 0 || map_size > MAX_MEMORY_MAP_SIZE || descriptor_size == 0
            || map_size % descriptor_size != 0
            || descriptor_version != UEFI_MEMORY_DESCRIPTOR_VERSION
        {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: firmware returned an invalid or unsupported memory map.\r\n\0"));
        }

        // This parser is allocation-free and checks every range for overflow and overlap.
        // SAFETY: GetMemoryMap filled exactly map_size bytes in this buffer.
        let map_bytes = unsafe { core::slice::from_raw_parts(destination.cast::<u8>(), map_size) };
        let parsed_map = UefiMemoryMap::parse(map_bytes, descriptor_size)
            .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: UEFI memory-map validation failed.\r\n\0"))?;
        if parsed_map.is_empty() {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: UEFI memory map is empty.\r\n\0"));
        }

        // The final map must continue to classify loader-owned handoff storage as
        // reserved, never conventional RAM. Otherwise the PMM could later allocate
        // memory still holding BootInfo or the map it is actively reading.
        let info_storage = if leaked_info.is_null() {
            info_address.cast::<u8>()
        } else {
            leaked_info.cast::<u8>()
        };
        let info_storage_start = info_storage as u64;
        if !parsed_map.range_is_fully_reserved(
            info_storage_start,
            core::mem::size_of::<BootInfo>() as u64,
        ).map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid BootInfo storage range in final memory map.\r\n\0"))? {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: BootInfo storage is incorrectly marked conventional RAM.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if image_uses_highhalf(_elf) {
            require_identity_mapped_ram(
                &parsed_map,
                info_storage_start,
                core::mem::size_of::<BootInfo>() as u64,
                b"ArchiaOS: BootInfo storage is not covered by the active high-half identity map.\r\n\0",
            )?;
        }
        let map_storage_start = destination as u64;
        if !parsed_map.range_is_fully_reserved(
            map_storage_start,
            MAX_MEMORY_MAP_SIZE as u64,
        ).map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid memory-map buffer range in final map.\r\n\0"))? {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: memory-map buffer is incorrectly marked conventional RAM.\r\n\0"));
        }
        #[cfg(target_arch = "x86_64")]
        if image_uses_highhalf(_elf) {
            require_identity_mapped_ram(
                &parsed_map,
                map_storage_start,
                MAX_MEMORY_MAP_SIZE as u64,
                b"ArchiaOS: final memory-map buffer is not covered by the active high-half identity map.\r\n\0",
            )?;
        }

        // Every PT_LOAD segment was reserved with AllocateAddress before this
        // final snapshot. Confirm firmware still marks each rounded physical range
        // as loader-owned/reserved instead of conventional RAM.
        for segment_index in 0.._elf.program_header_count() {
            let segment = match archiaos_bootloader::elf::load_segment(
                _kernel_image,
                _elf,
                segment_index,
            ) {
                Ok(segment) => segment,
                Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
                Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF segment while auditing final memory map.\r\n\0")),
            };
            let segment_end = segment.physical_address
                .checked_add(segment.memory_size)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF range overflow during final memory-map audit.\r\n\0"))?;
            let rounded_end = segment_end
                .checked_add(EFI_PAGE_SIZE - 1)
                .map(|value| value & !(EFI_PAGE_SIZE - 1))
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF rounded range overflow during final memory-map audit.\r\n\0"))?;
            let segment_length = rounded_end.checked_sub(segment.physical_address)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF range during final memory-map audit.\r\n\0"))?;
            if !parsed_map.range_is_fully_reserved(
                segment.physical_address,
                segment_length,
            ).map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: malformed ELF range in final memory map.\r\n\0"))? {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: kernel segment is incorrectly marked conventional RAM.\r\n\0"));
            }
            #[cfg(target_arch = "x86_64")]
            if image_uses_highhalf(_elf) {
                require_identity_mapped_ram(
                    &parsed_map,
                    segment.physical_address,
                    segment_length,
                    b"ArchiaOS: kernel segment is not covered by the active identity map.\r\n\0",
                )?;
            }
        }

        #[cfg(target_arch = "x86_64")]
        if let Some((loader_start, loader_end)) = loader_image_range {
            if !parsed_map.range_is_fully_reserved(
                loader_start,
                loader_end.checked_sub(loader_start)
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: inverted loader-image range in final map.\r\n\0"))?,
            ).map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid loader-image range in final map.\r\n\0"))? {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: loader image is incorrectly marked conventional RAM.\r\n\0"));
            }
        }
        #[cfg(target_arch = "x86_64")]
        if let Some(arena) = page_table_arena.as_ref() {
            let arena_bytes = (PAGE_TABLE_ARENA_PAGES as u64)
                .checked_mul(EFI_PAGE_SIZE)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena size overflow.\r\n\0"))?;
            if !parsed_map.range_is_fully_reserved(arena.physical_base, arena_bytes)
                .map_err(|_| error(EFI_LOAD_ERROR, b"ArchiaOS: invalid page-table arena range in final map.\r\n\0"))? {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena is incorrectly marked conventional RAM.\r\n\0"));
            }
            require_identity_mapped_ram(
                &parsed_map,
                arena.physical_base,
                arena_bytes,
                b"ArchiaOS: page-table arena is not covered by the active identity map.\r\n\0",
            )?;
        }

        #[cfg(target_arch = "x86_64")]
        let mut kernel_page_table_root = 0u64;
        #[cfg(target_arch = "x86_64")]
        if let Some(arena) = page_table_arena.as_mut() {
            // The arena and all handoff buffers were allocated before this snapshot.
            // Building tables does not call firmware and therefore preserves the map key.
            let (loader_image_start, loader_image_end) = loader_image_range
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: high-half page tables require a loader image range.\r\n\0"))?;
            kernel_page_table_root = unsafe {
                build_x86_64_page_tables(
                    arena,
                    &parsed_map,
                    _kernel_image,
                    _elf,
                    loader_image_start,
                    loader_image_end,
                    framebuffer_base,
                    framebuffer_size,
                )?
            };
        }

        let info_pointer = if leaked_info.is_null() { info_address } else { leaked_info };
        // SAFETY: the structure is held in a pre-allocated pool that remains reserved after EBS.
        unsafe {
            ptr::write(
                info_pointer,
                BootInfo {
                    // Dedicated CI-only feature corrupts the ABI magic after all normal
                    // loader checks, so the kernel rejection path is exercised end-to-end.
                    magic: if cfg!(feature = "test-invalid-bootinfo") { 0 } else { BOOT_INFO_MAGIC },
                    version: BOOT_INFO_VERSION,
                    struct_size: core::mem::size_of::<BootInfo>() as u32,
                    kernel_entry,
                    memory_map: destination.cast::<u8>(),
                    memory_map_size: map_size as u64,
                    memory_map_descriptor_size: descriptor_size as u64,
                    memory_map_descriptor_version: descriptor_version,
                    flags: 0,
                    acpi_rsdp,
                    device_tree,
                    framebuffer_base,
                    framebuffer_size,
                },
            );
        }

        // Read framebuffer metadata only after fully initializing BootInfo. The
        // current loader sets it to absent; this guard becomes active when GOP
        // discovery is wired in and prevents the PMM from treating framebuffer
        // MMIO as conventional free memory.
        let framebuffer_base = unsafe { (*info_pointer).framebuffer_base };
        let framebuffer_size = unsafe { (*info_pointer).framebuffer_size };
        if (framebuffer_base == 0) != (framebuffer_size == 0) {
            return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: incomplete framebuffer range in BootInfo.\\r\\n\\0"));
        }
        // GOP framebuffer apertures are device MMIO and may legitimately be absent
        // from the UEFI memory descriptors. Validate arithmetic here; the page-table
        // builder maps the aperture explicitly as uncached and excludes it from RAM.
        if framebuffer_base != 0 {
            framebuffer_base.checked_add(framebuffer_size)
                .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer range overflow in BootInfo.\\r\\n\\0"))?;
        }

        // The framebuffer is an MMIO aperture, not ordinary memory. Even if its
        // arithmetic is valid, it must not alias memory that the loader or kernel
        // will use for executable code, BootInfo, the final map, or page tables.
        if framebuffer_base != 0 {
            let framebuffer_conflicts_with = |start: u64, length: u64| {
                physical_ranges_overlap(framebuffer_base, framebuffer_size, start, length)
            };
            if framebuffer_conflicts_with(
                info_storage_start,
                core::mem::size_of::<BootInfo>() as u64,
            ) || framebuffer_conflicts_with(map_storage_start, MAX_MEMORY_MAP_SIZE as u64) {
                return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer overlaps BootInfo or memory-map storage.\\r\\n\\0"));
            }

            for segment_index in 0.._elf.program_header_count() {
                let segment = match archiaos_bootloader::elf::load_segment(
                    _kernel_image,
                    _elf,
                    segment_index,
                ) {
                    Ok(segment) => segment,
                    Err(archiaos_bootloader::elf::ElfError::NotLoadableSegment) => continue,
                    Err(_) => return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: invalid ELF segment during framebuffer overlap audit.\\r\\n\\0")),
                };
                let segment_end = segment.physical_address
                    .checked_add(segment.memory_size)
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF range overflow during framebuffer overlap audit.\\r\\n\\0"))?;
                let rounded_start = segment.physical_address & !(EFI_PAGE_SIZE - 1);
                let rounded_end = segment_end
                    .checked_add(EFI_PAGE_SIZE - 1)
                    .map(|value| value & !(EFI_PAGE_SIZE - 1))
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: ELF rounded range overflow during framebuffer overlap audit.\\r\\n\\0"))?;
                if framebuffer_conflicts_with(rounded_start, rounded_end - rounded_start) {
                    return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer overlaps a loaded kernel segment.\\r\\n\\0"));
                }
            }

            #[cfg(target_arch = "x86_64")]
            if let Some((loader_start, loader_end)) = loader_image_range {
                if framebuffer_conflicts_with(
                    loader_start,
                    loader_end.checked_sub(loader_start)
                        .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: inverted loader-image range during framebuffer overlap audit.\\r\\n\\0"))?,
                ) {
                    return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer overlaps the UEFI loader image.\\r\\n\\0"));
                }
            }
            #[cfg(target_arch = "x86_64")]
            if let Some(arena) = page_table_arena.as_ref() {
                let arena_bytes = (PAGE_TABLE_ARENA_PAGES as u64)
                    .checked_mul(EFI_PAGE_SIZE)
                    .ok_or_else(|| error(EFI_LOAD_ERROR, b"ArchiaOS: page-table arena size overflow during framebuffer overlap audit.\\r\\n\\0"))?;
                if framebuffer_conflicts_with(arena.physical_base, arena_bytes) {
                    return Err(error(EFI_LOAD_ERROR, b"ArchiaOS: framebuffer overlaps page-table storage.\\r\\n\\0"));
                }
            }
        }
        // Some UEFI implementations expose GOP framebuffer MMIO inside a
        // descriptor labelled EfiConventionalMemory. Do not reject the entire boot
        // on that firmware defect: the x86-64 page-table builder excludes every
        // framebuffer-overlapping page from its ordinary RAM identity map, and the
        // kernel PMM independently reserves those frames before publishing alloc_page().

        audit_platform_table_ranges(
            &parsed_map,
            acpi_rsdp,
            device_tree,
            image_uses_highhalf(_elf),
            framebuffer_base,
            framebuffer_size,
        )?;

        // Stop tracking the two pool buffers before the first EBS call, so no
        // destructor can call FreePool after boot services have been terminated.
        if leaked_info.is_null() {
            leaked_map = map_allocation.take_raw().cast::<u8>();
            leaked_info = info_allocation.take_raw().cast::<BootInfo>();
        }

        // Once EBS is attempted, never run a page-arena destructor that might call FreePages.
        // A stale map key retries with the same preallocated arena and no other UEFI services.
        #[cfg(target_arch = "x86_64")]
        if let Some(arena) = page_table_arena.as_mut() {
            arena.disarm();
        }
        // SAFETY: map_key is from the immediately preceding successful GetMemoryMap call.
        let status = unsafe { exit_boot_services(image_handle, map_key) };
        if status == EFI_SUCCESS {
            // Boot services are no longer available: do not call firmware or drop guards here.
            #[cfg(target_arch = "x86_64")]
            unsafe { jump_to_kernel(kernel_entry, leaked_info, kernel_page_table_root) };
            #[cfg(target_arch = "aarch64")]
            unsafe { jump_to_kernel(kernel_entry, leaked_info) };
        }
        if status != EFI_INVALID_PARAMETER {
            return Err(error(status, b"ArchiaOS: ExitBootServices failed.\r\n\0"));
        }
        if attempt + 1 == MAX_EXIT_BOOT_SERVICES_ATTEMPTS {
            return Err(error(status, b"ArchiaOS: ExitBootServices could not obtain a stable memory-map key.\r\n\0"));
        }
        // Retry GetMemoryMap immediately with the same buffer and no intervening service calls.
    }

    Err(error(EFI_LOAD_ERROR, b"ArchiaOS: boot handoff ended unexpectedly.\r\n\0"))
}

#[cfg(target_arch = "x86_64")]
unsafe fn jump_to_kernel(entry: u64, info: *const BootInfo, page_table_root: u64) -> ! {
    // RDI carries BootInfo. High-half images receive fresh identity + kernel mappings;
    // identity test kernels keep firmware paging by passing root=0. Before using
    // the new NX-marked mappings, enable EFER.NXE; CR0.WP also makes supervisor
    // writes respect read-only kernel text/rodata pages.
    unsafe {
        core::arch::asm!(
            "cli",
            "cld",
            // Save ABI inputs before COM1 setup clobbers RAX/RDX.
            "mov r8, rax",
            "mov r9, rdx",
            // No UEFI calls after ExitBootServices. 'P' marks trampoline entry;
            // 'T' means execution continued after the optional CR3 switch.
            "mov dx, 0x3f9",
            "xor al, al",
            "out dx, al",
            "mov dx, 0x3fb",
            "mov al, 0x80",
            "out dx, al",
            "mov dx, 0x3f8",
            "mov al, 0x03",
            "out dx, al",
            "mov dx, 0x3f9",
            "xor al, al",
            "out dx, al",
            "mov dx, 0x3fb",
            "mov al, 0x03",
            "out dx, al",
            "mov dx, 0x3fa",
            "mov al, 0xc7",
            "out dx, al",
            "mov dx, 0x3fc",
            "mov al, 0x0b",
            "out dx, al",
            "mov dx, 0x3f8",
            "mov al, 0x50",
            "out dx, al",
            "mov al, 0x0d",
            "out dx, al",
            "mov al, 0x0a",
            "out dx, al",
            "test r9, r9",
            "jz 2f",
            // R8/R9 survive RDMSR, which overwrites EDX:EAX.
            "mov ecx, 0xC0000080",
            "rdmsr",
            "or eax, 0x800",
            "wrmsr",
            "mov rax, cr0",
            "or rax, 0x10000",
            "mov cr0, rax",
            "mov cr3, r9",
            "2:",
            "mov dx, 0x3f8",
            "mov al, 0x54",
            "out dx, al",
            "mov al, 0x0d",
            "out dx, al",
            "mov al, 0x0a",
            "out dx, al",
            "mov rax, r8",
            "jmp rax",
            in("rax") entry,
            in("rdi") info,
            in("rdx") page_table_root,
            // This assembly never returns to Rust. The register clobbers are
            // therefore immaterial to Rust's caller, while RAX/RDX inputs are
            // explicitly copied to R8/R9 before RDMSR overwrites them.
            options(noreturn)
        )
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn jump_to_kernel(entry: u64, info: *const BootInfo) -> ! {
    // AArch64 kernel entry receives its first argument in x0 (AAPCS64).
    unsafe {
        core::arch::asm!(
            "msr daifset, #0xf",
            "br x1",
            in("x0") info,
            in("x1") entry,
            options(noreturn)
        )
    }
}


#[cfg(test)]
mod fdt_handoff_header_tests {
    use super::validated_fdt_prefix_length;

    fn header(magic: u32, length: u32) -> [u8; 8] {
        let mut bytes = [0; 8];
        bytes[..4].copy_from_slice(&magic.to_be_bytes());
        bytes[4..].copy_from_slice(&length.to_be_bytes());
        bytes
    }

    #[test]
    fn accepts_valid_fdt_prefix() {
        let bytes = header(
            archiaos_bootloader::device_tree::FDT_MAGIC,
            archiaos_bootloader::device_tree::HEADER_SIZE as u32,
        );
        assert_eq!(
            validated_fdt_prefix_length(&bytes),
            Some(archiaos_bootloader::device_tree::HEADER_SIZE as u64)
        );
    }

    #[test]
    fn rejects_wrong_magic_short_prefix_and_invalid_lengths() {
        assert_eq!(validated_fdt_prefix_length(&[0; 7]), None);
        assert_eq!(validated_fdt_prefix_length(&header(0, 40)), None);
        assert_eq!(
            validated_fdt_prefix_length(&header(
                archiaos_bootloader::device_tree::FDT_MAGIC,
                (archiaos_bootloader::device_tree::HEADER_SIZE - 1) as u32,
            )),
            None
        );
        assert_eq!(
            validated_fdt_prefix_length(&header(
                archiaos_bootloader::device_tree::FDT_MAGIC,
                archiaos_bootloader::device_tree::MAX_TOTAL_SIZE as u32 + 1,
            )),
            None
        );
    }
}

#[cfg(test)]
mod rsdp_handoff_signature_tests {
    use super::rsdp_signature_is_valid;

    #[test]
    fn accepts_only_the_acpi_rsdp_signature() {
        assert!(rsdp_signature_is_valid(archiaos_bootloader::acpi::RSDP_SIGNATURE));
        assert!(!rsdp_signature_is_valid(b"BAD PTR "));
        assert!(!rsdp_signature_is_valid(b"RSD PTR"));
        assert!(!rsdp_signature_is_valid(b""));
    }
}

#[cfg(test)]
mod rsdp_handoff_checksum_tests {
    use super::rsdp_checksums_are_valid;

    #[test]
    fn accepts_zero_sum_legacy_and_extended_rsdp_tables() {
        assert!(rsdp_checksums_are_valid(&[0; 20], 0, 20));
        assert!(rsdp_checksums_are_valid(&[0; 36], 2, 36));
    }

    #[test]
    fn rejects_bad_legacy_or_extended_checksums() {
        let mut legacy = [0; 20];
        legacy[19] = 1;
        assert!(!rsdp_checksums_are_valid(&legacy, 0, 20));

        let mut extended_legacy_bad = [0; 36];
        extended_legacy_bad[0] = 1;
        assert!(!rsdp_checksums_are_valid(&extended_legacy_bad, 2, 36));

        let mut extended_full_bad = [0; 36];
        extended_full_bad[35] = 1;
        assert!(!rsdp_checksums_are_valid(&extended_full_bad, 2, 36));
    }

    #[test]
    fn rejects_mismatched_lengths_and_truncated_tables() {
        assert!(!rsdp_checksums_are_valid(&[0; 19], 0, 19));
        assert!(!rsdp_checksums_are_valid(&[0; 20], 2, 20));
        assert!(!rsdp_checksums_are_valid(&[0; 36], 2, 37));
    }
}

#[cfg(test)]
mod rsdp_handoff_length_tests {
    use super::rsdp_length_is_valid;

    #[test]
    fn accepts_legacy_and_complete_extended_rsdp_lengths() {
        assert!(rsdp_length_is_valid(0, 20));
        assert!(rsdp_length_is_valid(2, 36));
        assert!(rsdp_length_is_valid(
            2,
            archiaos_bootloader::acpi::MAX_RSDP_SIZE as u64
        ));
    }

    #[test]
    fn rejects_truncated_extended_rsdp_and_oversized_lengths() {
        assert!(!rsdp_length_is_valid(1, 20));
        assert!(!rsdp_length_is_valid(1, 36));
        assert!(!rsdp_length_is_valid(2, 20));
        assert!(!rsdp_length_is_valid(2, 35));
        assert!(!rsdp_length_is_valid(
            2,
            archiaos_bootloader::acpi::MAX_RSDP_SIZE as u64 + 1
        ));
        assert!(!rsdp_length_is_valid(0, 19));
    }
}

#[cfg(test)]
mod configuration_table_bounds_tests {
    use super::configuration_table_range_is_bounded;

    #[test]
    fn accepts_small_bounded_configuration_table_arrays() {
        assert!(configuration_table_range_is_bounded(0x1000, 1));
        assert!(configuration_table_range_is_bounded(0x1000, 4096));
    }

    #[test]
    fn rejects_missing_oversized_and_wrapping_configuration_table_arrays() {
        assert!(!configuration_table_range_is_bounded(0, 1));
        assert!(!configuration_table_range_is_bounded(0x1000, 0));
        assert!(!configuration_table_range_is_bounded(0x1000, 4097));
        assert!(!configuration_table_range_is_bounded(usize::MAX - 7, 1));
    }
}

#[cfg(test)]
mod physical_range_overlap_tests {
    use super::physical_ranges_overlap;

    #[test]
    fn detects_partial_contained_and_exact_overlaps() {
        assert!(physical_ranges_overlap(0x1000, 0x3000, 0x2000, 0x1000));
        assert!(physical_ranges_overlap(0x2000, 0x1000, 0x1000, 0x3000));
        assert!(physical_ranges_overlap(0x1000, 0x1000, 0x1000, 0x1000));
        assert!(physical_ranges_overlap(0x1000, 0x3000, 0x1000, 0x3000));
    }

    #[test]
    fn allows_disjoint_adjacent_and_empty_ranges() {
        assert!(!physical_ranges_overlap(0x1000, 0x1000, 0x2000, 0x1000));
        assert!(!physical_ranges_overlap(0x3000, 0x1000, 0x1000, 0x1000));
        assert!(!physical_ranges_overlap(0x1000, 0, 0x1000, 0x1000));
        assert!(!physical_ranges_overlap(0x1000, 0x1000, 0x1000, 0));
    }

    #[test]
    fn treats_wrapping_ranges_as_conflicting() {
        assert!(physical_ranges_overlap(u64::MAX - 3, 8, 0x1000, 0x1000));
        assert!(physical_ranges_overlap(0x1000, 0x1000, u64::MAX - 3, 8));
    }
}
