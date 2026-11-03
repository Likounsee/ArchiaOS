//! Pure validation for the early x86-64 GOP text console.
//! Keep address/stride arithmetic testable without touching physical memory.

/// Highest physical address accepted by the current x86-64 bootstrap mapping.
pub const BOOTSTRAP_PHYSICAL_LIMIT: u64 = 1u64 << 39;

/// Validate a directly writable RGB/BGR GOP framebuffer and return its pixel count.
///
/// GOP bitmask modes are deliberately rejected: BootInfo v2 carries the format
/// number but not the channel masks required to encode colors correctly.
pub fn validated_pixel_count(
    base: u64,
    size: u64,
    width: u32,
    height: u32,
    stride: u32,
    pixel_format: u32,
) -> Option<usize> {
    if base == 0 || base % 4 != 0 || width == 0 || height == 0
        || stride < width || pixel_format > 1
    {
        return None;
    }

    let pixels = u64::from(stride).checked_mul(u64::from(height))?;
    let bytes = pixels.checked_mul(4)?;
    if bytes == 0 || bytes > size {
        return None;
    }
    let end = base.checked_add(bytes)?;
    if end > BOOTSTRAP_PHYSICAL_LIMIT {
        return None;
    }

    usize::try_from(pixels).ok()
}

#[cfg(test)]
mod tests {
    use super::{validated_pixel_count, BOOTSTRAP_PHYSICAL_LIMIT};

    #[test]
    fn accepts_rgb_and_bgr_with_padded_stride() {
        assert_eq!(validated_pixel_count(0xE000_0000, 800 * 480 * 4, 640, 480, 800, 0), Some(384_000));
        assert_eq!(validated_pixel_count(0xE000_0000, 800 * 480 * 4, 640, 480, 800, 1), Some(384_000));
    }

    #[test]
    fn rejects_unsupported_or_malformed_modes() {
        assert_eq!(validated_pixel_count(0xE000_0000, 640 * 480 * 4, 640, 480, 640, 2), None);
        assert_eq!(validated_pixel_count(0xE000_0000, 640 * 480 * 4, 640, 480, 640, 3), None);
        assert_eq!(validated_pixel_count(0, 640 * 480 * 4, 640, 480, 640, 0), None);
        assert_eq!(validated_pixel_count(0xE000_0001, 640 * 480 * 4, 640, 480, 640, 0), None);
        assert_eq!(validated_pixel_count(0xE000_0000, 640 * 480 * 4 - 1, 640, 480, 640, 0), None);
        assert_eq!(validated_pixel_count(0xE000_0000, 640 * 480 * 4, 640, 480, 639, 0), None);
        assert_eq!(validated_pixel_count(0xE000_0000, 640 * 480 * 4, 0, 480, 640, 0), None);
    }

    #[test]
    fn rejects_wrapping_or_unmapped_physical_ranges() {
        assert_eq!(validated_pixel_count(u64::MAX - 3, 4096, 2, 1, 2, 0), None);
        assert_eq!(validated_pixel_count(BOOTSTRAP_PHYSICAL_LIMIT - 4, 4096, 2, 1, 2, 0), None);
    }
}
