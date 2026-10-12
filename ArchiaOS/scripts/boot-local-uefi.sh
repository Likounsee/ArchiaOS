#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
OUT="${ARCHIAOS_BUILD_DIR:-$ROOT/build/local-uefi}"
IMAGE="${ARCHIAOS_IMAGE:-$OUT/archiaos-uefi.img}"
LOG="${ARCHIAOS_LOG:-$OUT/serial.log}"
SECONDS_LIMIT="${ARCHIAOS_BOOT_TIMEOUT:-25}"
mkdir -p "$OUT" "$(dirname "$IMAGE")" "$(dirname "$LOG")"
for tool in cargo rustup mkfs.vfat mmd mcopy qemu-system-x86_64 timeout; do
  command -v "$tool" >/dev/null || { echo "Outil manquant : $tool" >&2; exit 2; }
done
for target in x86_64-unknown-none x86_64-unknown-uefi; do
  rustup target list --installed | grep -Fxq "$target" || {
    echo "Cible Rust manquante : rustup target add $target" >&2; exit 2;
  }
done
if [[ -z "${OVMF_CODE:-}" && -z "${OVMF_VARS:-}" ]]; then
  for dir in /usr/share/OVMF /usr/share/ovmf; do
    for code in "$dir"/OVMF_CODE.fd "$dir"/OVMF_CODE_4M.fd; do
      [[ -s "$code" ]] || continue
      suffix="${code##*/OVMF_CODE}"; suffix="${suffix%.fd}"
      vars="$dir/OVMF_VARS${suffix}.fd"
      if [[ -s "$vars" ]]; then OVMF_CODE="$code"; OVMF_VARS="$vars"; break 2; fi
    done
  done
fi
if [[ -z "${OVMF_CODE:-}" || -z "${OVMF_VARS:-}" || ! -s "$OVMF_CODE" || ! -s "$OVMF_VARS" ]]; then
  echo "Paire OVMF CODE/VARS introuvable. Installe ovmf ou définis OVMF_CODE et OVMF_VARS." >&2
  exit 2
fi
echo "[1/4] Compilation du noyau de production"
cargo build --manifest-path Cargo.toml --release --features uefi-boot --target x86_64-unknown-none --target-dir target/production-kernel-uefi
kernel="$ROOT/target/production-kernel-uefi/x86_64-unknown-none/release/archiaos-kernel"
echo "[2/4] Compilation du chargeur UEFI"
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi --target-dir target
loader="$ROOT/target/x86_64-unknown-uefi/release/archiaos-bootloader.efi"
test -s "$kernel" && test -s "$loader"
echo "[3/4] Création de l'image FAT32 : $IMAGE"
rm -f "$IMAGE"; truncate -s 128M "$IMAGE"; mkfs.vfat -F 32 "$IMAGE" >/dev/null
mmd -i "$IMAGE" ::/EFI
mmd -i "$IMAGE" ::/EFI/BOOT
mmd -i "$IMAGE" ::/EFI/ArchiaOS
mcopy -i "$IMAGE" "$loader" ::/EFI/BOOT/BOOTX64.EFI
mcopy -i "$IMAGE" "$kernel" ::/EFI/ArchiaOS/kernel.elf
cp "$OVMF_VARS" "$OUT/OVMF_VARS.fd"
echo "[4/4] Démarrage OVMF/QEMU (timeout normal : le noyau se bloque après son auto-test)"
set +e
timeout "${SECONDS_LIMIT}s" qemu-system-x86_64 -machine q35 -m 512M \
  -drive "if=pflash,format=raw,readonly=on,file=$OVMF_CODE" \
  -drive "if=pflash,format=raw,file=$OUT/OVMF_VARS.fd" \
  -drive "file=$IMAGE,format=raw,if=ide" -boot order=c -display none \
  -serial stdio -monitor none -no-reboot -no-shutdown >"$LOG" 2>&1
status=$?
set -e
cat "$LOG"
if [[ "$status" -ne 0 && "$status" -ne 124 ]]; then
  echo "QEMU a échoué (code $status), journal : $LOG" >&2; exit "$status"
fi
for marker in \
  "ArchiaOS Bootloader: opening EFI kernel image" \
  "ArchiaOS Bootloader: validating ELF64 kernel" \
  "ArchiaOS Bootloader: loading high-half ELF segments" \
  "ArchiaOS Bootloader: exiting UEFI boot services" \
  "ArchiaOS: custom UEFI bootloader entered production kernel" \
  "ArchiaOS: UEFI BootInfo validated" \
  "ArchiaOS: UEFI PMM allocation/free self-test passed" \
  "ArchiaOS: UEFI kernel entry validated"; do
  grep -Fq "$marker" "$LOG" || { echo "BOOT ÉCHOUÉ : marqueur absent : $marker (voir $LOG)" >&2; exit 1; }
done
echo "BOOT VALIDÉ : le noyau de production a atteint le dernier marqueur P0."
echo "Image : $IMAGE"
echo "Journal série : $LOG"
