# ArchiaOS

**ArchiaOS est un système d’exploitation expérimental en cours de construction.** Le noyau x86-64 Rust `no_std` démarre avec Limine et est testé sous QEMU par GitHub Actions.

## État actuel

- **Démarrage :** l’ISO Limine BIOS/UEFI se construit et le noyau atteint son entrée Rust en mode long x86-64 ; la sortie COM1 est vérifiée par le smoke test QEMU.
- **Carte mémoire :** la réponse Limine est contrôlée pour les pointeurs, le nombre d’entrées, les longueurs nulles et les débordements.
- **Mémoire physique :** allocateur bitmap expérimental pour les pages sous 4 Gio. Le test QEMU confirme l’allocation de deux pages distinctes, l’alignement, la baisse puis le rétablissement du compteur, le rejet des libérations invalides et le rejet du double-free.
- **Langages :** Rust pour le noyau ; un petit stub assembleur aligne la pile à l’entrée. C/C++ ne sont pas ajoutés sans besoin démontré.
- **Build :** Cargo, linker ELF avec segments virtuels hauts, génération d’ISO Limine, vérification des segments ELF et CI de démarrage.

Le noyau ne fournit pas encore d’IDT, de gestionnaire d’exceptions, d’ordonnanceur, de processus isolés, d’appels système, de stockage, de système de fichiers, de réseau ni d’interface graphique. L’allocateur est limité à 4 Gio, initialisé une seule fois et non synchronisé pour SMP ; il reste expérimental.

## Compilation et lancement

Prérequis : Rust stable, cible `x86_64-unknown-none`, `curl`, `make`, `xorriso` et QEMU x86-64. Depuis le dossier du projet :

```sh
cd ArchiaOS
rustup target add x86_64-unknown-none
cargo build --release
scripts/make-iso.sh
qemu-system-x86_64 -cdrom build/archiaos.iso -boot d -serial stdio -display none -no-reboot -no-shutdown
```

Le script télécharge Limine v12.9.1 et vérifie son SHA-256. Les instructions détaillées se trouvent dans [ArchiaOS/README.md](ArchiaOS/README.md).

## Roadmap P0–P7

- **P0 — Fondations :** build reproductible, diagnostics, CI, tests et documentation alignés sur le code.
- **P1 — Noyau et matériel :** exceptions, interruptions, validation mémoire et allocateur de pages.
- **P2 — Exécution :** contextes, threads noyau, synchronisation et ordonnanceur.
- **P3 — Espace utilisateur :** protection, appels système, chargeur et programmes isolés.
- **P4 — Stockage :** interface bloc, pilote virtuel, système de fichiers et tests de corruption.
- **P5 — Interface utilisateur :** entrées, rendu graphique, composition et applications.
- **P6 — Réseau et performances :** réseau, compatibilité et mesures reproductibles.
- **P7 — Stabilisation :** audit de sécurité, tests prolongés et procédure de distribution.

Les phases ne seront terminées qu’après validation de leurs critères et tests. Le développement se fait exclusivement sur la branche **`ArchiaOS`** ; `main` ne doit pas être modifiée.
