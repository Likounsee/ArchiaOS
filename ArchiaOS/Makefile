CARGO ?= cargo
QEMU ?= qemu-system-x86_64
KERNEL := target/x86_64-unknown-none/release/archiaos-kernel
ISO := build/archiaos.iso

.PHONY: build iso run clean
build:
	$(CARGO) build --release

iso: build
	./scripts/make-iso.sh

run: iso
	$(QEMU) -cdrom $(ISO) -boot d -serial stdio -display none -no-reboot -no-shutdown

clean:
	$(CARGO) clean
	rm -rf build/iso-root build/limine-extract build/limine-binary build/limine-binary.tar.gz $(ISO)
