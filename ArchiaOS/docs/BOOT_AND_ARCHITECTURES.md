# Bootloader maison et prise en charge ARM64

## Objectif

ArchiaOS conserve le firmware UEFI standard de la machine, mais développe son propre bootloader et son propre noyau. Les composants communs doivent être séparés des éléments propres à chaque architecture. La chaîne Limine reste disponible pour le noyau de production x86-64 jusqu'à validation complète d'un démarrage indépendant.

## État réel du code

- **Noyau de production x86-64 :** il démarre encore avec Limine et utilise les réponses Limine pour initialiser la carte mémoire. Son image est liée en high-half ; le chargeur maison actuel refuse les segments qui nécessitent une traduction virtuelle.
- **Bootloader maison :** lit `\\EFI\\ArchiaOS\\kernel.elf`, borne sa taille, valide ELF64, réserve les pages physiques exactes pour les segments identité, copie les octets et met le BSS à zéro. Il construit le BootInfo versionné `ARCHIABI`, collecte la carte mémoire finale et quitte les Boot Services avec une logique de nouvelle tentative pour une clé de carte périmée.
- **Handoff x86-64 :** une image de test identité distincte démarre sous OVMF/QEMU et vérifie BootInfo. Le BootInfo reçoit l'adresse RSDP si les tables de configuration UEFI fournissent ACPI 2.0/1.0. Ce n'est pas encore le démarrage du noyau de production.
- **Handoff AArch64 :** le même chemin d'allocation et de transfert fonctionne sous AAVMF/QEMU. Le chargeur transmet le Device Tree validé si `EFI_DTB_TABLE_GUID` est publié, ou le RSDP ACPI validé lorsque le firmware ne fournit pas de FDT. Le noyau prototype reçoit BootInfo dans `x0`, valide l'ABI et la carte mémoire. Sur la configuration AAVMF actuelle, la console PL011 est découverte via ACPI SPCR. La CI a validé ce transfert, le PMM et le test d'exception synchrone.
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
3. Validation ELF64 des en-têtes, segments, débordements, alignements, recouvrements et point d'entrée exécutable : parseur et tests unitaires présents.
4. Chargement des segments identité, allocation physique exacte, copie des segments et effacement du BSS : implémentés sous la contrainte de segments identité en dessous de 4 Gio.
5. Contrat BootInfo versionné et carte mémoire finale : implémentés ; BootInfo contient RSDP/Device Tree lorsque les tables de configuration UEFI les fournissent.
6. `ExitBootServices` : implémenté avec nouvelle tentative après clé de carte périmée ; des échecs postérieurs à une première tentative de sortie provoquent un arrêt contrôlé sans nouveaux appels de service.
7. Test x86-64 OVMF/QEMU : job CI présent pour le noyau de handoff.
8. Test AArch64 AAVMF/QEMU : job CI ajouté pour le prototype noyau ; résultat en attente de confirmation.
9. Remplacement de Limine pour le noyau principal high-half : reste à faire, avec tables de pages et contrat mémoire noyau à concevoir.

## Jalons ARM64

1. Compilation de l'application UEFI AArch64 : présente dans la CI.
2. Entrée noyau AArch64, pile dédiée et diagnostic PL011 : implémentés.
3. Transmission et validation de BootInfo, de la carte mémoire et du header Device Tree : implémentées dans le prototype.
4. Chargement de segments identité et synchronisation I-cache : implémentés dans le loader, en attente du test QEMU UEFI de bout en bout.
5. Vecteurs d'exceptions, MMU/TLB, timer et contrôleur d'interruptions : à implémenter.
6. Découverte des régions mémoire et des interruptions/timers, gestion des propriétés `ranges` pour les bus non identitaires et abstractions de plateforme : à implémenter.
7. Partage progressif du cœur portable et tests de non-régression x86-64/AArch64 : à faire.

Le transfert UEFI vers un noyau de test ARM64 est désormais validé sous AAVMF/QEMU, avec ACPI SPCR sur cette configuration. Cela confirme le chemin testé, pas la compatibilité de tout le matériel ARM64 : pilotes, MMU, timers, interruptions et variantes de firmware restent à traiter.

## Conditions d’intégration d’OpenFS

OpenFS reste un projet distinct et n’est pas supposé prêt tant que son propre dépôt et ses tests ne l’attestent pas. Avant l'intégration, confirmer son API publique, sa licence, ses cibles, ses dépendances, son allocation mémoire, sa synchronisation et sa gestion des erreurs. Ne pas copier prématurément son code ni dépendre d'API instables. L'intégration doit être versionnée, reproductible et testée en CI.
