#pragma once

#include "uefi.h"

void boot_debug_init(EFI_SYSTEM_TABLE* systemTable);
void boot_debug(const char* text);
void boot_debug_hex(UINT64 value);
[[noreturn]] void boot_halt();
