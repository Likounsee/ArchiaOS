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

/// Whether an architectural CPU exception pushes an error-code word.
///
/// ISR stubs use this distinction to normalize their stack: exceptions with a
/// CPU-pushed error code push only the vector, while other vectors push a
/// synthetic zero before the vector. Values outside 0..=31 return false.
pub const fn cpu_exception_has_error_code(vector: u8) -> bool {
    matches!(vector, 8 | 10..=14 | 17 | 21 | 29 | 30)
}

/// Split a 64-bit handler address into the three offset fields of an x86-64 IDT gate.
/// Kept pure so boundary and high-half addresses can be tested without privileged CPU state.
pub const fn split_x86_64_idt_handler_address(handler: u64) -> (u16, u16, u32) {
    (handler as u16, (handler >> 16) as u16, (handler >> 32) as u32)
}

pub const fn classify_interrupt_vector(vector: u8) -> InterruptVectorClass {
    if vector < LEGACY_PIC_VECTOR_BASE {
        InterruptVectorClass::CpuException
    } else if let Some(irq) = legacy_pic_irq_for_vector(vector) {
        InterruptVectorClass::LegacyPicIrq(irq)
    } else {
        InterruptVectorClass::Other
    }
}

/// Decode the 64-bit vector value read from a normalized interrupt frame.
/// Keep the range check in the same width as the frame slot before converting
/// to a host pointer-sized index.
pub const fn pic_irq_for_frame_vector(vector: u64) -> Option<usize> {
    let base = LEGACY_PIC_VECTOR_BASE as u64;
    let count = LEGACY_PIC_IRQ_COUNT as u64;
    if vector < base || vector >= base + count {
        None
    } else {
        Some((vector - base) as usize)
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
/// Returns `None` for an invalid IRQ or an already inconsistent input mask
/// pair. The cascade line (IRQ2) is masked when masking the last enabled slave
/// IRQ leaves the entire slave PIC masked.
pub const fn masks_after_masking_irq(
    master_mask: u8,
    slave_mask: u8,
    irq: u8,
) -> Option<(u8, u8)> {
    if irq >= LEGACY_PIC_IRQ_COUNT || !pic_masks_are_consistent(master_mask, slave_mask) {
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
    fn splits_idt_handler_address_without_losing_bits() {
        assert_eq!(split_x86_64_idt_handler_address(0), (0, 0, 0));
        assert_eq!(split_x86_64_idt_handler_address(0x0000_0000_0000_ffff), (0xffff, 0, 0));
        assert_eq!(split_x86_64_idt_handler_address(0x0000_0000_1234_5678), (0x5678, 0x1234, 0));
        assert_eq!(split_x86_64_idt_handler_address(0xffff_8123_4567_89ab), (0x89ab, 0x4567, 0xffff_8123));
    }

    #[test]
    fn every_idt_handler_address_bit_survives_split_and_reassembly() {
        for bit in 0..64 {
            let address = 1u64 << bit;
            let (low, middle, high) = split_x86_64_idt_handler_address(address);
            let rebuilt = u64::from(low) | (u64::from(middle) << 16) | (u64::from(high) << 32);
            assert_eq!(rebuilt, address, "address bit {bit}");
        }

        for address in [
            0x0000_0000_0000_0000,
            0x0000_0000_0000_ffff,
            0x0000_0000_ffff_ffff,
            0xffff_8000_0000_0000,
            0xffff_ffff_ffff_ffff,
        ] {
            let (low, middle, high) = split_x86_64_idt_handler_address(address);
            let rebuilt = u64::from(low) | (u64::from(middle) << 16) | (u64::from(high) << 32);
            assert_eq!(rebuilt, address, "address {address:#018x}");
        }
    }

    #[test]
    fn identifies_cpu_pushed_error_code_vectors() {
        // Keep this as an explicit architectural expectation rather than
        // repeating the implementation's match expression in the test.
        const ERROR_CODE_VECTORS: [u8; 10] = [8, 10, 11, 12, 13, 14, 17, 21, 29, 30];

        for vector in 0u8..32 {
            let expected = ERROR_CODE_VECTORS.contains(&vector);
            assert_eq!(
                cpu_exception_has_error_code(vector),
                expected,
                "vector {vector}"
            );
        }

        // PIC and software/other vectors do not receive a CPU error-code word.
        for vector in 32u8..=u8::MAX {
            assert!(
                !cpu_exception_has_error_code(vector),
                "vector {vector}"
            );
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
    fn frame_vector_decoder_checks_full_u64_range() {
        for irq in 0u64..16 {
            assert_eq!(pic_irq_for_frame_vector(0x20 + irq), Some(irq as usize));
        }
        for vector in [0, 31, 48, 255, u32::MAX as u64, u64::MAX] {
            assert_eq!(pic_irq_for_frame_vector(vector), None, "vector {vector:#x}");
        }
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
    fn all_out_of_range_irq_values_are_rejected() {
        for irq in LEGACY_PIC_IRQ_COUNT..=u8::MAX {
            assert_eq!(legacy_pic_vector_for_irq(irq), None, "vector mapping IRQ {irq}");
            assert_eq!(eoi_plan_for_irq(irq), None, "EOI plan IRQ {irq}");
            assert_eq!(eoi_plan_for_handled_irq(irq, true), None, "handled EOI plan IRQ {irq}");
            assert_eq!(
                masks_after_masking_irq(0, 0xff, irq),
                None,
                "mask transition IRQ {irq}"
            );
        }
    }

    #[test]
    fn eoi_plans_are_exact_for_every_possible_irq_byte() {
        for irq in 0u8..=u8::MAX {
            let expected = match irq {
                0..=7 => Some((false, true)),
                8..=15 => Some((true, true)),
                _ => None,
            };
            assert_eq!(eoi_plan_for_irq(irq), expected, "IRQ {irq}");
            assert_eq!(eoi_plan_for_handled_irq(irq, false), None, "unhandled IRQ {irq}");
        }
    }

    #[test]
    fn refuses_to_transition_from_inconsistent_pic_masks() {
        // A slave line is enabled, but the master cascade is masked.
        assert_eq!(masks_after_masking_irq(0b0000_0100, 0b1111_1110, 0), None);
        // The inverse state is also invalid: a slave IRQ cannot be reached if
        // IRQ2 is masked, even when the requested change targets the slave.
        assert_eq!(masks_after_masking_irq(0b0000_0100, 0b1111_1101, 8), None);
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

    #[test]
    fn masking_preserves_pic_cascade_invariant_for_all_consistent_masks() {
        for master_mask in 0u8..=u8::MAX {
            for slave_mask in 0u8..=u8::MAX {
                if !pic_masks_are_consistent(master_mask, slave_mask) {
                    continue;
                }

                for irq in 0..LEGACY_PIC_IRQ_COUNT {
                    let Some((next_master, next_slave)) =
                        masks_after_masking_irq(master_mask, slave_mask, irq)
                    else {
                        // Masking IRQ2 would disconnect any enabled slave line.
                        // This is the one valid IRQ whose transition is rejected
                        // for some otherwise-consistent input mask pairs.
                        assert_eq!(irq, 2, "unexpected rejected IRQ {irq}");
                        assert_ne!(slave_mask, 0xff, "cascade may be masked when slave is fully masked");
                        continue;
                    };
                    assert!(
                        pic_masks_are_consistent(next_master, next_slave),
                        "inconsistent result: master={master_mask:#04x}, slave={slave_mask:#04x}, irq={irq}"
                    );
                    // This API only masks lines: no transition may clear a
                    // previously set mask bit (the masked state).
                    assert_eq!(next_master | master_mask, next_master);
                    assert_eq!(next_slave | slave_mask, next_slave);
                    if irq < 8 {
                        assert_ne!(next_master & (1u8 << irq), 0);
                        assert_eq!(next_master, master_mask | (1u8 << irq));
                        assert_eq!(next_slave, slave_mask);
                    } else {
                        let expected_slave = slave_mask | (1u8 << (irq - 8));
                        let expected_master = if expected_slave == 0xff {
                            master_mask | (1u8 << 2)
                        } else {
                            master_mask
                        };
                        assert_eq!(next_slave, expected_slave);
                        assert_eq!(next_master, expected_master);
                    }
                }
            }
        }
    }

}
