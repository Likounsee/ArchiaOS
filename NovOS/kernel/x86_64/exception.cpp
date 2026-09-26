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

    /* Test-only: UD2 is 2 bytes and RIP points to the faulting instruction. */
    if (frame->vector == 6)
        frame->rip += 2;
}
