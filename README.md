# ArchiaOS

**ArchiaOS est un système d’exploitation expérimental en cours de construction.** Le dépôt contient une première base de noyau bare-metal x86-64 en Rust `no_std`, utilisant le protocole de démarrage natif de Limine, une sortie série et une chaîne de build Cargo/ELF.

## État actuel

- **Cible :** x86-64 ; noyau ELF chargé par Limine en mode long.
- **Langages présents :** Rust pour le code noyau ; pas de C/C++ sans besoin d’intégration démontré.
- **Diagnostic :** port série COM1.
- **Build :** Cargo, script de liaison, configuration de cible et génération d’une ISO Limine.
- **Validation :** la compilation a réussi en CI après correction d’une relocation assembleur. Le test QEMU a révélé que le chargeur direct de QEMU ne charge pas cette image ELF64 ; la CI est désormais configurée pour démarrer une ISO Limine et vérifier la sortie série. Le démarrage ne sera considéré comme validé qu’après réussite effective de cette nouvelle étape.

Le noyau ne fournit pas encore de gestionnaire mémoire complet, d’IDT, d’ordonnanceur, de processus isolés, d’appels système, de pilotes de stockage, de système de fichiers, de réseau ni d’interface graphique. Il n’est pas annoncé comme prêt pour du matériel réel.

## Langages et architecture

- **Rust `no_std` :** nouveau code noyau lorsque les contraintes bare-metal le permettent.
- **Assembleur :** limité aux opérations CPU qui ne peuvent pas être évitées proprement.
- **C/C++ :** seulement avec justification technique.
- **Python :** outils auxiliaires éventuels ; langages web uniquement pour une future couche utilisateur avec runtime réel.

La prochaine priorité est la validation des données de démarrage, puis les exceptions/interruptions et la mémoire physique.

## Compilation et lancement

Prérequis : Rust stable, cible `x86_64-unknown-none`, `curl`, `make`, `xorriso` et QEMU x86-64. Depuis le dossier du projet :

```sh
cd ArchiaOS
rustup target add x86_64-unknown-none
cargo build --release
scripts/make-iso.sh
qemu-system-x86_64 -cdrom build/archiaos.iso -boot d -serial stdio -display none -no-reboot -no-shutdown
```

Le script télécharge Limine v12.9.1 et vérifie son SHA-256 avant utilisation. Les instructions techniques se trouvent dans [ArchiaOS/README.md](ArchiaOS/README.md).

## Roadmap P0–P7

- **P0 — Fondations :** build reproductible, diagnostics, CI, tests et documentation alignés sur le code.
- **P1 — Noyau et matériel :** exceptions, interruptions, validation des informations boot et mémoire.
- **P2 — Exécution :** contextes, threads noyau, synchronisation et ordonnanceur.
- **P3 — Espace utilisateur :** protection, appels système, chargeur et programmes isolés.
- **P4 — Stockage :** interface bloc, pilote virtuel, système de fichiers et tests de corruption.
- **P5 — Interface utilisateur :** entrées, rendu graphique, composition et applications.
- **P6 — Réseau et performances :** réseau, compatibilité et mesures reproductibles.
- **P7 — Stabilisation :** audit de sécurité, tests prolongés et procédure de distribution.

Les phases ne seront terminées qu’après validation de leurs critères et tests. Le développement se fait exclusivement sur la branche **`ArchiaOS`** ; `main` ne doit pas être modifiée.
