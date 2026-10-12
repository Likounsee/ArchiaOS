# Bootloader maison et prise en charge ARM64

## Objectif

ArchiaOS conserve le firmware UEFI de la machine, mais développe son propre chargeur et son propre noyau. Le code spécifique à une architecture doit être isolé du cœur commun. x86-64 reste la plateforme de référence jusqu'à ce que le nouveau démarrage soit validé ; ARM64/AArch64 est préparé dans le même dépôt, sans prétendre que le noyau actuel y démarre déjà.

## État réel

- **Démarrage x86-64 actuel :** le noyau Rust `no_std` est lancé par Limine et consomme encore les structures Limine pour obtenir la carte mémoire.
- **Bootloader ArchiaOS :** le crate UEFI sans dépendance externe ouvre `\\EFI\\ArchiaOS\\kernel.elf`, lit le contenu depuis la partition EFI et valide les en-têtes/segments ELF64 via une bibliothèque testée séparément. Il ne place pas encore les segments en mémoire et n'est pas le chemin de démarrage par défaut.
- **ARM64 :** les cibles UEFI du bootloader peuvent être compilées dans la CI. Le noyau actuel n'est pas encore compilable/démarrable en AArch64 : son entrée assembleur, ses E/S série et sa gestion de la carte mémoire sont spécifiques à x86-64/Limine.
- **Firmware :** UEFI standard fourni par le matériel ou par la machine virtuelle, non développé par ArchiaOS.

## Architecture cible

- `bootloader/`: chargeur UEFI développé par ArchiaOS ; lecture du noyau, validation ELF, allocation de pages, collecte des informations plateforme, sortie des services UEFI et transfert de contrôle.
- `kernel/core/`: logique portable, indépendante du jeu d'instructions autant que possible.
- `kernel/arch/x86_64/`: entrée, tables de pages, exceptions/interruptions, CPU et primitives x86-64.
- `kernel/arch/aarch64/`: entrée, exceptions, MMU/TLB, état des niveaux d'exception et primitives AArch64.
- `kernel/platform/`: différences de plateforme (ACPI/Device Tree, timer, contrôleur d'interruptions, console et périphériques).
- `vfs/` et adaptateur OpenFS : interfaces de stockage indépendantes du CPU.

Cette structure est une cible de migration, pas une description des répertoires déjà implémentés. Les fichiers existants ne doivent être déplacés qu'avec tests et commits cohérents.

## Jalons du bootloader

1. Entrée UEFI et diagnostics sur x86-64 et AArch64 — prototype présent.
2. Tests de compilation et tests unitaires du parseur ajoutés ; le lancement QEMU/OVMF du lecteur UEFI est maintenant prévu en CI.
3. Accès au système de fichiers EFI et lecture bornée d'un noyau — implémenté pour `\\EFI\\ArchiaOS\\kernel.elf`.
4. Parseur ELF64 avec tests de segments valides, tronqués, chevauchants, débordements, mauvaises tailles et alignements invalides.
5. Contrat de handoff versionné et commun aux architectures.
6. Carte mémoire finale, allocations et `ExitBootServices` robustes.
7. Transfert vers le noyau x86-64, puis retrait progressif des réponses Limine.
8. Boot AArch64 et tests QEMU dédiés.

## Jalons ARM64

1. Garder le code commun indépendant des registres et instructions spécifiques.
2. Créer une entrée AArch64 et une console de diagnostic adaptée (pas d'E/S port x86).
3. Définir un contrat de mémoire portable, puis séparer les sources de carte mémoire UEFI de l'ancien protocole Limine.
4. Implémenter l'initialisation MMU/TLB, les vecteurs d'exceptions et un timer AArch64.
5. Ajouter une cible de build et un test de démarrage QEMU AArch64 ; ensuite seulement élargir aux plateformes matérielles réelles.

## Critères de réussite

La compilation d'une application UEFI ARM64 ne signifie pas que le noyau ArchiaOS est compatible ARM64. Cette compatibilité ne sera annoncée qu'après compilation du noyau AArch64, démarrage sous QEMU, validation de la mémoire et exécution de tests automatisés. Le bootloader ne remplace Limine qu'après un test de bout en bout reproductible.
