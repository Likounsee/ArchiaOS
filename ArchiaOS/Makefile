CARGO ?= cargo
QEMU ?= qemu-system-x86_64
KERNEL := target/x86_64-unknown-none/release/archiaos-kernel
ISO := build/archiaos.iso

.PHONY: build iso run boot-local clean
build:
	$(CARGO) build --release

iso: build
	./scripts/make-iso.sh

# Build and boot the first-party UEFI path locally under OVMF/QEMU.
boot-local:
	./scripts/boot-local-uefi.sh

run: boot-local

clean:
	$(CARGO) clean
	rm -rf build/iso-root build/limine-extract build/limine-binary build/limine-binary.tar.gz $(ISO) build/local-uefi
