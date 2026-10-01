#include "boot_debug.h"

static EFI_SYSTEM_TABLE* g_system_table = nullptr;

static void debugcon_ascii(const char* text)
{
    for (UINTN i = 0; text[i]; ++i)
        asm volatile("outb %0,%1" : : "a"(text[i]), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
}

static void output_ascii(const char* text)
{
    debugcon_ascii(text);

    if (!g_system_table ||
        !g_system_table->BootServices ||
        !g_system_table->ConOut)
        return;

    CHAR16 buffer[128];
    UINTN i = 0;

    while (text[i] && i < 127)
    {
        buffer[i] = static_cast<CHAR16>(text[i]);
        ++i;
    }
    buffer[i] = 0;

    g_system_table->ConOut->OutputString(g_system_table->ConOut, buffer);
}

void boot_debug_init(EFI_SYSTEM_TABLE* systemTable)
{
    g_system_table = systemTable;
}

void boot_debug(const char* text)
{
#if NOVOS_BOOT_DEBUG
    output_ascii(text);
#endif
}

void boot_debug_hex(UINT64 value)
{
#if NOVOS_BOOT_DEBUG
    const char* digits = "0123456789ABCDEF";
    char text[19];
    text[0] = '0';
    text[1] = 'x';
    for (int i = 0; i < 16; ++i)
        text[2 + i] = digits[(value >> ((15 - i) * 4)) & 0xF];
    text[18] = 0;
    output_ascii(text);
#else
    (void)value;
#endif
}

[[noreturn]] void boot_halt()
{
    for (;;)
        asm volatile ("cli; hlt");
}
