#pragma once

#include <stdint.h>
#include <stddef.h>

/*
 * Stable UEFI -> OS handoff contract.
 *
 * ABI rules:
 *   - fixed-width integer fields only;
 *   - no compiler-dependent pointer types in the structure;
 *   - fields are appended for future versions;
 *   - all addresses are physical addresses valid after ExitBootServices();
 *   - the kernel validates magic, version and size before reading data.
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

    uint64_t kernel_image_base;
    uint64_t kernel_image_size;
    uint64_t boot_info_address;
    uint64_t boot_info_size;

    uint64_t pmm_bitmap_base;
    uint64_t pmm_bitmap_size;
};

static constexpr uint32_t BOOT_INFO_MAGIC = 0x4F564F4E;
static constexpr uint32_t BOOT_INFO_VERSION = 3;
static constexpr uint64_t BOOTLOADER_VERSION = 0x00020000ULL;

static_assert(sizeof(BootInfo) == 176, "BootInfo ABI size changed");
static_assert(offsetof(BootInfo, memory_map_address) == 16, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, framebuffer_base) == 48, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, acpi_rsdp_address) == 88, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, uefi_system_table) == 112, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, kernel_image_base) == 128, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, boot_info_address) == 144, "BootInfo ABI offset changed");
static_assert(offsetof(BootInfo, pmm_bitmap_base) == 160, "BootInfo ABI offset changed");
