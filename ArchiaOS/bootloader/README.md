# ArchiaOS first-party UEFI bootloader

ArchiaOS owns this Rust `no_std` loader; there are no third-party Rust dependencies. The standard machine/VM UEFI firmware remains in use.

## Implemented

- UEFI entry point and firmware-console diagnostics, mirrored to COM1 on x86-64 for headless tests.
- Opens `\\EFI\\ArchiaOS\\kernel.elf` from the EFI system volume using Loaded Image and Simple File System protocols.
- Bounds the image at 64 MiB and the `EFI_FILE_INFO` query allocation at 4 KiB.
- Validates ELF64 headers and load-segment bounds, arithmetic overflow, alignment, overlap and executable entry coverage.
- On x86-64, validates the complete identity-mapped segment plan below 4 GiB, allocates the physical pages, copies file content and zeros BSS tails.
- Defines versioned `BootInfo`, gets the final UEFI memory map, retries stale map keys and calls `ExitBootServices` without an intervening firmware call.
- CI includes a small identity-linked kernel that validates the handoff under QEMU/OVMF.

## Deliberate restrictions

The production kernel is still high-half linked and uses Limine responses; this loader explicitly rejects its address model. The end-to-end test only proves a low identity-mapped test image boots. Page-table construction for the main kernel remains future work.

The AArch64 target still only builds the loader and parses ELF; its own physical loader policy, platform mappings, MMU state and handoff must be implemented/tested before claiming end-to-end ARM64 boot. ACPI/Device Tree and framebuffer fields are zero for now.

## Build

From `ArchiaOS/`:

```sh
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```
