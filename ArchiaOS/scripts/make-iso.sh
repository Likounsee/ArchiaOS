#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$ROOT/build"
LIMINE_DIR="$BUILD_DIR/limine-binary"
ARCHIVE="$BUILD_DIR/limine-binary.tar.gz"
ISO_ROOT="$BUILD_DIR/iso-root"
KERNEL="$ROOT/target/x86_64-unknown-none/release/archiaos-kernel"
ISO_OUT="$BUILD_DIR/archiaos.iso"
LIMINE_VERSION="12.9.1"
LIMINE_SHA256="5cdebc518daa3af30b22c2322ba0dba2e0e2046fa8b087b9e13071b8dbcdcff4"

if [[ ! -f "$KERNEL" ]]; then
    echo "Kernel not found; run cargo build --release first." >&2
    exit 1
fi

mkdir -p "$BUILD_DIR"
if [[ ! -f "$LIMINE_DIR/limine-bios.sys" || ! -f "$LIMINE_DIR/limine-uefi-cd.bin" ]]; then
    rm -rf "$LIMINE_DIR"
    mkdir -p "$LIMINE_DIR"
    curl --fail --location --retry 3 \
        "https://github.com/limine-bootloader/limine/releases/download/v${LIMINE_VERSION}/limine-binary.tar.gz" \
        --output "$ARCHIVE"
    echo "${LIMINE_SHA256}  ${ARCHIVE}" | sha256sum --check -
    mkdir -p "$BUILD_DIR/limine-extract"
    tar -xzf "$ARCHIVE" -C "$BUILD_DIR/limine-extract"
    EXTRACTED_DIR="$(find "$BUILD_DIR/limine-extract" -type f -name limine-bios.sys -print -quit)"
    if [[ -z "$EXTRACTED_DIR" ]]; then
        echo "Limine binary archive did not contain limine-bios.sys" >&2
        exit 1
    fi
    cp -a "$(dirname "$EXTRACTED_DIR")/." "$LIMINE_DIR/"
fi

if [[ ! -x "$LIMINE_DIR/limine" ]]; then
    make -C "$LIMINE_DIR"
fi

rm -rf "$ISO_ROOT"
mkdir -p "$ISO_ROOT/boot/limine" "$ISO_ROOT/EFI/BOOT"
cp "$KERNEL" "$ISO_ROOT/boot/archiaos-kernel"
cp "$ROOT/limine.conf" "$ISO_ROOT/limine.conf"
cp "$ROOT/limine.conf" "$ISO_ROOT/boot/limine/limine.conf"
cp "$LIMINE_DIR/limine-bios.sys" "$LIMINE_DIR/limine-bios-cd.bin" \
   "$LIMINE_DIR/limine-uefi-cd.bin" "$ISO_ROOT/boot/limine/"
cp "$LIMINE_DIR/BOOTX64.EFI" "$ISO_ROOT/EFI/BOOT/BOOTX64.EFI"

xorriso -as mkisofs -R -r -J \
    -b boot/limine/limine-bios-cd.bin -no-emul-boot -boot-load-size 4 -boot-info-table \
    --efi-boot boot/limine/limine-uefi-cd.bin -efi-boot-part --efi-boot-image \
    --protective-msdos-label "$ISO_ROOT" -o "$ISO_OUT"
"$LIMINE_DIR/limine" bios-install "$ISO_OUT"
echo "Created $ISO_OUT"
