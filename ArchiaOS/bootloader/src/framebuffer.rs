pub mod framebuffer {
    /// Return whether firmware metadata describes a bounded linear framebuffer.
    /// GOP PixelBltOnly (format 3) does not provide a directly addressable pixel buffer.
    pub fn is_usable(
        base: u64,
        size: u64,
        horizontal_resolution: u32,
        vertical_resolution: u32,
        pixel_format: u32,
        pixels_per_scan_line: u32,
    ) -> bool {
        if base == 0 || size == 0 || horizontal_resolution == 0 || vertical_resolution == 0
            || pixels_per_scan_line < horizontal_resolution || pixel_format > 2
        {
            return false;
        }
        let Some(end) = base.checked_add(size) else {
            return false;
        };
        let Some(scanline_bytes) = (pixels_per_scan_line as u64).checked_mul(4) else {
            return false;
        };
        let Some(required_bytes) = scanline_bytes.checked_mul(vertical_resolution as u64) else {
            return false;
        };
        end > base && size >= required_bytes
    }

    #[cfg(test)]
    mod tests {
        use super::is_usable;

        #[test]
        fn accepts_complete_rgb_bgr_and_bitmask_framebuffers() {
            for format in 0..=2 {
                assert!(is_usable(0xE000_0000, 640 * 480 * 4, 640, 480, format, 640));
                // Scan-line padding is part of the physical framebuffer size.
                assert!(is_usable(0xE000_0000, 800 * 480 * 4, 640, 480, format, 800));
            }
        }

        #[test]
        fn rejects_short_or_malformed_framebuffer_metadata() {
            assert!(!is_usable(0xE000_0000, 640 * 480 * 4 - 1, 640, 480, 0, 640));
            assert!(!is_usable(0xE000_0000, 640 * 480 * 4, 640, 480, 0, 639));
            assert!(!is_usable(0xE000_0000, 640 * 480 * 4, 640, 480, 3, 640));
            assert!(!is_usable(0, 640 * 480 * 4, 640, 480, 0, 640));
            assert!(!is_usable(0xE000_0000, 0, 640, 480, 0, 640));
            assert!(!is_usable(0xE000_0000, 640 * 480 * 4, 0, 480, 0, 640));
            assert!(!is_usable(0xE000_0000, 640 * 480 * 4, 640, 0, 0, 640));
            assert!(!is_usable(u64::MAX - 0x1000, 0x2000, 1, 1, 0, 1));
        }
    }
}
