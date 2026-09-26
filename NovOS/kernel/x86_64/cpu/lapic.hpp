#pragma once

extern "C" bool lapic_initialize();
extern "C" void lapic_eoi();
extern "C" void lapic_timer_start();
extern "C" unsigned long long lapic_get_ticks();
extern "C" void lapic_stop_timer();
