//! Pure helpers for classifying remapped legacy 8259 PIC vectors.
//!
//! Kept independent of privileged instructions so the range logic can be
//! tested on the host as part of CI.

/// First vector used by the remapped master PIC.
pub const LEGACY_PIC_VECTOR_BASE: u8 = 0x20;
/// Number of IRQ lines exposed by the master and slave PICs together.
pub const LEGACY_PIC_IRQ_COUNT: u8 = 16;

/// High-level classification for a normalized x86-64 interrupt vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptVectorClass {
    /// CPU-defined exception range (0..=31).
    CpuException,
    /// Legacy 8259 PIC interrupt, carrying its IRQ line.
    LegacyPicIrq(u8),
    /// Any vector not owned by the CPU exception range or legacy PIC.
    Other,
}

/// Classify a vector without hardware access so dispatch decisions are testable.
pub const fn classify_interrupt_vector(vector: u8) -> InterruptVectorClass {
    if vector < LEGACY_PIC_VECTOR_BASE {
        InterruptVectorClass::CpuException
    } else if let Some(irq) = legacy_pic_irq_for_vector(vector) {
        InterruptVectorClass::LegacyPicIrq(irq)
    } else {
        InterruptVectorClass::Other
    }
}

/// Convert a remapped PIC vector (0x20..=0x2f) into an IRQ line (0..=15).
pub const fn legacy_pic_irq_for_vector(vector: u8) -> Option<u8> {
    match vector.checked_sub(LEGACY_PIC_VECTOR_BASE) {
        Some(irq) if irq < LEGACY_PIC_IRQ_COUNT => Some(irq),
        _ => None,
    }
}

/// Convert a PIC IRQ line (0..=15) into its remapped IDT vector.
pub const fn legacy_pic_vector_for_irq(irq: u8) -> Option<u8> {
    if irq < LEGACY_PIC_IRQ_COUNT {
        Some(LEGACY_PIC_VECTOR_BASE + irq)
    } else {
        None
    }
}


/// Validate the relationship between the PIC masks.
 ///
 /// If any slave IRQ is unmasked, master IRQ2 (the cascade) must also be
 /// unmasked or those slave interrupts cannot reach the CPU.
pub const fn pic_masks_are_consistent(master_mask: u8, slave_mask: u8) -> bool {
    slave_mask == 0xff || (master_mask & (1u8 << 2)) == 0
}

/// Return an EOI plan only after a handler has explicitly claimed the IRQ.
///
/// Unhandled IRQs must not be acknowledged as if service succeeded; callers
/// should route them to the fatal/unhandled path instead.
pub const fn eoi_plan_for_handled_irq(
    irq: u8,
    handler_handled: bool,
) -> Option<(bool, bool)> {
    if !handler_handled {
        None
    } else {
        eoi_plan_for_irq(irq)
    }
}

/// Describe the required legacy PIC EOI writes for an IRQ.
///
/// The tuple is (send slave EOI, send master EOI). Invalid IRQ lines have no
/// acknowledgement plan. The hardware dispatcher must perform slave first.
pub const fn eoi_plan_for_irq(irq: u8) -> Option<(bool, bool)> {
    if irq >= LEGACY_PIC_IRQ_COUNT {
        None
    } else if irq >= 8 {
        Some((true, true))
    } else {
        Some((false, true))
    }
}

