#pragma once

#include <stdint.h>
#include <stddef.h>

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
    uint32_t magic;
    uint32_t version;
    uint32_t size;
    uint32_t reserved0;

    uint64_t memory_map_address;
    uint64_t memory_map_size;
    uint32_t memory_descriptor_size;
    uint32_t memory_descriptor_version;
    uint64_t memory_descriptor_count;

    uint64_t framebuffer_base;
    uint64_t framebuffer_size;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_bpp;
    uint32_t framebuffer_pixel_format;
    uint32_t reserved1;

    uint64_t acpi_rsdp_address;
    uint64_t smbios_address;
    uint64_t smbios3_address;

    uint64_t uefi_system_table;
    uint64_t bootloader_version;
};

static constexpr uint32_t NOVOS_BOOT_INFO_MAGIC = 0x4F564F4E;
static constexpr uint32_t NOVOS_BOOT_INFO_VERSION = 1;
static constexpr uint64_t NOVOS_BOOTLOADER_VERSION = 0x00010000ULL;

static_assert(sizeof(BootInfo) == 128, "BootInfo ABI size changed");
static_assert(offsetof(BootInfo, memory_map_address) == 16, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, framebuffer_base) == 48, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, acpi_rsdp_address) == 88, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, uefi_system_table) == 112, "BootInfo ABI offset changed");
