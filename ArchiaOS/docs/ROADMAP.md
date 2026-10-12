# Roadmap et critères

- **P0 — Fondations : en cours.** Build bare-metal, ISO Limine, diagnostics, vérification ELF et smoke test QEMU. La CI valide aussi la carte mémoire et l'auto-test PMM de 130 pages. Restent un audit plus large des réservations et des tests supplémentaires.
- **P1 — Bootloader maison : fondations en place.** Validation ELF64, chargement high-half x86-64, BootInfo, `ExitBootServices` et tests de permissions sont implémentés ; restent les cas d’échec, le rollback et un audit plus large des réservations.
- **P2 — Ordonnanceur : non commencé.** Pas de contexte de tâche, synchronisation multiprocesseur ni ordonnancement.
- **P3 — Espace utilisateur : non commencé.** Pas de syscall, chargeur, ABI ni processus isolés.
- **P4 — Stockage : non commencé.** Pas de pilote bloc ni de système de fichiers. Préparer une couche **VFS (Virtual File System)** indépendante du format de stockage.
- **P4.1 — Préparation à OpenFS : à concevoir.** Définir le contrat d’intégration avec **Likounsee/OpenFS** : types et erreurs, fichiers/répertoires, volumes, propriété mémoire, synchronisation, limites de blocs et versionnement. Garder un adaptateur distinct du cœur du noyau.
- **P4.2 — Intégration OpenFS : future, après stabilisation d’OpenFS.** Confirmer son API, ses garanties et ses dépendances ; tester montage, lectures/écritures, répertoires, erreurs, corruption et accès concurrents pris en charge.
- **P5 — Graphique : non commencé.** Pas de framebuffer ou de compositeur.
- **P6 — Réseau et performances : non commencé.** Pas de pile réseau ni de benchmarks.
- **P7 — Distribution : non commencé.** Pas de version distribuable ni d’audit de sécurité final.

## Piste parallèle — bootloader maison et ARM64

Le firmware UEFI standard reste fourni par la machine. ArchiaOS développe son propre chargeur et son propre noyau. Ces jalons sont suivis séparément de P0–P7.

1. **B0 — Entrée UEFI : implémentée.** Crate Rust sans dépendance externe pour x86-64 et AArch64.
2. **B1 — Lecture du noyau : implémentée.** Ouvre `\\EFI\\ArchiaOS\\kernel.elf`, borne sa taille et gère les lectures partielles.
3. **B2 — Validation ELF64 : implémentée.** Contrôles de l'en-tête, de la table des programmes, des segments, tailles, alignements, débordements, recouvrements et entrée exécutable. Les segments RWX sont refusés ; tests unitaires présents.
4. **B3 — Chargement identité : implémenté sous contraintes.** Réserve les pages aux adresses physiques ELF, copie les octets et met le BSS à zéro. Le chemin identité reste celui du prototype AArch64.
5. **B4 — Chargement high-half x86-64 : validé en CI.** Construit des tables 4-niveaux, mappe la fenêtre high-half, rend les alias physiques du noyau NX et applique les permissions ELF. Le test vérifie que le code est RO+X, les données RW+NX et l'alias identité du code RO+NX.
6. **B5 — Protection x86-64 : validée en CI.** Le chargeur vérifie la capacité CPU NX, active EFER.NXE et CR0.WP avant le changement CR3 ; le noyau de test lit ensuite ces bits et examine les entrées de page effectives.
7. **B6 — Contrat BootInfo : implémenté.** ABI versionnée `ARCHIABI` contenant l'entrée noyau, la carte mémoire UEFI finale et les pointeurs de plateforme disponibles.
8. **B7 — Sortie UEFI : implémentée.** Collecte la carte mémoire immédiatement avant `ExitBootServices` et réessaie avec une clé fraîche en cas de `EFI_INVALID_PARAMETER`.
9. **B8 — Noyau x86-64 de production via le chargeur maison : validé en CI.** Le build `--features uefi-boot` démarre sous OVMF/QEMU, valide BootInfo et les descripteurs UEFI puis réussit le test PMM de 130 allocations/libérations. La chaîne Limine par défaut reste conservée.
10. **A0 — Entrée noyau ARM64 : implémentée.** Entrée AArch64, pile dédiée et console PL011 pour QEMU `virt`.
11. **A1 — Transfert UEFI ARM64 : validé sous AAVMF/QEMU.** Le chargeur copie les segments identité, synchronise l'I-cache, construit BootInfo puis branche au noyau. Sur le firmware testé, le noyau découvre la console via ACPI SPCR et valide BootInfo/la carte mémoire.
12. **A2 — Gestion mémoire ARM64 : PMM bitmap expérimental implémenté.** Initialise à partir des plages conventionnelles sous 4 Gio et réussit les tests d'allocations/libérations. Restent une MMU propre, les réservations exhaustives, la synchronisation SMP et l'extension au-delà de 4 Gio.
13. **A3 — Vecteurs d'exceptions ARM64 : première base implémentée.** Table alignée installée et test d'exception synchrone présents. Restent les handlers différenciés, retours d'exception contrôlés et interruptions asynchrones.
14. **A4 — Parité des architectures : en cours de conception.** Extraire les abstractions partagées tout en maintenant séparées les primitives spécifiques.
15. **B9 — Choix du démarrage par défaut : à faire.** Garder Limine tant que les tests de non-régression, les contrôles de sécurité et des essais sur matériel réel ne justifient pas la bascule générale vers le chargeur maison.

**Limites actuelles :** les adresses physiques chargées, le chargeur et les buffers de transfert doivent être accessibles dans l'identité sous 4 Gio ; la mémoire totale des segments est plafonnée à 128 Mio et le fichier noyau à 64 Mio. L'ARM64 testée utilise des segments identité ; la pagination high-half testée concerne x86-64. Les résultats démontrent les configurations QEMU CI, pas l'ensemble du matériel physique.

## Conditions d’intégration d’OpenFS

OpenFS reste un projet distinct et n’est pas supposé prêt tant que son propre dépôt et ses tests ne l’attestent pas. Avant l'intégration, confirmer son API publique, sa licence, ses cibles, ses dépendances, son allocation mémoire, sa synchronisation et sa gestion des erreurs. Ne pas copier prématurément son code ni dépendre d'API instables. L'intégration doit être versionnée, reproductible et testée en CI.

## Critères de progression

Une phase ne passe à 100 % qu’après satisfaction de tous ses critères et tests documentés. L'intégration OpenFS ne compte pas comme réalisée avant que le dépôt OpenFS soit suffisamment stable et que les tests d'intégration ArchiaOS passent.
