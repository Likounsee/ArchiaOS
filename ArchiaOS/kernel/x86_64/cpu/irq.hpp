#pragma once

#include "idt.hpp"

extern "C" bool irq_initialize();
extern "C" ExceptionFrame* irq_dispatch(ExceptionFrame* frame);
extern "C" void irq_enable();
extern "C" void irq_disable();
extern "C" unsigned long long irq_get_ticks();
extern "C" bool irq_test_timer();

extern "C" unsigned long long irq_dispatch_count_get();
extern "C" unsigned long long irq_keyboard_count_get();
extern "C" unsigned char irq_keyboard_last_scancode();
