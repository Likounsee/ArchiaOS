#include "irq.hpp"
#include "lapic.hpp"
#include "scheduler.hpp"

static volatile unsigned long long irq_dispatch_count = 0;
static volatile unsigned long long keyboard_irq_count = 0;
static volatile unsigned char keyboard_last_scancode = 0;

static inline unsigned char io_in8(unsigned short port)
{
    unsigned char value;
    asm volatile ("inb %1,%0" : "=a"(value) : "Nd"(port));
    return value;
}

static inline void pic_mask_all()
{
    const unsigned char mask = 0xFF;
    const unsigned short master_pic = 0x21;
    const unsigned short slave_pic = 0xA1;

    asm volatile ("outb %0, %1" : : "a"(mask), "Nd"(master_pic));
    asm volatile ("outb %0, %1" : : "a"(mask), "Nd"(slave_pic));
}

extern "C" bool irq_initialize()
{
    pic_mask_all();
    return lapic_initialize();
}

extern "C" ExceptionFrame* irq_dispatch(ExceptionFrame* frame)
{
    if (frame == nullptr)
        return nullptr;

    const unsigned long long vector = frame->vector;

    if (vector >= 0x20 && vector <= 0xFE)
        __atomic_fetch_add(&irq_dispatch_count, 1ULL, __ATOMIC_RELAXED);

    if (vector == 0x20)
    {
        lapic_timer_interrupt();
        frame = scheduler_timer_tick(0, frame);
        if (scheduler_current_task(0) == 1 && scheduler_switch_count(0) == 1)
        {
            const unsigned long long cs = frame->cs;
            const unsigned long long rip = frame->rip;
            asm volatile("outb %0,%1" : : "a"('S'), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
            asm volatile("outb %0,%1" : : "a"('W'), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
            (void)cs;
            (void)rip;
        }
    }
    else if (vector == 0x21)
    {
        keyboard_last_scancode = io_in8(0x60);
        __atomic_fetch_add(&keyboard_irq_count, 1ULL, __ATOMIC_RELAXED);
    }

    /* Vector 0xFF is the LAPIC spurious vector and must not receive EOI. */
    if (vector >= 0x20 && vector <= 0xFE)
        lapic_eoi();

    return frame;
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
    irq_dispatch_count = 0;

    lapic_timer_start();

    unsigned long long start = irq_get_ticks();

    irq_enable();

    for (unsigned long long timeout = 0;
         timeout < 200000000ULL;
         ++timeout)
    {
        if (irq_get_ticks() >= start + 3)
        {
            irq_disable();
            lapic_stop_timer();
            return irq_dispatch_count >= 3;
        }

        asm volatile ("pause");
    }

    irq_disable();
    lapic_stop_timer();
    return false;
}

extern "C" unsigned long long irq_dispatch_count_get()
{
    return __atomic_load_n(&irq_dispatch_count, __ATOMIC_ACQUIRE);
}

extern "C" unsigned long long irq_keyboard_count_get()
{
    return __atomic_load_n(&keyboard_irq_count, __ATOMIC_ACQUIRE);
}

extern "C" unsigned char irq_keyboard_last_scancode()
{
    return keyboard_last_scancode;
}
