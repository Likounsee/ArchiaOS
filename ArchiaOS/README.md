# ArchiaOS — documentation technique

## État actuel

Le noyau x86-64 expérimental est chargé au format ELF64 par Limine. Il valide la carte mémoire puis initialise un allocateur bitmap physique pour les frames sous 4 Gio. Un bitmap marque les frames gérées, un second les frames libres, ce qui empêche de libérer une adresse réservée arbitraire. Le smoke test QEMU vérifie deux allocations distinctes, l’alignement, le compteur libre, la libération, les adresses invalides et le double-free.

## Prérequis et commandes

Prérequis : Rust stable, cible `x86_64-unknown-none`, `curl`, `sha256sum`, `make`, `xorriso` et QEMU x86-64.

```sh
rustup target add x86_64-unknown-none
cargo build --release
scripts/make-iso.sh
qemu-system-x86_64 -cdrom build/archiaos.iso -boot d -serial stdio -display none -no-reboot -no-shutdown
```

La CI vérifie les adresses virtuelles ELF, crée l’ISO et démarre QEMU. Elle exige les messages d’entrée noyau, de validation de carte mémoire et de réussite de l’auto-test PMM.

## Limites

L’allocateur est limité aux 4 premiers Gio, initialisé une seule fois et non synchronisé pour SMP. Les plages du noyau et du bootloader sont exclues via la carte mémoire Limine, mais les réservations doivent encore être auditées avant un usage général. Aucun IDT, gestionnaire d’exceptions, ordonnanceur, processus, syscall, stockage ou interface graphique n’est encore implémenté.
