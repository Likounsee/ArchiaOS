#pragma once

extern "C" void gdt_initialize();
extern "C" void gdt_load_current();
extern "C" void gdt_initialize_cpu(unsigned int processor_index);
extern "C" void gdt_load_cpu(unsigned int processor_index);
