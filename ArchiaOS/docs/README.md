# Documentation ArchiaOS

Cette documentation décrit l'architecture, les composants, les validations et les priorités du projet. Les fonctionnalités décrites comme prévues ou en cours ne doivent pas être considérées comme implémentées.

## Architecture et démarrage
- [Architecture générale](ARCHITECTURE.md)
- [Démarrage UEFI](BOOT.md)
- [Démarrage et architectures](BOOT_AND_ARCHITECTURES.md)
- [Langages et conventions](LANGUAGES.md)

## Noyau et composants
- [Contrôle système](CONTROL_CENTER.md)
- [Pilotes](DRIVERS.md)
- [Système de fichiers](FILESYSTEM.md)
- [Contrôle matériel](HARDWARE_CONTROL.md)
- [Interruptions](INTERRUPTS.md)
- [Mémoire](MEMORY.md)
- [Ordonnanceur](SCHEDULER.md)
- [Appels système](SYSCALLS.md)
- [Espace utilisateur](USERLAND.md)

## Qualité, sécurité et performances
- [Politique de fonctionnalités CPU](CPU-FEATURE-POLICY.md)
- [Sécurité](SECURITY.md)
- [Performances](PERFORMANCE.md)
- [Tests et validation](TESTING.md)
- [Validation physique P1](P1-PHYSICAL-VALIDATION.md)
- [Preuves matérielles](preuves/README.md)

## Planification
- [Roadmap](ROADMAP.md)

Les sources Rust et les scripts de validation font foi pour l'état réel des fonctionnalités. La documentation sert de guide et doit rester alignée sur le code.
