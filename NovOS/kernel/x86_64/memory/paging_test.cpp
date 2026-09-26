#include "paging.hpp"

static inline void test_char(char c)
{
    asm volatile ("outb %0, %1" : : "a"(c), "Nd"(static_cast<unsigned short>(0xE9)));
}

static void test_str(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        test_char(s[i]);
}

static void test_hex(u64 v)
{
    const char* d = "0123456789ABCDEF";
    test_char('0'); test_char('x');
    for (int i = 0; i < 16; ++i)
        test_char(d[(v >> ((15-i)*4)) & 0xF]);
}

extern "C" void paging_run_tests()
{
    test_str("PAGING TEST START\n");

    if (!paging_initialize())
    {
        test_str("PAGING TEST FAIL: INIT\n");
        for (;;) asm volatile("cli; hlt");
    }

    test_str("PAGING TABLES CREATED\n");

    if (paging_pml4_physical() == 0)
    {
        test_str("PAGING TEST FAIL: PML4\n");
        for (;;) asm volatile("cli; hlt");
    }

    if (paging_translate(0x00100000ULL) != 0x00100000ULL)
    {
        test_str("PAGING TEST FAIL: TRANSLATION\n");
        test_hex(paging_translate(0x00100000ULL));
        test_str("\n");
        for (;;) asm volatile("cli; hlt");
    }

    test_str("PAGING IDENTITY MAP PASS (LOW 4 GiB)\n");

    if (!paging_is_enabled())
    {
        test_str("PAGING TEST FAIL: PAGING DISABLED\n");
        for (;;) asm volatile("cli; hlt");
    }

    test_str("UEFI PAGING ALREADY ENABLED\n");
    test_str("PAGING TEST PASS\n");
}
