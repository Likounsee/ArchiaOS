# ArchiaOS — documentation technique

ArchiaOS est un OS expérimental en Rust `no_std`. Le chemin de démarrage standard reste l'ISO Limine, mais le noyau de production x86-64 peut maintenant être compilé avec `uefi-boot` et démarré de bout en bout via notre bootloader UEFI maison sous OVMF/QEMU en CI.

## État

- **Chemin standard :** l'ISO Limine démarre le noyau x86-64, vérifie la carte mémoire et passe l'auto-test PMM de 130 pages.
- **Bootloader maison :** lit `\\EFI\\ArchiaOS\\kernel.elf`, valide ELF64, charge les segments, zero-fill BSS, prépare BootInfo, obtient la carte mémoire UEFI finale et appelle `ExitBootServices`.
- **Production x86-64 via UEFI maison :** le build `--features uefi-boot` démarre sous OVMF/QEMU, valide BootInfo v1, parcourt les descripteurs conventionnels et passe le test PMM de 130 allocations/libérations.
- **Pagination x86-64 :** le noyau high-half est mappé avec des pages de 4 Kio. Le test vérifie le code RO+X, les données RW+NX, les alias physiques du noyau RO+NX, le bit CR0.WP et EFER.NXE. Les segments ELF RWX sont refusés et le chargeur vérifie que le CPU supporte NX.
- **ARM64 :** le loader et le prototype noyau AArch64 passent l'assemblage et le transfert UEFI sous AAVMF/QEMU. Le noyau prototype valide BootInfo/la carte mémoire, découvre PL011 via ACPI SPCR sur cette configuration, initialise le PMM et réussit l'auto-test d'exception synchrone. Cela ne signifie pas que toutes les plateformes ARM64 sont compatibles.
- **Limites :** chargeur, buffers de transfert et adresses physiques du noyau restent dans la fenêtre identité sous 4 Gio. Le fichier noyau est plafonné à 64 Mio et la mémoire totale des segments à 128 Mio. Le high-half testé concerne x86-64 ; AArch64 utilise encore le chemin identité et n'a pas de MMU propre au noyau.

## Compilation du noyau x86-64

Depuis ce dossier :

```sh
cargo build --release

```

## Vérification locale du noyau avec entrée UEFI maison

Depuis ce dossier :

```sh
cargo build --release --target x86_64-unknown-none
```

Ce build produit le noyau high-half qui reçoit BootInfo au lieu des réponses Limine. L'image EFI de test complète est actuellement construite par la CI ; le script ISO habituel conserve Limine par défaut jusqu'à une bascule intentionnelle.

## Bootloader UEFI

```sh
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```

## Prototype noyau AArch64

Depuis `kernel/aarch64` :

```sh
cargo build --release --target aarch64-unknown-none
```

Le prototype cible QEMU `virt`. Une MMU propre, timers, contrôleur d'interruptions, gestion plus complète des exceptions et pilotes propres aux plateformes restent à implémenter.

Voir [la roadmap](docs/ROADMAP.md), [le plan boot/ARM64](docs/BOOT_AND_ARCHITECTURES.md), [le bootloader](bootloader/README.md) et [le test high-half x86-64](kernel/boot-test-highhalf-x86_64/README.md).

Le développement se fait exclusivement sur la branche `ArchiaOS`.
