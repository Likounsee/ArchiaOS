#include "../../../common/boot_info.h"

using UINT32 = unsigned int;
using UINT64 = unsigned long long;

static inline void debug_char(char c)
{
    asm volatile(
        "outb %0,%1"
        :
        : "a"(c), "Nd"(static_cast<unsigned short>(0xE9))
        : "memory");
}

static void debug_str(const char* s)
{
    for (int i = 0; s[i]; ++i)
        debug_char(s[i]);
}

static void halt()
{
    for (;;)
        asm volatile("cli; hlt");
}

static void put_pixel(
    volatile UINT32* fb,
    UINT32 pitchPixels,
    UINT32 x,
    UINT32 y,
    UINT32 pixel)
{
    fb[static_cast<UINT64>(y) * pitchPixels + x] = pixel;
}

static void fill_rect(
    volatile UINT32* fb,
    UINT32 pitchPixels,
    UINT32 width,
    UINT32 height,
    UINT32 x,
    UINT32 y,
    UINT32 w,
    UINT32 h,
    UINT32 pixel)
{
    if (x >= width || y >= height)
        return;

    if (w > width - x)
        w = width - x;

    if (h > height - y)
        h = height - y;

    for (UINT32 py = 0; py < h; ++py)
        for (UINT32 px = 0; px < w; ++px)
            put_pixel(fb, pitchPixels, x + px, y + py, pixel);
}

extern "C" void kernel_main(BootInfo* bootInfo)
{
    debug_str("ARCHIAOS KERNEL STARTED\n");
    debug_str("Architecture: x86_64\n");

    if (!bootInfo)
    {
        debug_str("[KERNEL] BootInfo NULL\n");
        halt();
    }

    if (bootInfo->magic != NOVOS_BOOT_INFO_MAGIC)
    {
        debug_str("[KERNEL] BootInfo BAD MAGIC\n");
        halt();
    }

    if (bootInfo->version != NOVOS_BOOT_INFO_VERSION ||
        bootInfo->size < sizeof(BootInfo))
    {
        debug_str("[KERNEL] BootInfo BAD VERSION/SIZE\n");
        halt();
    }

    if (!bootInfo->kernel_image_base ||
        !bootInfo->kernel_image_size ||
        !bootInfo->boot_info_address ||
        bootInfo->boot_info_size < sizeof(BootInfo))
    {
        debug_str("[KERNEL] BootInfo ABI addresses BAD\n");
        halt();
    }

    if (!bootInfo->memory_map_address ||
        !bootInfo->memory_map_size ||
        bootInfo->memory_descriptor_size < 40 ||
        bootInfo->memory_map_size %
            bootInfo->memory_descriptor_size != 0)
    {
        debug_str("[KERNEL] Memory map BAD\n");
        halt();
    }

    if (!bootInfo->framebuffer_base ||
        !bootInfo->framebuffer_width ||
        !bootInfo->framebuffer_height)
    {
        debug_str("[KERNEL] Framebuffer BAD\n");
        halt();
    }

    const UINT64 minimumPitch =
        static_cast<UINT64>(bootInfo->framebuffer_width) * 4ULL;

    if (static_cast<UINT64>(bootInfo->framebuffer_pitch) < minimumPitch)
    {
        debug_str("[KERNEL] Framebuffer PITCH BAD\n");
        halt();
    }

    const UINT64 requiredFramebufferBytes =
        static_cast<UINT64>(bootInfo->framebuffer_pitch) *
        static_cast<UINT64>(bootInfo->framebuffer_height);

    if (requiredFramebufferBytes > bootInfo->framebuffer_size)
    {
        debug_str("[KERNEL] Framebuffer SIZE BAD\n");
        halt();
    }

    debug_str("BootInfo: OK\n");
    debug_str("Memory Map: OK\n");
    debug_str("Framebuffer: OK\n");

    volatile UINT32* fb =
        reinterpret_cast<volatile UINT32*>(bootInfo->framebuffer_base);

    const UINT32 pitchPixels =
        bootInfo->framebuffer_pitch / 4;

    const UINT32 background =
        bootInfo->framebuffer_pixel_format == 0
            ? 0x00101820U
            : 0x00201810U;

    fill_rect(
        fb,
        pitchPixels,
        bootInfo->framebuffer_width,
        bootInfo->framebuffer_height,
        0, 0,
        bootInfo->framebuffer_width,
        bootInfo->framebuffer_height,
        background);

    fill_rect(
        fb,
        pitchPixels,
        bootInfo->framebuffer_width,
        bootInfo->framebuffer_height,
        48, 48, 640, 96,
        0x00FFFFFFU);

    fill_rect(
        fb,
        pitchPixels,
        bootInfo->framebuffer_width,
        bootInfo->framebuffer_height,
        60, 60, 616, 72,
        background);

    halt();
}
