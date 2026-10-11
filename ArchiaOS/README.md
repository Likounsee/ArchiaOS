# ArchiaOS — documentation technique

## État actuel

Le projet contient un noyau x86-64 expérimental écrit en Rust `no_std`, chargé au format ELF64 par Limine. Le noyau est lié en espace virtuel haut avec une adresse physique de chargement à partir de 1 Mio. Limine fournit l’entrée en mode long et les tables de pagination initiales ; un stub assembleur aligne la pile puis appelle Rust, qui initialise COM1 et écrit des diagnostics.

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

Le script télécharge Limine v12.9.1 et vérifie le SHA-256 de l’archive avant de créer l’ISO. Le message attendu sur le port série est `ArchiaOS: Limine entered x86_64 kernel`. La CI vérifie séparément la compilation, la création de l’ISO et le démarrage ; la réussite du build seul ne prouve pas que le noyau démarre.

## Architecture et limites

- Cible initiale : x86-64, ELF64, protocole Limine.
- Adresse virtuelle du noyau dans la moitié haute ; adresse physique de chargement à partir de 1 Mio.
- Rust `no_std` pour le noyau et les diagnostics.
- Pas encore de parsing de carte mémoire, allocateur, IDT, ordonnanceur, processus, syscall, stockage ou interface graphique.

Voir `docs/` pour les limites par sous-système. Travaillez exclusivement sur la branche `ArchiaOS` et ne revendiquez pas une fonctionnalité avant de l’avoir vérifiée.
