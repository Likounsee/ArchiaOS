# CPU security activation

## Scope

ArchiaOS now separates CPU capability detection from CPU security activation.

Detection answers whether the processor advertises a feature through CPUID. Activation changes architectural control state so the kernel actually opts into protections.

The current bootstrap security stage activates only protections that are safe with the current kernel architecture:

- CR0.WP — supervisor write protection is enabled.
- EFER.NXE — enabled when CPUID reports NX support.
- CR4.UMIP — enabled when CPUID reports UMIP support.

SMEP and SMAP are detected but intentionally remain deferred.

## Why SMEP/SMAP are deferred

The current bootstrap address space is a supervisor-only 2 MiB identity map. ArchiaOS does not have user-mode address spaces yet.

Enabling SMEP/SMAP becomes useful when the virtual-memory layer can create and audit user (U/S=1) mappings and the page-fault path can distinguish protection faults. Until then, enabling them would not exercise the intended isolation boundary.

The next memory-protection stage will therefore introduce:

1. page-table permission flags;
2. 4 KiB mappings where fine-grained permissions are required;
3. kernel/user address-space separation;
4. a page-fault handler that decodes the page-fault error code;
5. NX/RO/W^X mappings;
6. SMEP/SMAP activation and controlled tests.

## Current activation order

The kernel currently follows:

1. CPUID/features/topology detection;
2. GDT/TSS/IDT;
3. PMM;
4. bootstrap paging and CR3 activation;
5. CPU security activation;
6. exception/IRQ tests.

This order keeps CR0/EFER/CR4 changes inside the kernel and makes the activation state observable through the debug console.

## Architectural references

The implementation follows the x86-64 architectural model described by the Intel 64 and IA-32 Architectures Software Developer's Manual, Volume 3, and the AMD64 Architecture Programmer's Manual, Volume 2, System Programming.

In particular:

- AMD documents NX as requiring EFER.NXE and the CPUID NX capability.
- AMD documents CR0.WP as the control for supervisor writes to read-only pages.
- Intel documents UMIP as CR4 bit 11 and SMEP/SMAP as CR4 bits 20/21.

ArchiaOS deliberately treats these as architectural controls, not as vendor-specific shortcuts.

## Memory-protection bootstrap implemented

The paging layer now supports 4 KiB leaves in addition to the existing 2 MiB bootstrap leaves. When a 2 MiB PDE must be hardened, ArchiaOS splits it into a 512-entry 4 KiB page table while preserving the identity mapping.

Each 4 KiB mapping can independently select:

- supervisor/user (U/S);
- writable/read-only (R/W);
- executable/NX.

The mapping path uses INVLPG after changing a leaf.

The page-fault path now decodes CR2 and the architectural error-code bits. A controlled kernel test deliberately unmapped a PMM-owned page, accessed it, verified the #PF, and redirected only that armed test to a recovery label. This proves the exception path is live without leaving the kernel in a fault loop.

SMEP/SMAP are still deferred until user address spaces exist.
