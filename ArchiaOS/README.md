# ArchiaOS — documentation technique

## État actuel

La première tranche du projet est un noyau x86-64 expérimental écrit en Rust `no_std`, avec un point d’entrée assembleur Multiboot 1, une transition du mode protégé 32 bits au mode long 64 bits, une identité mémoire initiale limitée aux premiers 1 Gio et une sortie de diagnostic COM1. Elle ne constitue pas encore un OS complet : pas d’allocateur physique, d’IDT, d’ordonnanceur, d’appels système, de stockage ni d’espace utilisateur.

Cette base doit être compilée et validée en machine virtuelle avant de considérer le démarrage comme vérifié.

## Prérequis

- Rust stable/rustup et la cible `x86_64-unknown-none` (`rustup target add x86_64-unknown-none`).
- QEMU x86-64 dont le chargeur reconnaît les noyaux Multiboot.

## Compiler et lancer

Depuis ce répertoire :

```sh
rustup target add x86_64-unknown-none
cargo build --release
qemu-system-x86_64 -kernel target/x86_64-unknown-none/release/archiaos-kernel -serial stdio -display none -no-reboot -no-shutdown
```

Le résultat attendu est un message de démarrage sur le port série. La commande n’a pas été exécutée pendant cette modification ; la sortie doit être observée pour valider le démarrage.

## Architecture et limites

- Cible initiale : x86-64, démarrage Multiboot 1, noyau ELF.
- Rust `no_std` pour le noyau et les diagnostics ; assembleur seulement pour la transition CPU et les structures d’amorçage.
- La pagination initiale utilise des pages de 2 Mio et identité-map les premiers 1 Gio. Ce n’est pas un allocateur ni une politique de protection finale.
- Les informations mémoire Multiboot ne sont pas encore analysées ; aucune IDT, gestion mémoire générale, ordonnanceur, processus, syscall, stockage ou interface graphique n’existe encore.

## Documents

- [Architecture](docs/ARCHITECTURE.md) · [Langages](docs/LANGUAGES.md) · [Démarrage](docs/BOOT.md) · [Mémoire](docs/MEMORY.md)
- [Interruptions](docs/INTERRUPTS.md) · [Sécurité](docs/SECURITY.md) · [Roadmap](docs/ROADMAP.md) · [Tests](docs/TESTING.md)
- [Pilotes](docs/DRIVERS.md) · [Ordonnanceur](docs/SCHEDULER.md) · [Appels système](docs/SYSCALLS.md) · [Système de fichiers](docs/FILESYSTEM.md) · [Espace utilisateur](docs/USERLAND.md) · [Performances](docs/PERFORMANCE.md)

Travaillez exclusivement sur la branche `ArchiaOS`. Ne déclarez aucune fonctionnalité prête avant de l’avoir compilée et vérifiée dans son contexte.