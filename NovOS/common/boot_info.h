#pragma once

#include <cstdint>

/*
 * BootInfo Structure
 * 
 * Passed from bootloader (UEFI) to kernel (x86_64).
 * Contains essential information needed for kernel initialization.
 * 
 * No UEFI dependencies in kernel code — only basic types.
 */

#define NOVOS_BOOT_INFO_MAGIC   0x4E4F564F  // "NOVO" in hex
#define NOVOS_BOOT_INFO_VERSION 1

struct BootInfo
{
    /* Magic signature for validation */
    uint32_t magic;
    
    /* Structure version */
    uint32_t version;
    
    /* UEFI Memory Map Information */
    /* (This is the raw buffer from GetMemoryMap) */
    uint64_t memory_map_address;    // Physical address of EFI_MEMORY_DESCRIPTOR array
    uint64_t memory_map_size;       // Total size in bytes
    uint32_t memory_descriptor_size; // Size of each descriptor
    uint32_t memory_descriptor_count; // Number of descriptors
    
    /* Reserved for future use */
    uint8_t reserved[128];
};

static_assert(sizeof(BootInfo) <= 4096, "BootInfo must fit in a single page");
