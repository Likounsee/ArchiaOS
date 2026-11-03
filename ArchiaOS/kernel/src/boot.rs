//! Production kernel initialization after the UEFI BootInfo handoff.

use core::fmt::Write;

use super::console::{FramebufferConsole, Serial};
use super::{
    acpi_madt, address_space, apic_bringup, context, cpu, halt_forever, heap, interrupts, memory,
    pci, scheduler, scheduler_topology, simd, vmm,
};


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

fn show_framebuffer_boot_status(info: &archiaos_bootloader::bootinfo::BootInfo) -> u8 {
    let Some((_framebuffer, pixels)) = framebuffer_pixels(info) else {
        return 1;
    };
    if !FramebufferConsole::initialize(info) {
        return 6;
    }
    // Console initialization clears the visible framebuffer and writes the banner.
    let _ = pixels;
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
    if !interrupts::breakpoint_exception_self_test() {
        let _ = writeln!(serial, "ArchiaOS: controlled #BP exception/IRETQ self-test FAILED");
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: controlled #BP exception frame and IRETQ self-test passed");
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
        6 => { let _ = writeln!(serial, "ArchiaOS: framebuffer text console unavailable (unsupported GOP geometry or pixel format)"); }
        _ => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (size conversion failed)"); }
    }
    let _ = writeln!(serial, "ArchiaOS: boot-stage=early-framebuffer-probe-complete");

    // Read-only CPU inventory is the first hardware-telemetry primitive. Keep
    // it allocation-free and independent of sensor or device-driver support.
    let cpu_info = cpu::detect();
    if !cpu_info.has_sse41 {
        let _ = writeln!(serial, "ArchiaOS: unsupported CPU: SSE4.1 is required; refusing to continue");
        halt_forever();
    }
    // Context switching must preserve the legacy x87/SSE state before any
    // task is dispatched. AVX execution remains unavailable until XSAVE state
    // is added to the context ABI.
    unsafe { context::initialize_simd() };
    let _ = writeln!(serial, "ArchiaOS: x87/SSE context state initialized (FXSAVE64/FXRSTOR64)");
    if simd::sum_u32(&[1, 2, 3, 4, 5], cpu_info.has_sse41) != 15 {
        let _ = writeln!(serial, "ArchiaOS: SSE4.1 optimized integer self-test failed");
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: SSE4.1 optimized integer self-test passed");
    // Report topology only when architectural CPUID topology leaves are available.
    // Unknown cache/NUMA/class data stays unknown; this is not AP enumeration.
    match cpu::detect_current_topology_entry() {
        Some(topology) => {
            let _ = writeln!(
                serial,
                "ArchiaOS: current CPU topology APIC={} package={:?} core={:?} cache=unknown numa=unknown class=unknown",
                topology.apic_id, topology.package_id, topology.core_id
            );
        }
        None => {
            let _ = writeln!(serial, "ArchiaOS: current CPU topology unavailable (CPUID 0x1F/0x0B unsupported or invalid)");
        }
    }
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
            let _ = writeln!(serial, "ArchiaOS: UEFI memory initialization failed: {:?}", error);
            halt_forever();
        }
    };
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
        let _ = writeln!(serial, "ArchiaOS: UEFI PMM self-test failed: {:?}", error);
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: UEFI PMM allocation/free self-test passed");

    if let Err(error) = vmm::self_test(&mut serial) {
        let _ = writeln!(serial, "ArchiaOS: VMM self-test failed: {:?}", error);
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: VMM mapping self-test passed");

    // Discover ACPI topology from the firmware tables retained by the loader.
    // Validate the firmware topology, then stage ISA routes with every entry masked.
    let mut validated_madt_local_apic_base = None;
    match unsafe { apic_bringup::discover_madt(info.acpi_rsdp) } {
        Ok(topology) => {
            match acpi_madt::validate_xapic_topology(&topology) {
                Ok(()) => {
                    validated_madt_local_apic_base = Some(topology.local_apic_address);
                    let _ = writeln!(serial, "ArchiaOS: ACPI MADT validated: local APIC={:#x}, CPUs={}, IOAPICs={}, overrides={}, legacy PIC={}",
                        topology.local_apic_address, topology.enabled_processor_count(),
                        topology.io_apic_count, topology.override_count, topology.has_legacy_pic);

                    // Build the scheduler's eligible-CPU snapshot from the validated
                    // firmware list, then enrich only the bootstrap CPU if CPUID's
                    // x2APIC identity matches a MADT entry. This is metadata only:
                    // it neither starts APs nor enables SMP or preemption.
                    let processor_count = topology.processor_count.min(acpi_madt::MAX_MADT_CPUS);
                    match scheduler_topology::from_madt_processors(&topology.processors[..processor_count]) {
                        Ok(mut cpu_topology) => {
                            let discovered = cpu::detect_current_topology_entry();
                            let merge_status = match discovered {
                                Some(entry) => match cpu_topology.merge_cpuid_entry(entry) {
                                    Ok(()) => "matched",
                                    Err(scheduler_topology::TopologyError::UnknownApicId) => "CPUID APIC ID absent from MADT",
                                    Err(_) => "CPUID metadata rejected",
                                },
                                None => "CPUID topology unavailable",
                            };
                            let _ = writeln!(serial,
                                "ArchiaOS: scheduler topology snapshot CPUs={} current-CPU merge={}",
                                cpu_topology.len(), merge_status);
                            if let Some(entry) = discovered {
                                if let Some(merged) = cpu_topology.get_by_apic_id(entry.apic_id) {
                                    let _ = writeln!(serial,
                                        "ArchiaOS: scheduler CPU APIC={} package={:?} core={:?} NUMA={:?} class={:?} capacity={:?}",
                                        merged.apic_id, merged.package_id, merged.core_id,
                                        merged.numa_node, merged.class, merged.capacity);
                                }
                            }
                        }
                        Err(error) => {
                            let _ = writeln!(serial, "ArchiaOS: scheduler topology snapshot rejected: {:?}", error);
                        }
                    }

                    match unsafe { apic_bringup::initialize_ioapics_masked(&topology) } {
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
        Some(madt_base) => match unsafe { apic_bringup::initialize_local_apic(madt_base) } {
        Ok(apic_ref) => {
            let _ = writeln!(serial, "ArchiaOS: local xAPIC initialized; dispatcher published; external IOAPIC routes remain masked");
            if !interrupts::apic_dispatch_self_test() {
                let _ = writeln!(serial, "ArchiaOS: APIC software dispatch self-test FAILED");
                halt_forever();
            }
            let _ = writeln!(serial, "ArchiaOS: APIC software dispatch and IRETQ self-test passed");
            if !interrupts::apic_timer_interrupt_self_test(apic_ref) {
                let _ = writeln!(serial, "ArchiaOS: APIC one-shot hardware timer interrupt self-test FAILED");
                halt_forever();
            }
            let _ = writeln!(serial, "ArchiaOS: APIC one-shot timer IRQ, handler, EOI and IRETQ self-test passed; timer re-masked");
            match unsafe { apic_bringup::ioapic_irq0_interrupt_self_test() } {
                Some(true) => {
                    let _ = writeln!(serial, "ArchiaOS: external PIT IRQ0 traversed IOAPIC, IDT, handler and EOI; route re-masked");
                }
                Some(false) => {
                    let _ = writeln!(serial, "ArchiaOS: external PIT IRQ0 IOAPIC self-test FAILED");
                    halt_forever();
                }
                None => {
                    let _ = writeln!(serial, "ArchiaOS: external PIT IRQ0 self-test skipped; no validated IOAPIC route");
                }
            }
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
        let _ = writeln!(serial, "ArchiaOS: kernel heap initialization failed: {:?}", error);
        halt_forever();
    }
    if let Err(error) = heap::self_test(&mut serial) {
        let _ = writeln!(serial, "ArchiaOS: kernel heap self-test failed: {:?}", error);
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: kernel heap self-test passed");

    if let Err(error) = address_space::initialize(&mut serial) {
        let _ = writeln!(serial, "ArchiaOS: address-space initialization failed: {:?}", error);
        halt_forever();
    }
    if let Err(error) = address_space::self_test(&mut serial) {
        let _ = writeln!(serial, "ArchiaOS: address-space self-test failed: {:?}", error);
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: address-space self-test passed");

    match context::self_test() {
        Ok(()) => {
            let _ = writeln!(serial, "ArchiaOS: x86-64 task context and initial stack self-test passed");
        }
        Err(()) => {
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
            let _ = writeln!(serial, "ArchiaOS: scheduler state self-test failed: {:?}", error);
            halt_forever();
        }
    }
    match scheduler::runtime_self_test() {
        Ok(()) => {
            let _ = writeln!(serial, "ArchiaOS: three-task block/wake/yield/exit context-switch self-test passed");
        }
        Err(error) => {
            let _ = writeln!(serial, "ArchiaOS: three-task scheduler runtime self-test failed: {:?}", error);
            halt_forever();
        }
    }
    match scheduler::runtime_idle_wakeup_self_test() {
        Ok(()) => {
            let _ = writeln!(serial, "ArchiaOS: block-to-idle, wakeup and task-resume self-test passed");
        }
        Err(error) => {
            let _ = writeln!(serial, "ArchiaOS: idle/wakeup runtime self-test failed: {:?}", error);
            halt_forever();
        }
    }
    let _ = writeln!(serial, "ArchiaOS: UEFI kernel entry validated");
    halt_forever();
}
