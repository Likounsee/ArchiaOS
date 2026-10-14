//! Pure xAPIC register planning helpers (P2).
//!
//! These routines validate hardware metadata and build register values without
//! executing privileged instructions or touching MMIO. The caller must still
//! map the controller, validate CPU capability, and install a returning IRQ path
//! before enabling delivery.

use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

pub const IA32_APIC_BASE_MSR: u32 = 0x1b;
pub const APIC_BASE_ENABLE: u64 = 1 << 11;
pub const APIC_X2_MODE_ENABLE: u64 = 1 << 10;
pub const APIC_BASE_ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
pub const APIC_SPURIOUS_VECTOR_REGISTER: u32 = 0x0f0;
pub const APIC_VERSION_REGISTER: u32 = 0x030;
pub const APIC_TASK_PRIORITY_REGISTER: u32 = 0x080;
pub const APIC_ICR_LOW_REGISTER: u32 = 0x300;
pub const APIC_ICR_HIGH_REGISTER: u32 = 0x310;
pub const APIC_ICR_DELIVERY_PENDING: u32 = 1 << 12;
pub const APIC_LVT_TIMER_REGISTER: u32 = 0x320;
pub const APIC_INITIAL_COUNT_REGISTER: u32 = 0x380;
pub const APIC_CURRENT_COUNT_REGISTER: u32 = 0x390;
pub const APIC_DIVIDE_CONFIGURATION_REGISTER: u32 = 0x3e0;
pub const APIC_SPURIOUS_ENABLE: u32 = 1 << 8;
pub const APIC_LVT_MASKED: u32 = 1 << 16;
pub const APIC_LVT_PERIODIC: u32 = 1 << 17;
pub const APIC_LVT_TSC_DEADLINE: u32 = 2 << 17;
/// Dedicated software self-test vector installed by the interrupt subsystem.
pub const APIC_TEST_RETURN_VECTOR: u8 = 0xf1;
/// Architectural APIC spurious vector; it must never be assigned to a real IRQ.
pub const APIC_SPURIOUS_VECTOR: u8 = 0xff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApicError {
    InvalidPhysicalAddress,
    ApicDisabled,
    X2ApicUnsupported,
    BaseAddressMismatch,
    InvalidVector,
    InvalidDivide,
    InvalidPolarity,
    InvalidTriggerMode,
    InvalidDestination,
    InvalidMappedPointer,
    InvalidGlobalInterrupt,
    InvalidIpiVector,
    IpiTimeout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IpiKind {
    Fixed,
    InitAssert,
    InitDeassert,
    Startup,
}

/// Encode a physical-destination xAPIC IPI command without touching hardware.
pub fn ipi_command(kind: IpiKind, vector: u8) -> Result<u32, ApicError> {
    match kind {
        IpiKind::Fixed => {
            if vector < 32 { return Err(ApicError::InvalidIpiVector); }
            Ok(u32::from(vector))
        }
        IpiKind::InitAssert => Ok((0b101 << 8) | (1 << 14) | (1 << 15)),
        IpiKind::InitDeassert => Ok((0b101 << 8) | (1 << 15)),
        IpiKind::Startup => {
            if vector == 0 { return Err(ApicError::InvalidIpiVector); }
            Ok((0b110 << 8) | u32::from(vector))
        }
    }
}

/// Reject vectors reserved for CPU exceptions, the software return-path test,
/// or the local-APIC spurious gate before programming a hardware IRQ source.
pub fn validate_hardware_irq_vector(vector: u8) -> Result<(), ApicError> {
    if vector < 32
        || (0x20..=0x2f).contains(&vector)
        || vector == APIC_TEST_RETURN_VECTOR
        || vector == APIC_SPURIOUS_VECTOR
    {
        Err(ApicError::InvalidVector)
    } else {
        Ok(())
    }
}

/// Build the software-enable register while selecting a valid spurious vector.
pub fn spurious_vector_value(vector: u8) -> Result<u32, ApicError> {
    if vector < 32 { return Err(ApicError::InvalidVector); }
    Ok(u32::from(vector) | APIC_SPURIOUS_ENABLE)
}

/// xAPIC MMIO window whose 4 KiB register page has already been mapped.
///
/// Construction is unsafe because the caller must guarantee that the pointer
/// refers to a live, uncached/device-mapped local APIC page for this CPU. This
/// type deliberately does not enable the APIC or unmask interrupts by itself.
pub struct LocalApic {
    base: *mut u32,
}

impl LocalApic {
    /// # Safety
    /// The base pointer must refer to a writable 4 KiB xAPIC MMIO mapping,
    /// mapped with appropriate device/uncached attributes, and remain mapped
    /// for this value's lifetime. It must belong to the current processor.
    pub unsafe fn from_mapped_base(base: *mut u32) -> Result<Self, ApicError> {
        if base.is_null() || (base as usize & 0xfff) != 0 {
            return Err(ApicError::InvalidMappedPointer);
        }
        Ok(Self { base })
    }

    #[inline]
    unsafe fn write_register(&self, offset: u32, value: u32) {
        // SAFETY: constructor contract; offsets are aligned xAPIC register offsets.
        unsafe { core::ptr::write_volatile(self.base.add((offset / 4) as usize), value) };
    }

    #[inline]
    unsafe fn read_register(&self, offset: u32) -> u32 {
        // SAFETY: constructor contract; offsets are aligned xAPIC register offsets.
        unsafe { core::ptr::read_volatile(self.base.add((offset / 4) as usize)) }
    }

    /// Initialize the local APIC while keeping every supported LVT source masked.
    /// The spurious-vector register is enabled last; the IOAPIC routes remain masked.
    ///
    /// # Safety
    /// The mapping must be valid and xAPIC mode active. The IDT must already
    /// contain a safe handler for the spurious vector; hardware routes must remain
    /// masked until their handlers and EOI path are installed.
    pub unsafe fn initialize_masked(
        &self,
        spurious_vector: u8,
        timer_vector: u8,
        timer_divisor: u16,
    ) -> Result<(), ApicError> {
        let svr = spurious_vector_value(spurious_vector)?;
        let timer_lvt = masked_timer_lvt(timer_vector)?;
        let divide = timer_divide_encoding(timer_divisor)?;
        let version = unsafe { self.read_register(APIC_VERSION_REGISTER) };
        let max_lvt = (version >> 16) & 0xff;
        unsafe {
            self.write_register(APIC_TASK_PRIORITY_REGISTER, 0);
            self.write_register(APIC_LVT_TIMER_REGISTER, timer_lvt);
            if max_lvt >= 1 { self.write_register(0x330, APIC_LVT_MASKED); }
            if max_lvt >= 2 { self.write_register(0x340, APIC_LVT_MASKED); }
            if max_lvt >= 3 { self.write_register(0x350, APIC_LVT_MASKED); }
            if max_lvt >= 4 { self.write_register(0x360, APIC_LVT_MASKED); }
            if max_lvt >= 5 { self.write_register(0x370, APIC_LVT_MASKED); }
            if max_lvt >= 6 { self.write_register(0x2f0, APIC_LVT_MASKED); }
            self.write_register(APIC_DIVIDE_CONFIGURATION_REGISTER, divide);
            self.write_register(APIC_INITIAL_COUNT_REGISTER, 0);
            self.write_register(APIC_SPURIOUS_VECTOR_REGISTER, svr);
        }
        Ok(())
    }

    /// Send the local-APIC EOI after a local interrupt handler has completed.
    ///
    /// # Safety
    /// Call only for an interrupt delivered through this CPU's enabled xAPIC,
    /// after its handler has completed and exactly once per acknowledged IRQ.
    pub unsafe fn end_of_interrupt(&self) {
        unsafe { self.write_register(0x0b0, 0) };
    }

    /// Send a physical-destination IPI with a bounded wait for the APIC command
    /// register. INIT/SIPI sequencing and target readiness remain the caller's job.
    ///
    /// # Safety
    /// The mapping must be valid, xAPIC mode active, and destination APIC ID
    /// must identify a present processor. INIT/SIPI can reset or start CPUs;
    /// callers must follow the platform's required delays and startup protocol.
    pub unsafe fn send_ipi(&self, destination_apic_id: u8, kind: IpiKind, vector: u8) -> Result<(), ApicError> {
        let command = ipi_command(kind, vector)?;
        let mut remaining = 1_000_000usize;
        while unsafe { self.read_register(APIC_ICR_LOW_REGISTER) } & APIC_ICR_DELIVERY_PENDING != 0 {
            if remaining == 0 { return Err(ApicError::IpiTimeout); }
            remaining -= 1;
            spin_loop();
        }
        unsafe {
            self.write_register(APIC_ICR_HIGH_REGISTER, u32::from(destination_apic_id) << 24);
            self.write_register(APIC_ICR_LOW_REGISTER, command);
        }
        remaining = 1_000_000;
        while unsafe { self.read_register(APIC_ICR_LOW_REGISTER) } & APIC_ICR_DELIVERY_PENDING != 0 {
            if remaining == 0 { return Err(ApicError::IpiTimeout); }
            remaining -= 1;
            spin_loop();
        }
        Ok(())
    }

    /// Program the local APIC timer in a masked, non-firing state.
    ///
    /// # Safety
    /// The mapping must be valid and xAPIC mode must be active. This does not
    /// calibrate a timer or enable interrupt delivery.
    pub unsafe fn configure_timer_masked(&self, vector: u8, divisor: u16) -> Result<(), ApicError> {
        let lvt = masked_timer_lvt(vector)?;
        let divide = timer_divide_encoding(divisor)?;
        unsafe {
            self.write_register(APIC_LVT_TIMER_REGISTER, lvt);
            self.write_register(APIC_DIVIDE_CONFIGURATION_REGISTER, divide);
            self.write_register(APIC_INITIAL_COUNT_REGISTER, 0);
        }
        Ok(())
    }

    /// Read the current timer count for diagnostics/calibration.
    ///
    /// # Safety
    /// The mapping must be valid and xAPIC mode must be active.
    pub unsafe fn timer_current_count(&self) -> u32 {
        unsafe { self.read_register(APIC_CURRENT_COUNT_REGISTER) }
    }
}

/// IOAPIC indirect-register window with serialized selector/data accesses.
///
/// The caller supplies the global-system-interrupt base from the MADT. This
/// wrapper only programs masked routes; enabling a route remains an explicit,
/// later step after handlers and EOI paths are ready.
pub struct IoApic {
    base: *mut u32,
    gsi_base: u32,
    selector_lock: AtomicBool,
}

unsafe impl Send for IoApic {}
unsafe impl Sync for IoApic {}

impl IoApic {
    /// # Safety
    /// base must refer to a live, writable, uncached 4 KiB IOAPIC MMIO mapping
    /// and remain mapped for this object's lifetime. The supplied GSI base
    /// must come from a validated MADT IOAPIC record.
    pub unsafe fn from_mapped_base(base: *mut u32, gsi_base: u32) -> Result<Self, ApicError> {
        if base.is_null() || (base as usize & 0xfff) != 0 {
            return Err(ApicError::InvalidMappedPointer);
        }
        Ok(Self { base, gsi_base, selector_lock: AtomicBool::new(false) })
    }

    fn lock_selector(&self) -> IoApicGuard<'_> {
        loop {
            if self.selector_lock.compare_exchange_weak(
                false, true, Ordering::Acquire, Ordering::Relaxed
            ).is_ok() {
                return IoApicGuard(&self.selector_lock);
            }
            while self.selector_lock.load(Ordering::Relaxed) { spin_loop(); }
        }
    }

    unsafe fn read_register_locked(&self, register: u8) -> u32 {
        // SAFETY: caller holds selector lock; mapping validity is constructor contract.
        unsafe {
            core::ptr::write_volatile(self.base, u32::from(register));
            core::ptr::read_volatile(self.base.add(4))
        }
    }

    unsafe fn write_register_locked(&self, register: u8, value: u32) {
        // SAFETY: caller holds selector lock; mapping validity is constructor contract.
        unsafe {
            core::ptr::write_volatile(self.base, u32::from(register));
            core::ptr::write_volatile(self.base.add(4), value);
        }
    }

    /// Number of redirection entries reported by the controller.
    ///
    /// # Safety
    /// The IOAPIC mapping must remain valid.
    pub unsafe fn redirection_count(&self) -> u32 {
        let _guard = self.lock_selector();
        let version = unsafe { self.read_register_locked(1) };
        ((version >> 16) & 0xff) + 1
    }

    /// Configure a GSI route, but leave it masked at all times.
    ///
    /// # Safety
    /// The IOAPIC mapping must remain valid, and its GSI base must match the
    /// validated MADT record. No handler is installed or route unmasked here.
    pub unsafe fn configure_masked(
        &self,
        gsi: u32,
        vector: u8,
        polarity_flags: u16,
        trigger_flags: u16,
        destination_apic_id: u32,
    ) -> Result<(), ApicError> {
        let high = ioapic_destination_high(destination_apic_id)?;
        let low = masked_ioapic_low(vector, polarity_flags, trigger_flags)?;
        let _guard = self.lock_selector();
        let version = unsafe { self.read_register_locked(1) };
        let count = ((version >> 16) & 0xff) + 1;
        let (low_register, high_register) =
            ioapic_redirection_registers(gsi, self.gsi_base, count)?;
        // Keep the low dword masked before changing destination or trigger data.
        unsafe {
            self.write_register_locked(low_register, low | APIC_LVT_MASKED);
            self.write_register_locked(high_register, high);
            self.write_register_locked(low_register, low | APIC_LVT_MASKED);
        }
        Ok(())
    }

    /// Mask an already configured route without changing its other fields.
    ///
    /// # Safety
    /// The IOAPIC mapping must remain valid and GSI must belong to this controller.
    pub unsafe fn mask(&self, gsi: u32) -> Result<(), ApicError> {
        let _guard = self.lock_selector();
        let version = unsafe { self.read_register_locked(1) };
        let count = ((version >> 16) & 0xff) + 1;
        let (low_register, _) =
            ioapic_redirection_registers(gsi, self.gsi_base, count)?;
        let low = unsafe { self.read_register_locked(low_register) };
        unsafe { self.write_register_locked(low_register, low | APIC_LVT_MASKED) };
        Ok(())
    }
}

