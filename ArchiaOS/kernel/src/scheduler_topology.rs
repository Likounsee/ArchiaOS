//! Bounded, allocation-free CPU topology model for scheduler policy (P2).
//!
//! Discovery backends may populate this model from CPUID and ACPI. Missing
//! firmware/hardware details stay `None`; this module never infers topology
//! from a commercial CPU name and does not itself start or schedule CPUs.

pub const MAX_LOGICAL_CPUS: usize = 256;

/// One valid subleaf returned by CPUID leaf 0x1F or 0x0B.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuidTopologyLevel { pub shift: u8, pub logical_count: u16, pub level_type: u8, pub x2apic_id: u32 }

/// Decode topology levels without executing CPUID, allowing synthetic tests.
pub fn entry_from_cpuid_levels(levels: &[CpuidTopologyLevel]) -> Option<CpuTopologyEntry> {
    let first = levels.iter().find(|l| l.logical_count != 0 && l.level_type != 0)?;
    let apic_id = first.x2apic_id;
    if levels.iter().any(|l| l.logical_count != 0 && l.level_type != 0 && l.x2apic_id != apic_id) { return None; }
    let mut smt_shift = None;
    let mut core_shift = None;
    for level in levels.iter().filter(|l| l.logical_count != 0 && l.level_type != 0) {
        if level.shift > 31 || level.shift < smt_shift.unwrap_or(0) { return None; }
        match level.level_type { 1 => smt_shift = Some(level.shift), 2 => core_shift = Some(level.shift), _ => {} }
    }
    let (package_id, core_id) = if let Some(package_shift) = core_shift {
        let smt = smt_shift.unwrap_or(0);
        let width = package_shift.saturating_sub(smt);
        let mask = if width == 32 { u32::MAX } else { (1u32 << width) - 1 };
        (u16::try_from(apic_id >> package_shift).ok(), u16::try_from((apic_id >> smt) & mask).ok())
    } else { (None, None) };
    Some(CpuTopologyEntry { apic_id, package_id, core_id, cache_domain_id: None, numa_node: None, class: CpuClass::Unknown, capacity: None })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuClass {
    /// The hardware did not expose a reliable class.
    Unknown,
    /// Equivalent performance class, or no reliable heterogeneity signal.
    Uniform,
    Performance,
    Efficiency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuTopologyEntry {
    /// Local APIC/x2APIC identifier, not an array index.
    pub apic_id: u32,
    pub package_id: Option<u16>,
    pub core_id: Option<u16>,
    /// Identifier shared by logical CPUs that share a relevant cache domain.
    pub cache_domain_id: Option<u16>,
    pub numa_node: Option<u16>,
    pub class: CpuClass,
    /// Optional normalized capacity in 1..=255; None means unknown.
    pub capacity: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyError {
    Full,
    DuplicateApicId,
    InvalidCapacity,
    InvalidMadt,
}

/// Fixed-capacity topology snapshot. Entries are descriptive metadata only;
/// publishing a topology does not bring a CPU online or enable SMP.
pub struct CpuTopology {
    entries: [Option<CpuTopologyEntry>; MAX_LOGICAL_CPUS],
    len: usize,
}

impl CpuTopology {
    pub const fn new() -> Self {
        Self { entries: [None; MAX_LOGICAL_CPUS], len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get_by_apic_id(&self, apic_id: u32) -> Option<&CpuTopologyEntry> {
        self.entries[..self.len]
            .iter()
            .flatten()
            .find(|entry| entry.apic_id == apic_id)
    }

    pub fn insert(&mut self, entry: CpuTopologyEntry) -> Result<(), TopologyError> {
        if entry.capacity == Some(0) {
            return Err(TopologyError::InvalidCapacity);
        }
        if self.get_by_apic_id(entry.apic_id).is_some() {
            return Err(TopologyError::DuplicateApicId);
        }
        if self.len == MAX_LOGICAL_CPUS {
            return Err(TopologyError::Full);
        }
        self.entries[self.len] = Some(entry);
        self.len += 1;
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &CpuTopologyEntry> {
        self.entries[..self.len].iter().flatten()
    }
}

/// Convert MADT processor records into a scheduler topology snapshot.
/// Only enabled or online-capable processors are included; CPUID-derived
/// package/core fields are intentionally not copied to CPUs whose identity
/// has not been matched yet. Input records are validated before publication.
pub fn from_madt_processors(
    processors: &[Option<super::acpi_madt::Processor>],
) -> Result<CpuTopology, TopologyError> {
    if processors.len() > MAX_LOGICAL_CPUS { return Err(TopologyError::Full); }
    let mut topology = CpuTopology::new();
    for record in processors {
        let Some(cpu) = record else { return Err(TopologyError::InvalidMadt); };
        if !(cpu.enabled || cpu.online_capable) { continue; }
        topology.insert(CpuTopologyEntry {
            apic_id: cpu.apic_id, package_id: None, core_id: None,
            cache_domain_id: None, numa_node: None, class: CpuClass::Unknown, capacity: None,
        })?;
    }
    Ok(topology)
}

impl Default for CpuTopology {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(apic_id: u32) -> CpuTopologyEntry {
        CpuTopologyEntry {
            apic_id,
            package_id: None,
            core_id: None,
            cache_domain_id: None,
            numa_node: None,
            class: CpuClass::Unknown,
            capacity: None,
        }
    }

    #[test]
    fn cpuid_levels_decode_smt_core_and_package_ids() {
        let levels = [
            CpuidTopologyLevel { shift: 1, logical_count: 2, level_type: 1, x2apic_id: 0x2d },
            CpuidTopologyLevel { shift: 4, logical_count: 16, level_type: 2, x2apic_id: 0x2d },
        ];
        let cpu = entry_from_cpuid_levels(&levels).unwrap();
        assert_eq!(cpu.apic_id, 0x2d);
        assert_eq!(cpu.package_id, Some(2));
        assert_eq!(cpu.core_id, Some(6));
        assert_eq!(cpu.class, CpuClass::Unknown);
        assert_eq!(cpu.cache_domain_id, None);
    }

    #[test]
    fn cpuid_levels_reject_inconsistent_ids_and_empty_levels() {
        assert_eq!(entry_from_cpuid_levels(&[]), None);
        let levels = [
            CpuidTopologyLevel { shift: 1, logical_count: 2, level_type: 1, x2apic_id: 1 },
            CpuidTopologyLevel { shift: 4, logical_count: 8, level_type: 2, x2apic_id: 2 },
        ];
        assert_eq!(entry_from_cpuid_levels(&levels), None);
    }

    #[test]
    fn topology_preserves_unknown_fields_instead_of_guessing() {
        let mut topology = CpuTopology::new();
        let cpu = entry(0x1234);
        assert_eq!(topology.insert(cpu), Ok(()));
        assert_eq!(topology.len(), 1);
        assert_eq!(topology.get_by_apic_id(0x1234), Some(&cpu));
        assert_eq!(topology.get_by_apic_id(0x1235), None);
        assert_eq!(topology.iter().count(), 1);
    }

    #[test]
    fn topology_rejects_duplicate_ids_and_zero_capacity_without_mutation() {
        let mut topology = CpuTopology::new();
        let cpu = entry(7);
        assert_eq!(topology.insert(cpu), Ok(()));
        assert_eq!(topology.insert(cpu), Err(TopologyError::DuplicateApicId));

        let mut invalid = entry(8);
        invalid.capacity = Some(0);
        assert_eq!(topology.insert(invalid), Err(TopologyError::InvalidCapacity));
        assert_eq!(topology.len(), 1);
        assert_eq!(topology.get_by_apic_id(8), None);
    }

    #[test]
    fn topology_accepts_homogeneous_and_heterogeneous_metadata() {
        let mut topology = CpuTopology::new();
        let mut performance = entry(1);
        performance.class = CpuClass::Performance;
        performance.capacity = Some(240);
        performance.package_id = Some(0);
        performance.core_id = Some(2);
        performance.cache_domain_id = Some(1);
        assert_eq!(topology.insert(performance), Ok(()));

        let mut efficiency = entry(9);
        efficiency.class = CpuClass::Efficiency;
        efficiency.capacity = Some(120);
        efficiency.package_id = Some(0);
        efficiency.core_id = Some(6);
        efficiency.cache_domain_id = Some(3);
        efficiency.numa_node = Some(0);
        assert_eq!(topology.insert(efficiency), Ok(()));

        assert_eq!(topology.len(), 2);
        assert_eq!(topology.get_by_apic_id(1).unwrap().class, CpuClass::Performance);
        assert_eq!(topology.get_by_apic_id(9).unwrap().class, CpuClass::Efficiency);
    }

    #[test]
    fn madt_conversion_keeps_only_enabled_processors_and_unknown_metadata() {
        use super::super::acpi_madt::Processor;
        let records = [
            Some(Processor { acpi_uid: 0, apic_id: 0x1234, enabled: true, online_capable: false }),
            Some(Processor { acpi_uid: 1, apic_id: 0x1235, enabled: false, online_capable: false }),
            Some(Processor { acpi_uid: 2, apic_id: 0x1236, enabled: false, online_capable: true }),
        ];
        let topology = from_madt_processors(&records).unwrap();
        assert_eq!(topology.len(), 2);
        assert!(topology.get_by_apic_id(0x1234).is_some());
        assert!(topology.get_by_apic_id(0x1235).is_none());
        let online = topology.get_by_apic_id(0x1236).unwrap();
        assert_eq!(online.package_id, None);
        assert_eq!(online.class, CpuClass::Unknown);
    }

    #[test]
    fn madt_conversion_rejects_duplicate_ids_and_holes() {
        use super::super::acpi_madt::Processor;
        let duplicate = [
            Some(Processor { acpi_uid: 0, apic_id: 7, enabled: true, online_capable: false }),
            Some(Processor { acpi_uid: 1, apic_id: 7, enabled: true, online_capable: false }),
        ];
        assert_eq!(from_madt_processors(&duplicate).err(), Some(TopologyError::DuplicateApicId));
        let hole: [Option<Processor>; 1] = [None];
        assert_eq!(from_madt_processors(&hole).err(), Some(TopologyError::InvalidMadt));
    }

    #[test]
    fn topology_enforces_fixed_capacity() {
        let mut topology = CpuTopology::new();
        for id in 0..MAX_LOGICAL_CPUS {
            assert_eq!(topology.insert(entry(id as u32)), Ok(()));
        }
        assert_eq!(topology.len(), MAX_LOGICAL_CPUS);
        assert_eq!(topology.insert(entry(999)), Err(TopologyError::Full));
        assert_eq!(topology.len(), MAX_LOGICAL_CPUS);
    }
}
