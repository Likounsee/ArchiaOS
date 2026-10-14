#![no_std]
#![no_main]

mod memory_ranges;
mod memory;
mod vmm;
mod heap;
mod address_space;
mod cpu;
mod cpu_features;
mod simd;
mod pci;
mod scheduler;
mod context;
mod interrupts;
mod interrupt_vectors;
mod acpi_madt;
mod apic;
mod sync;

/// Legacy dual 8259 PIC setup. All lines stay masked until matching IDT
/// handlers and an interrupt-safe wakeup path are implemented.
mod pic {
    use super::{port_in, port_out};

    const MASTER_COMMAND: u16 = 0x20;
    const MASTER_DATA: u16 = 0x21;
    const SLAVE_COMMAND: u16 = 0xA0;
    const SLAVE_DATA: u16 = 0xA1;
    const ICW1_INITIALIZE: u8 = 0x11;
    const ICW4_8086: u8 = 0x01;
    const MASTER_VECTOR_BASE: u8 = 0x20;
    const SLAVE_VECTOR_BASE: u8 = 0x28;

    #[inline]
    unsafe fn wait() {
        // Port 0x80 is the conventional legacy I/O delay port.
        unsafe { port_out(0x80, 0) };
    }

    /// Remap legacy PIC vectors away from CPU exceptions and mask every IRQ.
    ///
    /// This deliberately does not enable interrupts or unmask any line. The
    /// IDT has vector-specific stubs, but IRQs must remain masked until a
    /// returning dispatcher and acknowledgement path are installed.
    /// Mask one legacy IRQ line without enabling any other line.
    ///
    /// This is intentionally the only mask-control API until real IRQ stubs
    /// and handlers are installed in the IDT.
    pub unsafe fn mask_irq(irq: u8) -> Result<(), ()> {
        if irq >= 16 {
            return Err(());
        }

        let master_mask = unsafe { port_in(MASTER_DATA) };
        let slave_mask = unsafe { port_in(SLAVE_DATA) };
        if !super::interrupt_vectors::pic_masks_are_consistent(master_mask, slave_mask) {
            // Refuse to mutate an already inconsistent PIC state. Silently
            // masking another line would hide the disconnected slave cascade.
            return Err(());
        }
        let Some((next_master, next_slave)) =
            super::interrupt_vectors::masks_after_masking_irq(master_mask, slave_mask, irq)
        else {
            return Err(());
        };

        // Write only masks computed by the pure, host-tested transition helper.
        // Keeping the cascade masked while any slave IRQ remains enabled avoids
        // disconnecting an active slave line.
        unsafe {
            if irq >= 8 {
                port_out(SLAVE_DATA, next_slave);
            }
            port_out(MASTER_DATA, next_master);
        }
        Ok(())
    }

    /// Acknowledge a legacy PIC IRQ only after its handler claims it.
    ///
    /// Slave EOI is emitted before master EOI for IRQ8..=IRQ15. Unhandled or
    /// invalid IRQs are rejected without touching either command port. This
    /// does not enable interrupts and does not alter masks.
    pub unsafe fn end_of_interrupt(irq: u8, handler_handled: bool) -> Result<(), ()> {
        let Some((send_slave_eoi, send_master_eoi)) =
            super::interrupt_vectors::eoi_plan_for_handled_irq(irq, handler_handled)
        else {
            return Err(());
        };

        // The pure plan validates the IRQ and preserves the required slave-
        // before-master acknowledgement order for cascaded IRQs.
        unsafe {
            if send_slave_eoi {
                port_out(SLAVE_COMMAND, 0x20);
            }
            if send_master_eoi {
                port_out(MASTER_COMMAND, 0x20);
            }
        }
        Ok(())
    }

    pub unsafe fn initialize_masked() {
        // Mask first so stale firmware/PIC state cannot deliver an IRQ during
        // the remapping sequence. IF is also kept clear by the boot entry.
        unsafe {
            port_out(MASTER_DATA, 0xFF);
            port_out(SLAVE_DATA, 0xFF);

            port_out(MASTER_COMMAND, ICW1_INITIALIZE);
            wait();
            port_out(SLAVE_COMMAND, ICW1_INITIALIZE);
            wait();
            port_out(MASTER_DATA, MASTER_VECTOR_BASE);
            wait();
            port_out(SLAVE_DATA, SLAVE_VECTOR_BASE);
            wait();
            port_out(MASTER_DATA, 1 << 2);
            wait();
            port_out(SLAVE_DATA, 2);
            wait();
            port_out(MASTER_DATA, ICW4_8086);
            wait();
            port_out(SLAVE_DATA, ICW4_8086);
            wait();

            // Keep all lines masked even if firmware left some enabled.
            port_out(MASTER_DATA, 0xFF);
            port_out(SLAVE_DATA, 0xFF);
        }
    }
}

use core::arch::{asm, global_asm};
use core::fmt::{self, Write};
use core::panic::PanicInfo;
use core::ptr;

// Keep the first load segment non-empty with an ArchiaOS-owned marker.
#[used]
#[link_section = ".archiaos_image_marker"]
static ARCHIAOS_UEFI_IMAGE_MARKER: [u64; 1] = [0x4152_4348_4941_5545];

