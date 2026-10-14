//! Defensive ACPI MADT parsing and xAPIC routing metadata (P2).
//!
//! This module deliberately parses firmware bytes only. It does not map MMIO,
//! enable the local APIC, program the IOAPIC, or unmask interrupts; those actions
//! require a platform-specific mapping and interrupt-dispatch lifecycle.

pub const MADT_SIGNATURE: &[u8; 4] = b"APIC";
pub const MAX_MADT_CPUS: usize = 256;
pub const MAX_MADT_IOAPICS: usize = 32;
pub const MAX_MADT_OVERRIDES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MadtError {
    TruncatedHeader,
    InvalidSignature,
    InvalidLength,
    TruncatedEntry,
    InvalidEntryLength,
    TooManyProcessors,
    TooManyIoApics,
    TooManyOverrides,
    InvalidOverride,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Processor {
    pub acpi_uid: u8,
    pub apic_id: u8,
    pub enabled: bool,
    pub online_capable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IoApic {
    pub id: u8,
    pub physical_address: u32,
    pub global_irq_base: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptOverride {
    pub bus: u8,
    pub source_irq: u8,
    pub global_irq: u32,
    /// ACPI polarity flags, preserved raw for a later validated decoder.
    pub polarity_flags: u16,
    /// ACPI trigger-mode flags, preserved raw for a later validated decoder.
    pub trigger_flags: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MadtInfo {
    pub local_apic_address: u64,
    pub processors: [Option<Processor>; MAX_MADT_CPUS],
    pub processor_count: usize,
    pub io_apics: [Option<IoApic>; MAX_MADT_IOAPICS],
    pub io_apic_count: usize,
    pub overrides: [Option<InterruptOverride>; MAX_MADT_OVERRIDES],
    pub override_count: usize,
    pub has_legacy_pic: bool,
}

impl MadtInfo {
    const fn empty(local_apic_address: u64, has_legacy_pic: bool) -> Self {
        Self {
            local_apic_address,
            processors: [None; MAX_MADT_CPUS],
            processor_count: 0,
            io_apics: [None; MAX_MADT_IOAPICS],
            io_apic_count: 0,
            overrides: [None; MAX_MADT_OVERRIDES],
            override_count: 0,
            has_legacy_pic,
        }
    }

    pub fn enabled_processor_count(&self) -> usize {
        self.processors[..self.processor_count]
            .iter()
            .flatten()
            .filter(|cpu| cpu.enabled || cpu.online_capable)
            .count()
    }
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*bytes.get(offset)?, *bytes.get(offset + 1)?]))
}
fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *bytes.get(offset)?, *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?, *bytes.get(offset + 3)?,
    ]))
}
fn u64_at(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes([
        *bytes.get(offset)?, *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?, *bytes.get(offset + 3)?,
        *bytes.get(offset + 4)?, *bytes.get(offset + 5)?,
        *bytes.get(offset + 6)?, *bytes.get(offset + 7)?,
    ]))
}

