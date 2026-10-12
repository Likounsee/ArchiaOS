//! Read-only PCI configuration-space discovery for early x86-64 bring-up.
//!
//! This first pass uses PCI Configuration Mechanism #1 and scans bus 0 only.
//! It writes only the host bridge's CF8 address selector, never device registers.
//! Systems requiring PCIe ECAM/ACPI discovery need a later backend.

use core::arch::asm;
use core::fmt::Write;

const CONFIG_ADDRESS: u16 = 0x0cf8;
const CONFIG_DATA: u16 = 0x0cfc;
const MAX_LOGGED_DEVICES: usize = 64;

#[inline]
unsafe fn config_read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xfc);
    // SAFETY: privileged x86 port I/O. CF8 selects a config address; CFC reads it.
    unsafe {
        asm!("out dx, eax", in("dx") CONFIG_ADDRESS, in("eax") address,
             options(nomem, nostack, preserves_flags));
    }
    let value: u32;
    // SAFETY: reads the selected PCI configuration dword without modifying the device.
    unsafe {
        asm!("in eax, dx", in("dx") CONFIG_DATA, out("eax") value,
             options(nomem, nostack, preserves_flags));
    }
    value
}

/// Enumerate present functions on PCI bus 0 without writing device config registers.
pub fn scan_bus0<W: Write>(log: &mut W) {
    let mut found = 0usize;
    let mut logged = 0usize;
    for device in 0u8..32 {
        // SAFETY: configuration reads are performed at kernel privilege; absent
        // functions return vendor ID 0xffff on PCI Mechanism #1 implementations.
        let id0 = unsafe { config_read(0, device, 0, 0x00) };
        if id0 as u16 == 0xffff { continue; }
        let header0 = unsafe { config_read(0, device, 0, 0x0c) };
        let functions = if ((header0 >> 16) as u8 & 0x80) != 0 { 8 } else { 1 };
        for function in 0u8..functions {
            let id = if function == 0 { id0 } else { unsafe { config_read(0, device, function, 0x00) } };
            let vendor = id as u16;
            if vendor == 0xffff { continue; }
            let class_info = unsafe { config_read(0, device, function, 0x08) };
            let header = if function == 0 { header0 } else { unsafe { config_read(0, device, function, 0x0c) } };
            found += 1;
            if logged < MAX_LOGGED_DEVICES {
                let _ = writeln!(log,
                    "ArchiaOS: PCI 00:{:02x}.{} vendor={:04x} device={:04x} class={:02x}:{:02x}:{:02x} header={:02x}",
                    device, function, vendor, (id >> 16) as u16,
                    (class_info >> 24) as u8, (class_info >> 16) as u8,
                    (class_info >> 8) as u8, (header >> 16) as u8);
                logged += 1;
            }
        }
    }
    let _ = writeln!(log, "ArchiaOS: PCI bus 0 scan complete: {} function(s), {} logged{}",
        found, logged, if found > logged { " (output capped)" } else { "" });
}
