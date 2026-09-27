#pragma once

#include "uefi.h"

struct LoadedKernel
{
    UINT64 entry;
    UINT64 base;
    UINT64 size;
};

EFI_STATUS load_kernel_elf(
    EFI_SYSTEM_TABLE* systemTable,
    EFI_FILE_PROTOCOL* file,
    LoadedKernel* outKernel);
