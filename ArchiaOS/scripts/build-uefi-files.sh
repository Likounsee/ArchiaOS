#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
OUT="${ARCHIAOS_EFI_OUT:-$ROOT/build/uefi-files}"
KERNEL_TARGET="$ROOT/target/production-kernel-uefi/x86_64-unknown-none/release/archiaos-kernel"
LOADER_TARGET="$ROOT/target/x86_64-unknown-uefi/release/archiaos-bootloader.efi"

rustup target add x86_64-unknown-none x86_64-unknown-uefi
echo "[1/3] Compilation du noyau ArchiaOS"
cargo build --manifest-path Cargo.toml --release --features uefi-boot --target x86_64-unknown-none --target-dir target/production-kernel-uefi
echo "[2/3] Compilation du chargeur UEFI x86_64"
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi --target-dir target
test -s "$KERNEL_TARGET"
test -s "$LOADER_TARGET"

echo "[3/3] Préparation des fichiers UEFI autonomes"
rm -rf "$OUT"
mkdir -p "$OUT/EFI/BOOT" "$OUT/EFI/ArchiaOS"
install -m 0644 "$LOADER_TARGET" "$OUT/EFI/BOOT/BOOTX64.EFI"
install -m 0644 "$KERNEL_TARGET" "$OUT/EFI/ArchiaOS/kernel.elf"
cat > "$OUT/README.txt" <<'EOF'
ArchiaOS UEFI boot files
========================

Copy the EFI directory to the root of a FAT32-formatted USB drive.
The fallback UEFI loader is EFI/BOOT/BOOTX64.EFI.
The ArchiaOS kernel must remain at EFI/ArchiaOS/kernel.elf.

This package contains files, not a disk image. The firmware launches
BOOTX64.EFI; the ArchiaOS loader then loads kernel.elf.
EOF
echo "Fichiers prêts dans : $OUT"
find "$OUT" -type f -printf '%P (%s bytes)\n' | sort
