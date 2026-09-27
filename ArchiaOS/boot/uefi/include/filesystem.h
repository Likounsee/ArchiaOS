#pragma once

#include "uefi.h"

EFI_STATUS open_kernel_file(
    EFI_HANDLE imageHandle,
    EFI_SYSTEM_TABLE* systemTable,
    EFI_FILE_PROTOCOL** outFile);