#[cfg(not(feature = "uefi-boot"))]
compile_error!("ArchiaOS production kernel requires the uefi-boot feature");

#[cfg(feature = "uefi-boot")]
global_asm!(
    ".section .text.boot, \"ax\"",
    ".global _start",
    ".type _start, @function",
    "_start:",
    "cli",
    "cld",
    // Bring up COM1 in assembly before touching the new stack. These single-byte
    // stage markers distinguish reaching the ELF entry from a stack/Rust failure.
    // They use no UEFI services and remain available after ExitBootServices.
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
    // 'E' means the ELF entry was reached after the UEFI handoff/page-table switch.
    "mov dx, 0x3f8",
    "mov al, 0x45",
    "out dx, al",
    "mov al, 0x0d",
    "out dx, al",
    "mov al, 0x0a",
    "out dx, al",
    // Do not depend on the EFI application's inherited stack after handoff.
    "lea rsp, [rip + __boot_stack_top]",
    "and rsp, -16",
    // 'S' means the kernel-owned boot stack is installed and aligned.
    "mov dx, 0x3f8",
    "mov al, 0x53",
    "out dx, al",
    "mov al, 0x0d",
    "out dx, al",
    "mov al, 0x0a",
    "out dx, al",
    // Bootloader contract: BootInfo arrives in RDI and is preserved into Rust.
    "call rust_kernel_main",
    ".Lhalt_uefi:",
    "cli",
    "hlt",
    "jmp .Lhalt_uefi",
    ".section .bss.stack, \"aw\", @nobits",
    ".balign 4096",
    ".global __boot_stack_bottom",
    "__boot_stack_bottom:",
    ".skip 65536",
    ".global __boot_stack_top",
    "__boot_stack_top:",
);

const COM1: u16 = 0x3F8;
struct Serial;

impl Serial {
    fn init(&mut self) {
        unsafe {
            port_out(COM1 + 1, 0x00);
            port_out(COM1 + 3, 0x80);
            port_out(COM1, 0x03);
            port_out(COM1 + 1, 0x00);
            port_out(COM1 + 3, 0x03);
            port_out(COM1 + 2, 0xC7);
            port_out(COM1 + 4, 0x0B);
        }
    }
    fn write_byte(&mut self, byte: u8) {
        for _ in 0..100_000 {
            if unsafe { port_in(COM1 + 5) } & 0x20 != 0 {
                unsafe { port_out(COM1, byte) };
                return;
            }
            core::hint::spin_loop();
        }
    }
}
impl Write for Serial {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            if byte == b'\n' { self.write_byte(b'\r'); }
            self.write_byte(byte);
        }
        Ok(())
    }
}

unsafe fn port_out(port: u16, value: u8) {
    // SAFETY: privileged x86 port I/O; called only by early kernel diagnostics.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };
}
unsafe fn port_in(port: u16) -> u8 {
    let value: u8;
    // SAFETY: privileged x86 port I/O; caller must execute at kernel privilege.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags)) };
    value
}

/// Persistent storage for the bootstrap CPU's local APIC. The dispatcher keeps
/// a pointer to this object for the kernel lifetime; it must never be moved.
static mut LOCAL_APIC_STORAGE: core::mem::MaybeUninit<apic::LocalApic> =
    core::mem::MaybeUninit::uninit();

/// Enable xAPIC mode and install its EOI dispatcher without enabling any device
/// routes. All external sources and the local timer remain masked at this stage.
unsafe fn initialize_local_apic(madt_base: u64) -> Result<(), &'static str> {
    // CPUID.1:EDX[9] advertises the architectural local APIC.
    if unsafe { core::arch::x86_64::__cpuid(1) }.edx & (1 << 9) == 0 {
        return Err("CPU does not advertise the local APIC");
    }

    const IA32_APIC_BASE: u32 = apic::IA32_APIC_BASE_MSR;
    let low: u32;
    let high: u32;
    unsafe {
        asm!("rdmsr", in("ecx") IA32_APIC_BASE, out("eax") low, out("edx") high,
             options(nomem, nostack, preserves_flags));
    }
    let mut base_msr = (u64::from(high) << 32) | u64::from(low);
    if base_msr & apic::APIC_X2_MODE_ENABLE != 0 {
        return Err("firmware enabled x2APIC; xAPIC MMIO mode is not selected");
    }
    if madt_base == 0 || madt_base & 0xfff != 0
        || (base_msr & apic::APIC_BASE_ADDRESS_MASK) != madt_base
    {
        return Err("IA32_APIC_BASE does not match the validated MADT address");
    }
    if base_msr & apic::APIC_BASE_ENABLE == 0 {
        base_msr |= apic::APIC_BASE_ENABLE;
        unsafe {
            asm!("wrmsr", in("ecx") IA32_APIC_BASE,
                 in("eax") base_msr as u32, in("edx") (base_msr >> 32) as u32,
                 options(nomem, nostack, preserves_flags));
        }
    }

    let physical_base = apic::validate_apic_base(base_msr, madt_base)
        .map_err(|_| "IA32_APIC_BASE failed validation against MADT")?;
    // SAFETY: the architectural APIC-base MSR supplies the device address;
    // VMM installs a dedicated uncached mapping, not a RAM allocation alias.
    let mapped = unsafe { vmm::map_mmio_page(physical_base) }
        .map_err(|_| "could not map local APIC MMIO page")?;
    // SAFETY: the new mapping is page-aligned, persistent and uncached.
    let local_apic = unsafe { apic::LocalApic::from_mapped_base(mapped.cast::<u32>()) }
        .map_err(|_| "local APIC rejected its MMIO mapping")?;

    // Publish stable storage before enabling the APIC's spurious-vector gate.
    let storage = core::ptr::addr_of_mut!(LOCAL_APIC_STORAGE);
    unsafe { (*storage).write(local_apic); }
    let apic_ref = unsafe { (*storage).assume_init_ref() };
    unsafe { interrupts::install_local_apic_dispatcher(apic_ref) }
        .map_err(|_| "local APIC dispatcher was already installed")?;

    // Vector 0x40 is reserved for the masked timer; no IOAPIC route is unmasked.
    unsafe { apic_ref.initialize_masked(apic::APIC_SPURIOUS_VECTOR, 0x40, 16) }
        .map_err(|_| "local APIC masked initialization failed")?;
    Ok(())
}


