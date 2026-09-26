#include "irq.hpp"
#include "lapic.hpp"

static inline void pic_mask_all()
{
    asm volatile ("outb %0, %1" : : "a"(0xFF), "Nd"(static_cast<unsigned short>(0x21)));
    asm volatile ("outb %0, %1" : : "a"(0xFF), "Nd"(static_cast<unsigned short>(0xA1)));
}

extern "C" bool irq_initialize()
{
    pic_mask_all();
    return lapic_initialize();
}

extern "C" void irq_dispatch(ExceptionFrame* frame)
{
    if (frame == nullptr)
        return;

    unsigned long long vector = frame->vector;

    if (vector == 0x20)
        lapic_timer_interrupt();

    if (vector >= 0x20 && vector <= 0xEF)
        lapic_eoi();
}

extern "C" void irq_enable()
{
    asm volatile ("sti" : : : "memory");
}

extern "C" void irq_disable()
{
    asm volatile ("cli" : : : "memory");
}

extern "C" unsigned long long irq_get_ticks()
{
    return lapic_get_ticks();
}

extern "C" bool irq_test_timer()
{
    irq_disable();

    lapic_timer_start();

    unsigned long long start = irq_get_ticks();

    irq_enable();

    for (volatile unsigned long long timeout = 0;
         timeout < 200000000ULL;
         ++timeout)
    {
        if (irq_get_ticks() >= start + 3)
        {
            irq_disable();
            lapic_stop_timer();
            return true;
        }

        asm volatile ("pause");
    }

    irq_disable();
    lapic_stop_timer();
    return false;
}
