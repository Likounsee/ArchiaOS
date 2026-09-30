# ArchiaOS CPU compatibility policy

## Goal

ArchiaOS Standard targets a broad modern x86-64 hardware range without making optional instruction-set extensions a hidden requirement of the kernel.

Current policy:

- Architecture: x86-64.
- Standard target: SSE4.1 present.
- Light target (future): older x86-64 processors below SSE4.1 may be supported with a reduced compatibility/security profile.
- Optional optimizations: SSE4.2, AVX, AVX2, AVX-512 and other extensions are detected at runtime and may be used by specialized code paths later.
- Security capabilities are detected independently from SIMD/instruction-set extensions.

SSE4.1 is a distribution/compatibility target, not a statement that every kernel subsystem must execute SSE4.1 instructions.

The kernel is currently compiled with SSE disabled. This is intentional until ArchiaOS has proper SIMD/XSAVE context ownership and context-switching support. Detecting SSE4.1 today must not cause the kernel to execute SIMD instructions before that infrastructure exists.

## Runtime detection

The kernel uses CPUID to discover vendor, processor identification, instruction-set capabilities, CPU security-related capabilities, and topology information.

The Standard/Light classification is currently:

- STANDARD: SSE4.1 is reported by CPUID.
- LIGHT: SSE4.1 is not reported.

The kernel does not halt on a Light result yet. This allows compatibility testing while the future Light distribution is being designed.

## Instruction-set policy

ArchiaOS must never execute an optional instruction merely because the compiler knows about it.

Future optimized implementations should provide a generic implementation, detect the required feature with CPUID, dispatch to the optimized implementation only when supported, and keep the generic implementation available.

This is especially important for AVX/AVX-512 because hardware support and OS-managed extended register state are separate concerns.

## Security policy

CPU hardening capabilities are independent fields: NX, SMEP, SMAP, PCID, INVPCID, x2APIC, UMIP, FSGSBASE, invariant TSC and other capabilities as the subsystem grows.

The current report exposes the capabilities but does not yet claim that every protection is fully enabled. Enabling a protection is a separate kernel implementation task.

## RAM policy

DDR3 is the minimum official platform target for the current Standard roadmap.

The memory manager does not need to identify DDR3/DDR4/DDR5 directly. UEFI supplies a physical memory map; the kernel manages usable physical ranges.

Consequently, the memory subsystem should remain generation-agnostic and should not contain branches such as if DDR4 or if DDR5.

DDR2 is not an official Standard target, but the kernel should avoid artificially rejecting a platform solely because its RAM generation is older if firmware and the rest of the platform can boot it.

## Storage policy

Storage is layered by protocol rather than physical form factor:

- SATA/AHCI: HDDs, SATA SSDs, mSATA and other SATA-form-factor devices;
- NVMe: PCIe-based NVMe devices;
- PCI/PCIe: common discovery/resource layer underneath device drivers.

mSATA is therefore not a separate storage protocol in the kernel architecture.

## Intel / AMD policy

Common x86-64 behaviour belongs in generic code.

Vendor-specific code is allowed only where the architectural interface actually differs. CPU topology is an example: Intel provides topology leaves such as CPUID.0BH/1FH, while AMD provides CPUID.8000001EH and related topology/cache leaves.

The future scheduler will consume a common topology model rather than hard-coding an Intel-only or AMD-only scheduler.

## Official references

- Intel Instruction Set Extensions Programming Reference: https://www.intel.com/content/dam/develop/external/us/en/documents/architecture-instruction-set-extensions-programming-reference-737410.pdf
- Intel CPUID enumeration: https://www.intel.com/content/www/us/en/developer/articles/technical/software-security-guidance/technical-documentation/cpuid-enumeration-and-architectural-msrs.html
- AMD64 Architecture Programmer's Manual: https://docs.amd.com/api/khub/documents/68GKiN0gMEd6bMddsmhPwg/content
- QEMU CPU model documentation: https://www.qemu.org/docs/master/system/qemu-cpu-models.html