/// Follow the bootloader-audited, identity-mapped ACPI chain and parse the MADT.
/// The loader validates and retains RSDP, root table, and child SDT ranges before
/// handoff; this routine still validates each table's own length/checksum again.
unsafe fn discover_madt(rsdp_physical: u64) -> Result<acpi_madt::MadtInfo, &'static str> {
    if rsdp_physical == 0 || rsdp_physical > usize::MAX as u64 {
        return Err("BootInfo has no usable ACPI RSDP address");
    }
    let rsdp_ptr = rsdp_physical as *const u8;
    // SAFETY: the bootloader retained and identity-mapped the minimum RSDP prefix.
    let prefix = unsafe { core::slice::from_raw_parts(rsdp_ptr, 20) };
    let revision = *prefix.get(15).ok_or("truncated ACPI RSDP")?;
    let rsdp_len = if revision >= 2 {
        // SAFETY: the bootloader audited the full extended RSDP range.
        let extended = unsafe { core::slice::from_raw_parts(rsdp_ptr, 36) };
        u32::from_le_bytes([extended[20], extended[21], extended[22], extended[23]]) as usize
    } else {
        20
    };
    if rsdp_len < 20 || rsdp_len > 4096 {
        return Err("ACPI RSDP length is outside the supported bounds");
    }
    // SAFETY: the complete RSDP was checked and retained by the bootloader.
    let rsdp_bytes = unsafe { core::slice::from_raw_parts(rsdp_ptr, rsdp_len) };
    let rsdp = acpi_madt::parse_rsdp(rsdp_bytes).map_err(|_| "ACPI RSDP validation failed")?;
    let (root_address, expected_xsdt) = match rsdp.xsdt_address {
        Some(address) => (address, true),
        None => (u64::from(rsdp.rsdt_address), false),
    };
    if root_address == 0 || root_address > usize::MAX as u64 {
        return Err("ACPI root table address is invalid");
    }
    let root_ptr = root_address as *const u8;
    // SAFETY: the bootloader audited the root SDT header and its full mapping.
    let root_prefix = unsafe { core::slice::from_raw_parts(root_ptr, 36) };
    let root_len = u32::from_le_bytes([root_prefix[4], root_prefix[5], root_prefix[6], root_prefix[7]]) as usize;
    if root_len < 36 || root_len > 1024 * 1024 {
        return Err("ACPI root table length is outside the supported bounds");
    }
    // SAFETY: the bootloader audited and retained the full root table.
    let root_bytes = unsafe { core::slice::from_raw_parts(root_ptr, root_len) };
    let root = acpi_madt::parse_root_table(root_bytes).map_err(|_| "ACPI root table validation failed")?;
    if root.xsdt != expected_xsdt {
        return Err("ACPI root signature does not match RSDP revision metadata");
    }

    for address in root.addresses.iter().take(root.count).flatten() {
        if *address == 0 || *address > usize::MAX as u64 {
            continue;
        }
        let table_ptr = *address as *const u8;
        // SAFETY: the bootloader audited every directly referenced root child SDT.
        let header = unsafe { core::slice::from_raw_parts(table_ptr, 36) };
        if header.get(0..4) != Some(b"APIC".as_slice()) {
            continue;
        }
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if length < 44 || length > 1024 * 1024 {
            return Err("ACPI MADT length is outside the supported bounds");
        }
        // SAFETY: the complete child table range was validated by the bootloader.
        let bytes = unsafe { core::slice::from_raw_parts(table_ptr, length) };
        return acpi_madt::parse_madt(bytes).map_err(|_| "ACPI MADT validation failed");
    }
    Err("ACPI root table contains no MADT")
}


/// Persistent bootstrap IOAPIC objects. All discovered redirection entries are
/// forced masked; this stage deliberately does not install or enable IRQ routes.
static mut IOAPIC_STORAGE: [core::mem::MaybeUninit<apic::IoApic>; acpi_madt::MAX_MADT_IOAPICS] =
    [const { core::mem::MaybeUninit::uninit() }; acpi_madt::MAX_MADT_IOAPICS];

