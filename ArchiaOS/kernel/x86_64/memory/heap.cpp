#include "heap.hpp"
#include "vmm.hpp"
#include "pmm.hpp"
#include "paging.hpp"

struct HeapBlock
{
    u64 base;
    u64 size;
    u64 requested_size;
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
    if (size == 0 ||
        size > UINT64_MAX - (PAGE_SIZE - 1ULL))
        return nullptr;

    HeapBlock* free_block = nullptr;
    for (auto& block : blocks)
    {
        if (!block.used)
        {
            free_block = &block;
            break;
        }
    }
    if (!free_block)
        return nullptr;

    const u64 pages = (size + PAGE_SIZE - 1ULL) / PAGE_SIZE;
    const u64 base = vmm_alloc_pages(pages, false, true, false);
    if (!base) return nullptr;

    *free_block = {base, pages * PAGE_SIZE, size, true};
    used_bytes += size;
    return reinterpret_cast<void*>(base);
}

extern "C" void kfree(void* pointer)
{
    if (!pointer) return;
    const u64 base = reinterpret_cast<u64>(pointer);
    for (auto& block : blocks)
    {
        if (block.used && block.base == base)
        {
            const u64 pages = block.size / PAGE_SIZE;
            if (vmm_free_pages(base, pages))
            {
                block.used = false;
                if (used_bytes >= block.requested_size) used_bytes -= block.requested_size;
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
    const u64 before = heap_used_bytes();
    if (kmalloc(UINT64_MAX) != nullptr ||
        heap_used_bytes() != before)
        for (;;) asm volatile("cli; hlt");

    void* a = kmalloc(1);
    void* b = kmalloc(8193);
    if (!a || !b || a == b)
        for (;;) asm volatile("cli; hlt");
    reinterpret_cast<volatile unsigned char*>(a)[0] = 0x11;
    reinterpret_cast<volatile unsigned char*>(b)[8192] = 0x22;
    if (reinterpret_cast<volatile unsigned char*>(a)[0] != 0x11 ||
        reinterpret_cast<volatile unsigned char*>(b)[8192] != 0x22)
        for (;;) asm volatile("cli; hlt");
    if (heap_used_bytes() != 8194)
        for (;;) asm volatile("cli; hlt");
    kfree(b);
    if (heap_used_bytes() != 1)
        for (;;) asm volatile("cli; hlt");
    kfree(a);
    if (heap_used_bytes() != 0)
        for (;;) asm volatile("cli; hlt");
}
