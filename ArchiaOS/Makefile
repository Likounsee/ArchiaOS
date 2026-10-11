CARGO ?= cargo
QEMU ?= qemu-system-x86_64
KERNEL := target/x86_64-unknown-none/release/archiaos-kernel

.PHONY: build clean run
build:
	$(CARGO) build --release
clean:
	$(CARGO) clean
run: build
	$(QEMU) -kernel $(KERNEL) -serial stdio -display none -no-reboot -no-shutdown