/// Map each MADT-declared IOAPIC and mask every hardware input before any route
/// can be enabled. The bootloader already audited the MADT's physical metadata.
unsafe fn initialize_ioapics_masked(
    topology: &acpi_madt::MadtInfo,
) -> Result<(usize, usize), &'static str> {
    acpi_madt::validate_xapic_topology(topology)
        .map_err(|_| "MADT topology failed validation before IOAPIC setup")?;
    let storage = core::ptr::addr_of_mut!(IOAPIC_STORAGE);
    // Keep hardware-reported GSI window sizes so overlapping controllers are
    // rejected before any firmware route is assigned to an arbitrary owner.
    let mut redirection_counts = [0u32; acpi_madt::MAX_MADT_IOAPICS];
    let mut initialized = 0usize;

    for record in topology.io_apics.iter().take(topology.io_apic_count).flatten() {
        let physical = u64::from(record.physical_address);
        // SAFETY: MADT records are checksum-validated and the topology validator
        // requires a non-zero, page-aligned device address.
        let mapped = unsafe { vmm::map_mmio_page(physical) }
            .map_err(|_| "could not map an IOAPIC MMIO page")?;
        // SAFETY: mapping is persistent, aligned and uncached; GSI base is from MADT.
        let controller = unsafe {
            apic::IoApic::from_mapped_base(mapped.cast::<u32>(), record.global_irq_base)
        }.map_err(|_| "IOAPIC rejected its mapped register window")?;

        // Keep the controller object alive for future route programming.
        unsafe { (*storage)[initialized].write(controller); }
        let ioapic = unsafe { (*storage)[initialized].assume_init_ref() };
        // The hardware-reported count is authoritative. Mask every input while
        // preserving its existing vector, destination and electrical mode.
        let count = unsafe { ioapic.redirection_count() };
        // IOAPIC redirection registers occupy selector indices 0x10..=0xfe,
        // so larger reported windows cannot be addressed through this driver.
        if count == 0 || count > 120 {
            return Err("IOAPIC reports an unsupported redirection-table size");
        }
        let end = record.global_irq_base.checked_add(count)
            .ok_or("IOAPIC GSI range overflows")?;
        for previous_index in 0..initialized {
            let previous = topology.io_apics[previous_index]
                .ok_or("validated IOAPIC prefix unexpectedly contains a hole")?;
            let previous_start = previous.global_irq_base;
            let previous_end = previous_start.checked_add(redirection_counts[previous_index])
                .ok_or("previous IOAPIC GSI range overflows")?;
            if record.global_irq_base < previous_end && previous_start < end {
                return Err("IOAPIC GSI ranges overlap");
            }
        }
        redirection_counts[initialized] = count;
        for pin in 0..count {
            let gsi = record.global_irq_base.checked_add(pin)
                .ok_or("IOAPIC GSI range overflows")?;
            unsafe { ioapic.mask(gsi) }
                .map_err(|_| "could not mask an IOAPIC redirection entry")?;
        }
        initialized += 1;
    }
    // Prepare all legacy ISA IRQ routes using MADT overrides, but keep each
    // redirection masked. This makes polarity/trigger and GSI selection explicit
    // before any device handler is allowed to receive an interrupt.
    let bsp_apic_id = (unsafe { core::arch::x86_64::__cpuid(1) }.ebx >> 24) & 0xff;
    let bsp_is_in_madt = topology.processors.iter()
        .take(topology.processor_count).flatten()
        .any(|cpu| cpu.apic_id as u32 == bsp_apic_id && (cpu.enabled || cpu.online_capable));
    if !bsp_is_in_madt {
        return Err("bootstrap CPU APIC ID is absent from the validated MADT");
    }
    acpi_madt::validate_isa_gsi_uniqueness(topology)
        .map_err(|_| "multiple ISA IRQs resolve to the same GSI")?;

    // Resolve and validate every ISA route before writing any route entry.
    // If the firmware describes an uncovered GSI, fail before the first route
    // is configured; this avoids leaving a partially staged routing table.
    let mut route_plan = [(0u32, 0u8, 0u16, 0u16); 16];
    let mut planned = 0usize;
    for irq in 0u8..16 {
        // ISA IRQ2 is the legacy 8259 master-to-slave cascade, not an
        // independent device input. On common MADTs IRQ0 is overridden to GSI2;
        // programming a second IRQ2 route there would overwrite IRQ0's vector.
        // The PIC remains fully masked, so do not synthesize an IOAPIC route for
        // the obsolete cascade line.
        if irq == 2 { continue; }

        let mut gsi = u32::from(irq);
        let mut polarity = 0u16; // ISA default: active-high.
        let mut trigger = 0u16;  // ISA default: edge-triggered.
        for override_record in topology.overrides.iter()
            .take(topology.override_count).flatten()
        {
            if override_record.bus == 0 && override_record.source_irq == irq {
                gsi = override_record.global_irq;
                polarity = override_record.polarity_flags;
                trigger = override_record.trigger_flags;
                break;
            }
        }

        let mut covered = false;
        for record in topology.io_apics.iter()
            .take(topology.io_apic_count).flatten()
        {
            let index = topology.io_apics.iter()
                .take(topology.io_apic_count).position(|candidate| {
                    candidate.map(|candidate| candidate.id) == Some(record.id)
                }).ok_or("validated IOAPIC record lost its index")?;
            let end = record.global_irq_base.checked_add(redirection_counts[index])
                .ok_or("IOAPIC GSI range overflows while preflighting ISA IRQ")?;
            if gsi >= record.global_irq_base && gsi < end {
                covered = true;
                break;
            }
        }
        if !covered {
            return Err("no IOAPIC covers an ISA IRQ's resolved GSI");
        }
        let vector = 0x50u8.checked_add(irq)
            .ok_or("ISA IRQ vector calculation overflowed")?;
        route_plan[planned] = (gsi, vector, polarity, trigger);
        planned += 1;
    }

    // All routes are now known to be unique and covered. Program them only
    // after preflight succeeds; configure_masked keeps delivery disabled.
    let mut configured = 0usize;
    for (gsi, vector, polarity, trigger) in route_plan.iter().take(planned).copied() {
        let mut routed = false;
        for (index, record) in topology.io_apics.iter()
            .take(topology.io_apic_count).flatten().enumerate()
        {
            let count = redirection_counts[index];
            let end = record.global_irq_base.checked_add(count)
                .ok_or("IOAPIC GSI range overflows while applying ISA route plan")?;
            if gsi < record.global_irq_base || gsi >= end { continue; }
            let ioapic = unsafe { (*storage)[index].assume_init_ref() };
            unsafe { ioapic.configure_masked(gsi, vector, polarity, trigger, bsp_apic_id) }
                .map_err(|_| "failed to configure a masked ISA IRQ route")?;
            routed = true;
            configured += 1;
            break;
        }
        if !routed {
            return Err("preflighted ISA GSI unexpectedly lost its IOAPIC");
        }
    }
    // Routes use vectors 0x50..=0x5f and are still masked; handlers and a
    // deliberate unmask policy are required before these can be delivered.
    Ok((initialized, configured))
}

