#![no_std]
#![no_main]

mod uefi;

use core::convert::Infallible;
use core::panic::PanicInfo;
use uefi::{BootError, Status, SystemTable, EFI_INVALID_PARAMETER, EFI_LOAD_ERROR, EFI_SUCCESS};

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

unsafe fn report_boot_error(
    output: *mut uefi::SimpleTextOutputProtocol,
    error: BootError,
) -> Status {
    // SAFETY: output is the firmware console interface while boot services are active.
    let _ = unsafe { print_ascii(output, error.message) };
    error.status
}

#[no_mangle]
pub extern "efiapi" fn efi_main(
    image_handle: uefi::Handle,
    system_table: *mut SystemTable,
) -> Status {
    if system_table.is_null() {
        return EFI_INVALID_PARAMETER;
    }
    // SAFETY: UEFI passes a valid SystemTable to this EFI application.
    let output = unsafe { (*system_table).con_out };
    // SAFETY: output came from the firmware system table.
    let mut status = unsafe {
        print_ascii(output, b"ArchiaOS Bootloader: first-party UEFI loader\r\n\0")
    };
    if status != EFI_SUCCESS {
        return status;
    }

    // Read from the same filesystem device from which this EFI application was loaded.
    // SAFETY: firmware handles and protocols are queried only while boot services are active.
    status = unsafe { print_ascii(output, b"ArchiaOS Bootloader: opening EFI kernel image\r\n\0") };
    if status != EFI_SUCCESS {
        return status;
    }
    let kernel_image = match unsafe { uefi::read_kernel_image(image_handle, system_table) } {
        Ok(image) => image,
        Err(error) => return unsafe { report_boot_error(output, error) },
    };

    // Validate the full ELF header/program table for the current firmware architecture.
    // SAFETY: the kernel file was read into a live bounded pool allocation.
    status = unsafe { print_ascii(output, b"ArchiaOS Bootloader: validating ELF64 kernel\r\n\0") };
    if status != EFI_SUCCESS {
        return status;
    }
    let parsed = match archiaos_bootloader::elf::parse_elf64(
        kernel_image.as_slice(),
        uefi::EXPECTED_ELF_MACHINE,
    ) {
        Ok(elf) => elf,
        Err(_) => {
            // SAFETY: output remains valid before ExitBootServices.
            let _ = unsafe {
                print_ascii(output, b"ArchiaOS Bootloader: ELF64 validation failed\r\n\0")
            };
            return EFI_LOAD_ERROR;
        }
    };

    // This initial loader supports identity-mapped physical ELF images. The production
    // x86-64 high-half kernel remains on Limine until page-table setup is implemented.
    // SAFETY: segments have been validated; the loader checks address/page overlap before allocation.
    status = unsafe {
        print_ascii(output, b"ArchiaOS Bootloader: loading identity-mapped ELF segments\r\n\0")
    };
    if status != EFI_SUCCESS {
        return status;
    }
    // SAFETY: firmware services and the kernel image buffer remain valid at this point.
    let kernel_entry = match unsafe {
        uefi::load_kernel_segments(
            unsafe { (*system_table).boot_services },
            kernel_image.as_slice(),
            &parsed,
        )
    } {
        Ok(entry) => entry,
        Err(error) => return unsafe { report_boot_error(output, error) },
    };
    // No longer needed; freeing it before the final memory-map snapshot avoids a stale EBS key.
    drop(kernel_image);

    // This message is printed before taking the final memory map. No console or allocation
    // service is called between the successful GetMemoryMap and ExitBootServices attempts.
    // SAFETY: output remains valid up until ExitBootServices succeeds.
    status = unsafe {
        print_ascii(output, b"ArchiaOS Bootloader: exiting UEFI boot services\r\n\0")
    };
    if status != EFI_SUCCESS {
        return status;
    }

    match unsafe { uefi::transfer_to_kernel(image_handle, system_table, kernel_entry) } {
        Ok(never) => match never {},
        Err(error) => unsafe { report_boot_error(output, error) },
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
