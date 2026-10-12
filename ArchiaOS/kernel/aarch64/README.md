# AArch64 kernel bring-up prototype

This is an isolated, dependency-free `no_std` kernel prototype for starting the AArch64 path without destabilizing the current x86-64/Limine kernel.

## Current scope

- Defines an AArch64 `_start` entry point, masks interrupts, and installs a dedicated 64 KiB stack.
- Initializes the PL011 UART at `0x09000000`, the conventional UART address for QEMU's `virt` machine, and emits diagnostic messages.
- Uses a simple linker layout at `0x40080000` for initial bring-up experiments.
- Is a compile-time prototype only at this stage. It has not yet been booted under QEMU and is not integrated with the ArchiaOS bootloader.

## Build

From this directory, with the Rust target installed:

```sh
cargo build --release --target aarch64-unknown-none
```

## Known limitations

This is not yet a general ARM64 kernel. It assumes the QEMU `virt` PL011 mapping, does not establish page tables, does not configure exception vectors or a timer, does not parse a Device Tree, and does not share the existing x86-64 memory allocator. Its load address and entry contract must be reconciled with the future ArchiaOS bootloader before attempting an end-to-end boot.
