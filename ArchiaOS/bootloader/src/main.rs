#![no_std]
#![no_main]

// First-party UEFI loader. It opens a kernel image from the EFI System
// Partition, validates the ELF64 image, and reports its entry point. The next
// milestone is allocating/loading PT_LOAD segments and performing the handoff.
use core::ffi::c_void;
use core::panic::PanicInfo;
use core::ptr::{read_unaligned, read_volatile};

use archiaos_bootloader::elf::parse_elf64;

type Status = usize;
type Handle = *mut c_void;

// UEFI errors set the high bit of EFI_STATUS on 64-bit targets. These values
// must not be compared with the small warning/success codes.
const EFI_SUCCESS: Status = 0;
const EFI_ERROR_BIT: Status = 1usize << (usize::BITS - 1);
const EFI_LOAD_ERROR: Status = EFI_ERROR_BIT | 1;
const EFI_INVALID_PARAMETER: Status = EFI_ERROR_BIT | 2;
const EFI_UNSUPPORTED: Status = EFI_ERROR_BIT | 3;
const EFI_BAD_BUFFER_SIZE: Status = EFI_ERROR_BIT | 4;
const EFI_BUFFER_TOO_SMALL: Status = EFI_ERROR_BIT | 5;
const EFI_NOT_FOUND: Status = EFI_ERROR_BIT | 14;
const EFI_LOADER_DATA: u32 = 2;
const EFI_FILE_MODE_READ: u64 = 1;
const MAX_KERNEL_IMAGE_SIZE: u64 = 64 * 1024 * 1024;
const EXPECTED_ELF_MACHINE: u16 = if cfg!(target_arch = "x86_64") { 62 } else { 183 };

