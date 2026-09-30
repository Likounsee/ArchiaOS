# CPU compatibility validation

ArchiaOS uses two complementary validation methods.

## 1. Real CPUID execution

The kernel executes CPUID inside the guest and prints vendor, family/model/stepping, logical processors, cores per package, threads per core, APIC ID, SIMD features and security-related CPU capabilities.

CI/runtime markers:

- CPU: FEATURES DETECTED OK
- CPU: SECURITY FEATURES DETECTED OK
- CPU: TOPOLOGY DETECTED OK

## 2. QEMU CPU models

QEMU supports named x86 CPU models and per-feature configuration. This lets CI expose different virtual CPU generations to the same ArchiaOS image.

The planned validation matrix includes, when supported by the installed QEMU version:

- qemu64 — generic x86-64 baseline;
- core2duo — pre-SSE4.1-era compatibility test for the future Light path;
- Penryn — older Intel model with SSE4.1-era capabilities;
- Nehalem — older SSE4.2-era Intel model;
- Haswell — AVX2-era Intel model;
- Skylake-Client — close to the physical i7-6700 generation.

The purpose is not to prove that every real processor behaves identically to QEMU. It is to verify that the CPUID abstraction correctly adapts to materially different feature sets.

Real hardware validation remains necessary for motherboard, firmware, microcode, RAM and peripheral combinations.

## Expected behaviour

A CPU exposing SSE4.1 should print:

CPU: compatibility: STANDARD (SSE4.1)

A CPU without SSE4.1 should print:

CPU: compatibility: LIGHT (below SSE4.1)

Both modes currently continue booting. A future Light distribution can enforce a different build/policy without changing the underlying CPU detection subsystem.

Reference: https://www.qemu.org/docs/master/system/qemu-cpu-models.html
