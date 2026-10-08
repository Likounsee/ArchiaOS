#include "ioapic.hpp"
#include "lapic.hpp"
#include "../memory/paging.hpp"

static volatile unsigned int* ioapic_base = nullptr;
static unsigned int ioapic_gsi_base = 0;
static unsigned int ioapic_redirection_count = 0;
static bool ioapic_initialized = false;

static constexpr unsigned int IOAPIC_REGSEL = 0x00;
static constexpr unsigned int IOAPIC_WINDOW = 0x10;
static constexpr unsigned int IOAPIC_VERSION = 0x01;
static constexpr unsigned int IOAPIC_REDIR_BASE = 0x10;
static constexpr unsigned long long IOAPIC_MASK = 1ULL << 16;

static inline void ioapic_write_register(unsigned int reg, unsigned int value)
{
    auto base = reinterpret_cast<unsigned long long>(ioapic_base);
    *reinterpret_cast<volatile unsigned int*>(base + IOAPIC_REGSEL) = reg;
    *reinterpret_cast<volatile unsigned int*>(base + IOAPIC_WINDOW) = value;
}
static inline unsigned int ioapic_read_register(unsigned int reg)
{
    auto base = reinterpret_cast<unsigned long long>(ioapic_base);
    *reinterpret_cast<volatile unsigned int*>(base + IOAPIC_REGSEL) = reg;
    return *reinterpret_cast<volatile unsigned int*>(base + IOAPIC_WINDOW);
}
static void ioapic_write_redirection(unsigned int index, unsigned long long value)
{
    unsigned int reg = IOAPIC_REDIR_BASE + index * 2U;
    ioapic_write_register(reg, static_cast<unsigned int>(value));
    ioapic_write_register(reg + 1U, static_cast<unsigned int>(value >> 32));
}
static unsigned long long ioapic_read_redirection_index(unsigned int index)
{
    unsigned int reg = IOAPIC_REDIR_BASE + index * 2U;
    unsigned long long low = ioapic_read_register(reg);
    unsigned long long high = ioapic_read_register(reg + 1U);
    return low | (high << 32);
}

extern "C" bool ioapic_initialize(const AcpiInfo* acpi)
{
    if (ioapic_initialized)
        return true;

    if (acpi == nullptr || acpi->ioapic_address == 0 || acpi->ioapic_count == 0)
        return false;
    const unsigned long long base = acpi->ioapic_address & ~(PAGE_SIZE - 1ULL);
    if (base >= PMM_MAX_PHYSICAL_ADDRESS ||
        !paging_map_4k(base, base, PagingFlags{true, false, false, true, true}))
        return false;

    ioapic_base = reinterpret_cast<volatile unsigned int*>(base);
    unsigned int version = ioapic_read_register(IOAPIC_VERSION);
    unsigned int count = ((version >> 16) & 0xFFU) + 1U;
    if (count == 0 || count > 256)
        return false;

    ioapic_gsi_base = acpi->ioapic_gsi_base;
    ioapic_redirection_count = count;
    for (unsigned int i = 0; i < count; ++i)
        ioapic_write_redirection(i, IOAPIC_MASK);

    ioapic_initialized = true;
    return true;
}

extern "C" bool ioapic_route_isa_irq(const AcpiInfo* acpi, unsigned int isa_irq, unsigned char vector, unsigned int destination_apic_id)
{
    if (ioapic_base == nullptr || acpi == nullptr || isa_irq > 15U ||
        vector < 0x20U || vector == 0xFFU ||
        destination_apic_id > 0xFFU)
        return false;

    unsigned int gsi = isa_irq;
    unsigned short flags = 0;
    for (unsigned int i = 0; i < acpi->interrupt_override_count && i < AcpiInfo::MAX_INTERRUPT_OVERRIDES; ++i)
    {
        if (acpi->interrupt_override_source[i] == isa_irq)
        {
            gsi = acpi->interrupt_override_gsi[i];
            flags = acpi->interrupt_override_flags[i];
            break;
        }
    }
    if (gsi < ioapic_gsi_base)
        return false;
    unsigned int index = gsi - ioapic_gsi_base;
    if (index >= ioapic_redirection_count)
        return false;

    unsigned long long entry = vector;
    if ((flags & 0x3U) == 0x3U)
        entry |= 1ULL << 13;
    if (((flags >> 2) & 0x3U) == 0x3U)
        entry |= 1ULL << 15;
    entry |= static_cast<unsigned long long>(destination_apic_id) << 56;
    ioapic_write_redirection(index, entry);
    return true;
}

extern "C" bool ioapic_read_redirection(unsigned int gsi, unsigned long long* value)
{
    if (ioapic_base == nullptr || value == nullptr || gsi < ioapic_gsi_base)
        return false;
    unsigned int index = gsi - ioapic_gsi_base;
    if (index >= ioapic_redirection_count)
        return false;
    *value = ioapic_read_redirection_index(index);
    return true;
}
