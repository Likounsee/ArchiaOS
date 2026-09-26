static inline void debug_char(char c)
{
    asm volatile (
        "outb %0, %1"
        :
        : "a"(c),
          "Nd"(static_cast<unsigned short>(0xE9))
    );
}

static void debug_str(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        debug_char(s[i]);
}

extern "C" void kernel_main()
{
    debug_str("NOVOS KERNEL STARTED\n");

    volatile unsigned short* vga =
        reinterpret_cast<volatile unsigned short*>(0xB8000);

    const char* text = "NOVOS KERNEL STARTED";

    for (int i = 0; text[i] != '\0'; ++i)
    {
        vga[i] = static_cast<unsigned short>(0x0700 | text[i]);
    }

    for (;;)
        asm volatile ("hlt");
}
