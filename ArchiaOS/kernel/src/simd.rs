//! Runtime-dispatched x86-64 integer kernels.
//!
//! This module keeps the baseline implementation free of optional ISA
//! assumptions. AVX-family kernels are intentionally absent until XSAVE/XRSTOR
//! state is part of every task context.

/// Sum u32 values with wrapping u64 semantics.
///
/// The sse41_supported flag must come from the CPUID result for the current CPU.
/// The kernel baseline contract already requires SSE4.1, but retaining a scalar
/// fallback makes this helper safe to reuse in synthetic tests and early boot.
pub(crate) fn sum_u32(values: &[u32], sse41_supported: bool) -> u64 {
    if sse41_supported {
        // SAFETY: callers pass the decoded CPUID SSE4.1 bit. This kernel's
        // baseline gate independently refuses CPUs without SSE4.1.
        unsafe { sum_u32_sse41(values) }
    } else {
        sum_u32_scalar(values)
    }
}

fn sum_u32_scalar(values: &[u32]) -> u64 {
    values.iter().fold(0u64, |sum, value| sum.wrapping_add(*value as u64))
}

#[target_feature(enable = "sse4.1")]
unsafe fn sum_u32_sse41(values: &[u32]) -> u64 {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi64, _mm_cvtepu32_epi64, _mm_loadl_epi64,
        _mm_setzero_si128, _mm_storeu_si128,
    };

    let mut lanes = _mm_setzero_si128();
    let mut index = 0usize;
    while index + 1 < values.len() {
        // SAFETY: at least two u32 elements remain; loadl reads exactly 64 bits.
        let pair = unsafe { _mm_loadl_epi64(values.as_ptr().add(index).cast::<__m128i>()) };
        let widened = _mm_cvtepu32_epi64(pair);
        lanes = _mm_add_epi64(lanes, widened);
        index += 2;
    }

    let mut partial = [0u64; 2];
    // SAFETY: partial is a valid 16-byte destination; unaligned store is allowed.
    unsafe { _mm_storeu_si128(partial.as_mut_ptr().cast::<__m128i>(), lanes) };
    let mut sum = partial[0].wrapping_add(partial[1]);
    if index < values.len() {
        sum = sum.wrapping_add(values[index] as u64);
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_fallback_handles_empty_odd_and_even_lengths() {
        assert_eq!(sum_u32(&[], false), 0);
        assert_eq!(sum_u32(&[7], false), 7);
        assert_eq!(sum_u32(&[1, 2, 3, 4], false), 10);
        assert_eq!(sum_u32(&[u32::MAX, 1], false), u32::MAX as u64 + 1);
    }

    #[test]
    fn sse41_path_matches_scalar_for_varied_lengths() {
        for length in 0..33 {
            let values: [u32; 33] = core::array::from_fn(|i| {
                (i as u32).wrapping_mul(0x1020_3041).rotate_left((i % 31) as u32)
            });
            let input = &values[..length];
            assert_eq!(sum_u32(input, true), sum_u32(input, false), "length={length}");
        }
    }
}