struct IoApicGuard<'a>(&'a AtomicBool);
impl Drop for IoApicGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Resolve a GSI to the IOAPIC's low/high redirection registers after the
/// controller's redirection count has been read and validated.
pub fn ioapic_redirection_registers(
    gsi: u32,
    gsi_base: u32,
    redirection_count: u32,
) -> Result<(u8, u8), ApicError> {
    let pin = gsi.checked_sub(gsi_base).ok_or(ApicError::InvalidGlobalInterrupt)?;
    if pin >= redirection_count {
        return Err(ApicError::InvalidGlobalInterrupt);
    }
    let low = 0x10u32
        .checked_add(pin.checked_mul(2).ok_or(ApicError::InvalidGlobalInterrupt)?)
        .ok_or(ApicError::InvalidGlobalInterrupt)?;
    let high = low.checked_add(1).ok_or(ApicError::InvalidGlobalInterrupt)?;
    if high > u8::MAX as u32 {
        return Err(ApicError::InvalidGlobalInterrupt);
    }
    Ok((low as u8, high as u8))
}

/// Validate the APIC base MSR before any local-APIC MMIO access.
pub fn validate_apic_base(msr: u64, madt_base: u64) -> Result<u64, ApicError> {
    if madt_base == 0 || madt_base & 0xfff != 0 || madt_base & !APIC_BASE_ADDRESS_MASK != 0 {
        return Err(ApicError::InvalidPhysicalAddress);
    }
    if msr & APIC_X2_MODE_ENABLE != 0 {
        return Err(ApicError::X2ApicUnsupported);
    }
    if msr & APIC_BASE_ENABLE == 0 {
        return Err(ApicError::ApicDisabled);
    }
    let actual = msr & APIC_BASE_ADDRESS_MASK;
    if actual != madt_base {
        return Err(ApicError::BaseAddressMismatch);
    }
    Ok(actual)
}

