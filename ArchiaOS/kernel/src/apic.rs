//! Pure xAPIC register planning helpers (P2).
//!
//! These routines validate hardware metadata and build register values without
//! executing privileged instructions or touching MMIO. The caller must still
//! map the controller, validate CPU capability, and install a returning IRQ path
//! before enabling delivery.

pub const IA32_APIC_BASE_MSR: u32 = 0x1b;
pub const APIC_BASE_ENABLE: u64 = 1 << 11;
pub const APIC_X2_MODE_ENABLE: u64 = 1 << 10;
pub const APIC_BASE_ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
pub const APIC_SPURIOUS_VECTOR_REGISTER: u32 = 0x0f0;
pub const APIC_LVT_TIMER_REGISTER: u32 = 0x320;
pub const APIC_INITIAL_COUNT_REGISTER: u32 = 0x380;
pub const APIC_CURRENT_COUNT_REGISTER: u32 = 0x390;
pub const APIC_DIVIDE_CONFIGURATION_REGISTER: u32 = 0x3e0;
pub const APIC_SPURIOUS_ENABLE: u32 = 1 << 8;
pub const APIC_LVT_MASKED: u32 = 1 << 16;
pub const APIC_LVT_PERIODIC: u32 = 1 << 17;
pub const APIC_LVT_TSC_DEADLINE: u32 = 2 << 17;

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

    /// Send the local-APIC EOI after a local interrupt handler has completed.
    ///
    /// # Safety
    /// Call only for an interrupt delivered through this CPU's enabled xAPIC,
    /// after its handler has completed and exactly once per acknowledged IRQ.
    pub unsafe fn end_of_interrupt(&self) {
        unsafe { self.write_register(0x0b0, 0) };
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
    if vector < 32 { return Err(ApicError::InvalidVector); }
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
    if vector < 32 { return Err(ApicError::InvalidVector); }
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
    fn timer_configuration_is_masked_and_validated() {
        assert_eq!(masked_timer_lvt(0x40), Ok(0x40 | APIC_LVT_MASKED));
        assert_eq!(masked_timer_lvt(31), Err(ApicError::InvalidVector));
        assert_eq!(timer_divide_encoding(1), Ok(0b1011));
        assert_eq!(timer_divide_encoding(16), Ok(0b0011));
        assert_eq!(timer_divide_encoding(3), Err(ApicError::InvalidDivide));
    }

    #[test]
    fn ioapic_routes_default_masked_and_decode_acpi_flags() {
        assert_eq!(masked_ioapic_low(0x40, 0, 0), Ok(0x10040));
        assert_eq!(masked_ioapic_low(0x40, 3, 3), Ok(0x1a040));
        assert_eq!(masked_ioapic_low(0x40, 2, 0), Err(ApicError::InvalidPolarity));
        assert_eq!(masked_ioapic_low(0x40, 0, 2), Err(ApicError::InvalidTriggerMode));
        assert_eq!(ioapic_destination_high(2), Ok(0x0200_0000));
        assert_eq!(ioapic_destination_high(256), Err(ApicError::InvalidDestination));
    }
}
