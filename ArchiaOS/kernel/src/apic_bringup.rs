//! ACPI MADT discovery and masked xAPIC/IOAPIC bootstrap.

use core::arch::asm;
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use super::{acpi_madt, apic, interrupts, port_out, vmm};

/// Persistent storage for the bootstrap CPU's local APIC. The dispatcher keeps
/// a pointer to this object for the kernel lifetime; it must never be moved.
static mut LOCAL_APIC_STORAGE: core::mem::MaybeUninit<apic::LocalApic> =
    core::mem::MaybeUninit::uninit();

/// Enable xAPIC mode and install its EOI dispatcher without enabling any device
/// routes. All external sources and the local timer remain masked at this stage.
pub(super) unsafe fn initialize_local_apic(madt_base: u64) -> Result<&'static apic::LocalApic, &'static str> {
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
    Ok(apic_ref)
}


/// Follow the bootloader-audited, identity-mapped ACPI chain and parse the MADT.
/// The loader validates and retains RSDP, root table, and child SDT ranges before
/// handoff; this routine still validates each table's own length/checksum again.
pub(super) unsafe fn discover_madt(rsdp_physical: u64) -> Result<acpi_madt::MadtInfo, &'static str> {
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

/// Published only after IRQ0's IOAPIC entry has been configured and left masked.
static IOAPIC_IRQ0_CONTROLLER: AtomicUsize = AtomicUsize::new(0);
static IOAPIC_IRQ0_GSI: AtomicU32 = AtomicU32::new(u32::MAX);
static PIT_IRQ0_COUNT: AtomicU64 = AtomicU64::new(0);

extern "C" fn pit_irq0_test_handler(vector: u8) -> bool {
    if vector != 0x50 {
        return false;
    }
    PIT_IRQ0_COUNT.fetch_add(1, Ordering::Release);
    true
}

/// Exercise a real external interrupt through PIT -> IOAPIC -> IDT -> EOI.
/// Returns None if firmware did not provide a usable IOAPIC IRQ0 route.
pub(super) unsafe fn ioapic_irq0_interrupt_self_test() -> Option<bool> {
    let raw = IOAPIC_IRQ0_CONTROLLER.load(Ordering::Acquire);
    let gsi = IOAPIC_IRQ0_GSI.load(Ordering::Acquire);
    if raw == 0 || gsi == u32::MAX {
        return None;
    }
    if interrupts::register_apic_vector_handler(0x50, pit_irq0_test_handler).is_err() {
        return Some(false);
    }
    // SAFETY: the pointer is published from static IOAPIC_STORAGE and stays live
    // for the kernel lifetime; the route was staged masked before publication.
    let ioapic = unsafe { &*(raw as *const apic::IoApic) };
    // Preserve IF and disable delivery before touching the PIT or IOAPIC route.
    let flags: u64;
    unsafe { asm!("pushfq", "pop {}", "cli", out(reg) flags, options(nomem)); }
    PIT_IRQ0_COUNT.store(0, Ordering::Release);

    // Program PIT channel 0 to mode 2 at approximately 100 Hz.
    // The route is still masked while the source is programmed.
    unsafe {
        port_out(0x43, 0x34);
        port_out(0x40, 11932u16 as u8);
        port_out(0x40, (11932u16 >> 8) as u8);
    }
    if unsafe { ioapic.unmask(gsi) }.is_err() {
        if flags & (1 << 9) != 0 {
            unsafe { asm!("sti", options(nomem, nostack)); }
        }
        return Some(false);
    }

    unsafe { asm!("sti", options(nomem, nostack)); }
    for _ in 0..10_000_000 {
        if PIT_IRQ0_COUNT.load(Ordering::Acquire) != 0 {
            break;
        }
        core::hint::spin_loop();
    }
    unsafe { asm!("cli", options(nomem, nostack)); }
    let delivered = PIT_IRQ0_COUNT.load(Ordering::Acquire) != 0;

    // Always re-mask the external source before returning, even on timeout.
    let masked = unsafe { ioapic.mask(gsi) }.is_ok();
    if flags & (1 << 9) != 0 {
        unsafe { asm!("sti", options(nomem, nostack)); }
    }
    Some(delivered && masked)
}

/// Map each MADT-declared IOAPIC and mask every hardware input before any route
/// can be enabled. The bootloader already audited the MADT's physical metadata.
pub(super) unsafe fn initialize_ioapics_masked(
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
        for (index, record) in topology.io_apics.iter()
            .take(topology.io_apic_count).flatten().enumerate()
        {
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
            if vector == 0x50 {
                IOAPIC_IRQ0_GSI.store(gsi, Ordering::Release);
                IOAPIC_IRQ0_CONTROLLER.store(ioapic as *const apic::IoApic as usize, Ordering::Release);
            }
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