#[repr(C)]
struct TableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const LOADED_IMAGE_GUID: Guid = Guid {
    data1: 0x5b1b31a1,
    data2: 0x9562,
    data3: 0x11d2,
    data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

const SIMPLE_FILE_SYSTEM_GUID: Guid = Guid {
    data1: 0x964e5b22,
    data2: 0x6459,
    data3: 0x11d2,
    data4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

const FILE_INFO_GUID: Guid = Guid {
    data1: 0x09576e92,
    data2: 0x6d3f,
    data3: 0x11d2,
    data4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

#[repr(C)]
struct SimpleTextOutputProtocol {
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

type HandleProtocolFn = unsafe extern "efiapi" fn(
    handle: Handle,
    protocol: *const Guid,
    interface: *mut *mut c_void,
) -> Status;

type AllocatePoolFn = unsafe extern "efiapi" fn(
    pool_type: u32,
    size: usize,
    buffer: *mut *mut c_void,
) -> Status;

type FreePoolFn = unsafe extern "efiapi" fn(buffer: *mut c_void) -> Status;

#[repr(C)]
struct BootServices {
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
struct SystemTable {
    header: TableHeader,
    firmware_vendor: *mut u16,
    firmware_revision: u32,
    _padding: u32,
    console_in_handle: Handle,
    con_in: *mut c_void,
    console_out_handle: Handle,
    con_out: *mut SimpleTextOutputProtocol,
    standard_error_handle: Handle,
    std_err: *mut SimpleTextOutputProtocol,
    runtime_services: *mut c_void,
    boot_services: *mut BootServices,
    number_of_table_entries: usize,
    configuration_table: *mut c_void,
}

#[repr(C)]
struct LoadedImageProtocol {
    revision: u32,
    _padding: u32,
    parent_handle: Handle,
    system_table: *mut SystemTable,
    device_handle: Handle,
    file_path: *mut c_void,
    reserved: *mut c_void,
    load_options_size: u32,
    _padding2: u32,
    load_options: *mut c_void,
    image_base: *mut c_void,
    image_size: u64,
    image_code_type: u32,
    image_data_type: u32,
    unload: usize,
}

#[repr(C)]
struct SimpleFileSystemProtocol {
    revision: u64,
    open_volume: Option<
        unsafe extern "efiapi" fn(
            this: *mut SimpleFileSystemProtocol,
            root: *mut *mut FileProtocol,
        ) -> Status,
    >,
}

#[repr(C)]
struct FileProtocol {
    revision: u64,
    open: Option<
        unsafe extern "efiapi" fn(
            this: *mut FileProtocol,
            new_handle: *mut *mut FileProtocol,
            file_name: *const u16,
            open_mode: u64,
            attributes: u64,
        ) -> Status,
    >,
    close: Option<unsafe extern "efiapi" fn(this: *mut FileProtocol) -> Status>,
    delete: usize,
    read: Option<
        unsafe extern "efiapi" fn(
            this: *mut FileProtocol,
            buffer_size: *mut usize,
            buffer: *mut c_void,
        ) -> Status,
    >,
    write: usize,
    get_position: usize,
    set_position: usize,
    get_info: Option<
        unsafe extern "efiapi" fn(
            this: *mut FileProtocol,
            information_type: *const Guid,
            buffer_size: *mut usize,
            buffer: *mut c_void,
        ) -> Status,
    >,
    set_info: usize,
    flush: usize,
}

const fn ascii_to_utf16<const N: usize>(bytes: &[u8; N]) -> [u16; N] {
    let mut result = [0u16; N];
    let mut i = 0;
    while i < N {
        result[i] = bytes[i] as u16;
        i += 1;
    }
    result
}

static BANNER: [u16; 42] =
    ascii_to_utf16(b"ArchiaOS Bootloader: UEFI entry reached\r\n\0");
static STAGE_FS: [u16; 48] =
    ascii_to_utf16(b"ArchiaOS Bootloader: opening EFI kernel image\r\n\0");
static STAGE_ELF: [u16; 47] =
    ascii_to_utf16(b"ArchiaOS Bootloader: validating ELF64 kernel\r\n\0");
static SUCCESS: [u16; 61] =
    ascii_to_utf16(b"ArchiaOS Bootloader: kernel ELF validated; handoff pending\r\n\0");
static FAIL_SERVICES: [u16; 58] =
    ascii_to_utf16(b"ArchiaOS Bootloader: required UEFI boot service missing\r\n\0");
static FAIL_FS: [u16; 53] =
    ascii_to_utf16(b"ArchiaOS Bootloader: cannot open EFI system volume\r\n\0");
static FAIL_FILE: [u16; 59] =
    ascii_to_utf16(b"ArchiaOS Bootloader: cannot read EFI/ArchiaOS/kernel.elf\r\n\0");
static FAIL_SIZE: [u16; 50] =
    ascii_to_utf16(b"ArchiaOS Bootloader: kernel image size rejected\r\n\0");
static FAIL_ELF: [u16; 50] =
    ascii_to_utf16(b"ArchiaOS Bootloader: invalid ELF64 kernel image\r\n\0");

static KERNEL_PATH: [u16; 25] =
    ascii_to_utf16(b"\\EFI\\ArchiaOS\\kernel.elf\0");

struct Serial {
    output: *mut SimpleTextOutputProtocol,
}

impl Serial {
    unsafe fn print(&mut self, message: *const u16) -> Result<(), Status> {
        // Mirror diagnostics to COM1 on x86-64 so headless QEMU/CI can observe
        // loader progress even when the firmware's graphical console is hidden.
        #[cfg(target_arch = "x86_64")]
        unsafe {
            mirror_to_com1(message);
        }

        if self.output.is_null() {
            return Err(EFI_INVALID_PARAMETER);
        }
        let write = unsafe { (*self.output).output_string }.ok_or(EFI_UNSUPPORTED)?;
        let status = unsafe { write(self.output, message) };
        if status == EFI_SUCCESS {
            Ok(())
        } else {
            Err(status)
        }
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn port_out(port: u16, value: u8) {
    use core::arch::asm;
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn port_in(port: u16) -> u8 {
    use core::arch::asm;
    let value: u8;
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

#[cfg(target_arch = "x86_64")]
unsafe fn mirror_to_com1(message: *const u16) {
    const COM1: u16 = 0x3f8;
    // Configure the QEMU-compatible 16550 UART. This path is diagnostic only;
    // UEFI Text Output remains the normal, architecture-neutral console API.
    unsafe {
        port_out(COM1 + 1, 0x00);
        port_out(COM1 + 3, 0x80);
        port_out(COM1, 0x03);
        port_out(COM1 + 1, 0x00);
        port_out(COM1 + 3, 0x03);
        port_out(COM1 + 2, 0xc7);
        port_out(COM1 + 4, 0x0b);
    }

    for index in 0..256usize {
        let unit = unsafe { read_volatile(message.add(index)) };
        if unit == 0 {
            break;
        }
        let byte = if unit <= 0x7f { unit as u8 } else { b'?' };
        for _ in 0..100_000 {
            if unsafe { port_in(COM1 + 5) } & 0x20 != 0 {
                break;
            }
            core::hint::spin_loop();
        }
        unsafe { port_out(COM1, byte) };
    }
}

unsafe fn protocol<T>(
    boot_services: *mut BootServices,
    handle: Handle,
    guid: *const Guid,
) -> Result<*mut T, Status> {
    if boot_services.is_null() || handle.is_null() {
        return Err(EFI_INVALID_PARAMETER);
    }
    let handle_protocol = unsafe { (*boot_services).handle_protocol }.ok_or(EFI_UNSUPPORTED)?;
    let mut interface: *mut c_void = core::ptr::null_mut();
    let status = unsafe { handle_protocol(handle, guid, &mut interface) };
    if status != EFI_SUCCESS {
        return Err(status);
    }
    if interface.is_null() {
        return Err(EFI_UNSUPPORTED);
    }
    Ok(interface.cast::<T>())
}

unsafe fn allocate_pool(
    boot_services: *mut BootServices,
    size: usize,
) -> Result<*mut c_void, Status> {
    let allocate = unsafe { (*boot_services).allocate_pool }.ok_or(EFI_UNSUPPORTED)?;
    let mut buffer = core::ptr::null_mut();
    let status = unsafe { allocate(EFI_LOADER_DATA, size, &mut buffer) };
    if status != EFI_SUCCESS {
        return Err(status);
    }
    if buffer.is_null() {
        return Err(EFI_LOAD_ERROR);
    }
    Ok(buffer)
}

unsafe fn free_pool(boot_services: *mut BootServices, buffer: *mut c_void) {
    if !buffer.is_null() {
        if let Some(free) = unsafe { (*boot_services).free_pool } {
            let _ = unsafe { free(buffer) };
        }
    }
}

unsafe fn close_file(file: *mut FileProtocol) {
    if !file.is_null() {
        if let Some(close) = unsafe { (*file).close } {
            let _ = unsafe { close(file) };
        }
    }
}

unsafe fn read_kernel_image(
    image_handle: Handle,
    system_table: *mut SystemTable,
) -> Result<(*mut u8, usize), Status> {
    if system_table.is_null() {
        return Err(EFI_INVALID_PARAMETER);
    }
    let boot_services = unsafe { (*system_table).boot_services };
    if boot_services.is_null() {
        return Err(EFI_UNSUPPORTED);
    }

    let loaded_image = unsafe {
        protocol::<LoadedImageProtocol>(
            boot_services,
            image_handle,
            &LOADED_IMAGE_GUID,
        )?
    };
    let device_handle = unsafe { read_volatile(core::ptr::addr_of!((*loaded_image).device_handle)) };
    let file_system = unsafe {
        protocol::<SimpleFileSystemProtocol>(
            boot_services,
            device_handle,
            &SIMPLE_FILE_SYSTEM_GUID,
        )?
    };
    let open_volume = unsafe { (*file_system).open_volume }.ok_or(EFI_UNSUPPORTED)?;
    let mut root: *mut FileProtocol = core::ptr::null_mut();
    let status = unsafe { open_volume(file_system, &mut root) };
    if status != EFI_SUCCESS || root.is_null() {
        return Err(if status == EFI_SUCCESS { EFI_UNSUPPORTED } else { status });
    }

    let open = unsafe { (*root).open }.ok_or(EFI_UNSUPPORTED)?;
    let mut file: *mut FileProtocol = core::ptr::null_mut();
    let status = unsafe {
        open(root, &mut file, KERNEL_PATH.as_ptr(), EFI_FILE_MODE_READ, 0)
    };
    if status != EFI_SUCCESS || file.is_null() {
        unsafe { close_file(root) };
        return Err(if status == EFI_SUCCESS { EFI_UNSUPPORTED } else { status });
    }

    let get_info = match unsafe { (*file).get_info } {
        Some(function) => function,
        None => {
            unsafe { close_file(file); close_file(root); }
            return Err(EFI_UNSUPPORTED);
        }
    };

    let mut info_size: usize = 0;
    let status = unsafe {
        get_info(file, &FILE_INFO_GUID, &mut info_size, core::ptr::null_mut())
    };
    if status != EFI_BUFFER_TOO_SMALL || info_size < 16 || info_size > 4096 {
        unsafe { close_file(file); close_file(root); }
        return Err(if status == EFI_SUCCESS || status == EFI_BUFFER_TOO_SMALL {
            EFI_BAD_BUFFER_SIZE
        } else {
            status
        });
    }

    let info = match unsafe { allocate_pool(boot_services, info_size) } {
        Ok(buffer) => buffer,
        Err(status) => {
            unsafe { close_file(file); close_file(root); }
            return Err(status);
        }
    };
    let status = unsafe {
        get_info(file, &FILE_INFO_GUID, &mut info_size, info)
    };
    if status != EFI_SUCCESS || info_size < 16 {
        unsafe { free_pool(boot_services, info); close_file(file); close_file(root); }
        return Err(if status == EFI_SUCCESS { EFI_BAD_BUFFER_SIZE } else { status });
    }

    // EFI_FILE_INFO.FileSize is the second u64 field (offset 8).
    let file_size = unsafe { read_unaligned((info as *const u8).add(8).cast::<u64>()) };
    unsafe { free_pool(boot_services, info) };
    if file_size < 64 || file_size > MAX_KERNEL_IMAGE_SIZE || file_size > usize::MAX as u64 {
        unsafe { close_file(file); close_file(root); }
        return Err(EFI_BAD_BUFFER_SIZE);
    }

    let image_buffer = match unsafe { allocate_pool(boot_services, file_size as usize) } {
        Ok(buffer) => buffer.cast::<u8>(),
        Err(status) => {
            unsafe { close_file(file); close_file(root); }
            return Err(status);
        }
    };
    let read = match unsafe { (*file).read } {
        Some(function) => function,
        None => {
            unsafe {
                free_pool(boot_services, image_buffer.cast::<c_void>());
                close_file(file);
                close_file(root);
            }
            return Err(EFI_UNSUPPORTED);
        }
    };
    let mut actual_size = file_size as usize;
    let status = unsafe {
        read(file, &mut actual_size, image_buffer.cast::<c_void>())
    };
    unsafe { close_file(file); close_file(root); }

    if status != EFI_SUCCESS || actual_size != file_size as usize {
        unsafe { free_pool(boot_services, image_buffer.cast::<c_void>()) };
        return Err(if status == EFI_SUCCESS { EFI_BAD_BUFFER_SIZE } else { status });
    }
    Ok((image_buffer, actual_size))
}

#[no_mangle]
pub extern "efiapi" fn efi_main(image_handle: Handle, system_table: *mut SystemTable) -> Status {
    if system_table.is_null() {
        return EFI_INVALID_PARAMETER;
    }
    let mut serial = Serial {
        // SAFETY: system_table was checked for null and is provided by UEFI.
        output: unsafe { (*system_table).con_out },
    };
    if unsafe { serial.print(BANNER.as_ptr()) }.is_err() {
        return EFI_INVALID_PARAMETER;
    }

    if unsafe { serial.print(STAGE_FS.as_ptr()) }.is_err() {
        return EFI_UNSUPPORTED;
    }
    let (image, image_size) = match unsafe { read_kernel_image(image_handle, system_table) } {
        Ok(image) => image,
        Err(status) => {
            let message = match status {
                EFI_BAD_BUFFER_SIZE => FAIL_SIZE.as_ptr(),
                EFI_NOT_FOUND => FAIL_FILE.as_ptr(),
                EFI_UNSUPPORTED => FAIL_SERVICES.as_ptr(),
                _ => FAIL_FS.as_ptr(),
            };
            let _ = unsafe { serial.print(message) };
            return EFI_LOAD_ERROR;
        }
    };

    let _ = unsafe { serial.print(STAGE_ELF.as_ptr()) };
    // SAFETY: UEFI returned a pool allocation of image_size bytes; it remains
    // allocated until FreePool is called below.
    let bytes = unsafe { core::slice::from_raw_parts(image, image_size) };
    let parsed = parse_elf64(bytes, EXPECTED_ELF_MACHINE);
    let valid = parsed.is_ok();
    let boot_services = unsafe { (*system_table).boot_services };
    unsafe { free_pool(boot_services, image.cast::<c_void>()) };

    if !valid {
        let _ = unsafe { serial.print(FAIL_ELF.as_ptr()) };
        return EFI_LOAD_ERROR;
    }

    // This milestone only reads and validates the kernel. It does not jump to
    // the ELF entry: x86-64 currently needs its own high-half page tables and
    // both architectures need an explicit, versioned handoff contract.
    let _ = unsafe { serial.print(SUCCESS.as_ptr()) };
    EFI_SUCCESS
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
