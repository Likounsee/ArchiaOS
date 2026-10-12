# High-half UEFI handoff test kernel (x86-64)

This isolated `no_std` kernel validates the custom loader's first x86-64 high-half mapping. Its ELF virtual addresses begin at `0xffffffff80200000`, while physical segment load addresses begin at 2 MiB. The test requires the loader to map the RAM ranges identity, map each ELF segment at its high virtual address, load CR3 and branch into the high-half entry.

After entry it confirms that RIP is high-half, validates the shared `ARCHIABI` BootInfo contract, reads the identity-mapped UEFI memory map, confirms conventional memory exists, writes a serial success marker and halts.

This test is not the production kernel. The production high-half kernel is still Limine-dependent and is not switched to the new loader until its own boot-info and memory-init path is implemented.

CI builds the kernel with `linker.ld` and starts it under OVMF/QEMU using the custom loader.