fn halt_forever() -> ! {
    loop { unsafe { asm!("cli; hlt", options(nomem, nostack)) }; }
}

// Early visual diagnostics use colored framebuffer bands until a text renderer
// and GOP geometry are part of the boot ABI. Each band is a persistent stage
// marker; a red band means the corresponding operation failed.
const FB_BACKGROUND: u32 = 0x0010_1828;
const FB_ACTIVE: u32 = 0x0050_D8_F0;
const FB_BOOTINFO_OK: u32 = 0x0000_C050;
const FB_MEMORY_OK: u32 = 0x00D0_A020;
const FB_PMM_OK: u32 = 0x0000_80E0;
const FB_CONTEXT_OK: u32 = 0x00D0_40C0;
const FB_SCHEDULER_OK: u32 = 0x00E0_E0_E0;
const FB_FAIL: u32 = 0x0000_3030;
const FB_PIXELS_PER_STATUS_LINE: usize = 32_768;

fn framebuffer_pixels(info: &archiaos_bootloader::bootinfo::BootInfo) -> Option<(*mut u32, usize)> {
    const MAX_LOW_CANONICAL_ADDRESS: u64 = 1u64 << 39;
    if info.framebuffer_base == 0 || info.framebuffer_size < 4 || info.framebuffer_base % 4 != 0 {
        return None;
    }
    let end = info.framebuffer_base.checked_add(info.framebuffer_size)?;
    if end > MAX_LOW_CANONICAL_ADDRESS {
        return None;
    }
    let pixels = usize::try_from(info.framebuffer_size / 4).ok()?;
    if pixels == 0 { return None; }
    Some((info.framebuffer_base as *mut u32, pixels))
}

fn paint_status_line(info: &archiaos_bootloader::bootinfo::BootInfo, line: usize, color: u32) {
    let Some((framebuffer, pixels)) = framebuffer_pixels(info) else { return; };
    let Some(start) = line.checked_mul(FB_PIXELS_PER_STATUS_LINE) else { return; };
    if start >= pixels { return; }
    let end = start.saturating_add(FB_PIXELS_PER_STATUS_LINE).min(pixels);
    for index in start..end {
        // SAFETY: the index is bounded by the firmware-reported framebuffer range.
        unsafe { ptr::write_volatile(framebuffer.add(index), color) };
    }
}

fn show_framebuffer_boot_status(info: &archiaos_bootloader::bootinfo::BootInfo) -> u8 {
    let Some((framebuffer, pixels)) = framebuffer_pixels(info) else {
        return 1;
    };
    for index in 0..pixels {
        // SAFETY: the index is bounded by the firmware-reported framebuffer range.
        unsafe { ptr::write_volatile(framebuffer.add(index), FB_BACKGROUND) };
    }
    paint_status_line(info, 0, FB_ACTIVE);
    0
}

