#include "acpi.hpp"

static AcpiInfo acpi_info = {};
static AcpiStatus acpi_status = ACPI_STATUS_OK;

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

/*
 * Early paging currently identity-maps the first 512 GiB.
 * ACPI firmware structures and MMIO used during early boot must
 * therefore stay inside that range until a full virtual-memory
 * manager is available.
 */
static bool address_is_mapped(unsigned long long address)
{
    return address != 0 && address < 0x8000000000ULL;
}

static bool range_is_mapped(
    unsigned long long address,
    unsigned long long length)
{
    if (length == 0 || !address_is_mapped(address))
        return false;

    const unsigned long long limit = 0x8000000000ULL;
    return length <= limit - address;
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
    unsigned int ebda_segment = read16(0x40E);
    unsigned long long ebda =
        static_cast<unsigned long long>(ebda_segment) << 4;

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
    if (!range_is_mapped(root_address, 36))
        return 0;

    if (!signature4(root_address, xsdt ? "XSDT" : "RSDT"))
        return 0;

    unsigned int length = read32(root_address + 4);

    if (length < 36 || length > 0x100000 ||
        !range_is_mapped(root_address, length))
        return 0;

    if (!checksum_ok(root_address, length))
        return 0;

    unsigned int entry_size = xsdt ? 8 : 4;
    if ((length - 36) % entry_size != 0)
        return 0;
    unsigned int entries = (length - 36) / entry_size;

    for (unsigned int i = 0; i < entries; ++i)
    {
        unsigned long long entry =
            root_address +
            36ULL +
            static_cast<unsigned long long>(i) * entry_size;

        unsigned long long table_address =
            xsdt
                ? read64(entry)
                : static_cast<unsigned long long>(read32(entry));

        if (!address_is_mapped(table_address))
            continue;

        if (signature4(table_address, "APIC"))
            return table_address;
    }

    return 0;
}

static bool parse_madt(unsigned long long madt)
{
    if (!range_is_mapped(madt, 36))
        return false;

    if (!signature4(madt, "APIC"))
        return false;

    unsigned int length = read32(madt + 4);

    if (length < 44 || length > 0x100000 ||
        !range_is_mapped(madt, length))
        return false;

    if (!checksum_ok(madt, length))
        return false;

    acpi_info.local_apic_address = read32(madt + 36);

    unsigned long long current = madt + 44;
    unsigned long long end = madt + length;

    while (current + 2 <= end)
    {
        unsigned char type = read8(current);
        unsigned char entry_length = read8(current + 1);

        if (entry_length < 2 || current + entry_length > end)
            return false;

        if (type == 0 && entry_length >= 8)
        {
            unsigned int flags = read32(current + 4);

            if (flags & 1U)
            {
                if (acpi_info.processor_count < AcpiInfo::MAX_PROCESSORS)
                    acpi_info.processor_apic_ids[acpi_info.processor_count] = read8(current + 3);
                ++acpi_info.processor_count;
            }
        }
        else if (type == 9 && entry_length >= 16)
        {
            /*
             * Processor Local x2APIC structure:
             * flags are at offset 8 and use the same enabled bit as
             * the legacy Local APIC structure. This is required on
             * systems whose MADT describes processors through x2APIC.
             */
            unsigned int flags = read32(current + 8);

            if (flags & 1U)
            {
                if (acpi_info.processor_count < AcpiInfo::MAX_PROCESSORS)
                    acpi_info.processor_apic_ids[acpi_info.processor_count] = read32(current + 4);
                ++acpi_info.processor_count;
            }
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
            if (acpi_info.interrupt_override_count < AcpiInfo::MAX_INTERRUPT_OVERRIDES)
            {
                const unsigned int i = acpi_info.interrupt_override_count;
                acpi_info.interrupt_override_source[i] = read8(current + 3);
                acpi_info.interrupt_override_gsi[i] = read32(current + 4);
                acpi_info.interrupt_override_flags[i] = read16(current + 8);
            }
            ++acpi_info.interrupt_override_count;
        }
        else if (type == 5 && entry_length >= 12)
        {
            acpi_info.local_apic_address = read64(current + 4);
        }

        current += entry_length;
    }

    return acpi_info.local_apic_address != 0;
}

