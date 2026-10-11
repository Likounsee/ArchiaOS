# ArchiaOS — documentation technique

## État actuel

Le noyau x86-64 expérimental est chargé au format ELF64 par Limine. Il est lié en espace virtuel haut et demande désormais la carte mémoire au bootloader. Au démarrage, le noyau vérifie la révision de protocole, valide le nombre d’entrées, les pointeurs non nuls, les tailles et les débordements des plages, puis rapporte le nombre de régions utilisables et leur taille totale. Cela ne constitue pas encore un allocateur physique.

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

Le script télécharge Limine v12.9.1 et vérifie le SHA-256 de l’archive avant de créer l’ISO. La CI vérifie la compilation, les adresses virtuelles ELF, la création de l’ISO et le démarrage ; elle exige le message d’entrée noyau et celui de validation de la carte mémoire.

## Limites

Aucun allocateur physique, tas, IDT, gestionnaire d’exceptions, ordonnanceur, processus, syscall, stockage ou interface graphique n’est encore implémenté. Le comptage des régions utilisables ne signifie pas que ces régions sont sûres à allouer : le noyau doit encore réserver ses propres segments, la pile, les tables de pages et toutes les plages requises par le bootloader.

Voir `docs/` pour les limites par sous-système. Travaillez exclusivement sur la branche `ArchiaOS`.
