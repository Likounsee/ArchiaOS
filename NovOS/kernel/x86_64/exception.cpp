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
    debug_str("EXCEPTION VECTOR: ");
    debug_hex64(frame->vector);
    debug_str("\nERROR CODE: ");
    debug_hex64(frame->error_code);
    debug_str("\nRIP: ");
    debug_hex64(frame->rip);
    debug_str("\nRFLAGS: ");
    debug_hex64(frame->rflags);
    debug_str("\n");

    if (frame->vector == 14)
    {
        debug_str("NESTED #PF -> EXPECTING #DF ON IST1\n");
        asm volatile (
            "mov $0x28, %%ax\n\t"
            "mov %%ax, %%ds"
            :
            :
            : "rax", "memory"
        );
    }

    if (frame->vector == 8)
    {
        unsigned long long frame_address =
            reinterpret_cast<unsigned long long>(frame);
        unsigned long long ist1_base =
            reinterpret_cast<unsigned long long>(tss_ist1_stack);
        unsigned long long ist1_top =
            reinterpret_cast<unsigned long long>(tss_ist1_stack_top);

        debug_str("DOUBLE FAULT HANDLER RUNNING\n");
        debug_str("EXCEPTION FRAME: ");
        debug_hex64(frame_address);
        debug_str("\nIST1 BASE: ");
        debug_hex64(ist1_base);
        debug_str("\nIST1 TOP: ");
        debug_hex64(ist1_top);
        debug_str("\n");

        if (frame_address >= ist1_base && frame_address < ist1_top)
            debug_str("IST1 STACK VALIDATED\n");
        else
            debug_str("IST1 STACK VALIDATION FAILED\n");

        for (;;)
            asm volatile ("cli; hlt");
    }
}
