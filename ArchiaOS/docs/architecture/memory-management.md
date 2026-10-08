# ArchiaOS memory-management foundation

## Current paging model

ArchiaOS currently boots with four-level x86_64 paging and keeps two aliases of the same bootstrap page tables:

- low identity map: virtual address equals physical address for the first 64 GiB;
- HHDM/direct map: `0xFFFF800000000000 + physical_address`.

The HHDM is attached at PML4 index 256 and reuses the same PDPT/PD structures as the identity map. This deliberately avoids maintaining two independent copies of the physical-memory mapping during the bootstrap phase.

The HHDM base is 48-bit canonical for four-level paging. x86-64 requires linear addresses to be canonical when LAM is not being used; ArchiaOS does not enable LAM as part of this bootstrap. The four-level paging model therefore keeps the HHDM in the upper canonical half.

## Physical memory range

The PMM and bootstrap mappings now use the same 64 GiB physical-address ceiling:

`0x0000000000000000 .. 0x0000000FFFFFFFFF`.

The PMM bitmap covers every 4 KiB frame in that range. Firmware-reserved pages remain unavailable; only reclaimable UEFI memory types are released to the allocator.

## HHDM API

`paging_physical_to_virtual()` converts a managed physical address to its stable HHDM address.

`paging_virtual_to_physical()` validates that an address belongs to the HHDM range and resolves it through the active page tables.

These APIs are the foundation for later page-table manipulation without requiring physical addresses to be directly dereferenced as virtual pointers.

## 4 KiB permissions

The existing 4 KiB mapping layer can split a 2 MiB bootstrap leaf into a 512-entry page table. Individual mappings support:

- Present;
- supervisor/user;
- read-only/read-write;
- executable/NX when the CPU advertises NX;
- `INVLPG` after a mapping change.

A controlled non-present page fault is part of the QEMU smoke test.

## Next memory stages

The intended sequence is:

1. move page-table and early allocator accesses onto the HHDM;
2. introduce explicit kernel virtual-address layout;
3. establish dedicated kernel stacks and guard pages;
4. track page-table ownership and mapping lifetimes;
5. build kernel heap allocation on top of PMM + virtual mappings;
6. create isolated user address spaces with USER/RW/NX mappings, a guarded user stack, and ELF loading;
7. enforce ring-3 syscall frame, code, and stack validation;
8. enable SMEP/SMAP now that kernel/user mappings are established and audited;
9. add PCID/INVPCID where the detected CPU and address-space design permit it.

The HHDM is therefore an architectural foundation. User address spaces and the kernel VMM are implemented on top of it, while the virtual-memory subsystem is still not a complete general-purpose allocator for every future mapping class.

## Validation

CI validates HHDM translation at several physical addresses, including the last managed 4 KiB page, and checks both directions of the conversion API. The same CPU-model QEMU matrix used for the existing paging/security smoke tests must also reach:

`PAGING HHDM 64GiB PASS`.
