# ArchiaOS — documentation technique

## État actuel

Le projet contient un noyau x86-64 expérimental écrit en Rust `no_std`. Le protocole natif Limine fournit l’entrée en mode long et les structures de pagination initiales ; le noyau initialise COM1 et écrit des diagnostics. Cela ne constitue pas encore un OS complet : pas d’allocateur physique, d’IDT, d’ordonnanceur, d’appels système, de stockage ou d’espace utilisateur.

## Prérequis

- Rust stable et la cible `x86_64-unknown-none`.
- `curl`, `sha256sum`, `make`, `xorriso`.
- QEMU x86-64 pour le test de démarrage.

## Compiler, créer l’ISO et démarrer

Depuis ce dossier :

```sh
rustup target add x86_64-unknown-none
cargo build --release
scripts/make-iso.sh
qemu-system-x86_64 -cdrom build/archiaos.iso -boot d -serial stdio -display none -no-reboot -no-shutdown
```

Le script télécharge une version épinglée de Limine et vérifie son SHA-256 avant de construire l’ISO. Le message attendu sur le port série est `ArchiaOS: Limine entered x86_64 kernel`. La CI exécute le même test de démarrage ; la réussite du build seul ne prouve pas que le noyau démarre.

## Architecture et limites

- Cible initiale : x86-64, ELF64, protocole Limine.
- Rust `no_std` pour le noyau et les diagnostics.
- Limine gère le passage initial en mode long et la pagination de démarrage.
- Aucun parsing de carte mémoire, allocateur, IDT, ordonnanceur, processus, syscall, stockage ou interface graphique n’est encore implémenté.

Voir `docs/` pour les limites par sous-système. Travaillez exclusivement sur la branche `ArchiaOS` et ne revendiquez pas une fonctionnalité avant de l’avoir vérifiée.
