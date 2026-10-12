CARGO ?= cargo

.PHONY: build boot-local run uefi-files clean

# Build the default x86-64 kernel.
build:
	$(CARGO) build --release --features uefi-boot --target x86_64-unknown-none

# Boot the first-party UEFI path locally under OVMF/QEMU.
boot-local:
	bash ./scripts/boot-local-uefi.sh

# Alias for the local virtual boot test.
run: boot-local

# Produce standalone EFI files for a FAT32 USB drive.
uefi-files:
	bash ./scripts/build-uefi-files.sh

# Remove generated Rust and packaging outputs.
clean:
	$(CARGO) clean
	rm -rf build
