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

include!("uefi/image.rs");

include!("uefi/kernel_loading.rs");

include!("uefi/platform_validation.rs");

include!("uefi/handoff.rs");

include!("uefi/architecture_jump.rs");

#[cfg(test)]
include!("uefi/tests.rs");

