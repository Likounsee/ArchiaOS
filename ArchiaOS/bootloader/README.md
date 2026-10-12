# ArchiaOS first-party UEFI bootloader

This crate is the beginning of ArchiaOS's own UEFI bootloader. It has no third-party Rust dependencies and uses only `core` plus the UEFI ABI. It is intentionally separate from the existing kernel crate and the Limine-based boot path.

## Current milestone

- Defines the UEFI system-table and text-output layouts needed for the entry point.
- Exposes `efi_main`, writes diagnostic messages through UEFI Text Output and mirrors them to COM1 on x86-64 for headless QEMU tests.
- Opens `\EFI\ArchiaOS\kernel.elf` from the same EFI System Partition as the bootloader using UEFI Loaded Image and Simple File System protocols.
- Bounds the file size to 64 MiB and the `EFI_FILE_INFO` query buffer to 4 KiB before allocating loader-owned pool memory.
- Includes an allocation-free ELF64 validation library for x86-64 and AArch64 executable images. It checks header format, program-table bounds, segment bounds, arithmetic overflow, alignment, virtual segment overlap and executable entry-point coverage.
- CI runs host unit tests for valid and malformed ELF images and compiles the UEFI application for `x86_64-unknown-uefi` and `aarch64-unknown-uefi`.
- A QEMU/OVMF smoke-test job builds a FAT EFI disk with the loader and kernel and checks the loader's read/validation diagnostics.
- It does **not** yet copy `PT_LOAD` segments into physical pages, establish kernel page tables, build the final handoff structure, call `ExitBootServices`, or transfer control to the kernel.
- It does **not** replace the current Limine path yet.

## Build

From this directory, with the corresponding Rust target installed:

```sh
cargo test --lib --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-uefi
cargo build --release --target aarch64-unknown-uefi
```

## Required next milestones

1. Finish the QEMU/OVMF smoke test and fix any firmware-specific protocol issues it reveals.
2. Allocate pages safely and copy each validated `PT_LOAD` segment, zeroing `p_memsz - p_filesz`; enforce per-architecture address constraints before writing memory.
3. Define an architecture-neutral, versioned handoff structure for the memory map, ACPI/RSDP or Device Tree, framebuffer and loaded kernel.
4. For x86-64, build page tables that map the kernel's high-half virtual segments to their physical load addresses while preserving the loader stack until the final transfer.
5. Implement `GetMemoryMap`/`ExitBootServices` retry handling and ensure no boot-service call occurs after successful exit.
6. Remove the kernel's Limine-only memory-map dependency by teaching the kernel to consume the new handoff contract.
7. Boot the kernel x86-64 end to end, then add the AArch64 loader handoff and run its QEMU smoke test.
8. Keep Limine available as a temporary fallback until the new loader passes repeatable end-to-end tests.

The firmware supplied by the motherboard or virtual machine remains the standard UEFI implementation. ArchiaOS owns the bootloader code, not the platform firmware.