extern "C" bool acpi_initialize(unsigned long long rsdp_address)
{
    acpi_info = {};
    acpi_status = ACPI_STATUS_OK;

    unsigned long long rsdp = 0;

    /*
     * On UEFI systems the ACPI specification requires the loader
     * to obtain the RSDP from the EFI Configuration Table before
     * ExitBootServices(). BootInfo carries that physical address.
     *
     * Keep the legacy memory scan as a fallback for older firmware
     * and future BIOS/CSM support.
     */
    if (address_is_mapped(rsdp_address) &&
        signature8(rsdp_address, "RSD PTR ") &&
        checksum_ok(rsdp_address, 20))
    {
        unsigned char revision = read8(rsdp_address + 15);

        if (revision < 2)
        {
            rsdp = rsdp_address;
        }
        else
        {
            unsigned int length = read32(rsdp_address + 20);

            if (length >= 36 &&
                length <= 4096 &&
                checksum_ok(rsdp_address, length))
            {
                rsdp = rsdp_address;
            }
        }
    }

    if (rsdp == 0)
        rsdp = find_rsdp();

    if (rsdp == 0)
    {
        acpi_status = ACPI_STATUS_RSDP_NOT_FOUND;
        return false;
    }

    acpi_info.rsdp_address = rsdp;

    unsigned char revision = read8(rsdp + 15);
    unsigned long long root_address = 0;
    bool xsdt = false;

    if (revision >= 2)
    {
        unsigned long long candidate = read64(rsdp + 24);

        if (address_is_mapped(candidate) &&
            signature4(candidate, "XSDT"))
        {
            root_address = candidate;
            xsdt = true;
        }
    }

    if (root_address == 0)
    {
        unsigned long long candidate =
            static_cast<unsigned long long>(read32(rsdp + 16));

        if (address_is_mapped(candidate) &&
            signature4(candidate, "RSDT"))
        {
            root_address = candidate;
            xsdt = false;
        }
    }

    if (root_address == 0)
    {
        acpi_status = ACPI_STATUS_ROOT_NOT_FOUND;
        return false;
    }

    acpi_info.root_table_address = root_address;

    /*
     * Validate the selected root table before looking for MADT.
     */
    if (!range_is_mapped(root_address, 36))
    {
        acpi_status = ACPI_STATUS_ROOT_INVALID;
        return false;
    }

    unsigned int root_length = read32(root_address + 4);

    if (root_length < 36 ||
        root_length > 0x100000 ||
        !range_is_mapped(root_address, root_length) ||
        !checksum_ok(root_address, root_length))
    {
        acpi_status = ACPI_STATUS_ROOT_INVALID;
        return false;
    }

    unsigned long long madt = find_madt(root_address, xsdt);

    /*
     * Some firmware exposes a valid RSDT even when the XSDT path
     * is unusable. Fall back cleanly.
     */
    if (madt == 0 && xsdt)
    {
        unsigned long long rsdt =
            static_cast<unsigned long long>(read32(rsdp + 16));

        if (address_is_mapped(rsdt) &&
            signature4(rsdt, "RSDT"))
        {
            acpi_info.root_table_address = rsdt;
            root_address = rsdt;
            xsdt = false;
            madt = find_madt(root_address, false);
        }
    }

    if (madt == 0)
    {
        acpi_status = ACPI_STATUS_MADT_NOT_FOUND;
        return false;
    }

    acpi_info.madt_address = madt;

    if (!parse_madt(madt))
    {
        acpi_status = ACPI_STATUS_MADT_INVALID;
        return false;
    }

    acpi_status = ACPI_STATUS_OK;
    return true;
}

extern "C" const AcpiInfo* acpi_get_info()
{
    return &acpi_info;
}

extern "C" AcpiStatus acpi_get_status()
{
    return acpi_status;
}
