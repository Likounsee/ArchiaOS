#![no_std]

#[cfg(test)]
extern crate std;

/// Versioned x86-64/AArch64 boot contract passed in the first argument register.
/// The memory map remains owned by the bootloader allocation and is not copied.
#[path = "contracts/bootinfo.rs"]
pub mod bootinfo;

/// Validation for a directly addressable UEFI Graphics Output Protocol framebuffer.
#[path = "graphics/framebuffer.rs"]
pub mod framebuffer;

/// Allocation-free validation and access to UEFI memory-map descriptors.
#[path = "memory/memory_map.rs"]
pub mod memory_map;

/// Bounded RSDP validation for ACPI platform metadata.
#[path = "firmware/acpi.rs"]
pub mod acpi;

/// Minimal, allocation-free parser for the flattened Device Tree (FDT) header.
#[path = "firmware/device_tree.rs"]
pub mod device_tree;

// Defensive, allocation-free ELF64 validation for the first-party loader.
// Parsing is separate from firmware I/O so malformed-image cases can be tested
// on the host before any physical memory is allocated.
#[path = "formats/elf.rs"]
pub mod elf;

#[cfg(test)]
#[path = "tests/bootinfo_elf.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/device_tree.rs"]
mod device_tree_tests;

#[cfg(test)]
#[path = "tests/acpi.rs"]
mod acpi_tests;

#[cfg(test)]
#[path = "tests/memory_map.rs"]
mod memory_map_tests;
