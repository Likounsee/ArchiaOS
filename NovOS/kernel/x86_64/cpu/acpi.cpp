#include "acpi.hpp"

static AcpiInfo acpi_info = {};

static inline unsigned char read8(unsigned long long address)
{
    return *reinterpret_cast<volatile unsigned char*>(address);
}

static inline unsigned short read16(unsigned long long address)
{
    return *reinterpret_cast<volatile unsigned short*>(address);
}

static inline unsigned int read32(unsigned long long address)
{
    return *reinterpret_cast<volatile unsigned int*>(address);
}

static inline unsigned long long read64(unsigned long long address)
{
    return *reinterpret_cast<volatile unsigned long long*>(address);
}

static inline bool signature8(unsigned long long address, const char* value)
{
    for (int i = 0; i < 8; ++i)
    {
        if (read8(address + static_cast<unsigned long long>(i)) !=
            static_cast<unsigned char>(value[i]))
            return false;
    }

    return true;
}

static inline bool signature4(unsigned long long address, const char* value)
{
    for (int i = 0; i < 4; ++i)
    {
        if (read8(address + static_cast<unsigned long long>(i)) !=
            static_cast<unsigned char>(value[i]))
            return false;
    }

    return true;
}

static bool checksum_ok(unsigned long long address, unsigned int length)
{
    unsigned char sum = 0;

    for (unsigned int i = 0; i < length; ++i)
        sum = static_cast<unsigned char>(
            sum + read8(address + static_cast<unsigned long long>(i)));

    return sum == 0;
}

static bool address_is_mapped(unsigned long long address)
{
    return address != 0 && address < 0x40000000ULL;
}

static unsigned long long find_rsdp_in_range(
    unsigned long long start,
    unsigned long long end)
{
    start &= ~0xFULL;

    for (unsigned long long address = start;
         address + 36 <= end;
         address += 16)
    {
        if (!signature8(address, "RSD PTR "))
            continue;

        if (!checksum_ok(address, 20))
            continue;

        unsigned char revision = read8(address + 15);

        if (revision >= 2)
        {
            unsigned int length = read32(address + 20);

            if (length < 36 || length > 4096)
                continue;

            if (!checksum_ok(address, length))
                continue;
        }

        return address;
    }

    return 0;
}

static unsigned long long find_rsdp()
{
    /*
     * The EBDA pointer is located in the BIOS data area at 0x40E.
     * ACPI also permits the RSDP to be located in the 0xE0000-0xFFFFF
     * region. These areas are identity-mapped by the current early
     * paging setup.
     */
    unsigned int ebda_segment = read16(0x40E);
    unsigned long long ebda = static_cast<unsigned long long>(ebda_segment) << 4;

    if (ebda >= 0x80000ULL && ebda < 0xA0000ULL)
    {
        unsigned long long result =
            find_rsdp_in_range(ebda, ebda + 1024);

        if (result != 0)
            return result;
    }

    return find_rsdp_in_range(0xE0000ULL, 0x100000ULL);
}

static unsigned long long find_madt(
    unsigned long long root_address,
    bool xsdt)
{
    if (!address_is_mapped(root_address))
        return 0;

    if (!signature4(root_address, xsdt ? "XSDT" : "RSDT"))
        return 0;

    unsigned int length = read32(root_address + 4);

    if (length < 36 || length > 0x100000)
        return 0;

    if (!checksum_ok(root_address, length))
        return 0;

    unsigned int entry_size = xsdt ? 8 : 4;
    unsigned int entries = (length - 36) / entry_size;

    for (unsigned int i = 0; i < entries; ++i)
    {
        unsigned long long entry =
            root_address + 36ULL + static_cast<unsigned long long>(i) * entry_size;

        unsigned long long table_address =
            xsdt ? read64(entry) : static_cast<unsigned long long>(read32(entry));

        if (!address_is_mapped(table_address))
            continue;

        if (signature4(table_address, "APIC"))
            return table_address;
    }

    return 0;
}

static void parse_madt(unsigned long long madt)
{
    acpi_info.local_apic_address = read32(madt + 36);

    unsigned int length = read32(madt + 4);

    if (length < 44 || length > 0x100000)
        return;

    unsigned long long current = madt + 44;
    unsigned long long end = madt + length;

    while (current + 2 <= end)
    {
        unsigned char type = read8(current);
        unsigned char entry_length = read8(current + 1);

        if (entry_length < 2 || current + entry_length > end)
            break;

        if (type == 0 && entry_length >= 8)
        {
            unsigned int flags = read32(current + 4);

            if (flags & 1U)
                ++acpi_info.processor_count;
        }
        else if (type == 1 && entry_length >= 12)
        {
            if (acpi_info.ioapic_count == 0)
            {
                acpi_info.ioapic_address = read32(current + 4);
                acpi_info.ioapic_gsi_base = read32(current + 8);
            }

            ++acpi_info.ioapic_count;
        }
        else if (type == 2 && entry_length >= 10)
        {
            ++acpi_info.interrupt_override_count;
        }
        else if (type == 5 && entry_length >= 12)
        {
            acpi_info.local_apic_address = read64(current + 4);
        }

        current += entry_length;
    }
}

extern "C" bool acpi_initialize()
{
    acpi_info = {};

    unsigned long long rsdp = find_rsdp();

    if (rsdp == 0)
        return false;

    acpi_info.rsdp_address = rsdp;

    unsigned char revision = read8(rsdp + 15);
    unsigned long long root_address = 0;
    bool xsdt = false;

    if (revision >= 2)
    {
        root_address = read64(rsdp + 24);

        if (address_is_mapped(root_address))
            xsdt = true;
    }

    if (root_address == 0)
    {
        root_address = read32(rsdp + 16);
        xsdt = false;
    }

    if (root_address == 0)
        return false;

    acpi_info.root_table_address = root_address;

    unsigned long long madt = find_madt(root_address, xsdt);

    if (madt == 0 && xsdt)
    {
        root_address = read32(rsdp + 16);
        acpi_info.root_table_address = root_address;
        madt = find_madt(root_address, false);
    }

    if (madt == 0)
        return false;

    acpi_info.madt_address = madt;

    parse_madt(madt);

    return true;
}

extern "C" const AcpiInfo* acpi_get_info()
{
    return &acpi_info;
}
