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
    InvalidChecksum,
    InvalidLength,
    TruncatedEntry,
    InvalidEntryLength,
    TooManyProcessors,
    TooManyIoApics,
    TooManyOverrides,
    InvalidOverride,
    InvalidLocalApicAddress,
    InvalidIoApicAddress,
    DuplicateApicId,
    DuplicateIoApicId,
    DuplicateInterruptOverride,
    NoEnabledProcessor,
    InvalidRootSignature,
    InvalidRootEntrySize,
    TooManyRootEntries,
    InvalidRsdpSignature,
    InvalidRsdpLength,
    InvalidRsdpChecksum,
    UnsupportedRsdpRevision,
    InvalidTopologyCounts,
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
        self.processors.iter().take(self.processor_count.min(MAX_MADT_CPUS))
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RsdpInfo {
    pub revision: u8,
    pub rsdt_address: u32,
    pub xsdt_address: Option<u64>,
}

/// Validate an ACPI RSDP byte image before using its root-table addresses.
/// The returned addresses are metadata only; this function never dereferences
/// physical addresses or assumes they are mapped in the current address space.
pub fn parse_rsdp(bytes: &[u8]) -> Result<RsdpInfo, MadtError> {
    if bytes.len() < 20 { return Err(MadtError::InvalidRsdpLength); }
    if bytes.get(0..8) != Some(b"RSD PTR ".as_slice()) {
        return Err(MadtError::InvalidRsdpSignature);
    }
    if bytes[..20].iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
        return Err(MadtError::InvalidRsdpChecksum);
    }
    let revision = bytes[15];
    if revision == 1 {
        return Err(MadtError::UnsupportedRsdpRevision);
    }
    let rsdt_address = u32_at(bytes, 16).ok_or(MadtError::InvalidRsdpLength)?;
    if revision < 2 {
        return Ok(RsdpInfo { revision, rsdt_address, xsdt_address: None });
    }
    if bytes.len() < 36 { return Err(MadtError::InvalidRsdpLength); }
    let length = u32_at(bytes, 20).ok_or(MadtError::InvalidRsdpLength)? as usize;
    if length < 36 || length > bytes.len() { return Err(MadtError::InvalidRsdpLength); }
    if bytes[..length].iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
        return Err(MadtError::InvalidRsdpChecksum);
    }
    let xsdt_address = u64_at(bytes, 24).ok_or(MadtError::InvalidRsdpLength)?;
    Ok(RsdpInfo {
        revision,
        rsdt_address,
        xsdt_address: if xsdt_address == 0 { None } else { Some(xsdt_address) },
    })
}

/// Maximum root-table entries retained without allocating.
pub const MAX_ACPI_ROOT_ENTRIES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcpiRootTable {
    pub xsdt: bool,
    pub addresses: [Option<u64>; MAX_ACPI_ROOT_ENTRIES],
    pub count: usize,
}

/// Parse a checksum-verified RSDT/XSDT into physical-address metadata.
/// This function never dereferences those addresses; a mapping layer must
/// validate each referenced SDT before reading its signature or contents.
pub fn parse_root_table(root: &[u8]) -> Result<AcpiRootTable, MadtError> {
    const SDT_HEADER: usize = 36;
    if root.len() < SDT_HEADER { return Err(MadtError::TruncatedHeader); }
    let xsdt = match root.get(0..4) {
        Some(sig) if sig == b"XSDT" => true,
        Some(sig) if sig == b"RSDT" => false,
        _ => return Err(MadtError::InvalidRootSignature),
    };
    let length = u32_at(root, 4).ok_or(MadtError::InvalidLength)? as usize;
    if length < SDT_HEADER || length > root.len() { return Err(MadtError::InvalidLength); }
    if root[..length].iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
        return Err(MadtError::InvalidChecksum);
    }
    let entry_size = if xsdt { 8 } else { 4 };
    if (length - SDT_HEADER) % entry_size != 0 { return Err(MadtError::InvalidRootEntrySize); }
    let count = (length - SDT_HEADER) / entry_size;
    if count > MAX_ACPI_ROOT_ENTRIES { return Err(MadtError::TooManyRootEntries); }
    let mut table = AcpiRootTable { xsdt, addresses: [None; MAX_ACPI_ROOT_ENTRIES], count };
    for index in 0..count {
        let offset = SDT_HEADER + index * entry_size;
        let address = if xsdt {
            u64_at(root, offset).ok_or(MadtError::InvalidRootEntrySize)?
        } else {
            u32_at(root, offset).ok_or(MadtError::InvalidRootEntrySize)? as u64
        };
        if address == 0 { return Err(MadtError::InvalidRootEntrySize); }
        table.addresses[index] = Some(address);
    }
    Ok(table)
}

