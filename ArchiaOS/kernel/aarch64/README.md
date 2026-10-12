# AArch64 kernel bring-up prototype

This is an isolated `no_std` kernel prototype for advancing AArch64 support without destabilizing the current x86-64/Limine production kernel. It has no third-party dependencies and depends on ArchiaOS's own bootloader library to share the BootInfo ABI and tested FDT header/token parser.

## Implemented in the prototype

- AArch64 `_start` masks interrupts and installs a dedicated 64 KiB stack.
- The first-argument register `x0` is preserved across stack setup and passed to Rust as a potential BootInfo pointer.
- PL011 diagnostics use the QEMU `virt` UART at `0x09000000`.
- The kernel shares the exact `BootInfo` ABI with the loader through an internal path dependency, validates the versioned `ARCHIABI` structure, and uses the common allocation-free memory-map parser to validate each descriptor's stride, page alignment, non-zero range and overflow-safe physical end.
- Direct QEMU `-kernel` smoke testing remains supported: in that mode `x0` normally points to the Device Tree, which is distinguished from BootInfo by its magic value. Both direct boot and the UEFI path use the shared FDT header/token parser, including reservation-map termination and property-name bounds.
- The UEFI loader now has an AArch64 identity-mapped ELF load/handoff path, including executable-cache synchronization, `ExitBootServices`, and transfer to `_start`. A dedicated OVMF/AAVMF QEMU job tests this end-to-end path.

## Build and direct QEMU smoke test

From this directory:

```sh
cargo build --release --target aarch64-unknown-none
timeout 15s qemu-system-aarch64 -machine virt -cpu cortex-a57 -m 512M -kernel target/aarch64-unknown-none/release/archiaos-kernel-aarch64 -display none -serial stdio -monitor none -no-reboot -no-shutdown
```

The project CI also compiles the UEFI application and attempts to boot this kernel through the custom loader. The CI result must pass before considering the end-to-end ARM64 handoff validated.

## Limits

This is not yet a general ARM64 kernel. The current loader requires identity-mapped ELF64 segments below 4 GiB and the kernel is tailored to QEMU `virt`. The kernel does not yet establish its own MMU tables, exception vectors, timer, interrupt controller, or semantic Device Tree discovery for address/size cells, memory ranges, interrupt controllers and UART properties. It does not yet share the x86-64 PMM implementation. No production ARM64 compatibility is claimed yet.
