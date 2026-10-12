# ArchiaOS — documentation technique

ArchiaOS est un OS expérimental en Rust `no_std`. Le noyau de production x86-64 démarre encore avec Limine ; un chargeur UEFI maison et le bring-up ARM64 sont en développement actif.

## État

- **Noyau de production x86-64 :** entrée série, validation de la carte mémoire Limine et test PMM de 130 pages sous QEMU.
- **Bootloader maison :** lit `\\EFI\\ArchiaOS\\kernel.elf`, valide ELF64, charge les segments identité x86-64 ou AArch64 sous 4 Gio, met à zéro le BSS, construit BootInfo, obtient la carte mémoire finale et appelle `ExitBootServices`. Sur ARM64, la maintenance I-cache est ajoutée avant le transfert. Les pointeurs ACPI/FDT sont vérifiés avant d'être transmis au noyau ; leurs parseurs de signature/checksum et de structure FDT ont des tests unitaires.
- **Handoff x86-64 :** image de test distincte validée par un job OVMF/QEMU de la CI ; le noyau de production reste high-half et dépend de Limine.
- **Handoff AArch64 :** le noyau prototype reçoit et valide BootInfo dans `x0`. Un job CI dédié construit l'image EFI, prépare un volume FAT avec le noyau et tente de démarrer la chaîne complète sous AAVMF/QEMU. Le résultat doit être vérifié avant de déclarer le jalon terminé.

## Build du noyau principal x86-64

```sh
cargo build --release
scripts/make-iso.sh
```

## Build et tests du bootloader

```sh
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```

## Build du prototype noyau AArch64

Depuis `kernel/aarch64` :

```sh
cargo build --release --target aarch64-unknown-none
```

Le test direct QEMU et le test de transfert UEFI AArch64 sont tous deux définis dans la CI. Le noyau reste un prototype QEMU `virt`, mais il localise désormais la console PL011 à partir des propriétés `compatible`/`reg` du Device Tree, avec une adresse de secours limitée à QEMU. Il réutilise la même ABI BootInfo, le même parseur FDT et le parseur de carte mémoire que le chargeur. Les exceptions, la MMU, les traductions `ranges`, la découverte mémoire complète et les pilotes ARM64 restent à implémenter.

Voir [la roadmap](docs/ROADMAP.md), [le plan boot/ARM64](docs/BOOT_AND_ARCHITECTURES.md), [le bootloader](bootloader/README.md) et [le test de transfert UEFI x86-64](kernel/boot-test-x86_64/README.md).

Le développement se fait exclusivement sur la branche `ArchiaOS` ; `main` ne doit pas être modifiée.