#[no_mangle]
pub extern "C" fn rust_kernel_main(
    boot_info: *const archiaos_bootloader::bootinfo::BootInfo,
) -> ! {
    let mut serial = Serial;
    serial.init();
    // Install fatal exception diagnostics before touching firmware-provided data.
    // SAFETY: early boot is single-threaded and maskable interrupts remain disabled.
    unsafe { interrupts::install() };
    let _ = writeln!(serial, "ArchiaOS: x86-64 IDT installed (256 vectors; legacy PIC IRQs remain masked)");
    if !interrupts::return_path_self_test() {
        let _ = writeln!(serial, "ArchiaOS: ring-0 interrupt return-path self-test FAILED");
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: ring-0 software interrupt save/IRETQ self-test passed");
    if !interrupts::register_preservation_self_test() {
        let _ = writeln!(serial, "ArchiaOS: general-purpose register preservation self-test FAILED");
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: software interrupt preserved all 15 general-purpose registers");
    if !interrupts::spurious_return_path_self_test() {
        let _ = writeln!(serial, "ArchiaOS: APIC spurious-vector return-path self-test FAILED");
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: APIC spurious-vector return path passed (no EOI)");
    // Remap the legacy PIC away from CPU exception vectors, but keep every
    // hardware IRQ masked until dedicated IRQ gates and dispatch are ready.
    unsafe { pic::initialize_masked() };
    let _ = writeln!(serial, "ArchiaOS: legacy PIC remapped to vectors 32-47 (all IRQs masked)");
    let _ = writeln!(serial, "ArchiaOS: custom UEFI bootloader entered production kernel");
    let _ = writeln!(serial, "ArchiaOS: validating ARCHIABI BootInfo and UEFI memory map");

    // Validate the fixed BootInfo header before touching framebuffer metadata.
    // Paint the early status immediately after this bounded validation, before
    // CPUID/PCI discovery and PMM setup, so a later initialization failure still
    // leaves a visible indication on the physical monitor.
    if boot_info.is_null()
        || (boot_info as usize) % core::mem::align_of::<archiaos_bootloader::bootinfo::BootInfo>() != 0
    {
        let _ = writeln!(serial, "ArchiaOS: boot-stage=bootinfo-pointer-invalid");
        halt_forever();
    }
    let info = unsafe { &*boot_info };
    if archiaos_bootloader::bootinfo::validate_header(info).is_err() {
        let _ = writeln!(serial, "ArchiaOS: boot-stage=bootinfo-header-invalid");
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: boot-stage=bootinfo-header-valid");
    let _ = writeln!(
        serial,
        "ArchiaOS: framebuffer candidate base={:#x} size={} bytes",
        info.framebuffer_base,
        info.framebuffer_size
    );
    let framebuffer_probe = show_framebuffer_boot_status(info);
    match framebuffer_probe {
        0 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=written"); }
        1 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=unavailable (GOP base is zero)"); }
        2 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (range too small)"); }
        3 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (range overflow)"); }
        4 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (above low-canonical 512 GiB)"); }
        5 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (unaligned base)"); }
        _ => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (size conversion failed)"); }
    }
    paint_status_line(info, 1, FB_BOOTINFO_OK);
    let _ = writeln!(serial, "ArchiaOS: boot-stage=early-framebuffer-probe-complete");

    // Read-only CPU inventory is the first hardware-telemetry primitive. Keep
    // it allocation-free and independent of sensor or device-driver support.
    let cpu_info = cpu::detect();
    if !cpu_info.has_sse41 {
        paint_status_line(info, 1, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: unsupported CPU: SSE4.1 is required; refusing to continue");
        halt_forever();
    }
    // Context switching must preserve the legacy x87/SSE state before any
    // task is dispatched. AVX execution remains unavailable until XSAVE state
    // is added to the context ABI.
    unsafe { context::initialize_simd() };
    let _ = writeln!(serial, "ArchiaOS: x87/SSE context state initialized (FXSAVE64/FXRSTOR64)");
    if simd::sum_u32(&[1, 2, 3, 4, 5], cpu_info.has_sse41) != 15 {
        paint_status_line(info, 1, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: SSE4.1 optimized integer self-test failed");
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: SSE4.1 optimized integer self-test passed");
    let vendor = cpu::printable_field(&cpu_info.vendor);
    let brand = cpu::printable_field(&cpu_info.brand);
    let _ = writeln!(serial, "ArchiaOS: CPU vendor: {}", vendor);
    if !brand.is_empty() {
        let _ = writeln!(serial, "ArchiaOS: CPU brand: {}", brand);
    }
    let _ = writeln!(
        serial,
        "ArchiaOS: CPU family {:04x}, model {:04x}, stepping {}",
        cpu_info.family,
        cpu_info.model,
        cpu_info.stepping
    );
    let _ = writeln!(
        serial,
        "ArchiaOS: CPU features TSC={} SSE2={} SSE4.1={} SSE4.2={} AES={} XSAVE={} OSXSAVE={} AVX(hw/usable)={}/{} AVX2(hw/usable)={}/{} AVX512F(hw/usable)={}/{} FMA(hw/usable)={}/{} BMI1={} BMI2={}",
        cpu_info.has_tsc as u8,
        cpu_info.has_sse2 as u8,
        cpu_info.has_sse41 as u8,
        cpu_info.has_sse42 as u8,
        cpu_info.has_aesni as u8,
        cpu_info.has_xsave as u8,
        cpu_info.has_osxsave as u8,
        cpu_info.has_avx as u8,
        cpu_info.avx_state_usable as u8,
        cpu_info.has_avx2 as u8,
        cpu_info.avx2_usable as u8,
        cpu_info.has_avx512f as u8,
        cpu_info.avx512f_usable as u8,
        cpu_info.has_fma as u8,
        cpu_info.fma_usable as u8,
        cpu_info.has_bmi1 as u8,
        cpu_info.has_bmi2 as u8
    );
    let _ = writeln!(
        serial,
        "ArchiaOS: CPUID maximum basic={:#x}, extended={:#x}",
        cpu_info.max_basic_leaf,
        cpu_info.max_extended_leaf
    );

    // Read-only legacy PCI configuration discovery (bus 0 only). This does not
    // write to PCI device configuration registers; ECAM/ACPI extends coverage later.
    pci::scan_bus0(&mut serial);

    // The custom loader maps the handoff allocation and provides this pointer in
    // RDI. The initializer verifies alignment, version, structure size, bounds,
    // descriptor stride and every memory range before seeding the PMM bitmap.
    let stats = match unsafe { memory::initialize_from_bootinfo(&mut serial, boot_info) } {
        Ok(stats) => stats,
        Err(error) => {
            paint_status_line(info, 2, FB_FAIL);
            let _ = writeln!(serial, "ArchiaOS: UEFI memory initialization failed: {:?}", error);
            halt_forever();
        }
    };
    paint_status_line(info, 2, FB_MEMORY_OK);
    let _ = writeln!(serial, "ArchiaOS: UEFI BootInfo validated");
    let info = unsafe { &*boot_info };
    if info.framebuffer_size == 0 {
        let _ = writeln!(serial, "ArchiaOS: UEFI GOP framebuffer unavailable (headless mode)");
    } else {
        let _ = writeln!(
            serial,
            "ArchiaOS: UEFI GOP framebuffer base={:#x} size={} bytes",
            info.framebuffer_base,
            info.framebuffer_size
        );
    }
    let _ = writeln!(serial, "ArchiaOS: UEFI memory map entries: {}", stats.map_entries);
    let _ = writeln!(serial, "ArchiaOS: conventional memory ranges: {}, bytes: {}", stats.usable_ranges, stats.usable_bytes);
    let _ = writeln!(serial, "ArchiaOS: free physical pages in PMM window (up to 512 GiB): {}", stats.free_pages);

    if let Err(error) = memory::self_test(&mut serial) {
        paint_status_line(info, 3, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: UEFI PMM self-test failed: {:?}", error);
        halt_forever();
    }
    paint_status_line(info, 3, FB_PMM_OK);
    let _ = writeln!(serial, "ArchiaOS: UEFI PMM allocation/free self-test passed");

    if let Err(error) = vmm::self_test(&mut serial) {
        paint_status_line(info, 3, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: VMM self-test failed: {:?}", error);
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: VMM mapping self-test passed");

    // Discover ACPI topology from the firmware tables retained by the loader.
    // Validate the firmware topology, then stage ISA routes with every entry masked.
    let mut validated_madt_local_apic_base = None;
    match unsafe { discover_madt(info.acpi_rsdp) } {
        Ok(topology) => {
            match acpi_madt::validate_xapic_topology(&topology) {
                Ok(()) => {
                    validated_madt_local_apic_base = Some(topology.local_apic_address);
                    let _ = writeln!(serial, "ArchiaOS: ACPI MADT validated: local APIC={:#x}, CPUs={}, IOAPICs={}, overrides={}, legacy PIC={}",
                        topology.local_apic_address, topology.enabled_processor_count(),
                        topology.io_apic_count, topology.override_count, topology.has_legacy_pic);
                    match unsafe { initialize_ioapics_masked(&topology) } {
                        Ok((count, routes)) => {
                            let _ = writeln!(serial, "ArchiaOS: {} IOAPIC controller(s) mapped; {} ISA routes configured and kept masked", count, routes);
                        }
                        Err(reason) => {
                            let _ = writeln!(serial, "ArchiaOS: IOAPIC initialization deferred: {}", reason);
                        }
                    }
                }
                Err(error) => {
                    let _ = writeln!(serial, "ArchiaOS: ACPI MADT topology rejected: {:?}", error);
                }
            }
        }
        Err(reason) => {
            let _ = writeln!(serial, "ArchiaOS: ACPI MADT unavailable: {}", reason);
        }
    }

    // Initialize the bootstrap CPU's xAPIC only after the page-table allocator
    // is operational. No IOAPIC routes or timer interrupts are enabled yet.
    match validated_madt_local_apic_base {
        Some(madt_base) => match unsafe { initialize_local_apic(madt_base) } {
        Ok(()) => {
            let _ = writeln!(serial, "ArchiaOS: local xAPIC initialized; dispatcher published; sources remain masked");
        }
        Err(reason) => {
            let _ = writeln!(serial, "ArchiaOS: local xAPIC initialization skipped: {}", reason);
        }
    },
        None => {
            let _ = writeln!(serial, "ArchiaOS: local xAPIC initialization skipped: no validated MADT base address");
        }
    }

    if let Err(error) = heap::initialize(&mut serial) {
        paint_status_line(info, 3, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: kernel heap initialization failed: {:?}", error);
        halt_forever();
    }
    if let Err(error) = heap::self_test(&mut serial) {
        paint_status_line(info, 3, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: kernel heap self-test failed: {:?}", error);
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: kernel heap self-test passed");

    if let Err(error) = address_space::initialize(&mut serial) {
        paint_status_line(info, 3, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: address-space initialization failed: {:?}", error);
        halt_forever();
    }
    if let Err(error) = address_space::self_test(&mut serial) {
        paint_status_line(info, 3, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: address-space self-test failed: {:?}", error);
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: address-space self-test passed");

    match context::self_test() {
        Ok(()) => {
            paint_status_line(info, 4, FB_CONTEXT_OK);
            let _ = writeln!(serial, "ArchiaOS: x86-64 task context and initial stack self-test passed");
        }
        Err(()) => {
            paint_status_line(info, 4, FB_FAIL);
            let _ = writeln!(serial, "ArchiaOS: x86-64 task context self-test failed");
            halt_forever();
        }
    }

    // Validate the state model and then perform a real three-task round-robin
    // context-switch test. Each task has a separate static stack and CPU context;
    // block/wake, yielding, and exit must return safely to the boot stack.
    match scheduler::Scheduler::self_test() {
        Ok(()) => {
            let _ = writeln!(serial, "ArchiaOS: cooperative scheduler state self-test passed");
        }
        Err(error) => {
            paint_status_line(info, 5, FB_FAIL);
            let _ = writeln!(serial, "ArchiaOS: scheduler state self-test failed: {:?}", error);
            halt_forever();
        }
    }
    match scheduler::runtime_self_test() {
        Ok(()) => {
            paint_status_line(info, 5, FB_SCHEDULER_OK);
            let _ = writeln!(serial, "ArchiaOS: three-task block/wake/yield/exit context-switch self-test passed");
        }
        Err(error) => {
            paint_status_line(info, 5, FB_FAIL);
            let _ = writeln!(serial, "ArchiaOS: three-task scheduler runtime self-test failed: {:?}", error);
            halt_forever();
        }
    }
    match scheduler::runtime_idle_wakeup_self_test() {
        Ok(()) => {
            let _ = writeln!(serial, "ArchiaOS: block-to-idle, wakeup and task-resume self-test passed");
        }
        Err(error) => {
            paint_status_line(info, 5, FB_FAIL);
            let _ = writeln!(serial, "ArchiaOS: idle/wakeup runtime self-test failed: {:?}", error);
            halt_forever();
        }
    }
    let _ = writeln!(serial, "ArchiaOS: UEFI kernel entry validated");
    halt_forever();
}

/// Fatal early-kernel exception dispatcher called by the assembly stubs.
/// The stubs normalize the stack to pass the vector and CPU error code.
#[no_mangle]
pub extern "C" fn archiaos_exception_dispatch(vector: u64, error_code: u64, instruction_pointer: u64) -> ! {
    let mut serial = Serial;
    serial.init();

    // Until a returning IRQ dispatcher exists, classify legacy PIC vectors
    // accurately and stop without sending an EOI from this fatal path.
    if vector <= u8::MAX as u64 {
        if let interrupts::InterruptVectorClass::LegacyPicIrq(irq) =
            interrupts::classify_interrupt_vector(vector as u8)
        {
            let _ = writeln!(
                serial,
                "ArchiaOS: FATAL unhandled legacy PIC IRQ vector={} irq={} (no EOI; interrupts remain disabled)",
                vector,
                irq
            );
            let _ = writeln!(serial, "ArchiaOS: interrupted RIP={:#018x}", instruction_pointer);
            halt_forever();
        }
    }

    let name = match vector {
        0 => "divide error", 1 => "debug", 2 => "non-maskable interrupt",
        3 => "breakpoint", 4 => "overflow", 5 => "bound range",
        6 => "invalid opcode", 7 => "device not available", 8 => "double fault",
        9 => "coprocessor segment overrun", 10 => "invalid TSS",
        11 => "segment not present", 12 => "stack-segment fault",
        13 => "general protection fault", 14 => "page fault",
        16 => "x87 floating-point exception", 17 => "alignment check",
        18 => "machine check", 19 => "SIMD floating-point exception",
        20 => "virtualization exception", 21 => "control protection exception",
        29 => "VMM communication exception", 30 => "security exception",
        _ => "reserved or unclassified exception",
    };
    let _ = writeln!(serial, "ArchiaOS: FATAL x86-64 exception vector={} ({})", vector, name);
    let _ = writeln!(serial, "ArchiaOS: exception error_code={:#018x}", error_code);
    let _ = writeln!(serial, "ArchiaOS: exception RIP={:#018x}", instruction_pointer);
    if vector == 14 {
        let fault_address: u64;
        // SAFETY: CR2 is read only in the page-fault diagnostic path at kernel privilege.
        unsafe { asm!("mov {}, cr2", out(reg) fault_address, options(nomem, nostack, preserves_flags)) };
        let _ = writeln!(serial, "ArchiaOS: page-fault address={:#018x}", fault_address);
    }
    halt_forever()
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let mut serial = Serial;
    serial.init();
    let _ = writeln!(serial, "ArchiaOS: PANIC: {}", info);
    halt_forever();
}
