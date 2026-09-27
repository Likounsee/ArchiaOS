#include "boot_debug.h"

static EFI_SYSTEM_TABLE* g_system_table = nullptr;

static void output_ascii(const char* text)
{
    if (!g_system_table || !g_system_table->ConOut)
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