/// Parse a complete ACPI MADT (including its 36-byte SDT header).
/// Duplicate records are retained rather than silently choosing one; callers
/// must validate APIC IDs and routing conflicts before programming hardware.
pub fn parse_madt(bytes: &[u8]) -> Result<MadtInfo, MadtError> {
    const SDT_HEADER: usize = 36;
    const MADT_FIXED: usize = 44;
    if bytes.len() < SDT_HEADER { return Err(MadtError::TruncatedHeader); }
    if bytes.get(0..4) != Some(MADT_SIGNATURE.as_slice()) {
        return Err(MadtError::InvalidSignature);
    }
    let length = u32_at(bytes, 4).ok_or(MadtError::TruncatedHeader)? as usize;
    if length < MADT_FIXED || length > bytes.len() {
        return Err(MadtError::InvalidLength);
    }
    let mut info = MadtInfo::empty(
        u32_at(bytes, 36).ok_or(MadtError::InvalidLength)? as u64,
        u32_at(bytes, 40).ok_or(MadtError::InvalidLength)? & 1 != 0,
    );
    let mut cursor = MADT_FIXED;
    while cursor < length {
        if length - cursor < 2 { return Err(MadtError::TruncatedEntry); }
        let kind = bytes[cursor];
        let entry_len = bytes[cursor + 1] as usize;
        if entry_len < 2 { return Err(MadtError::InvalidEntryLength); }
        let end = cursor.checked_add(entry_len).ok_or(MadtError::InvalidEntryLength)?;
        if end > length { return Err(MadtError::TruncatedEntry); }
        let e = &bytes[cursor..end];
        match kind {
            0 => {
                if entry_len < 8 { return Err(MadtError::InvalidEntryLength); }
                if info.processor_count == MAX_MADT_CPUS { return Err(MadtError::TooManyProcessors); }
                let flags = u32_at(e, 4).ok_or(MadtError::InvalidEntryLength)?;
                info.processors[info.processor_count] = Some(Processor {
                    acpi_uid: e[2], apic_id: e[3],
                    enabled: flags & 1 != 0, online_capable: flags & 2 != 0,
                });
                info.processor_count += 1;
            }
            1 => {
                if entry_len < 12 { return Err(MadtError::InvalidEntryLength); }
                if info.io_apic_count == MAX_MADT_IOAPICS { return Err(MadtError::TooManyIoApics); }
                info.io_apics[info.io_apic_count] = Some(IoApic {
                    id: e[2],
                    physical_address: u32_at(e, 4).ok_or(MadtError::InvalidEntryLength)?,
                    global_irq_base: u32_at(e, 8).ok_or(MadtError::InvalidEntryLength)?,
                });
                info.io_apic_count += 1;
            }
            2 => {
                if entry_len < 10 { return Err(MadtError::InvalidEntryLength); }
                if info.override_count == MAX_MADT_OVERRIDES { return Err(MadtError::TooManyOverrides); }
                let flags = u16_at(e, 8).ok_or(MadtError::InvalidEntryLength)?;
                // ACPI reserves polarity and trigger encodings 2; reject them.
                if flags & 0b11 == 0b10 || (flags >> 2) & 0b11 == 0b10 {
                    return Err(MadtError::InvalidOverride);
                }
                info.overrides[info.override_count] = Some(InterruptOverride {
                    bus: e[2], source_irq: e[3],
                    global_irq: u32::from(u16_at(e, 4).ok_or(MadtError::InvalidEntryLength)?),
                    polarity_flags: flags & 0b11,
                    trigger_flags: (flags >> 2) & 0b11,
                });
                info.override_count += 1;
            }
            5 => {
                if entry_len < 12 { return Err(MadtError::InvalidEntryLength); }
                info.local_apic_address = u64_at(e, 4).ok_or(MadtError::InvalidEntryLength)?;
            }
            // Unknown ACPI-defined/future records are skipped by their validated length.
            _ => {}
        }
        cursor = end;
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(flags: u32, entries: &[u8]) -> std::vec::Vec<u8> {
        let mut b = vec![0u8; 44];
        b[0..4].copy_from_slice(b"APIC");
        b[36..40].copy_from_slice(&0xfee0_0000u32.to_le_bytes());
        b[40..44].copy_from_slice(&flags.to_le_bytes());
        b.extend_from_slice(entries);
        let len = b.len() as u32;
        b[4..8].copy_from_slice(&len.to_le_bytes());
        b
    }

    #[test]
    fn parses_processor_ioapic_and_isa_override() {
        let entries = [
            0, 8, 0, 2, 1, 0, 0, 0,             // enabled processor
            1, 12, 3, 0, 0, 0, 0xc0, 0xfe, 0, 0, 0, 0, // IOAPIC at FEC00000
            2, 10, 0, 0, 2, 0, 0, 0, 0x0d, 0, // IRQ0 -> GSI2, active-low/level
        ];
        let parsed = parse_madt(&base(0, &entries)).unwrap();
        assert_eq!(parsed.local_apic_address, 0xfee0_0000);
        assert_eq!(parsed.processor_count, 1);
        assert_eq!(parsed.enabled_processor_count(), 1);
        assert_eq!(parsed.processors[0], Some(Processor {
            acpi_uid: 0, apic_id: 2, enabled: true, online_capable: false,
        }));
        assert_eq!(parsed.io_apics[0], Some(IoApic {
            id: 3, physical_address: 0xfec0_0000, global_irq_base: 0,
        }));
        assert_eq!(parsed.overrides[0], Some(InterruptOverride {
            bus: 0, source_irq: 0, global_irq: 2, polarity_flags: 1, trigger_flags: 3,
        }));
    }

    #[test]
    fn parses_lapic_address_override_and_legacy_pic_flag() {
        let entries = [5, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let parsed = parse_madt(&base(1, &entries)).unwrap();
        assert_eq!(parsed.local_apic_address, 0);
        assert!(parsed.has_legacy_pic);
    }

    #[test]
    fn rejects_bad_signature_lengths_and_truncated_records() {
        assert_eq!(parse_madt(&[]), Err(MadtError::TruncatedHeader));
        let mut wrong = base(0, &[]);
        wrong[0] = b'X';
        assert_eq!(parse_madt(&wrong), Err(MadtError::InvalidSignature));
        let mut short = base(0, &[]);
        short[4..8].copy_from_slice(&43u32.to_le_bytes());
        assert_eq!(parse_madt(&short), Err(MadtError::InvalidLength));
        let mut truncated = base(0, &[1, 12, 1, 0]);
        assert_eq!(parse_madt(&truncated), Err(MadtError::TruncatedEntry));
        truncated[45] = 1;
    }

    #[test]
    fn rejects_reserved_interrupt_override_flags() {
        let entries = [2, 10, 0, 0, 0, 0, 0, 0, 0x02, 0];
        assert_eq!(parse_madt(&base(0, &entries)), Err(MadtError::InvalidOverride));
    }

    #[test]
    fn skips_well_formed_unknown_records() {
        let parsed = parse_madt(&base(0, &[0x7f, 4, 0xaa, 0xbb])).unwrap();
        assert_eq!(parsed.processor_count, 0);
    }
}
