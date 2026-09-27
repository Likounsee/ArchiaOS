#include "memory_map.h"

static bool add_overflow(UINTN a, UINTN b, UINTN* out)
{
    if (b > ~a)
        return true;
    *out = a + b;
    return false;
}

EFI_STATUS prepare_memory_map(
    EFI_SYSTEM_TABLE* st,
    FinalMemoryMap* out)
{
    if (!st || !st->BootServices || !out)
        return EFI_INVALID_PARAMETER;

    auto getMemoryMap =
        reinterpret_cast<EFI_GET_MEMORY_MAP>(
            st->BootServices->GetMemoryMap);
    auto allocatePool =
        reinterpret_cast<EFI_ALLOCATE_POOL>(
            st->BootServices->AllocatePool);

    UINTN required = 0;
    UINTN descriptorSize = 0;
    UINT32 version = 0;
    UINTN key = 0;

    EFI_STATUS status = getMemoryMap(
        &required, nullptr, &key, &descriptorSize, &version);

    if (status != EFI_BUFFER_TOO_SMALL || descriptorSize == 0)
        return status;

    UINTN extra;
    if (descriptorSize > (~static_cast<UINTN>(0) / 64))
        return EFI_OUT_OF_RESOURCES;

    extra = descriptorSize * 64;
    UINTN capacity;
    if (add_overflow(required, extra, &capacity))
        return EFI_OUT_OF_RESOURCES;

    void* buffer = nullptr;
    status = allocatePool(
        EfiLoaderData,
        capacity,
        &buffer);

    if (status != EFI_SUCCESS)
        return status;

    out->buffer = buffer;
    out->capacity = capacity;
    out->size = 0;
    out->descriptorSize = descriptorSize;
    out->descriptorVersion = version;
    out->mapKey = 0;

    return EFI_SUCCESS;
}

EFI_STATUS exit_boot_services(
    EFI_HANDLE imageHandle,
    EFI_SYSTEM_TABLE* st,
    FinalMemoryMap* map,
    BootInfo* info)
{
    auto getMemoryMap =
        reinterpret_cast<EFI_GET_MEMORY_MAP>(
            st->BootServices->GetMemoryMap);
    auto exitBootServices =
        reinterpret_cast<EFI_EXIT_BOOT_SERVICES>(
            st->BootServices->ExitBootServices);

    UINTN size = map->capacity;
    UINTN key = 0;
    UINTN descriptorSize = map->descriptorSize;
    UINT32 descriptorVersion = map->descriptorVersion;

    EFI_STATUS status = getMemoryMap(
        &size,
        reinterpret_cast<EFI_MEMORY_DESCRIPTOR*>(map->buffer),
        &key,
        &descriptorSize,
        &descriptorVersion);

    if (status != EFI_SUCCESS)
        return status;

    info->memory_map_address =
        reinterpret_cast<UINT64>(map->buffer);
    info->memory_map_size = size;
    info->memory_descriptor_size = descriptorSize;
    info->memory_descriptor_version = descriptorVersion;
    info->memory_descriptor_count = size / descriptorSize;

    status = exitBootServices(imageHandle, key);

    if (status == EFI_SUCCESS)
        return EFI_SUCCESS;

    if (status != EFI_INVALID_PARAMETER)
        return status;

    /*
     * The retry is deliberately tiny: GetMemoryMap() immediately followed
     * by ExitBootServices(). No allocation, free, console, filesystem or
     * other Boot Service is permitted in this window.
     */
    size = map->capacity;
    status = getMemoryMap(
        &size,
        reinterpret_cast<EFI_MEMORY_DESCRIPTOR*>(map->buffer),
        &key,
        &descriptorSize,
        &descriptorVersion);

    if (status != EFI_SUCCESS)
        return status;

    info->memory_map_address =
        reinterpret_cast<UINT64>(map->buffer);
    info->memory_map_size = size;
    info->memory_descriptor_size = descriptorSize;
    info->memory_descriptor_version = descriptorVersion;
    info->memory_descriptor_count = size / descriptorSize;

    return exitBootServices(imageHandle, key);
}
