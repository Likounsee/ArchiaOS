# ArchiaOS first-party UEFI bootloader

This crate implements a first-party UEFI loader using Rust and `core`, with no third-party Rust dependencies. Standard UEFI firmware remains a platform prerequisite; the loader code itself is maintained by ArchiaOS.

## Current implementation

- UEFI entry point and diagnostic console output.
- Opens `\\EFI\\ArchiaOS\\kernel.elf` on the same device from which the loader was started.
- Bounds the image size, reads the file (including partial reads), and validates ELF64 for the current target architecture.
- Validates headers and loadable segments, checked arithmetic, file bounds, alignment, virtual overlap and executable entry coverage.
- Reserves pages at the ELF physical addresses, rejects non-identity mappings and addresses outside the supported sub-4-GiB range, copies each segment and zeroes its BSS tail.
- On AArch64, synchronizes the data and instruction caches for executable ranges before transferring control.
- Finds ACPI 2.0/1.0 RSDP pointers and the UEFI Device Tree table in the system configuration table. On AArch64 the loader validates the FDT magic, size, version and top-level block offsets before accepting the handoff.
- Builds the versioned `ARCHIABI` BootInfo structure and obtains the final UEFI memory map immediately before `ExitBootServices`; retries stale map keys without making unrelated service calls after the first exit attempt.
- Passes BootInfo in `RDI` on x86-64 or `x0` on AArch64 and branches to the ELF entry point.
- CI includes an OVMF/QEMU x86-64 handoff test and an AAVMF/QEMU AArch64 handoff test. The AArch64 job's result must pass before the full path is considered validated.

## Build and test

From the `ArchiaOS` directory:

```sh
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```

The current loading contract is intentionally limited to identity-mapped ELF64 segments below 4 GiB, at most 128 MiB of total segment memory, and images no larger than 64 MiB. That permits dedicated bring-up test kernels but does not yet support the high-half linked production x86-64 kernel. The new loader is not yet the default boot path for ArchiaOS.

## Next

1. Confirm both OVMF/AAVMF QEMU end-to-end jobs on the current commit.
2. Expand tests for the firmware configuration-table scanner and malformed FDT headers.
3. Validate the ACPI RSDP signature/checksum and build a bounded FDT parser for memory and platform devices.
4. Design the production kernel's page-table/bootstrap contract before supporting high-half loading.
5. Add broader malformed ELF tests and CI fuzz-style parser cases.
