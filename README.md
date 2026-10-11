# ArchiaOS

**ArchiaOS est un système d’exploitation expérimental en cours de construction.** Le dépôt contient désormais une première base de noyau bare-metal x86-64 en Rust `no_std`, une entrée assembleur Multiboot 1, un script de liaison, une configuration Cargo et une CI. Cette base est volontairement minimale : elle ne constitue pas encore un OS complet.

## État actuel

- **Cible visée :** x86-64, noyau ELF démarré par un chargeur compatible Multiboot 1.
- **Langages réellement présents :** Rust pour le code noyau et assembleur x86 pour la transition initiale vers le mode long 64 bits. C et C++ ne sont pas introduits faute de besoin démontré.
- **Diagnostic :** sortie série COM1.
- **Pagination d’amorçage :** identité-mapping des premiers 1 Gio avec des pages de 2 Mio ; ce n’est pas un gestionnaire de mémoire complet.
- **Build :** Cargo, cible `x86_64-unknown-none`, linker script et workflow GitHub Actions.
- **Validation :** le build a d’abord révélé une erreur de relocation assembleur, puis un correctif a fait passer la compilation CI. Une étape de démarrage QEMU est maintenant configurée ; consulter son exécution pour connaître le résultat du test de démarrage le plus récent.

Aucune prise en charge de matériel réel, aucune isolation utilisateur, aucun ordonnanceur, aucune IDT, aucun syscall, aucun système de fichiers et aucune interface graphique ne sont déclarés disponibles à ce stade.

## Vision et architecture cible

Le projet doit progresser par composants testables : démarrage et diagnostic, exceptions et interruptions, validation des informations boot, mémoire physique et virtuelle, ordonnanceur, processus et appels système, pilotes, stockage, système de fichiers, puis environnement utilisateur et graphique. Les dépendances et les résultats de tests priment sur l’ajout artificiel de modules.

## Langages

- **Rust `no_std` :** choix principal pour les nouveaux composants noyau lorsque les contraintes bare-metal le permettent.
- **Assembleur :** uniquement pour les opérations CPU indispensables à la transition de démarrage, aux interruptions ou au changement de contexte.
- **C :** uniquement pour les interfaces bas niveau ou l’intégration de composants qui le justifient.
- **C++ :** uniquement si un composant ciblé présente un avantage technique concret.
- **Python :** outils auxiliaires de génération et de test.
- **Langages web :** éventuellement pour une future couche utilisateur disposant réellement d’un runtime adapté, jamais pour le noyau.

Les choix détaillés et les limites actuelles figurent dans [ArchiaOS/docs/LANGUAGES.md](ArchiaOS/docs/LANGUAGES.md).

## Compilation et lancement

Prérequis : Rust stable, cible `x86_64-unknown-none` et QEMU x86-64. Depuis le dossier du projet :

```sh
cd ArchiaOS
rustup target add x86_64-unknown-none
cargo build --release
qemu-system-x86_64 -kernel target/x86_64-unknown-none/release/archiaos-kernel -serial stdio -display none -no-reboot -no-shutdown
```

Le message attendu sur la console série est `ArchiaOS: kernel entry reached (x86_64)`. Le démarrage n’est considéré comme validé que lorsque le test QEMU correspondant réussit effectivement dans la CI.

Instructions détaillées : [ArchiaOS/README.md](ArchiaOS/README.md).

## Roadmap P0–P7

- **P0 — Fondations :** build reproductible, diagnostics, CI, tests et documentation alignés sur le code.
- **P1 — Noyau et matériel :** exceptions, interruptions, mémoire et validation des informations de démarrage.
- **P2 — Exécution :** contextes, threads noyau, synchronisation et ordonnanceur.
- **P3 — Espace utilisateur :** protection, appels système, chargeur et programmes isolés.
- **P4 — Stockage :** interface bloc, pilote virtuel, système de fichiers et tests de corruption.
- **P5 — Interface utilisateur :** entrées, rendu graphique, composition et applications.
- **P6 — Réseau et performances :** réseau, compatibilité et mesures reproductibles.
- **P7 — Stabilisation :** audit de sécurité, tests prolongés et procédure de distribution.

Une phase n’est terminée qu’après validation de ses critères techniques et de ses tests. Le développement se fait exclusivement sur la branche **`ArchiaOS`** ; la branche `main` ne doit pas être modifiée.
