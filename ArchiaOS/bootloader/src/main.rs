#![no_std]
#![no_main]

// First-party UEFI entry-point prototype. This deliberately uses only core and
// the UEFI ABI; it does not yet load a kernel or call ExitBootServices.
use core::ffi::c_void;
use core::panic::PanicInfo;

type Status = usize;
type Handle = *mut c_void;

const EFI_SUCCESS: Status = 0;
const EFI_INVALID_PARAMETER: Status = 2;

#[repr(C)]
struct TableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

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
    boot_services: *mut c_void,
    number_of_table_entries: usize,
    configuration_table: *mut c_void,
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

static BANNER: [u16; 43] =
    ascii_to_utf16(b"ArchiaOS Bootloader: UEFI entry reached\r\n\0");
static STATUS: [u16; 77] = ascii_to_utf16(
    b"ArchiaOS Bootloader: first-party prototype; kernel loading is not implemented yet.\r\n\0",
);

unsafe fn print(
    output: *mut SimpleTextOutputProtocol,
    message: *const u16,
) -> Result<(), Status> {
    if output.is_null() {
        return Err(EFI_INVALID_PARAMETER);
    }
    let write = unsafe { (*output).output_string }.ok_or(EFI_INVALID_PARAMETER)?;
    let status = unsafe { write(output, message) };
    if status == EFI_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

#[no_mangle]
pub extern "efiapi" fn efi_main(_image_handle: Handle, system_table: *mut SystemTable) -> Status {
    if system_table.is_null() {
        return EFI_INVALID_PARAMETER;
    }

    // SAFETY: UEFI passes a valid SystemTable pointer to an EFI application.
    let output = unsafe { (*system_table).con_out };
    // SAFETY: the protocol pointer is provided by the validated system table.
    if unsafe { print(output, BANNER.as_ptr()) }.is_err() {
        return EFI_INVALID_PARAMETER;
    }
    // Explicitly report the prototype's current limits; this must not be mistaken
    // for a bootloader that already loads or starts the ArchiaOS kernel.
    match unsafe { print(output, STATUS.as_ptr()) } {
        Ok(()) => EFI_SUCCESS,
        Err(status) => status,
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
