#pragma once

#include "uefi.h"
#include "elf_loader.h"
#include "memory_map.h"

EFI_STATUS prepare_boot_paging(
    EFI_SYSTEM_TABLE* systemTable,
    const FinalMemoryMap* memoryMap,
    const LoadedKernel* kernel,
    UINT64 framebufferBase,
    UINT64 framebufferSize,
    UINT64* outPml4Physical);
