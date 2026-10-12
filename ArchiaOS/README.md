# ArchiaOS — documentation technique

ArchiaOS est un OS expérimental en Rust `no_std`. Le noyau de production x86-64 démarre encore avec Limine ; un bootloader UEFI maison est désormais testé de bout en bout avec des noyaux de test x86-64 et AArch64.

## État

- **Noyau de production x86-64 :** entrée série, validation de la carte mémoire Limine et auto-test PMM de 130 pages sous QEMU.
- **Bootloader maison :** lit `\\EFI\\ArchiaOS\\kernel.elf`, valide ELF64, charge les segments identité sous 4 Gio, met le BSS à zéro, construit BootInfo, obtient la carte mémoire finale et appelle `ExitBootServices`. Sur AArch64, la maintenance I-cache précède le transfert. Les pointeurs ACPI/FDT transmis sont validés.
- **Handoff x86-64 :** test de bout en bout réussi sous OVMF/QEMU avec un noyau de test identité ; le noyau de production reste high-half et dépend de Limine.
- **Handoff AArch64 :** test de bout en bout réussi sous AAVMF/QEMU. Le noyau prototype reçoit BootInfo, découvre le PL011 via ACPI SPCR sur cette plateforme, valide la carte mémoire, initialise le PMM et réussit l'auto-test d'exception synchrone.
- **ARM64 direct QEMU :** entrée noyau, diagnostics série et test d'exception synchrone validés.

Le chargeur accepte pour l'instant des segments ELF identité, alignés sur page et dans la fenêtre physique 1 Mio–4 Gio. Le fichier noyau est plafonné à 64 Mio, et la mémoire des segments est plafonnée à 128 Mio. Ces tests valident les configurations QEMU de la CI, pas tous les appareils ARM64. Le chargeur maison n'est pas encore le chemin de démarrage du noyau de production.

## Compilation

Depuis le dossier `ArchiaOS` :

```sh
cargo build --release
scripts/make-iso.sh
```

## Bootloader UEFI

Depuis le dossier `ArchiaOS` :

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

Le prototype cible QEMU `virt`. Le MMU, les timers, le contrôleur d'interruptions, le retour depuis exceptions et les pilotes propres aux plateformes restent à implémenter.

Voir [la roadmap](docs/ROADMAP.md), [le plan boot/ARM64](docs/BOOT_AND_ARCHITECTURES.md), [le bootloader](bootloader/README.md) et [le test de transfert UEFI x86-64](kernel/boot-test-x86_64/README.md).

Le développement se fait exclusivement sur la branche `ArchiaOS` ; `main` ne doit pas être modifiée.
