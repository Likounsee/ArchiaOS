#include "uefi.h"
#include "boot_debug.h"
#include "filesystem.h"
#include "elf_loader.h"
#include "framebuffer.h"
#include "memory_map.h"
#include "handoff.h"
#include "../../../common/boot_info.h"

extern "C" void* memset(void* destination, int value, UINTN size)
{
    UINT8* bytes = reinterpret_cast<UINT8*>(destination);
    UINT8 byte = static_cast<UINT8>(value);
    for (UINTN i = 0; i < size; ++i)
        bytes[i] = byte;
    return destination;
}

static EFI_STATUS allocate_boot_info(EFI_SYSTEM_TABLE* st, BootInfo** out)
{
    EFI_PHYSICAL_ADDRESS address = 0;
    auto allocatePages =
        reinterpret_cast<EFI_ALLOCATE_PAGES>(st->BootServices->AllocatePages);

    EFI_STATUS status = allocatePages(
        EFI_ALLOCATE_ANY_PAGES, EfiLoaderData, 1, &address);

    if (status != EFI_SUCCESS)
        return status;

    *out = reinterpret_cast<BootInfo*>(address);

    UINT8* bytes = reinterpret_cast<UINT8*>(*out);
    for (UINTN i = 0; i < 4096; ++i)
        bytes[i] = 0;

    return EFI_SUCCESS;
}

static UINT64 find_table(EFI_SYSTEM_TABLE* st, const EFI_GUID& guid)
{
    if (!st || !st->ConfigurationTable)
        return 0;

    for (UINTN i = 0; i < st->NumberOfTableEntries; ++i)
    {
        const EFI_GUID& current = st->ConfigurationTable[i].VendorGuid;

        if (current.Data1 == guid.Data1 &&
            current.Data2 == guid.Data2 &&
            current.Data3 == guid.Data3)
        {
            bool equal = true;

            for (int j = 0; j < 8; ++j)
            {
                if (current.Data4[j] != guid.Data4[j])
                    equal = false;
            }

            if (equal)
                return reinterpret_cast<UINT64>(
                    st->ConfigurationTable[i].VendorTable);
        }
    }

    return 0;
}

extern "C" EFI_STATUS EFIAPI efi_main(
    EFI_HANDLE imageHandle,
    EFI_SYSTEM_TABLE* systemTable)
{
    boot_debug_init(systemTable);
    boot_debug("ARCHIAOS UEFI BOOT\r\n");

    if (!systemTable || !systemTable->BootServices)
        boot_halt();

    EFI_FILE_PROTOCOL* kernelFile = nullptr;
    EFI_STATUS status = open_kernel_file(
        imageHandle, systemTable, &kernelFile);

    if (status != EFI_SUCCESS || !kernelFile)
        boot_halt();

    LoadedKernel kernel{};
    status = load_kernel_elf(
        systemTable, kernelFile, &kernel);

    kernelFile->Close(kernelFile);

    if (status != EFI_SUCCESS)
        boot_halt();

    BootInfo* bootInfo = nullptr;
    status = allocate_boot_info(systemTable, &bootInfo);

    if (status != EFI_SUCCESS)
        boot_halt();

    bootInfo->magic = NOVOS_BOOT_INFO_MAGIC;
    bootInfo->version = NOVOS_BOOT_INFO_VERSION;
    bootInfo->size = sizeof(BootInfo);
    bootInfo->uefi_system_table =
        reinterpret_cast<UINT64>(systemTable);
    bootInfo->bootloader_version = NOVOS_BOOTLOADER_VERSION;
    bootInfo->kernel_image_base = kernel.base;
    bootInfo->kernel_image_size = kernel.size;
    bootInfo->boot_info_address =
        reinterpret_cast<UINT64>(bootInfo);
    bootInfo->boot_info_size = sizeof(BootInfo);

    status = discover_framebuffer(systemTable, bootInfo);

    if (status != EFI_SUCCESS)
        boot_halt();

    static constexpr EFI_GUID acpi20Guid = {
        0x8868e871, 0xe4f1, 0x11d3,
        {0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81}
    };

    static constexpr EFI_GUID acpi10Guid = {
        0xeb9d2d30, 0x2d88, 0x11d3,
        {0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d}
    };

    static constexpr EFI_GUID smbiosGuid = {
        0xeb9d2d31, 0x2d88, 0x11d3,
        {0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d}
    };

    static constexpr EFI_GUID smbios3Guid = {
        0xf2fd1544, 0x9794, 0x4a2c,
        {0x99, 0x2e, 0xe5, 0xbb, 0xcf, 0x20, 0xe3, 0x94}
    };

    bootInfo->acpi_rsdp_address =
        find_table(systemTable, acpi20Guid);

    if (!bootInfo->acpi_rsdp_address)
        bootInfo->acpi_rsdp_address =
            find_table(systemTable, acpi10Guid);

    bootInfo->smbios_address =
        find_table(systemTable, smbiosGuid);

    bootInfo->smbios3_address =
        find_table(systemTable, smbios3Guid);

    boot_debug("boot: BootInfo prepared\r\n");

    FinalMemoryMap memoryMap{};
    status = prepare_memory_map(systemTable, &memoryMap);

    if (status != EFI_SUCCESS)
    {
        boot_debug("boot: prepare_memory_map FAILED: ");
        boot_debug_hex(status);
        boot_debug("\r\n");
        boot_halt();
    }

    boot_debug("boot: Memory map prepared\r\n");

    boot_debug("boot: calling ExitBootServices\r\n");

    status = exit_boot_services(
        imageHandle, systemTable, &memoryMap, bootInfo);

    if (status != EFI_SUCCESS)
    {
        boot_debug("boot: ExitBootServices FAILED: ");
        boot_debug_hex(status);
        boot_debug("\r\n");
        boot_halt();
    }

    boot_debug("boot: ExitBootServices OK\r\n");

    /*
     * No UEFI Boot Service calls are allowed after this point.
     * handoff.S converts Microsoft x64 -> SysV AMD64 and jumps.
     */
    archiaos_x86_64_handoff(kernel.entry, bootInfo);
}
