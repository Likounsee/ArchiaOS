#pragma once

#include <stdint.h>

/*
 * Stable boot contract between the NovOS UEFI loader and the kernel.
 *
 * The structure is versioned by (magic, version, size). New fields are
 * appended; older kernels can safely ignore fields beyond their known size.
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

static constexpr uint32_t NOVOS_BOOT_INFO_MAGIC = 0x4F564F4E; /* "NOVO" */
static constexpr uint32_t NOVOS_BOOT_INFO_VERSION = 1;
static constexpr uint64_t NOVOS_BOOTLOADER_VERSION = 0x00010000ULL;
