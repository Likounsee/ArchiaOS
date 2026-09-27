#include "idt.hpp"

static inline void debug_char(char c)
{
    asm volatile ("outb %0, %1" : : "a"(c), "Nd"(static_cast<unsigned short>(0xE9)));
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

    if (frame->vector >= 0x20)
    {
        irq_dispatch(frame);
        return;
    }

    if (frame->vector == 6)
    {
        /* UD2 is a 2-byte instruction. Advance RIP so iretq resumes
           after the test instruction instead of executing UD2 again. */
        frame->rip += 2;
        debug_str("[EXC] Invalid-opcode RIP advanced by 2\n");
        return;
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
}
