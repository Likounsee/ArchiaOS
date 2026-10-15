//! Bounded, allocation-free CPU topology model for scheduler policy (P2).
//!
//! Discovery backends may populate this model from CPUID and ACPI. Missing
//! firmware/hardware details stay `None`; this module never infers topology
//! from a commercial CPU name and does not itself start or schedule CPUs.

pub const MAX_LOGICAL_CPUS: usize = 256;

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