/// Compute PIC masks after masking one IRQ, without touching hardware.
///
/// Returns `None` for an invalid IRQ. The cascade line (IRQ2) is masked when
/// masking the last enabled slave IRQ leaves the entire slave PIC masked.
pub const fn masks_after_masking_irq(
    master_mask: u8,
    slave_mask: u8,
    irq: u8,
) -> Option<(u8, u8)> {
    if irq >= LEGACY_PIC_IRQ_COUNT {
        return None;
    }

    // IRQ2 is the only path to the slave PIC. Refuse to mask it while
    // any slave line remains unmasked, otherwise IRQ8..=IRQ15 become unreachable.
    if irq == 2 && slave_mask != 0xff {
        return None;
    }

    if irq < 8 {
        Some((master_mask | (1u8 << irq), slave_mask))
    } else {
        let next_slave = slave_mask | (1u8 << (irq - 8));
        let next_master = if next_slave == 0xff {
            master_mask | (1u8 << 2)
        } else {
            master_mask
        };
        Some((next_master, next_slave))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_all_256_vectors_without_overlap() {
        for vector in 0u8..=u8::MAX {
            let class = classify_interrupt_vector(vector);
            match vector {
                0..=31 => assert_eq!(class, InterruptVectorClass::CpuException),
                32..=47 => assert_eq!(class, InterruptVectorClass::LegacyPicIrq(vector - 32)),
                _ => assert_eq!(class, InterruptVectorClass::Other),
            }
        }
    }

    #[test]
    fn classifies_pic_range_boundaries() {
        assert_eq!(classify_interrupt_vector(0x1f), InterruptVectorClass::CpuException);
        assert_eq!(classify_interrupt_vector(0x20), InterruptVectorClass::LegacyPicIrq(0));
        assert_eq!(classify_interrupt_vector(0x2f), InterruptVectorClass::LegacyPicIrq(15));
        assert_eq!(classify_interrupt_vector(0x30), InterruptVectorClass::Other);
        assert_eq!(classify_interrupt_vector(0xff), InterruptVectorClass::Other);
    }

    #[test]
    fn maps_every_legacy_irq_to_its_vector_and_back() {
        for irq in 0..LEGACY_PIC_IRQ_COUNT {
            let vector = legacy_pic_vector_for_irq(irq).expect("valid IRQ must map");
            assert_eq!(vector, LEGACY_PIC_VECTOR_BASE + irq);
            assert_eq!(legacy_pic_irq_for_vector(vector), Some(irq));
        }
    }

    #[test]
    fn rejects_vectors_outside_legacy_pic_range() {
        for vector in [0x00, 0x1f, 0x30, 0x80, 0xff] {
            assert_eq!(legacy_pic_irq_for_vector(vector), None, "vector {vector:#04x}");
        }
    }

    #[test]
    fn rejects_irq_lines_outside_pic_range() {
        for irq in [16, 17, 0x80, 0xff] {
            assert_eq!(legacy_pic_vector_for_irq(irq), None, "IRQ {irq}");
        }
    }

    #[test]
    fn includes_both_range_boundaries() {
        assert_eq!(legacy_pic_irq_for_vector(0x20), Some(0));
        assert_eq!(legacy_pic_irq_for_vector(0x2f), Some(15));
        assert_eq!(legacy_pic_vector_for_irq(0), Some(0x20));
        assert_eq!(legacy_pic_vector_for_irq(15), Some(0x2f));
    }
    #[test]
    fn masking_master_irq_preserves_slave_mask_and_other_bits() {
        assert_eq!(masks_after_masking_irq(0b0000_0000, 0b0101_0101, 1), Some((0b0000_0010, 0b0101_0101)));
    }

    #[test]
    fn masking_slave_irq_preserves_cascade_while_slave_has_enabled_lines() {
        assert_eq!(masks_after_masking_irq(0b0000_0000, 0b1111_1100, 8), Some((0b0000_0000, 0b1111_1101)));
    }

    #[test]
    fn masking_last_enabled_slave_irq_masks_cascade() {
        assert_eq!(masks_after_masking_irq(0b0000_0000, 0b0111_1111, 15), Some((0b0000_0100, 0b1111_1111)));
    }

    #[test]
    fn mask_consistency_requires_open_cascade_for_enabled_slave_lines() {
        assert!(pic_masks_are_consistent(0b0000_0000, 0b1111_1110));
        assert!(!pic_masks_are_consistent(0b0000_0100, 0b1111_1110));
        assert!(pic_masks_are_consistent(0b0000_0100, 0xff));
        assert!(pic_masks_are_consistent(0b0000_0000, 0xff));
    }

    #[test]
    fn master_irq_eoi_only_targets_master_pic() {
        for irq in 0..8 {
            assert_eq!(eoi_plan_for_irq(irq), Some((false, true)));
        }
    }

    #[test]
    fn slave_irq_eoi_targets_slave_then_master() {
        for irq in 8..16 {
            assert_eq!(eoi_plan_for_irq(irq), Some((true, true)));
        }
    }

    #[test]
    fn only_claimed_irqs_receive_an_eoi_plan() {
        for irq in 0..16 {
            assert_eq!(eoi_plan_for_handled_irq(irq, false), None);
            assert_eq!(eoi_plan_for_handled_irq(irq, true), eoi_plan_for_irq(irq));
        }
        assert_eq!(eoi_plan_for_handled_irq(16, true), None);
    }

    #[test]
    fn invalid_irq_has_no_eoi_plan() {
        assert_eq!(eoi_plan_for_irq(16), None);
        assert_eq!(eoi_plan_for_irq(0xff), None);
    }

    #[test]
    fn refuses_to_mask_cascade_while_slave_irqs_are_enabled() {
        assert_eq!(masks_after_masking_irq(0, 0b1111_1110, 2), None);
    }

    #[test]
    fn allows_masking_cascade_when_slave_pic_is_fully_masked() {
        assert_eq!(masks_after_masking_irq(0, 0xff, 2), Some((0b0000_0100, 0xff)));
    }

    #[test]
    fn invalid_irq_does_not_produce_masks() {
        assert_eq!(masks_after_masking_irq(0, 0, 16), None);
    }

}