/// Construct a masked, one-shot local APIC timer LVT value.
/// It stays masked until a calibrated initial count and safe handler exist.
pub fn masked_timer_lvt(vector: u8) -> Result<u32, ApicError> {
    validate_hardware_irq_vector(vector)?;
    Ok(u32::from(vector) | APIC_LVT_MASKED)
}

/// APIC timer divide register encodings from Intel SDM, in accepted divisor order.
pub fn timer_divide_encoding(divisor: u16) -> Result<u32, ApicError> {
    match divisor {
        2 => Ok(0b0000),
        4 => Ok(0b0001),
        8 => Ok(0b0010),
        16 => Ok(0b0011),
        32 => Ok(0b1000),
        64 => Ok(0b1001),
        128 => Ok(0b1010),
        1 => Ok(0b1011),
        _ => Err(ApicError::InvalidDivide),
    }
}

/// Build a masked IOAPIC redirection low dword from ACPI MADT flag encodings.
/// Polarity: 0=conforms, 1=active-high, 3=active-low.
/// Trigger: 0=conforms, 1=edge, 3=level. Conforms defaults to ISA high/edge.
pub fn masked_ioapic_low(
    vector: u8,
    polarity_flags: u16,
    trigger_flags: u16,
) -> Result<u32, ApicError> {
    validate_hardware_irq_vector(vector)?;
    let polarity = match polarity_flags {
        0 | 1 => false,
        3 => true,
        _ => return Err(ApicError::InvalidPolarity),
    };
    let level = match trigger_flags {
        0 | 1 => false,
        3 => true,
        _ => return Err(ApicError::InvalidTriggerMode),
    };
    let mut value = u32::from(vector) | (1 << 16);
    if polarity { value |= 1 << 13; }
    if level { value |= 1 << 15; }
    Ok(value)
}

