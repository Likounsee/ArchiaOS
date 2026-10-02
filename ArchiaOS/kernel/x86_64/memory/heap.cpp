#include "heap.hpp"
#include "vmm.hpp"
#include "pmm.hpp"
#include "paging.hpp"

struct HeapBlock
{
    u64 base;
    u64 size;
    bool used;
};

static constexpr unsigned int HEAP_MAX_BLOCKS = 1024;
static HeapBlock blocks[HEAP_MAX_BLOCKS];
static u64 used_bytes = 0;

extern "C" bool heap_initialize()
{
    for (auto& block : blocks) block = {};
    used_bytes = 0;
    return vmm_initialize();
}

extern "C" void* kmalloc(u64 size)
{
    if (size == 0) return nullptr;
    const u64 pages = (size + NOVOS_PAGE_SIZE - 1ULL) / NOVOS_PAGE_SIZE;
    const u64 base = vmm_alloc_pages(pages, false, true, false);
    if (!base) return nullptr;
    for (auto& block : blocks)
    {
        if (!block.used)
        {
            block = {base, pages * NOVOS_PAGE_SIZE, true};
            used_bytes += size;
            return reinterpret_cast<void*>(base);
        }
    }
    vmm_free_pages(base, pages);
    return nullptr;
}

extern "C" void kfree(void* pointer)
{
    if (!pointer) return;
    const u64 base = reinterpret_cast<u64>(pointer);
    for (auto& block : blocks)
    {
        if (block.used && block.base == base)
        {
            const u64 pages = block.size / NOVOS_PAGE_SIZE;
            if (vmm_free_pages(base, pages))
            {
                block.used = false;
                if (used_bytes >= block.size) used_bytes -= block.size;
                else used_bytes = 0;
            }
            return;
        }
    }
}

extern "C" u64 heap_used_bytes()
{
    return used_bytes;
}

extern "C" void heap_run_tests()
{
    void* a = kmalloc(1);
    void* b = kmalloc(8193);
    if (!a || !b || a == b)
        for (;;) asm volatile("cli; hlt");
    reinterpret_cast<volatile unsigned char*>(a)[0] = 0x11;
    reinterpret_cast<volatile unsigned char*>(b)[8192] = 0x22;
    if (reinterpret_cast<volatile unsigned char*>(a)[0] != 0x11 ||
        reinterpret_cast<volatile unsigned char*>(b)[8192] != 0x22)
        for (;;) asm volatile("cli; hlt");
    kfree(a);
    kfree(b);
    if (heap_used_bytes() != 0)
        for (;;) asm volatile("cli; hlt");
}
