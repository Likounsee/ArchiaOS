# Bootloader maison et prise en charge ARM64

## Objectif

ArchiaOS conserve le firmware UEFI standard de la machine, mais développe son propre bootloader et son propre noyau. Les composants communs sont séparés progressivement des éléments propres à chaque architecture. Le bootloader UEFI ArchiaOS est le chemin de démarrage par défaut ; la CI doit confirmer la bascule après retrait des anciens scripts Limine.

## État réel du code

- **Noyau de production x86-64 :** l’entrée UEFI ArchiaOS est désormais le chemin de compilation par défaut. Le noyau reçoit BootInfo maison et initialise le PMM depuis les descripteurs UEFI. La CI doit être verte sur le nouveau HEAD pour revalider ce chemin après suppression de Limine.
- **Bootloader maison :** lit `\\EFI\\ArchiaOS\\kernel.elf`, borne la taille du fichier, valide ELF64, réserve les pages physiques exactes, copie les segments et met le BSS à zéro. Il construit le BootInfo versionné `ARCHIABI`, récupère la carte mémoire finale et appelle `ExitBootServices` en réessayant si la clé de carte devient périmée.
- **Pagination x86-64 :** le chargeur construit des tables 4-niveaux pour les segments high-half de la fenêtre ArchiaOS. Les tables identitaires couvrent uniquement la RAM reconnue sous 4 Gio. Les alias physiques du noyau sont NX, le code high-half est en lecture seule/exécutable, les données sont NX et seules les sections ELF marquées écriture sont inscriptibles. Les segments ELF RWX sont refusés. Le chargeur vérifie le support CPU de NX et active EFER.NXE/CR0.WP avant de basculer. Le trampoline UEFI reste brièvement exécutable, en lecture seule, dans l'espace identité pour achever la transition CR3.
- **Handoff du noyau de production x86-64 :** la CI a compilé et démarré le vrai crate `archiaos-kernel` avec `uefi-boot` sous OVMF/QEMU. Le noyau a validé BootInfo et la carte mémoire UEFI puis passé son auto-test PMM de 130 allocations/libérations. Un noyau high-half de test vérifie aussi les bits NX/W^X et la protection superviseur.
- **Handoff AArch64 :** le chemin UEFI maison démarre sous AAVMF/QEMU. Le noyau prototype reçoit BootInfo dans `x0`, valide la carte mémoire, découvre la console PL011 via ACPI SPCR sur la configuration testée, initialise le PMM et réussit l'auto-test d'exception synchrone.
- **Limites communes actuelles :** noyaux et buffers de transfert doivent rester dans la fenêtre de mémoire identité prise en charge sous 4 Gio ; la mémoire totale des segments ELF est plafonnée à 128 Mio et le fichier noyau à 64 Mio. Le chemin ARM64 accepte des segments identité, tandis que la traduction high-half spécifique est actuellement x86-64. Les tests couvrent les machines virtuelles CI, pas toutes les implémentations physiques.
- **Firmware :** UEFI standard fourni par le matériel ou la machine virtuelle ; aucun firmware maison n'est développé.

## Architecture cible

- `bootloader/` : chargeur UEFI ArchiaOS, lecteur via les protocoles firmware, parseur ELF, allocation des segments et handoff.
- `kernel/core/` : logique portable, indépendante du jeu d'instructions.
- `kernel/arch/x86_64/` : entrée, tables de pages, exceptions/interruptions, CPU et primitives x86-64.
- `kernel/arch/aarch64/` : entrée, exceptions, MMU/TLB, niveaux d'exception et primitives AArch64.
- `kernel/platform/` : ACPI/Device Tree, timers, contrôleur d'interruptions, console et périphériques propres à la plateforme.
- `vfs/` et adaptateur OpenFS : interfaces de stockage indépendantes du CPU.

Cette structure est une cible de migration, pas une description des dossiers déjà implémentés. Les réorganisations doivent se faire avec des tests et des commits cohérents.

## Jalons du bootloader

1. Entrée UEFI et diagnostics x86-64/AArch64 : implémentés.
2. Lecture du noyau depuis la partition EFI et contrôle de taille : implémentés.
3. Validation ELF64 des en-têtes, segments, débordements, alignements, recouvrements et point d'entrée exécutable : parseur, tests unitaires et refus des segments RWX présents.
4. Chargement identité : implémenté pour les images identité sous la fenêtre physique supportée ; demeure le chemin AArch64 testé.
5. Chargement x86-64 high-half : tables de pages propres, alias physiques NX, permissions W^X et transition avec CR0.WP/EFER.NXE testés sous OVMF/QEMU.
6. Contrat BootInfo versionné et carte mémoire finale : implémentés ; BootInfo transporte le RSDP/Device Tree disponible.
7. `ExitBootServices` : implémenté avec nouvelle tentative pour une clé de carte périmée.
8. Démarrage du noyau de production avec le bootloader maison : validé en CI sous OVMF/QEMU avec `uefi-boot`, BootInfo UEFI et auto-test PMM.
9. Bascule du chemin par défaut : code basculé vers le bootloader maison ; la CI sur le nouveau HEAD et des essais sur matériel réel restent nécessaires avant de considérer la migration complètement validée.

## Jalons ARM64

1. Compilation de l'application UEFI AArch64 : présente dans la CI.
2. Entrée noyau AArch64, pile dédiée et diagnostic PL011 : implémentés.
3. Transmission et validation de BootInfo et de la carte mémoire UEFI : implémentées.
4. Chargement de segments identité et synchronisation I-cache : testés dans le transfert UEFI AAVMF/QEMU de bout en bout.
5. PMM bitmap et auto-test d'allocations/libérations : validés dans le prototype.
6. Vecteurs d'exceptions et piège synchrone : première base implémentée et testée.
7. MMU/TLB propre, remappage mémoire, timers et contrôleur d'interruptions : à implémenter.
8. Traduction de `ranges` et des régions de périphériques, gestion plus complète du Device Tree, partage du cœur portable et tests sur du matériel ARM64 : à faire.

Le transfert UEFI vers le prototype ARM64 est validé sur AAVMF/QEMU, avec ACPI SPCR sur cette configuration. Cela ne démontre pas la compatibilité de tout le matériel ARM64 : pilotes, MMU, timers, interruptions et variantes de firmware restent à traiter.

## Conditions d’intégration d’OpenFS

OpenFS reste un projet distinct et n’est pas supposé prêt tant que son propre dépôt et ses tests ne l’attestent pas. Avant l'intégration, confirmer son API publique, sa licence, ses cibles, ses dépendances, son allocation mémoire, sa synchronisation et sa gestion des erreurs. Ne pas copier prématurément son code ni dépendre d'API instables. L'intégration doit être versionnée, reproductible et testée en CI.
