# ArchiaOS first-party UEFI bootloader

This crate implements a first-party UEFI loader using Rust and `core`, with no third-party Rust dependencies. Standard UEFI firmware remains a platform prerequisite; the loader code itself is maintained by ArchiaOS.

## Current implementation

- UEFI entry point and diagnostic console output.
- Opens `\\EFI\\ArchiaOS\\kernel.elf` on the same device from which the loader was started.
- Bounds the image size, handles partial reads, and validates ELF64 for the current target architecture.
- Validates headers and loadable segments, checked arithmetic, file bounds, alignment, virtual overlap and executable entry coverage. Writable-executable (`PF_W | PF_X`) segments are rejected.
- Reserves pages at the ELF physical addresses, copies segment bytes and zeroes the BSS tail. Kernel physical addresses must remain in the supported 1 MiB–4 GiB range; the file is capped at 64 MiB and total segment memory at 128 MiB.
- **x86-64 high-half support:** builds fresh 4-level page tables for the ArchiaOS high-half virtual window. It identity-maps supported RAM below 4 GiB and maps the kernel segments at their high virtual addresses.
- **W^X/NX protection:** identity aliases of kernel PT_LOAD pages are NX and are not writable for executable code; the high-half code mapping is read-only/executable, rodata is read-only/NX and data is writable/NX. The UEFI loader image is temporarily mapped read-only/executable so the handoff trampoline can finish after CR3 switches. Before switching, the loader verifies CPU NX support and enables EFER.NXE and CR0.WP. A high-half QEMU test reads back these controls and inspects the effective page-table entries.
- **AArch64:** current loading requires identity-mapped segments. Executable ranges receive the required data/instruction-cache synchronization before transfer; high-half mappings and a kernel-owned MMU transition are not implemented yet.
- Finds ACPI 2.0/1.0 RSDP pointers and validates signature, length and checksums. It validates a Device Tree through the shared, unit-tested FDT parser when `EFI_DTB_TABLE_GUID` is provided. On the AAVMF/QEMU CI configuration the kernel discovers the PL011 console through ACPI SPCR.
- Builds versioned `ARCHIABI` BootInfo and obtains the final UEFI memory map immediately before `ExitBootServices`; retries stale map keys without unrelated firmware calls after the first exit attempt. The kernel verifies handoff metadata and parses the descriptor map before using its conventional ranges.
- Passes BootInfo in `RDI` on x86-64 or `x0` on AArch64.
- CI has passed the custom loader booting the production x86-64 kernel high-half under OVMF/QEMU. The kernel validates BootInfo and passes the 130-page PMM allocation/free self-test. CI also validates the AArch64 UEFI handoff under AAVMF/QEMU, including memory-map/PMM tests and a synchronous exception trap.

## Build and test

From the `ArchiaOS` directory:

```sh
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```

The production kernel's custom-loader build is:

```sh
cargo build --release --features uefi-boot --target x86_64-unknown-none
```

The ordinary ISO build still uses Limine. The high-half kernel boot is currently integrated in the CI's FAT/OVMF test image rather than selected as the default local boot path.

## Next

1. Confirm the latest NX/W^X hardening and BootInfo buffer checks on all CI jobs.
2. Broaden malformed-ELF and firmware-provided ACPI/FDT pointer tests.
3. Expand Device Tree discovery for memory regions, bus `ranges` translation, interrupt controllers and timers; raw `reg` addresses are not a complete driver abstraction.
4. Implement a kernel-owned MMU transition and exception/timer infrastructure for AArch64.
5. Audit all boot-time allocations and the 4 GiB physical-address limitation before wider hardware use.
6. Switch the default startup path away from Limine only after broader regressions, security review and real-hardware tests.
