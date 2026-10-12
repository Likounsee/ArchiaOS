# ArchiaOS — documentation technique

ArchiaOS est un OS expérimental en Rust `no_std`. Le noyau principal x86-64 démarre encore avec Limine. Une chaîne de démarrage UEFI maison est en cours de construction.

## État

- **Noyau principal x86-64 :** entrée série, validation de la carte mémoire Limine et test PMM de 130 pages sous QEMU.
- **Bootloader maison :** lit `\\EFI\\ArchiaOS\\kernel.elf`, valide ELF64 et peut charger les segments x86-64 identité sous 4 Gio, construire un `BootInfo`, quitter les Boot Services et passer le contrôle à un noyau de test isolé.
- **Noyau principal inchangé :** le noyau de production est high-half lié et attend les réponses Limine. Il ne peut pas encore être démarré par le nouveau loader.
- **ARM64 :** un prototype de noyau AArch64 démarre sous QEMU avec diagnostics PL011. Le transfert UEFI vers ce noyau reste à implémenter.

Le test de transfert est vérifié séparément dans la CI. Il ne constitue pas encore un démarrage end-to-end du noyau de production ni une prise en charge ARM64 complète.

## Compilation du noyau principal

```sh
cargo build --release
scripts/make-iso.sh
```

## Compilation du bootloader UEFI

```sh
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```

Voir [la roadmap](docs/ROADMAP.md), [le plan boot/ARM64](docs/BOOT_AND_ARCHITECTURES.md) et [le test de transfert UEFI](kernel/boot-test-x86_64/README.md).

Le développement se fait exclusivement sur la branche `ArchiaOS`; `main` ne doit pas être modifiée.
