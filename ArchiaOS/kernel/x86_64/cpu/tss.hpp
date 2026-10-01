#pragma once

extern "C" void tss_initialize();
extern "C" void tss_initialize_cpu(unsigned int processor_index);
extern "C" void tss_load();
extern "C" void tss_load_cpu(unsigned int processor_index);
extern "C" unsigned int tss_current_selector();
