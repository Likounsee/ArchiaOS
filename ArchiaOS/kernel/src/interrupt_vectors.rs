//! Pure helpers for classifying remapped legacy 8259 PIC vectors.
//!
//! Kept independent of privileged instructions so the range logic can be
//! tested on the host as part of CI.

/// First vector used by the remapped master PIC.
pub const LEGACY_PIC_VECTOR_BASE: u8 = 0x20;
/// Number of IRQ lines exposed by the master and slave PICs together.
pub const LEGACY_PIC_IRQ_COUNT: u8 = 16;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