/// Build an IOAPIC redirection high dword for a physical destination APIC ID.
pub fn ioapic_destination_high(apic_id: u32) -> Result<u32, ApicError> {
    if apic_id > u8::MAX as u32 { return Err(ApicError::InvalidDestination); }
    Ok(apic_id << 24)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapped_controller_constructors_reject_null_and_unaligned_addresses() {
        assert!(matches!(
            unsafe { LocalApic::from_mapped_base(core::ptr::null_mut()) },
            Err(ApicError::InvalidMappedPointer)
        ));
        assert!(matches!(
            unsafe { LocalApic::from_mapped_base(0xfee0_0001usize as *mut u32) },
            Err(ApicError::InvalidMappedPointer)
        ));
        assert!(matches!(
            unsafe { IoApic::from_mapped_base(core::ptr::null_mut(), 0) },
            Err(ApicError::InvalidMappedPointer)
        ));
        assert!(matches!(
            unsafe { IoApic::from_mapped_base(0xfec0_0001usize as *mut u32, 0) },
            Err(ApicError::InvalidMappedPointer)
        ));
    }

    #[test]
    fn validates_apic_msr_without_touching_hardware() {
        let base = 0xfee0_0000;
        assert_eq!(validate_apic_base(base | APIC_BASE_ENABLE, base), Ok(base));
        assert_eq!(validate_apic_base(base, base), Err(ApicError::ApicDisabled));
        assert_eq!(
            validate_apic_base(base | APIC_BASE_ENABLE | APIC_X2_MODE_ENABLE, base),
            Err(ApicError::X2ApicUnsupported)
        );
        assert_eq!(
            validate_apic_base(0xfec0_0000 | APIC_BASE_ENABLE, base),
            Err(ApicError::BaseAddressMismatch)
        );
        assert_eq!(
            validate_apic_base(0xfee0_0001 | APIC_BASE_ENABLE, 0xfee0_0001),
            Err(ApicError::InvalidPhysicalAddress)
        );
    }

    #[test]
    fn spurious_vector_must_be_architecturally_valid() {
        assert_eq!(spurious_vector_value(0xFF), Ok(0x1FF));
        assert_eq!(spurious_vector_value(31), Err(ApicError::InvalidVector));
    }

    #[test]
    fn ipi_commands_validate_vectors_and_encode_delivery_modes() {
        assert_eq!(ipi_command(IpiKind::Fixed, 0x40), Ok(0x40));
        assert_eq!(ipi_command(IpiKind::Fixed, 31), Err(ApicError::InvalidIpiVector));
        assert_eq!(ipi_command(IpiKind::InitAssert, 0), Ok((5 << 8) | (1 << 14) | (1 << 15)));
        assert_eq!(ipi_command(IpiKind::InitDeassert, 0), Ok((5 << 8) | (1 << 15)));
        assert_eq!(ipi_command(IpiKind::Startup, 8), Ok((6 << 8) | 8));
        assert_eq!(ipi_command(IpiKind::Startup, 0), Err(ApicError::InvalidIpiVector));
    }

    #[test]
    fn timer_configuration_is_masked_and_validated() {
        assert_eq!(masked_timer_lvt(0x40), Ok(0x40 | APIC_LVT_MASKED));
        assert_eq!(masked_timer_lvt(31), Err(ApicError::InvalidVector));
        assert_eq!(masked_timer_lvt(APIC_TEST_RETURN_VECTOR), Err(ApicError::InvalidVector));
        assert_eq!(masked_timer_lvt(APIC_SPURIOUS_VECTOR), Err(ApicError::InvalidVector));
        assert_eq!(masked_timer_lvt(0x20), Err(ApicError::InvalidVector));
        assert_eq!(masked_timer_lvt(0x2f), Err(ApicError::InvalidVector));
        assert_eq!(timer_divide_encoding(1), Ok(0b1011));
        assert_eq!(timer_divide_encoding(16), Ok(0b0011));
        assert_eq!(timer_divide_encoding(3), Err(ApicError::InvalidDivide));
    }

    #[test]
    fn ioapic_gsi_mapping_checks_ranges_and_register_overflow() {
        assert_eq!(ioapic_redirection_registers(0, 0, 24), Ok((0x10, 0x11)));
        assert_eq!(ioapic_redirection_registers(23, 0, 24), Ok((0x3e, 0x3f)));
        assert_eq!(ioapic_redirection_registers(24, 0, 24), Err(ApicError::InvalidGlobalInterrupt));
        assert_eq!(ioapic_redirection_registers(9, 10, 24), Err(ApicError::InvalidGlobalInterrupt));
        assert_eq!(ioapic_redirection_registers(u32::MAX, 0, u32::MAX), Err(ApicError::InvalidGlobalInterrupt));
        assert_eq!(ioapic_redirection_registers(119, 0, 120), Ok((0xfe, 0xff)));
        assert_eq!(ioapic_redirection_registers(120, 0, 121), Err(ApicError::InvalidGlobalInterrupt));
    }

    #[test]
    fn ioapic_routes_default_masked_and_decode_acpi_flags() {
        assert_eq!(masked_ioapic_low(0x40, 0, 0), Ok(0x10040));
        assert_eq!(masked_ioapic_low(0x40, 3, 3), Ok(0x1a040));
        assert_eq!(masked_ioapic_low(APIC_TEST_RETURN_VECTOR, 0, 0), Err(ApicError::InvalidVector));
        assert_eq!(masked_ioapic_low(APIC_SPURIOUS_VECTOR, 0, 0), Err(ApicError::InvalidVector));
        assert_eq!(masked_ioapic_low(0x20, 0, 0), Err(ApicError::InvalidVector));
        assert_eq!(masked_ioapic_low(0x2f, 0, 0), Err(ApicError::InvalidVector));
        assert_eq!(masked_ioapic_low(0x40, 2, 0), Err(ApicError::InvalidPolarity));
        assert_eq!(masked_ioapic_low(0x40, 0, 2), Err(ApicError::InvalidTriggerMode));
        assert_eq!(ioapic_destination_high(2), Ok(0x0200_0000));
        assert_eq!(ioapic_destination_high(256), Err(ApicError::InvalidDestination));
    }

    #[repr(align(4096))]
    struct MockApicPage([u32; 1024]);

    #[test]
    fn ipi_delivery_pending_timeout_does_not_overwrite_destination() {
        let mut page = MockApicPage([0; 1024]);
        page.0[(APIC_ICR_LOW_REGISTER / 4) as usize] = APIC_ICR_DELIVERY_PENDING;
        page.0[(APIC_ICR_HIGH_REGISTER / 4) as usize] = 0x1234_5678;
        // SAFETY: this is an aligned writable mock register page; its ICR
        // pending bit stays set, deterministically exercising the bounded timeout.
        let apic = unsafe {
            LocalApic::from_mapped_base(page.0.as_mut_ptr()).expect("aligned mock page")
        };
        // SAFETY: this mock never sends a hardware IPI.
        assert_eq!(
            unsafe { apic.send_ipi(3, IpiKind::Fixed, 0x40) },
            Err(ApicError::IpiTimeout)
        );
        assert_eq!(page.0[(APIC_ICR_HIGH_REGISTER / 4) as usize], 0x1234_5678);
    }

    #[test]
    fn ipi_sender_programs_destination_and_delivery_mode_in_mock_registers() {
        let mut page = MockApicPage([0; 1024]);
        // SAFETY: this aligned page is a writable mock APIC register window.
        let apic = unsafe {
            LocalApic::from_mapped_base(page.0.as_mut_ptr()).expect("aligned mock page")
        };

        // SAFETY: only ordinary memory is accessed by this test double; ICR
        // reads never report delivery-pending, so the bounded wait completes.
        unsafe { apic.send_ipi(7, IpiKind::Fixed, 0x40) }.expect("fixed IPI");
        assert_eq!(page.0[(APIC_ICR_HIGH_REGISTER / 4) as usize], 7 << 24);
        assert_eq!(page.0[(APIC_ICR_LOW_REGISTER / 4) as usize], 0x40);

        unsafe { apic.send_ipi(2, IpiKind::InitAssert, 0) }.expect("INIT assert");
        assert_eq!(page.0[(APIC_ICR_HIGH_REGISTER / 4) as usize], 2 << 24);
        assert_eq!(
            page.0[(APIC_ICR_LOW_REGISTER / 4) as usize],
            (5 << 8) | (1 << 14) | (1 << 15)
        );

        unsafe { apic.send_ipi(2, IpiKind::Startup, 8) }.expect("startup IPI");
        assert_eq!(
            page.0[(APIC_ICR_LOW_REGISTER / 4) as usize],
            (6 << 8) | 8
        );
    }

    #[test]
    fn masked_local_apic_initialization_programs_registers_without_unmasking_sources() {
        let mut page = MockApicPage([0; 1024]);
        page.0[(APIC_VERSION_REGISTER / 4) as usize] = 6 << 16;
        // SAFETY: the aligned test page is a valid writable 4 KiB mock register
        // window. No privileged instructions or real hardware are accessed.
        let apic = unsafe {
            LocalApic::from_mapped_base(page.0.as_mut_ptr()).expect("aligned mock page")
        };
        // SAFETY: this test owns the mock page and has no concurrent MMIO users.
        unsafe { apic.initialize_masked(0xff, 0x40, 16) }.expect("valid masked setup");

        assert_eq!(page.0[(APIC_TASK_PRIORITY_REGISTER / 4) as usize], 0);
        assert_eq!(page.0[(APIC_LVT_TIMER_REGISTER / 4) as usize], 0x40 | APIC_LVT_MASKED);
        for offset in [0x330u32, 0x340, 0x350, 0x360, 0x370, 0x2f0] {
            assert_eq!(
                page.0[(offset / 4) as usize],
                APIC_LVT_MASKED,
                "LVT register {offset:#x} must stay masked"
            );
        }
        assert_eq!(page.0[(APIC_DIVIDE_CONFIGURATION_REGISTER / 4) as usize], 0b0011);
        assert_eq!(page.0[(APIC_INITIAL_COUNT_REGISTER / 4) as usize], 0);
        assert_eq!(page.0[(APIC_SPURIOUS_VECTOR_REGISTER / 4) as usize], 0x1ff);
    }
}
