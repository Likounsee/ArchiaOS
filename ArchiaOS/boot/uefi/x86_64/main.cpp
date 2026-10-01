#include "uefi.h"
#include "boot_debug.h"
#include "filesystem.h"
#include "elf_loader.h"
#include "framebuffer.h"
#include "memory_map.h"
#include "handoff.h"
#include "boot_paging.h"
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


static EFI_STATUS allocate_pmm_bitmap(
    EFI_SYSTEM_TABLE* st,
    FinalMemoryMap* map,
    BootInfo* bootInfo)
{
    auto getMemoryMap = reinterpret_cast<EFI_GET_MEMORY_MAP>(st->BootServices->GetMemoryMap);
    auto allocatePages = reinterpret_cast<EFI_ALLOCATE_PAGES>(st->BootServices->AllocatePages);
    auto setMem = reinterpret_cast<EFI_SET_MEM>(st->BootServices->SetMem);
    if (!getMemoryMap || !allocatePages || !setMem || !map || !map->buffer || !bootInfo)
        return EFI_INVALID_PARAMETER;

    UINTN size = map->capacity;
    UINTN key = 0;
    UINTN descriptorSize = map->descriptorSize;
    UINT32 version = map->descriptorVersion;
    EFI_STATUS status = getMemoryMap(&size, reinterpret_cast<EFI_MEMORY_DESCRIPTOR*>(map->buffer), &key, &descriptorSize, &version);
    if (status != EFI_SUCCESS || descriptorSize < sizeof(EFI_MEMORY_DESCRIPTOR) || size % descriptorSize != 0)
        return status != EFI_SUCCESS ? status : EFI_INVALID_PARAMETER;

    constexpr UINT64 maxPhysical = 0x8000000000ULL;
    UINT64 highest = 0;
    for (UINTN offset = 0; offset < size; offset += descriptorSize)
    {
        auto* d = reinterpret_cast<EFI_MEMORY_DESCRIPTOR*>(reinterpret_cast<UINT8*>(map->buffer) + offset);
        if (d->Type != EfiLoaderCode && d->Type != EfiLoaderData &&
            d->Type != EfiBootServicesCode && d->Type != EfiBootServicesData &&
            d->Type != EfiConventionalMemory)
            continue;
        if (d->PhysicalStart >= maxPhysical)
            continue;
        UINT64 pages = d->NumberOfPages;
        UINT64 available = (maxPhysical - d->PhysicalStart) / 4096ULL;
        if (pages > available) pages = available;
        UINT64 end = d->PhysicalStart + pages * 4096ULL;
        if (end > highest) highest = end;
    }
    if (highest < 0x200000ULL)
        return EFI_OUT_OF_RESOURCES;

    const UINT64 frames = (highest + 4095ULL) / 4096ULL;
    const UINT64 words = (frames + 63ULL) / 64ULL;
    const UINT64 bitmapBytes = words * sizeof(UINT64);
    const UINT64 totalBytes = bitmapBytes * 2ULL;
    const UINT64 pages = (totalBytes + 4095ULL) / 4096ULL;
    if (pages == 0 || pages > 0xFFFFFFFFULL)
        return EFI_OUT_OF_RESOURCES;

    EFI_PHYSICAL_ADDRESS address = 0;
    status = allocatePages(EFI_ALLOCATE_ANY_PAGES, EfiLoaderData, static_cast<UINTN>(pages), &address);
    if (status != EFI_SUCCESS)
        return status;
    setMem(reinterpret_cast<void*>(address), static_cast<UINTN>(pages * 4096ULL), 0);
    bootInfo->pmm_bitmap_base = address;
    bootInfo->pmm_bitmap_size = pages * 4096ULL;
    map->size = size;
    map->descriptorSize = descriptorSize;
    map->descriptorVersion = version;
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

    status = allocate_pmm_bitmap(systemTable, &memoryMap, bootInfo);
    if (status != EFI_SUCCESS)
        boot_halt();
    boot_debug("boot: PMM bitmap allocated\r\n");

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

    /*
     * ExitBootServices() has succeeded. From this point onward the UEFI
     * console and all other Boot Services are unavailable; do not call
     * boot_debug or any EFI service before the kernel handoff.
     * handoff.S converts Microsoft x64 -> SysV AMD64 and jumps.
     */
    archiaos_x86_64_handoff(kernel.entry, bootInfo, bootPml4Physical);
}
