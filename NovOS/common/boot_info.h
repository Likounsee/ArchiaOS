#pragma once

#include <stdint.h>

/*
 * Stable UEFI -> NovOS handoff contract.
 *
 * ABI rules:
 *   - fixed-width integer fields only;
 *   - no pointers or compiler-dependent types in the structure;
 *   - fields are appended for future versions;
 *   - kernel validates magic, version and size before reading optional data.
 *
 * The addresses are physical addresses valid after ExitBootServices().
 */
struct BootInfo
{
    uint32_t magic;                    /* NOVOS_BOOT_INFO_MAGIC */
    uint32_t version;                  /* structure format version */
    uint32_t size;                     /* bytes known by the producer */
    uint32_t reserved0;

    uint64_t memory_map_address;       /* EFI_MEMORY_DESCRIPTOR array */
    uint64_t memory_map_size;          /* bytes */
    uint32_t memory_descriptor_size;   /* firmware-provided stride */
    uint32_t memory_descriptor_version;
    uint64_t memory_descriptor_count;

    uint64_t framebuffer_base;         /* physical framebuffer address */
    uint64_t framebuffer_size;         /* bytes */
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint32_t framebuffer_pitch;        /* bytes per scanline */
    uint32_t framebuffer_bpp;
    uint32_t framebuffer_pixel_format; /* UEFI GOP pixel format */
    uint32_t reserved1;

    uint64_t acpi_rsdp_address;        /* ACPI 2.0 RSDP, or ACPI 1.0 RSDP */
    uint64_t smbios_address;           /* SMBIOS 2 entry point, or 0 */
    uint64_t smbios3_address;          /* SMBIOS 3 entry point, or 0 */

    uint64_t uefi_system_table;        /* only fields valid after EBS may be used */
    uint64_t bootloader_version;
};

static constexpr uint32_t NOVOS_BOOT_INFO_MAGIC = 0x4F564F4E;
static constexpr uint32_t NOVOS_BOOT_INFO_VERSION = 1;
static constexpr uint64_t NOVOS_BOOTLOADER_VERSION = 0x00010000ULL;

static_assert(sizeof(BootInfo) == 120, "BootInfo ABI size changed");
static_assert(offsetof(BootInfo, memory_map_address) == 16, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, framebuffer_base) == 56, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, acpi_rsdp_address) == 96, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, uefi_system_table) == 120 - 16, "BootInfo ABI offset changed");
