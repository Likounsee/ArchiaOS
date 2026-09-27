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

    if (descriptorSize > (~static_cast<UINTN>(0) / 128))
        return EFI_OUT_OF_RESOURCES;

    const UINTN extra = descriptorSize * 128;
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

static EFI_STATUS capture_final_memory_map(
    EFI_SYSTEM_TABLE* st,
    FinalMemoryMap* map,
    UINTN* outKey)
{
    auto getMemoryMap =
        reinterpret_cast<EFI_GET_MEMORY_MAP>(
            st->BootServices->GetMemoryMap);

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

    if (status != EFI_SUCCESS || descriptorSize < sizeof(EFI_MEMORY_DESCRIPTOR) ||
        descriptorSize > 0xFFFFFFFFULL)
        return status != EFI_SUCCESS ? status : EFI_INVALID_PARAMETER;

    map->size = size;
    map->mapKey = key;
    map->descriptorSize = descriptorSize;
    map->descriptorVersion = descriptorVersion;

    if (descriptorSize < sizeof(EFI_MEMORY_DESCRIPTOR) ||
        descriptorSize > 0xFFFFFFFFULL ||
        size == 0 || size % descriptorSize != 0)
        return EFI_INVALID_PARAMETER;

    *outKey = key;
    return EFI_SUCCESS;
}

EFI_STATUS exit_boot_services(
    EFI_HANDLE imageHandle,
    EFI_SYSTEM_TABLE* st,
    FinalMemoryMap* map,
    BootInfo* info)
{
    if (!st || !st->BootServices || !map || !map->buffer || !info)
        return EFI_INVALID_PARAMETER;

    auto exitBootServices =
        reinterpret_cast<EFI_EXIT_BOOT_SERVICES>(
            st->BootServices->ExitBootServices);

    UINTN key = 0;
    EFI_STATUS status = capture_final_memory_map(st, map, &key);
    if (status != EFI_SUCCESS)
        return status;

    info->memory_map_address =
        reinterpret_cast<UINT64>(map->buffer);
    info->memory_map_size = map->size;
    info->memory_descriptor_size = static_cast<UINT32>(map->descriptorSize);
    info->memory_descriptor_version = map->descriptorVersion;
    info->memory_descriptor_count = map->size / map->descriptorSize;

    status = exitBootServices(imageHandle, key);

    if (status == EFI_SUCCESS)
        return EFI_SUCCESS;

    if (status != EFI_INVALID_PARAMETER)
        return status;

    /*
     * UEFI requires the map to be reacquired after EFI_INVALID_PARAMETER.
     * This retry performs only GetMemoryMap followed immediately by
     * ExitBootServices. No allocation, free, console, filesystem, GOP,
     * or other Boot Service occurs between these calls.
     */
    status = capture_final_memory_map(st, map, &key);
    if (status != EFI_SUCCESS)
        return status;

    info->memory_map_address =
        reinterpret_cast<UINT64>(map->buffer);
    info->memory_map_size = map->size;
    info->memory_descriptor_size = static_cast<UINT32>(map->descriptorSize);
    info->memory_descriptor_version = map->descriptorVersion;
    info->memory_descriptor_count = map->size / map->descriptorSize;

    return exitBootServices(imageHandle, key);
}
