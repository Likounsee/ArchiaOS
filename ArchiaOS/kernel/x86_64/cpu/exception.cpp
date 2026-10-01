#include "idt.hpp"

static inline void debug_char(char c)
{
    asm volatile ("outb %0, %1" : : "a"(c), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
}

static void debug_str(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        debug_char(s[i]);
}

static void debug_hex64(unsigned long long value)
{
    const char* digits = "0123456789ABCDEF";
    debug_char('0');
    debug_char('x');
    for (int i = 0; i < 16; ++i)
        debug_char(digits[(value >> ((15 - i) * 4)) & 0xF]);
}

struct PageFaultTestState
{
    unsigned long long recovery_rip;
    unsigned long long expected_cr2;
    unsigned long long error_mask;
    unsigned long long error_value;
};

static PageFaultTestState page_fault_test = {};
static unsigned long long invalid_opcode_test_rip = 0;

extern "C" void exception_expect_page_fault(
    unsigned long long recovery_rip,
    unsigned long long expected_cr2,
    unsigned long long error_mask,
    unsigned long long error_value)
{
    page_fault_test.recovery_rip = recovery_rip;
    page_fault_test.expected_cr2 = expected_cr2;
    page_fault_test.error_mask = error_mask;
    page_fault_test.error_value = error_value;
}

extern "C" bool exception_page_fault_test_active()
{
    return page_fault_test.recovery_rip != 0;
}

extern "C" void exception_expect_invalid_opcode(unsigned long long rip)
{
    invalid_opcode_test_rip = rip;
}

extern "C" bool exception_invalid_opcode_test_active()
{
    return invalid_opcode_test_rip != 0;
}

extern "C" unsigned char tss_ist1_stack[];
extern "C" unsigned char tss_ist1_stack_top[];

extern "C" void exception_dispatch(ExceptionFrame* frame)
{
    debug_str("[EXC] Vector: ");
    debug_hex64(frame->vector);
    debug_str("  Error: ");
    debug_hex64(frame->error_code);
    debug_str("  RIP: ");
    debug_hex64(frame->rip);
    debug_str("  RFLAGS: ");
    debug_hex64(frame->rflags);
    debug_str("\n");

    if ((frame->vector >= 0x20 && frame->vector <= 0x2F) ||
        frame->vector == 0xFF)
    {
        irq_dispatch(frame);
        return;
    }

    if (frame->vector == 6)
    {
        if (invalid_opcode_test_rip != 0 &&
            frame->rip == invalid_opcode_test_rip)
        {
            invalid_opcode_test_rip = 0;
            frame->rip += 2;
            debug_str("[EXC] Controlled invalid-opcode recovery\n");
            return;
        }

        debug_str("[EXC] Fatal invalid-opcode exception\n");
        for (;;)
            asm volatile ("cli; hlt");
    }

    if (frame->vector == 14)
    {
        unsigned long long cr2 = 0;
        asm volatile("mov %%cr2, %0" : "=r"(cr2));

        const unsigned long long error = frame->error_code;

        debug_str("[PF] Linear address: ");
        debug_hex64(cr2);
        debug_str("  error: ");
        debug_hex64(error);
        debug_str("\n");

        debug_str("[PF] reason: ");
        if ((error & (1ULL << 0)) == 0)
            debug_str("NOT-PRESENT");
        else
            debug_str("PROTECTION");
        debug_str(" access: ");

        if (error & (1ULL << 4))
            debug_str("INSTRUCTION-FETCH");
        else if (error & (1ULL << 1))
            debug_str("WRITE");
        else
            debug_str("READ");

        debug_str(" privilege: ");
        debug_str((error & (1ULL << 2)) ? "USER" : "SUPERVISOR");
        debug_str("\n");

        debug_str("[PF] page-fault handler reached\n");

        const bool expected =
            page_fault_test.recovery_rip != 0 &&
            cr2 == page_fault_test.expected_cr2 &&
            (error & page_fault_test.error_mask) == page_fault_test.error_value;

        if (expected)
        {
            const unsigned long long recovery = page_fault_test.recovery_rip;
            page_fault_test = {};
            frame->rip = recovery;
            debug_str("[PF] controlled test recovery\n");
            return;
        }

        if (page_fault_test.recovery_rip != 0)
            debug_str("[PF] unexpected fault during armed test\n");

        for (;;)
            asm volatile ("cli; hlt");
    }

    if (frame->vector == 8)
    {
        unsigned long long frame_address =
            reinterpret_cast<unsigned long long>(frame);
        unsigned long long ist1_base =
            reinterpret_cast<unsigned long long>(tss_ist1_stack);
        unsigned long long ist1_top =
            reinterpret_cast<unsigned long long>(tss_ist1_stack_top);

        debug_str("[EXC] Double-fault handler entered\n");
        debug_str("[EXC] Frame: ");
        debug_hex64(frame_address);
        debug_str("  IST1 base: ");
        debug_hex64(ist1_base);
        debug_str("  IST1 top: ");
        debug_hex64(ist1_top);
        debug_str("\n");

        if (frame_address >= ist1_base && frame_address < ist1_top)
            debug_str("[EXC] IST1 stack validation passed\n");
        else
            debug_str("[EXC] IST1 stack validation FAILED\n");

        for (;;)
            asm volatile ("cli; hlt");
    }

    /* All other CPU exceptions are not safely recoverable yet. */
    debug_str("[EXC] Fatal exception; halting\n");
    for (;;)
        asm volatile ("cli; hlt");
}
