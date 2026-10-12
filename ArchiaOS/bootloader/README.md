# ArchiaOS first-party UEFI bootloader

This crate is the beginning of ArchiaOS's own UEFI bootloader. It has no third-party Rust dependencies and uses only `core` plus the UEFI ABI. It is intentionally separate from the existing kernel crate and the Limine-based boot path.

## Current milestone

- Defines the UEFI system-table and text-output layouts needed for the entry point.
- Exposes `efi_main` and writes a diagnostic banner to the firmware console.
- Includes an allocation-free ELF64 validation library for x86-64 and AArch64 executable images. It checks headers, program-table bounds, load-segment bounds, alignment, arithmetic overflow, segment overlap and executable entry-point coverage.
- CI runs unit tests for valid images and malformed ELF cases, then compiles the UEFI app for both `x86_64-unknown-uefi` and `aarch64-unknown-uefi`.
- The ELF parser is not yet connected to firmware file I/O or physical-memory loading.
- Does **not** yet load the kernel, allocate/map its segments, create page tables, obtain a final memory map, call `ExitBootServices`, or transfer control to the kernel.
- Does **not** replace the current Limine path yet.

## Build

From this directory, with the corresponding Rust target installed:

```sh
cargo test --lib --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-uefi
cargo build --release --target aarch64-unknown-uefi
```

## Required next milestones

1. Validate the UEFI protocol structures and build a QEMU/OVMF launch test.
2. Implement UEFI Simple File System access and read the kernel image from the EFI System Partition.
3. Connect the ELF parser to the loader and validate platform-specific address limits, permissions and physical allocation before copying any segment.
4. Define an architecture-neutral handoff structure for the memory map, framebuffer, ACPI/RSDP or Device Tree and loaded kernel.
5. Call `GetMemoryMap` and `ExitBootServices` with retry handling; do not call boot services after exit.
6. Add architecture-specific handoff code for x86-64 and AArch64, including CPU state and page-table requirements.
7. Boot the existing x86-64 kernel through the new loader only after it has a handoff path that no longer requires Limine responses.
8. Keep Limine available as a temporary fallback until both the new loader and its tests are reliable.

The firmware supplied by the motherboard or virtual machine remains the standard UEFI implementation. ArchiaOS owns the bootloader code, not the platform firmware.
