#pragma once

#include <stdint.h>

struct SmpTrampolineMailbox
{
    uint64_t cr3_physical;
    uint64_t entry_virtual;
    uint64_t stack_virtual;
    uint64_t cr0;
    uint64_t cr4;
    uint64_t efer;
    uint32_t processor_index;
    uint32_t apic_id;
    volatile uint32_t started;
    uint32_t reserved;
};

static_assert(sizeof(SmpTrampolineMailbox) == 64);

extern "C" bool smp_initialize(const struct AcpiInfo* acpi);
extern "C" unsigned int smp_online_count();
