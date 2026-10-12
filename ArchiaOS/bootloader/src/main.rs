#![no_std]
#![no_main]

mod uefi;

use core::panic::PanicInfo;
use uefi::{BootError, Status, SystemTable, EFI_INVALID_PARAMETER, EFI_SUCCESS};

unsafe fn print_ascii(output: *mut uefi::SimpleTextOutputProtocol, message: &[u8]) -> Status {
    if output.is_null() {
        return EFI_INVALID_PARAMETER;
    }
    // Static ASCII diagnostics are NUL-terminated; use a fixed UTF-16 stack
    // buffer so early boot does not depend on an allocator.
    let mut wide = [0u16; 192];
    let mut written = 0usize;
    while written < wide.len() {
        let byte = match message.get(written) {
            Some(byte) => *byte,
            None => 0,
        };
        wide[written] = byte as u16;
        written += 1;
        if byte == 0 {
            break;
        }
    }
    if written == wide.len() && wide[written - 1] != 0 {
        wide[wide.len() - 1] = 0;
    }
    // SAFETY: output is checked and wide is a NUL-terminated UTF-16 buffer.
    let output_string = unsafe { (*output).output_string };
    let output_string = match output_string {
        Some(function) => function,
        None => return EFI_INVALID_PARAMETER,
    };
    // SAFETY: UEFI provides and owns this console protocol.
    unsafe { output_string(output, wide.as_ptr()) }
}

unsafe fn print_entry(output: *mut uefi::SimpleTextOutputProtocol, entry: u64) -> Status {
    if output.is_null() {
        return EFI_INVALID_PARAMETER;
    }
    let mut line = [0u16; 32];
    let prefix = *b"entry=0x";
    for (index, byte) in prefix.iter().enumerate() {
        line[index] = *byte as u16;
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for digit in 0..16 {
        let shift = (15 - digit) * 4;
        line[8 + digit] = HEX[((entry >> shift) & 0xf) as usize] as u16;
    }
    line[24] = b'\r' as u16;
    line[25] = b'\n' as u16;
    line[26] = 0;
    // SAFETY: output is checked and line is NUL-terminated UTF-16.
    let output_string = unsafe { (*output).output_string };
    let output_string = match output_string {
        Some(function) => function,
        None => return EFI_INVALID_PARAMETER,
    };
    // SAFETY: UEFI provides and owns this console protocol.
    unsafe { output_string(output, line.as_ptr()) }
}

#[no_mangle]
pub extern "efiapi" fn efi_main(
    image_handle: uefi::Handle,
    system_table: *mut SystemTable,
) -> Status {
    if system_table.is_null() {
        return EFI_INVALID_PARAMETER;
    }

    // SAFETY: UEFI passes a valid SystemTable to an EFI application.
    let output = unsafe { (*system_table).con_out };
    // SAFETY: output is obtained from the system table provided by firmware.
    let status = unsafe {
        print_ascii(output, b"ArchiaOS Bootloader: first-party UEFI loader\r\n\0")
    };
    if status != EFI_SUCCESS {
        return status;
    }

    // This milestone reads the image from the same filesystem as the EFI
    // application and validates the full ELF program table. It deliberately
    // does not jump to the kernel until memory mapping and handoff are ready.
    match unsafe { uefi::inspect_kernel(image_handle, system_table) } {
        Ok(entry) => {
            // SAFETY: output is the firmware console interface, valid before EBS.
            let status = unsafe {
                print_ascii(output, b"ArchiaOS: kernel ELF validated for this architecture; handoff pending.\r\n\0")
            };
            if status != EFI_SUCCESS {
                return status;
            }
            // SAFETY: this prints the validated address; it does not dereference it.
            let status = unsafe { print_entry(output, entry) };
            if status != EFI_SUCCESS {
                return status;
            }
            EFI_SUCCESS
        }
        Err(BootError { status, message }) => {
            // SAFETY: output remains valid while the EFI application is running.
            let _ = unsafe { print_ascii(output, message) };
            status
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
