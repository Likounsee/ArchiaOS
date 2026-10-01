#pragma once
#include "acpi.hpp"
extern "C" bool ioapic_initialize(const AcpiInfo* acpi);
extern "C" bool ioapic_route_isa_irq(const AcpiInfo* acpi, unsigned int isa_irq, unsigned char vector, unsigned int destination_apic_id);
extern "C" bool ioapic_read_redirection(unsigned int gsi, unsigned long long* value);
