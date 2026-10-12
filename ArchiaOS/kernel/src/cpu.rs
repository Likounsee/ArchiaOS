//! Read-only x86-64 CPU identification for ArchiaOS hardware telemetry.
//!
//! CPUID reports processor identification and advertised instruction features.
//! It does not probe temperatures, clocks, power, or other live sensor values.

use core::arch::x86_64::__cpuid_count;

/// Fixed-size CPU information: usable before a heap allocator exists.
pub struct CpuInfo {
    pub vendor: [u8; 12],
    pub brand: [u8; 48],
    pub family: u16,
    pub model: u16,
    pub stepping: u8,
    pub has_tsc: bool,
    pub has_sse2: bool,
    pub has_avx: bool,
    pub has_avx2: bool,
    pub has_aesni: bool,
    pub has_xsave: bool,
    pub max_basic_leaf: u32,
    pub max_extended_leaf: u32,
}

/// Query CPUID only for leaves advertised by the processor.
pub fn detect() -> CpuInfo {
    // SAFETY: CPUID is available on x86-64. Leaf 0 is the architectural
    // discovery leaf and is safe to query before checking other leaf numbers.
    let basic = unsafe { __cpuid_count(0, 0) };
    let max_basic_leaf = basic.eax;

    let mut vendor = [0u8; 12];
    vendor[0..4].copy_from_slice(&basic.ebx.to_le_bytes());
    vendor[4..8].copy_from_slice(&basic.edx.to_le_bytes());
    vendor[8..12].copy_from_slice(&basic.ecx.to_le_bytes());

    // SAFETY: Extended leaf 0x80000000 is the discovery leaf for that range.
    let extended = unsafe { __cpuid_count(0x8000_0000, 0) };
    let max_extended_leaf = extended.eax;

    let mut brand = [0u8; 48];
    if max_extended_leaf >= 0x8000_0004 {
        for (index, leaf) in (0x8000_0002u32..=0x8000_0004).enumerate() {
            // SAFETY: the maximum extended leaf confirms this leaf exists.
            let registers = unsafe { __cpuid_count(leaf, 0) };
            let offset = index * 16;
            brand[offset..offset + 4].copy_from_slice(&registers.eax.to_le_bytes());
            brand[offset + 4..offset + 8].copy_from_slice(&registers.ebx.to_le_bytes());
            brand[offset + 8..offset + 12].copy_from_slice(&registers.ecx.to_le_bytes());
            brand[offset + 12..offset + 16].copy_from_slice(&registers.edx.to_le_bytes());
        }
    }

    let mut family = 0u16;
    let mut model = 0u16;
    let mut stepping = 0u8;
    let mut has_tsc = false;
    let mut has_sse2 = false;
    let mut has_avx = false;
    let mut has_aesni = false;
    let mut has_xsave = false;
    let mut has_avx2 = false;

    if max_basic_leaf >= 1 {
        // SAFETY: leaf 1 is available according to leaf 0.
        let leaf1 = unsafe { __cpuid_count(1, 0) };
        let base_family = ((leaf1.eax >> 8) & 0x0f) as u16;
        let base_model = ((leaf1.eax >> 4) & 0x0f) as u16;
        let ext_family = ((leaf1.eax >> 20) & 0xff) as u16;
        let ext_model = ((leaf1.eax >> 16) & 0x0f) as u16;
        family = if base_family == 0x0f { base_family + ext_family } else { base_family };
        model = if base_family == 0x06 || base_family == 0x0f {
            (ext_model << 4) | base_model
        } else {
            base_model
        };
        stepping = (leaf1.eax & 0x0f) as u8;
        has_tsc = leaf1.edx & (1 << 4) != 0;
        has_sse2 = leaf1.edx & (1 << 26) != 0;
        has_aesni = leaf1.ecx & (1 << 25) != 0;
        has_xsave = leaf1.ecx & (1 << 26) != 0;
        has_avx = leaf1.ecx & (1 << 28) != 0;
    }

    if max_basic_leaf >= 7 {
        // SAFETY: leaf 7 is available according to leaf 0.
        let leaf7 = unsafe { __cpuid_count(7, 0) };
        has_avx2 = leaf7.ebx & (1 << 5) != 0;
    }

    CpuInfo {
        vendor,
        brand,
        family,
        model,
        stepping,
        has_tsc,
        has_sse2,
        has_avx,
        has_avx2,
        has_aesni,
        has_xsave,
        max_basic_leaf,
        max_extended_leaf,
    }
}

/// Convert a CPUID-provided fixed-width ASCII field to a string slice.
pub fn printable_field(field: &[u8]) -> &str {
    let end = field.iter().rposition(|byte| *byte != 0 && *byte != b' ').map_or(0, |i| i + 1);
    // CPUID vendor and brand strings are defined as ASCII bytes.
    core::str::from_utf8(&field[..end]).unwrap_or("unknown")
}
