#pragma once

/*
 * BootInfo Structure
 * 
 * Passed from bootloader (UEFI) to kernel (x86_64).
 * Contains essential information needed for kernel initialization.
 * 
 * No UEFI dependencies in kernel code — only basic types.
 * Completely freestanding — no libc, no cstdint, no OS headers.
 */

/* Freestanding integer types */
typedef unsigned char      u8;
typedef unsigned short     u16;
typedef unsigned int       u32;
typedef unsigned long long u64;

#define NOVOS_BOOT_INFO_MAGIC   0x4E4F564Fu  /* "NOVO" */
#define NOVOS_BOOT_INFO_VERSION 2u

struct BootInfo
{
    /* Magic signature for validation */
    u32 magic;
    
    /* Structure version */
    u32 version;
    
    /* UEFI Memory Map Information */
    /* Physical address of EFI_MEMORY_DESCRIPTOR array */
    u64 memory_map_address;
    
    /* Total size in bytes */
    u64 memory_map_size;
    
    /* Size of each descriptor in the array */
    u32 memory_descriptor_size;
    
    /* Number of descriptors in the array */
    u32 memory_descriptor_count;
    
    /* Physical address of the ACPI RSDP supplied by UEFI */
    u64 acpi_rsdp_address;
    
    /* Reserved for future use */
    u8 reserved[128];
};

/* Static assertion: BootInfo must fit in a single page */
static_assert(sizeof(struct BootInfo) <= 4096, "BootInfo must fit in a single page");
