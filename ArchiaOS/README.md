# ArchiaOS

**ArchiaOS est un système d'exploitation expérimental en cours de construction.** Le noyau x86-64 Rust `no_std` démarre actuellement avec Limine et est testé sous QEMU par GitHub Actions. Le remplacement par un bootloader UEFI développé par ArchiaOS a commencé, mais n'est pas encore le chemin de démarrage par défaut.

## État actuel

- **Démarrage x86-64 existant :** l'ISO Limine BIOS/UEFI se construit et le noyau atteint son entrée Rust en mode long x86-64 ; la sortie COM1 est vérifiée par le smoke test QEMU.
- **Bootloader ArchiaOS :** premier crate Rust `no_std` sans dépendance externe, avec entrée UEFI et diagnostic sur la console du firmware. Il ne charge pas encore le noyau et ne remplace pas Limine.
- **Préparation ARM64 :** la CI compile l'application UEFI du bootloader pour `x86_64-unknown-uefi` et `aarch64-unknown-uefi`, et compile un prototype noyau AArch64 distinct. Le noyau principal n'est pas encore compatible ARM64.
- **Carte mémoire :** la réponse Limine est contrôlée pour les pointeurs, le nombre d'entrées, les longueurs nulles et les débordements.
- **Mémoire physique :** allocateur bitmap expérimental pour les pages sous 4 Gio. Le test QEMU confirme l'allocation de 130 pages distinctes, traversant plusieurs mots bitmap, l'alignement, la baisse puis le rétablissement du compteur, le rejet des libérations invalides et le rejet du double-free.
- **Langages :** Rust pour le noyau et le bootloader ; un petit stub assembleur aligne la pile à l'entrée x86-64. C/C++ ne sont pas ajoutés sans besoin démontré.
- **Build :** Cargo, linker ELF avec segments virtuels hauts, génération d'ISO Limine, vérification des segments ELF et CI de démarrage.

Le noyau principal ne fournit pas encore d'IDT, de gestionnaire d'exceptions, d'ordonnanceur, de processus isolés, d'appels système, de stockage, de système de fichiers, de réseau ni d'interface graphique. L'allocateur est limité à 4 Gio, initialisé une seule fois et non synchronisé pour SMP ; il reste expérimental. Le prototype AArch64 séparé est uniquement une base de compilation et de bring-up, pas encore un démarrage testé.

## Compilation du noyau x86-64 existant

Prérequis : Rust stable, cible `x86_64-unknown-none`, `curl`, `make`, `xorriso` et QEMU x86-64. Depuis le dossier `ArchiaOS` :

```sh
cargo build --release
scripts/make-iso.sh
qemu-system-x86_64 -cdrom build/archiaos.iso -boot d -serial stdio -display none -no-reboot -no-shutdown
```

## Compilation du bootloader UEFI

Depuis le dossier `ArchiaOS` :

```sh
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```

Le prototype affiche un diagnostic puis rend la main au firmware. Il ne doit pas être utilisé comme chargeur du noyau tant que la lecture du noyau, la validation ELF, le transfert mémoire et les tests de démarrage ne sont pas implémentés.

## Compilation du prototype noyau AArch64

Depuis le dossier `ArchiaOS/kernel/aarch64` :

```sh
cargo build --release --target aarch64-unknown-none
```

Ce prototype cible la machine `virt` de QEMU et sa console PL011. Il n'est pas encore connecté au bootloader et n'a pas encore été validé en exécution.

## Documentation

- [Roadmap P0–P7](docs/ROADMAP.md)
- [Architecture du bootloader et préparation ARM64](docs/BOOT_AND_ARCHITECTURES.md)
- [Prototype UEFI](bootloader/README.md)
- [Prototype noyau AArch64](kernel/aarch64/README.md)

Le développement se fait exclusivement sur la branche **`ArchiaOS`** ; `main` ne doit pas être modifiée.
