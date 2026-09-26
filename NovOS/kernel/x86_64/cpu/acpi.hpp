#pragma once

struct AcpiInfo
{
    unsigned long long rsdp_address;
    unsigned long long root_table_address;
    unsigned long long madt_address;
    unsigned long long local_apic_address;
    unsigned long long ioapic_address;
    unsigned int ioapic_gsi_base;
    unsigned int processor_count;
    unsigned int ioapic_count;
    unsigned int interrupt_override_count;
};

extern "C" bool acpi_initialize();
extern "C" const AcpiInfo* acpi_get_info();