/// Validate topology invariants required before enabling xAPIC routing.
/// This is intentionally pure: passing validation does not imply MMIO is mapped
/// or that the processor supports xAPIC mode.
pub fn validate_xapic_topology(info: &MadtInfo) -> Result<(), MadtError> {
    if info.processor_count > MAX_MADT_CPUS
        || info.io_apic_count > MAX_MADT_IOAPICS
        || info.override_count > MAX_MADT_OVERRIDES
    {
        return Err(MadtError::InvalidTopologyCounts);
    }
    if info.local_apic_address == 0 || info.local_apic_address & 0xfff != 0 {
        return Err(MadtError::InvalidLocalApicAddress);
    }
    if info.enabled_processor_count() == 0 {
        return Err(MadtError::NoEnabledProcessor);
    }
    // Counts describe dense prefixes produced by parse_madt. Reject holes
    // rather than letting hardware initialization silently skip firmware records.
    if info.processors[..info.processor_count].iter().any(Option::is_none)
        || info.io_apics[..info.io_apic_count].iter().any(Option::is_none)
        || info.overrides[..info.override_count].iter().any(Option::is_none)
    {
        return Err(MadtError::InvalidTopologyCounts);
    }
    for i in 0..info.processor_count {
        let Some(cpu) = info.processors[i] else {
            return Err(MadtError::InvalidTopologyCounts);
        };
        if !(cpu.enabled || cpu.online_capable) { continue; }
        for other in 0..i {
            if let Some(previous) = info.processors[other] {
                if (previous.enabled || previous.online_capable) && previous.apic_id == cpu.apic_id {
                    return Err(MadtError::DuplicateApicId);
                }
            }
        }
    }
    for i in 0..info.io_apic_count {
        let Some(io) = info.io_apics[i] else {
            return Err(MadtError::InvalidTopologyCounts);
        };
        if io.physical_address == 0 || io.physical_address & 0xfff != 0
            || u64::from(io.physical_address) == info.local_apic_address
        {
            // Local APIC and IOAPIC register windows must never alias: the
            // drivers use different register protocols at these MMIO pages.
            return Err(MadtError::InvalidIoApicAddress);
        }
        for other in 0..i {
            if let Some(previous) = info.io_apics[other] {
                if previous.id == io.id { return Err(MadtError::DuplicateIoApicId); }
                if previous.physical_address == io.physical_address {
                    return Err(MadtError::InvalidIoApicAddress);
                }
            }
        }
    }
    for i in 0..info.override_count {
        let Some(route) = info.overrides[i] else {
            return Err(MadtError::InvalidTopologyCounts);
        };
        // This bring-up path implements ISA interrupt-source overrides only.
        // Reject records that cannot be mapped to one of the 16 legacy ISA IRQs
        // instead of silently ignoring them while constructing the route table.
        // Also validate flags here because MadtInfo can be built by callers
        // without going through parse_madt().
        if route.bus != 0
            || route.source_irq >= 16
            || !matches!(route.polarity_flags, 0 | 1 | 3)
            || !matches!(route.trigger_flags, 0 | 1 | 3)
        {
            return Err(MadtError::InvalidOverride);
        }
        for other in 0..i {
            if let Some(previous) = info.overrides[other] {
                if previous.bus == route.bus && previous.source_irq == route.source_irq {
                    return Err(MadtError::DuplicateInterruptOverride);
                }
            }
        }
    }
    Ok(())
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
    // ACPI SDT checksum covers exactly the declared table length, not trailing
    // bytes that may belong to another allocation or firmware object.
    if bytes[..length].iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
        return Err(MadtError::InvalidChecksum);
    }
    let mut info = MadtInfo::empty(
        u32_at(bytes, 36).ok_or(MadtError::InvalidLength)? as u64,
        u32_at(bytes, 40).ok_or(MadtError::InvalidLength)? & 1 != 0,
    );
    let mut cursor = MADT_FIXED;
    // ACPI permits one Local APIC Address Override. Accepting several and
    // silently taking the last would make the effective MMIO base depend on
    // record order, even though every individual record is structurally valid.
    let mut saw_local_apic_override = false;
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
                // Only bits 0..=3 are defined by the MADT override flags:
                // polarity in bits 0..1 and trigger mode in bits 2..3.
                // Reject both reserved encodings and any reserved high bits.
                if flags & !0x0f != 0
                    || flags & 0b11 == 0b10
                    || (flags >> 2) & 0b11 == 0b10
                {
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
                if saw_local_apic_override {
                    return Err(MadtError::InvalidLocalApicAddress);
                }
                let address = u64_at(e, 4).ok_or(MadtError::InvalidEntryLength)?;
                if address == 0 || address & 0xfff != 0 {
                    return Err(MadtError::InvalidLocalApicAddress);
                }
                info.local_apic_address = address;
                saw_local_apic_override = true;
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
        // ACPI SDT checksum byte is at offset 9; zero it before calculation.
        b[9] = 0;
        let checksum = 0u8.wrapping_sub(b.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)));
        b[9] = checksum;
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
        let mut bad_checksum = base(0, &[]);
        bad_checksum[20] ^= 0x80;
        assert_eq!(parse_madt(&bad_checksum), Err(MadtError::InvalidChecksum));
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
    fn topology_rejects_isa_overrides_outside_implemented_irq_domain() {
        let processor = [0, 8, 0, 2, 1, 0, 0, 0];
        // Bus 0 is ISA; its source IRQ field must be in 0..=15.
        let invalid_irq = [2, 10, 0, 16, 1, 0, 0, 0, 0, 0];
        let mut entries = processor.to_vec();
        entries.extend_from_slice(&invalid_irq);
        let topology = parse_madt(&base(0, &entries)).unwrap();
        assert_eq!(validate_xapic_topology(&topology), Err(MadtError::InvalidOverride));
    }

    #[test]
    fn rejects_duplicate_or_misaligned_local_apic_address_overrides() {
        let first = [5, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut duplicate = first.to_vec();
        duplicate.extend_from_slice(&first);
        assert_eq!(
            parse_madt(&base(0, &duplicate)),
            Err(MadtError::InvalidLocalApicAddress)
        );

        let misaligned = [5, 12, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            parse_madt(&base(0, &misaligned)),
            Err(MadtError::InvalidLocalApicAddress)
        );
    }

    #[test]
    fn rejects_reserved_high_bits_in_interrupt_override_flags() {
        let entries = [2, 10, 0, 0, 0, 0, 0, 0, 0, 0x10];
        assert_eq!(parse_madt(&base(0, &entries)), Err(MadtError::InvalidOverride));
    }

    #[test]
    fn topology_rejects_unsupported_override_bus_and_invalid_raw_flags() {
        let processor = [0, 8, 0, 2, 1, 0, 0, 0];
        let non_isa_bus = [2, 10, 1, 0, 1, 0, 0, 0, 0, 0];
        let mut entries = processor.to_vec();
        entries.extend_from_slice(&non_isa_bus);
        let topology = parse_madt(&base(0, &entries)).unwrap();
        assert_eq!(validate_xapic_topology(&topology), Err(MadtError::InvalidOverride));

        let mut topology = parse_madt(&base(0, &processor)).unwrap();
        topology.override_count = 1;
        topology.overrides[0] = Some(InterruptOverride {
            bus: 0, source_irq: 0, global_irq: 0,
            polarity_flags: 2, trigger_flags: 0,
        });
        assert_eq!(validate_xapic_topology(&topology), Err(MadtError::InvalidOverride));
    }

    #[test]
    fn parses_and_checks_legacy_and_extended_rsdp() {
        fn checksum(bytes: &mut [u8], index: usize, end: usize) {
            bytes[index] = 0;
            bytes[index] = 0u8.wrapping_sub(bytes[..end].iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)));
        }
        let mut legacy = vec![0u8; 20];
        legacy[..8].copy_from_slice(b"RSD PTR ");
        legacy[15] = 0;
        legacy[16..20].copy_from_slice(&0x1234u32.to_le_bytes());
        checksum(&mut legacy, 8, 20);
        assert_eq!(parse_rsdp(&legacy), Ok(RsdpInfo { revision: 0, rsdt_address: 0x1234, xsdt_address: None }));

        let mut extended = vec![0u8; 36];
        extended[..8].copy_from_slice(b"RSD PTR ");
        extended[15] = 2;
        extended[16..20].copy_from_slice(&0x1234u32.to_le_bytes());
        extended[20..24].copy_from_slice(&36u32.to_le_bytes());
        extended[24..32].copy_from_slice(&0x1_0000_5678u64.to_le_bytes());
        checksum(&mut extended, 8, 20);
        checksum(&mut extended, 32, 36);
        // Extended checksum includes the legacy checksum byte, so refresh it last.
        checksum(&mut extended, 8, 20);
        checksum(&mut extended, 32, 36);
        assert_eq!(parse_rsdp(&extended), Ok(RsdpInfo { revision: 2, rsdt_address: 0x1234, xsdt_address: Some(0x1_0000_5678) }));

        let mut invalid = extended.clone();
        invalid[30] ^= 1;
        assert_eq!(parse_rsdp(&invalid), Err(MadtError::InvalidRsdpChecksum));

        let mut unsupported = legacy.clone();
        unsupported[15] = 1;
        checksum(&mut unsupported, 8, 20);
        assert_eq!(parse_rsdp(&unsupported), Err(MadtError::UnsupportedRsdpRevision));
    }

    #[test]
    fn parses_checked_rsdt_and_xsdt_address_lists() {
        fn root(sig: &[u8; 4], addresses: &[u64], wide: bool) -> std::vec::Vec<u8> {
            let entry_size = if wide { 8 } else { 4 };
            let mut b = vec![0u8; 36 + addresses.len() * entry_size];
            b[0..4].copy_from_slice(sig);
            for (i, address) in addresses.iter().enumerate() {
                let off = 36 + i * entry_size;
                if wide {
                    b[off..off + 8].copy_from_slice(&address.to_le_bytes());
                } else {
                    b[off..off + 4].copy_from_slice(&(*address as u32).to_le_bytes());
                }
            }
            let len = b.len() as u32;
            b[4..8].copy_from_slice(&len.to_le_bytes());
            b[9] = 0;
            b[9] = 0u8.wrapping_sub(b.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)));
            b
        }
        let rsdt = parse_root_table(&root(b"RSDT", &[0x1234, 0x5678], false)).unwrap();
        assert!(!rsdt.xsdt);
        assert_eq!(rsdt.count, 2);
        assert_eq!(rsdt.addresses[1], Some(0x5678));

        let xsdt = parse_root_table(&root(b"XSDT", &[0x1_0000_1234], true)).unwrap();
        assert!(xsdt.xsdt);
        assert_eq!(xsdt.addresses[0], Some(0x1_0000_1234));
    }

    #[test]
    fn rejects_corrupted_topology_counts_without_panicking() {
        let mut info = MadtInfo::empty(0xfee0_0000, true);
        info.processor_count = MAX_MADT_CPUS + 1;
        assert_eq!(info.enabled_processor_count(), 0);
        assert_eq!(validate_xapic_topology(&info), Err(MadtError::InvalidTopologyCounts));

        info.processor_count = 0;
        info.io_apic_count = MAX_MADT_IOAPICS + 1;
        assert_eq!(validate_xapic_topology(&info), Err(MadtError::InvalidTopologyCounts));

        info.io_apic_count = 0;
        info.override_count = MAX_MADT_OVERRIDES + 1;
        assert_eq!(validate_xapic_topology(&info), Err(MadtError::InvalidTopologyCounts));
    }

    #[test]
    fn validates_topology_before_hardware_initialization() {
        let valid = base(0, &[
            0, 8, 0, 2, 1, 0, 0, 0,
            1, 12, 3, 0, 0, 0xc0, 0xfe, 0, 0, 0, 0, 0,
        ]);
        let parsed = parse_madt(&valid).unwrap();
        assert_eq!(validate_xapic_topology(&parsed), Ok(()));

        let duplicate_cpu = parse_madt(&base(0, &[
            0, 8, 0, 2, 1, 0, 0, 0,
            0, 8, 1, 2, 1, 0, 0, 0,
        ])).unwrap();
        assert_eq!(validate_xapic_topology(&duplicate_cpu), Err(MadtError::DuplicateApicId));

        let invalid_io = parse_madt(&base(0, &[
            0, 8, 0, 2, 1, 0, 0, 0,
            1, 12, 3, 0, 0x01, 0, 0, 0, 0, 0, 0, 0,
        ])).unwrap();
        assert_eq!(validate_xapic_topology(&invalid_io), Err(MadtError::InvalidIoApicAddress));
    }

    #[test]
    fn skips_well_formed_unknown_records() {
        let parsed = parse_madt(&base(0, &[0x7f, 4, 0xaa, 0xbb])).unwrap();
        assert_eq!(parsed.processor_count, 0);
    }
}
