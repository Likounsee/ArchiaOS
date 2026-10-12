# UEFI handoff test kernel (x86-64)

This tiny first-party `no_std` image exists only to exercise our UEFI loader. The production kernel remains high-half linked and Limine-dependent; it is not replaced by this test.

It is linked at identity addresses starting at 2 MiB, receives `BootInfo` in RDI, establishes a 64 KiB stack, validates the ABI and memory-map descriptors, writes a serial success marker and halts.

CI build command from this directory:

```sh
RUSTFLAGS="-C link-arg=-Tlinker.ld -C relocation-model=static -C no-redzone=yes" cargo build --release --target x86_64-unknown-none --target-dir ../../target/boot-test-x86_64
```

This verifies the loader handoff only, not production MMU, IDT, scheduler or kernel memory management.
