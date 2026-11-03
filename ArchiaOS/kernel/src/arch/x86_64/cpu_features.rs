//! Pure CPUID feature decoding and OS-state gating.
//!
//! Keep these helpers independent of privileged instructions so synthetic CPU
//! profiles can be tested on the host and in CI without depending on the runner's
//! physical processor. This module must never execute an optional ISA itself.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Leaf1Features {
    pub tsc: bool,
    pub sse2: bool,
    pub sse41: bool,
    pub sse42: bool,
    pub aesni: bool,
    pub xsave: bool,
    pub osxsave: bool,
    pub avx: bool,
    pub fma: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Leaf7Features {
    pub bmi1: bool,
    pub avx2: bool,
    pub bmi2: bool,
    pub avx512f: bool,
}

/// Decode CPUID leaf 1 ECX/EDX feature bits without executing those features.
pub const fn decode_leaf1(ecx: u32, edx: u32) -> Leaf1Features {
    Leaf1Features {
        tsc: edx & (1 << 4) != 0,
        sse2: edx & (1 << 26) != 0,
        sse41: ecx & (1 << 19) != 0,
        sse42: ecx & (1 << 20) != 0,
        aesni: ecx & (1 << 25) != 0,
        xsave: ecx & (1 << 26) != 0,
        osxsave: ecx & (1 << 27) != 0,
        avx: ecx & (1 << 28) != 0,
        fma: ecx & (1 << 12) != 0,
    }
}

/// Decode CPUID leaf 7 subleaf 0 EBX feature bits.
pub const fn decode_leaf7(ebx: u32) -> Leaf7Features {
    Leaf7Features {
        bmi1: ebx & (1 << 3) != 0,
        avx2: ebx & (1 << 5) != 0,
        bmi2: ebx & (1 << 8) != 0,
        avx512f: ebx & (1 << 16) != 0,
    }
}

/// AVX, AVX2 and FMA need CPU support plus OSXSAVE and XMM/YMM state in XCR0.
pub const fn avx_state_usable(xsave: bool, osxsave: bool, avx: bool, xcr0: u64) -> bool {
    xsave && osxsave && avx && (xcr0 & 0x6 == 0x6)
}

/// AVX-512 additionally requires opmask, ZMM-hi256 and hi16-ZMM state.
pub const fn avx512_state_usable(
    xsave: bool,
    osxsave: bool,
    avx: bool,
    avx512f: bool,
    xcr0: u64,
) -> bool {
    avx_state_usable(xsave, osxsave, avx, xcr0)
        && avx512f
        && (xcr0 & 0xE6 == 0xE6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_without_sse41_is_rejected_by_the_baseline_contract() {
        // A conservative older x86-64 profile: SSE2 exists, SSE4.1 does not.
        let profile = decode_leaf1(0, 1 << 26);
        assert!(profile.sse2);
        assert!(!profile.sse41);
    }

    #[test]
    fn sse41_only_profile_does_not_require_avx_or_xsave() {
        let profile = decode_leaf1(1 << 19, 1 << 26);
        assert!(profile.sse41);
        assert!(!profile.avx);
        assert!(!profile.xsave);
        assert!(!avx_state_usable(profile.xsave, profile.osxsave, profile.avx, 0));
    }

    #[test]
    fn avx_profile_requires_all_cpu_os_and_xcr0_prerequisites() {
        let profile = decode_leaf1((1 << 19) | (1 << 26) | (1 << 27) | (1 << 28), 1 << 26);
        assert!(profile.sse41);
        assert!(avx_state_usable(profile.xsave, profile.osxsave, profile.avx, 0x6));
        assert!(!avx_state_usable(profile.xsave, false, profile.avx, 0x6));
        assert!(!avx_state_usable(profile.xsave, profile.osxsave, profile.avx, 0x2));
    }

    #[test]
    fn avx512_requires_the_complete_extended_state_mask() {
        assert!(avx512_state_usable(true, true, true, true, 0xE6));
        assert!(!avx512_state_usable(true, true, true, true, 0x6));
        assert!(!avx512_state_usable(true, true, true, false, 0xE6));
    }

    #[test]
    fn leaf7_features_are_independent() {
        let profile = decode_leaf7((1 << 3) | (1 << 5) | (1 << 8));
        assert!(profile.bmi1 && profile.avx2 && profile.bmi2);
        assert!(!profile.avx512f);
    }
}
