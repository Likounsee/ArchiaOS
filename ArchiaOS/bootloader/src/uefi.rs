use core::convert::Infallible;
use core::ffi::c_void;
use core::ptr;

use archiaos_bootloader::bootinfo::{
    BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION, UEFI_MEMORY_DESCRIPTOR_VERSION,
};
use archiaos_bootloader::memory_map::UefiMemoryMap;

// UEFI ABI declarations are kept in the same module scope so their private
// fields and function-pointer aliases remain inaccessible outside the loader.
include!("uefi/firmware.rs");

// Firmware file access and the bounded kernel-image owner.
include!("uefi/image.rs");

// ELF segment reservation/copying and architecture-specific page-table setup.
include!("uefi/kernel_segments.rs");

include!("uefi/x86_64_paging.rs");

// GOP, ACPI, and FDT discovery plus physical-range validation for handoff.
include!("uefi/firmware_discovery.rs");

include!("uefi/platform_memory_audit.rs");

include!("uefi/platform_tables.rs");

include!("uefi/handoff.rs");

include!("uefi/architecture_jump.rs");

#[cfg(test)]
include!("uefi/tests.rs");

