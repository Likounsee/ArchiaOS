//! Legacy dual 8259 PIC initialization and masked IRQ control.

    use super::{port_in, port_out};

const MASTER_COMMAND: u16 = 0x20;
const MASTER_DATA: u16 = 0x21;
const SLAVE_COMMAND: u16 = 0xA0;
const SLAVE_DATA: u16 = 0xA1;
const ICW1_INITIALIZE: u8 = 0x11;
const ICW4_8086: u8 = 0x01;
const MASTER_VECTOR_BASE: u8 = 0x20;
const SLAVE_VECTOR_BASE: u8 = 0x28;

#[inline]
unsafe fn wait() {
    // Port 0x80 is the conventional legacy I/O delay port.
    unsafe { port_out(0x80, 0) };
}

/// Remap legacy PIC vectors away from CPU exceptions and mask every IRQ.
///
/// This deliberately does not enable interrupts or unmask any line. The
/// IDT has vector-specific stubs, but IRQs must remain masked until a
/// returning dispatcher and acknowledgement path are installed.
/// Mask one legacy IRQ line without enabling any other line.
///
/// This is intentionally the only mask-control API until real IRQ stubs
/// and handlers are installed in the IDT.
pub unsafe fn mask_irq(irq: u8) -> Result<(), ()> {
    if irq >= 16 {
        return Err(());
    }

    let master_mask = unsafe { port_in(MASTER_DATA) };
    let slave_mask = unsafe { port_in(SLAVE_DATA) };
    if !super::interrupt_vectors::pic_masks_are_consistent(master_mask, slave_mask) {
        // Refuse to mutate an already inconsistent PIC state. Silently
        // masking another line would hide the disconnected slave cascade.
        return Err(());
    }
    let Some((next_master, next_slave)) =
        super::interrupt_vectors::masks_after_masking_irq(master_mask, slave_mask, irq)
    else {
        return Err(());
    };

    // Write only masks computed by the pure, host-tested transition helper.
    // Keeping the cascade masked while any slave IRQ remains enabled avoids
    // disconnecting an active slave line.
    unsafe {
        if irq >= 8 {
            port_out(SLAVE_DATA, next_slave);
        }
        port_out(MASTER_DATA, next_master);
    }
    Ok(())
}

/// Acknowledge a legacy PIC IRQ only after its handler claims it.
///
/// Slave EOI is emitted before master EOI for IRQ8..=IRQ15. Unhandled or
/// invalid IRQs are rejected without touching either command port. This
/// does not enable interrupts and does not alter masks.
pub unsafe fn end_of_interrupt(irq: u8, handler_handled: bool) -> Result<(), ()> {
    let Some((send_slave_eoi, send_master_eoi)) =
        super::interrupt_vectors::eoi_plan_for_handled_irq(irq, handler_handled)
    else {
        return Err(());
    };

    // The pure plan validates the IRQ and preserves the required slave-
    // before-master acknowledgement order for cascaded IRQs.
    unsafe {
        if send_slave_eoi {
            port_out(SLAVE_COMMAND, 0x20);
        }
        if send_master_eoi {
            port_out(MASTER_COMMAND, 0x20);
        }
    }
    Ok(())
}

pub unsafe fn initialize_masked() {
    // Mask first so stale firmware/PIC state cannot deliver an IRQ during
    // the remapping sequence. IF is also kept clear by the boot entry.
    unsafe {
        port_out(MASTER_DATA, 0xFF);
        port_out(SLAVE_DATA, 0xFF);

        port_out(MASTER_COMMAND, ICW1_INITIALIZE);
        wait();
        port_out(SLAVE_COMMAND, ICW1_INITIALIZE);
        wait();
        port_out(MASTER_DATA, MASTER_VECTOR_BASE);
        wait();
        port_out(SLAVE_DATA, SLAVE_VECTOR_BASE);
        wait();
        port_out(MASTER_DATA, 1 << 2);
        wait();
        port_out(SLAVE_DATA, 2);
        wait();
        port_out(MASTER_DATA, ICW4_8086);
        wait();
        port_out(SLAVE_DATA, ICW4_8086);
        wait();

        // Keep all lines masked even if firmware left some enabled.
        port_out(MASTER_DATA, 0xFF);
        port_out(SLAVE_DATA, 0xFF);
    }
}
