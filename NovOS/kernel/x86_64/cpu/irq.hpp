#pragma once

#include "idt.hpp"

extern "C" void irq_initialize();
extern "C" void irq_dispatch(ExceptionFrame* frame);
extern "C" void irq_enable();
extern "C" void irq_disable();
extern "C" unsigned long long irq_get_ticks();
extern "C" bool irq_test_timer();
