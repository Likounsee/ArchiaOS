#pragma once

#include "uefi.h"
#include "../../../common/boot_info.h"

struct FinalMemoryMap
{
    void* buffer;
    UINTN capacity;
    UINTN size;
    UINTN descriptorSize;
    UINT32 descriptorVersion;
    UINTN mapKey;
};

EFI_STATUS prepare_memory_map(
    EFI_SYSTEM_TABLE* systemTable,
    FinalMemoryMap* outMap);

EFI_STATUS exit_boot_services(
    EFI_HANDLE imageHandle,
    EFI_SYSTEM_TABLE* systemTable,
    FinalMemoryMap* map,
    BootInfo* bootInfo);
