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
    pub has_sse41: bool,
    pub has_sse42: bool,
    /// Hardware advertises AVX, regardless of whether OS state is enabled.
    pub has_avx: bool,
    pub has_avx2: bool,
    pub has_avx512f: bool,
    pub has_fma: bool,
    pub fma_usable: bool,
    pub has_bmi1: bool,
    pub has_bmi2: bool,
    pub has_aesni: bool,
    pub has_xsave: bool,
    pub has_osxsave: bool,
    /// AVX state is enabled by the OS in CR4/XCR0; safe to execute AVX now.
    pub avx_state_usable: bool,
    pub avx2_usable: bool,
    pub avx512f_usable: bool,
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
    let mut has_sse41 = false;
    let mut has_sse42 = false;
    let mut has_avx = false;
    let mut has_avx2 = false;
    let mut has_avx512f = false;
    let mut has_fma = false;
    let mut has_bmi1 = false;
    let mut has_bmi2 = false;
    let mut has_aesni = false;
    let mut has_xsave = false;
    let mut has_osxsave = false;

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
        has_sse41 = leaf1.ecx & (1 << 19) != 0;
        has_sse42 = leaf1.ecx & (1 << 20) != 0;
        has_aesni = leaf1.ecx & (1 << 25) != 0;
        has_xsave = leaf1.ecx & (1 << 26) != 0;
        has_osxsave = leaf1.ecx & (1 << 27) != 0;
        has_avx = leaf1.ecx & (1 << 28) != 0;
        has_fma = leaf1.ecx & (1 << 12) != 0;
    }

    if max_basic_leaf >= 7 {
        // SAFETY: leaf 7 is available according to leaf 0.
        let leaf7 = unsafe { __cpuid_count(7, 0) };
        has_bmi1 = leaf7.ebx & (1 << 3) != 0;
        has_avx2 = leaf7.ebx & (1 << 5) != 0;
        has_bmi2 = leaf7.ebx & (1 << 8) != 0;
        has_avx512f = leaf7.ebx & (1 << 16) != 0;
    }

    // AVX-family instructions require both CPU support and OS-enabled state.
    // XGETBV is legal only after CPUID reports OSXSAVE (CR4.OSXSAVE is set).
    let avx_state_usable = if has_xsave && has_osxsave && has_avx {
        let low: u32;
        let high: u32;
        // SAFETY: CPUID.OSXSAVE confirms the OS enabled XGETBV access.
        unsafe {
            core::arch::asm!(
                "xgetbv",
                in("ecx") 0u32,
                out("eax") low,
                out("edx") high,
                options(nomem, nostack, preserves_flags)
            );
        }
        let xcr0 = ((high as u64) << 32) | low as u64;
        xcr0 & 0b110 == 0b110
    } else {
        false
    };
    let avx2_usable = has_avx2 && avx_state_usable;
    let fma_usable = has_fma && avx_state_usable;
    let avx512f_usable = has_avx512f && avx_state_usable && {
        let low: u32;
        let high: u32;
        // SAFETY: same OSXSAVE guard as the XGETBV above.
        unsafe {
            core::arch::asm!(
                "xgetbv",
                in("ecx") 0u32,
                out("eax") low,
                out("edx") high,
                options(nomem, nostack, preserves_flags)
            );
        }
        let xcr0 = ((high as u64) << 32) | low as u64;
        xcr0 & 0xE6 == 0xE6
    };

    CpuInfo {
        vendor,
        brand,
        family,
        model,
        stepping,
        has_tsc,
        has_sse2,
        has_sse41,
        has_sse42,
        has_avx,
        has_avx2,
        has_avx512f,
        has_fma,
        fma_usable,
        has_bmi1,
        has_bmi2,
        has_aesni,
        has_xsave,
        has_osxsave,
        avx_state_usable,
        avx2_usable,
        avx512f_usable,
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
