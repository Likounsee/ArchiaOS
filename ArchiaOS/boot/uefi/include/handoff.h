#pragma once

#include "uefi.h"
#include "../../../common/boot_info.h"

extern "C" [[noreturn]] void novos_x86_64_handoff(
    UINT64 kernelEntry,
    BootInfo* bootInfo);
