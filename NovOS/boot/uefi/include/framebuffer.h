#pragma once

#include "uefi.h"
#include "../../../common/boot_info.h"

EFI_STATUS discover_framebuffer(
    EFI_SYSTEM_TABLE* systemTable,
    BootInfo* bootInfo);
