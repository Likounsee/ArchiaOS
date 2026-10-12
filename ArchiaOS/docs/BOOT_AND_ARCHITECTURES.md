# Bootloader maison et prise en charge ARM64

## Objectif

ArchiaOS conserve le firmware UEFI standard de la machine, mais développe son propre bootloader et son propre noyau. Les composants communs doivent être séparés des éléments propres à chaque architecture. La chaîne Limine reste disponible pour le noyau de production x86-64 jusqu'à validation complète d'un démarrage indépendant.

## État réel du code

- **Noyau de production x86-64 :** il démarre encore avec Limine et utilise les réponses Limine pour initialiser la carte mémoire. Son image est liée en high-half ; le chargeur maison actuel refuse les segments qui nécessitent une traduction virtuelle.
- **Bootloader maison :** lit `\\EFI\\ArchiaOS\\kernel.elf`, limite la taille, valide ELF64, réserve les pages physiques exactes pour les segments identité, copie les octets et met le BSS à zéro. Il construit le BootInfo versionné `ARCHIABI`, collecte la carte mémoire finale et quitte les Boot Services avec une logique de nouvelle tentative pour une clé de carte périmée.
- **Handoff x86-64 :** une image de test identité distincte démarre sous OVMF/QEMU et vérifie BootInfo. Ce n'est pas encore le démarrage du noyau de production.
- **Handoff AArch64 :** le même chemin d'allocation et de transfert est maintenant activé pour la cible UEFI AArch64, avec maintenance du cache d'instructions avant l'exécution. Le noyau prototype reçoit le pointeur BootInfo dans `x0`, valide l'ABI et les descripteurs de mémoire, et conserve également le mode de démarrage direct QEMU. Un job AAVMF/QEMU de bout en bout est ajouté à la CI ; son résultat doit être confirmé avant d'annoncer la compatibilité ARM64 validée.
- **Firmware :** UEFI standard fourni par le matériel ou la machine virtuelle ; aucun firmware maison n'est développé.

## Architecture cible

- `bootloader/` : code UEFI ArchiaOS, lecteur FAT via les protocoles firmware, parseur ELF, allocation des segments et handoff.
- `kernel/core/` : logique portable, indépendante du jeu d'instructions.
- `kernel/arch/x86_64/` : entrée, tables de pages, exceptions/interruptions, CPU et primitives x86-64.
- `kernel/arch/aarch64/` : entrée, exceptions, MMU/TLB, niveau d'exception et primitives AArch64.
- `kernel/platform/` : différences de plateforme : ACPI/Device Tree, timer, contrôleur d'interruptions, console et périphériques.
- `vfs/` et adaptateur OpenFS : interfaces de stockage indépendantes du CPU.

Cette structure est une cible de migration, pas une description des dossiers déjà implémentés. Les réorganisations doivent se faire avec des tests et des commits cohérents.

## Jalons du bootloader

1. Entrée UEFI et diagnostics x86-64/AArch64 : code présent.
2. Lecture du fichier noyau depuis la partition EFI et contrôle de taille : implémentés.
3. Validation ELF64 des en-têtes, segments, débordements, alignements, recouvrements et point d'entrée : parseur et tests unitaires présents.
4. Chargement des segments identité, allocation physique exacte, copie des segments et effacement du BSS : implémentés sous la contrainte de segments identité en dessous de 4 Gio.
5. Contrat BootInfo versionné et carte mémoire finale : implémentés ; BootInfo est passé dans `RDI` sur x86-64 ou `x0` sur AArch64.
6. `ExitBootServices` : implémenté avec nouvelle tentative après clé de carte périmée ; des échecs postérieurs à une première tentative provoquent un arrêt contrôlé sans nouveaux appels de service.
7. Test x86-64 OVMF/QEMU : job CI présent pour le noyau de handoff.
8. Test AArch64 AAVMF/QEMU : job CI ajouté pour le prototype noyau ; résultat en attente de confirmation.
9. Remplacement de Limine pour le noyau principal high-half : reste à faire, avec tables de pages et contrat mémoire noyau à concevoir.

## Jalons ARM64

1. Compilation de l'application UEFI AArch64 : prévue dans la CI.
2. Entrée noyau AArch64, pile dédiée et diagnostic PL011 : implémentés.
3. Transmission de BootInfo et validation de la carte mémoire : implémentées dans le prototype.
4. Chargement de segments identité et synchronisation I-cache : implémentés dans le loader, en attente du test QEMU UEFI de bout en bout.
5. Vecteurs d'exceptions, MMU/TLB, timer et contrôleur d'interruptions : à implémenter.
6. Support Device Tree et abstractions de plateforme : à implémenter.
7. Partage progressif du cœur portable et tests de non-régression x86-64/AArch64 : à faire.

La compilation ARM64 seule n'est pas une preuve de compatibilité. Celle-ci nécessite que la CI compile, démarre le noyau via UEFI et confirme le transfert BootInfo avec les marqueurs attendus.
