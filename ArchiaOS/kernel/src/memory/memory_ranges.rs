//! Pure physical-frame range arithmetic shared by the PMM and host tests.
//! No bootloader or hardware dependencies: edge cases can be tested on the host.

pub const PAGE_SIZE: u64 = 4096;
/// PMM tracking ceiling (512 GiB, one low-canonical PML4 span on 4-level x86-64).
/// This does not imply that every CPU or platform supports every physical address.
pub const MAX_PHYSICAL_ADDRESS: u64 = 1u64 << 39;
pub const MAX_FRAMES: u64 = MAX_PHYSICAL_ADDRESS / PAGE_SIZE;

/// Return complete pages fully contained in a byte range, clipped to the PMM.
pub fn frame_range(base: u64, length: u64) -> Option<(u64, u64)> {
    let end = base.checked_add(length)?;
    let mut first = base / PAGE_SIZE;
    if base % PAGE_SIZE != 0 { first = first.checked_add(1)?; }
    let first = first.min(MAX_FRAMES);
    let last = (end / PAGE_SIZE).min(MAX_FRAMES);
    Some((first.min(last), last))
}

/// Return all PMM frames touched by a reserved range, rounding outward.
pub fn reserved_frame_range(base: u64, length: u64) -> Option<(u64, u64)> {
    if length == 0 { return Some((0, 0)); }
    let end = base.checked_add(length)?;
    let first = (base / PAGE_SIZE).min(MAX_FRAMES);
    // Clip before rounding to avoid overflow for out-of-window MMIO near u64::MAX.
    if first == MAX_FRAMES { return Some((MAX_FRAMES, MAX_FRAMES)); }
    let clipped_end = end.min(MAX_PHYSICAL_ADDRESS);
    let rounded_end = clipped_end.checked_add(PAGE_SIZE - 1)? / PAGE_SIZE;
    Some((first, rounded_end.min(MAX_FRAMES)))
}

/// Number of PMM frames shared by two half-open frame intervals.
pub fn overlapping_frames(first: u64, last: u64, reserved_first: u64, reserved_last: u64) -> u64 {
    last.min(reserved_last).saturating_sub(first.max(reserved_first))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_page_ranges_handle_alignment_empty_ranges_and_ceiling() {
        assert_eq!(frame_range(0x1000, 0x1000), Some((1, 2)));
        assert_eq!(frame_range(0x1001, 0x0fff), Some((2, 2)));
        assert_eq!(frame_range(0x1000, 0x1001), Some((1, 2)));
        assert_eq!(frame_range(0, 0), Some((0, 0)));
        assert_eq!(frame_range(MAX_PHYSICAL_ADDRESS, PAGE_SIZE), Some((MAX_FRAMES, MAX_FRAMES)));
        assert_eq!(frame_range(u64::MAX - 1, 8), None);
    }

    #[test]
    fn addresses_above_previous_four_and_sixty_four_gib_limits_are_managed() {
        for address in [1u64 << 32, 1u64 << 36, (1u64 << 39) - PAGE_SIZE] {
            assert_eq!(frame_range(address, PAGE_SIZE), Some((address / PAGE_SIZE, address / PAGE_SIZE + 1)));
            assert_eq!(reserved_frame_range(address, PAGE_SIZE), Some((address / PAGE_SIZE, address / PAGE_SIZE + 1)));
        }
    }

    #[test]
    fn reserved_ranges_round_outward_and_clip_at_five_hundred_twelve_gib() {
        assert_eq!(reserved_frame_range(0x1001, 1), Some((1, 2)));
        assert_eq!(reserved_frame_range(0x1fff, 2), Some((1, 3)));
        assert_eq!(reserved_frame_range((1u64 << 36) - PAGE_SIZE, PAGE_SIZE * 2), Some(((1u64 << 36) / PAGE_SIZE - 1, (1u64 << 36) / PAGE_SIZE + 1)));
        assert_eq!(reserved_frame_range(MAX_PHYSICAL_ADDRESS - PAGE_SIZE, PAGE_SIZE * 2), Some((MAX_FRAMES - 1, MAX_FRAMES)));
        assert_eq!(reserved_frame_range(MAX_PHYSICAL_ADDRESS, PAGE_SIZE), Some((MAX_FRAMES, MAX_FRAMES)));
        assert_eq!(reserved_frame_range(u64::MAX - 1, 1), Some((MAX_FRAMES, MAX_FRAMES)));
        assert_eq!(reserved_frame_range(0, 0), Some((0, 0)));
    }

    #[test]
    fn interval_overlap_is_half_open_and_never_underflows() {
        assert_eq!(overlapping_frames(10, 20, 15, 25), 5);
        assert_eq!(overlapping_frames(10, 20, 0, 10), 0);
        assert_eq!(overlapping_frames(10, 20, 30, 40), 0);
        assert_eq!(overlapping_frames(10, 20, 0, 30), 10);
        assert_eq!(overlapping_frames(10, 10, 0, 30), 0);
    }
}
