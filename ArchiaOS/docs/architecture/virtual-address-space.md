# ArchiaOS virtual address space

ArchiaOS uses four-level x86_64 paging for the current kernel bootstrap. With four-level paging, the processor translates 48-bit canonical linear addresses; the upper and lower halves are separated by the canonical-address rule. The current HHDM is placed at `0xFFFF800000000000`, which is the start of the upper-half PML4 slot used by the bootstrap address space.

## Current bootstrap layout

| Region | Virtual range | Status |
|---|---|---|
| Identity map | `0x0000000000000000` – `0x0000000FFFFFFFFF` | Temporary bootstrap mapping |
| HHDM | `0xFFFF800000000000` – `0xFFFF80FFFFFFFFFF` | Active direct physical map, 64 GiB |
| Kernel high-half | `0xFFFFFFFF80000000` onward | Reserved layout, not mapped yet |
| Kernel heap | `0xFFFF900000000000` – `0xFFFF900FFFFFFFFF` | VMM-managed virtual allocation window |
| User space | `0x0000000000400000` – `0x00007FFFFFFFF000` | Reserved for future address spaces |

The HHDM is intentionally a direct map: virtual address = `NOVOS_HHDM_BASE + physical address`. The page tables currently reuse the same 2 MiB mappings for the identity and HHDM regions, so no second physical copy of the bootstrap mapping is required.

## Page-table access rule

Before CR3 activation, newly allocated page-table pages are accessed through the identity mapping because the new address space is not active yet. Immediately after CR3 activation, page-table management switches to the HHDM. This is an important prerequisite for eventually removing the identity mapping: code that edits page tables must not itself depend on low identity addresses.

## Current VMM state

The kernel virtual memory manager currently manages the kernel-heap window with fixed metadata slots. It supports page-backed allocations with explicit USER/RW/NX permissions, exact-size frees, and metadata-only reservations. Reservations do not allocate physical pages and can be released with `vmm_release()`; mapped allocations must be released with `vmm_free_pages()`.

This is an implemented foundation, not yet the final general-purpose virtual-address manager: it still uses a fixed region table and is not yet the complete allocator needed for every kernel mapping class.

## Next transition

The next memory stages should be implemented in this order:

1. map the kernel image into the reserved high-half kernel region;
2. keep the HHDM available for physical-memory and page-table access;
3. create a virtual-address allocator for the kernel heap region;
4. introduce explicit kernel stack mappings and guard pages;
5. remove identity mappings only after all post-bootstrap code and data references have migrated to explicit virtual mappings;
6. create per-process address-space roots while preserving the kernel/HHDM portion as appropriate.

No identity-map removal is claimed by this document; the current mapping remains deliberately conservative until those dependencies are migrated.
