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

enum AcpiStatus
{
    ACPI_STATUS_OK = 0,
    ACPI_STATUS_RSDP_NOT_FOUND,
    ACPI_STATUS_ROOT_NOT_FOUND,
    ACPI_STATUS_ROOT_INVALID,
    ACPI_STATUS_MADT_NOT_FOUND,
    ACPI_STATUS_MADT_INVALID
};

extern "C" bool acpi_initialize(unsigned long long rsdp_address);
extern "C" const AcpiInfo* acpi_get_info();
extern "C" AcpiStatus acpi_get_status();
