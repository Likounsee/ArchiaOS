# Roadmap et critères

- **P0 — Fondations : en cours.** Build bare-metal, ISO Limine, diagnostics, vérification ELF et smoke test QEMU réussis. La CI vérifie aussi la carte mémoire et l’auto-test PMM sur 130 pages. Restent un contrôle plus large des réservations, le formatage automatisé et des tests supplémentaires.
- **P1 — Noyau et matériel : premières fondations.** Entrée Limine, sortie série, validation de la carte mémoire et allocateur bitmap physique testés en QEMU. Restent IDT, exceptions, réservations mémoire complètes, gestion des fautes et synchronisation.
- **P2 — Ordonnanceur : non commencé.** Pas de contexte de tâche, synchronisation multiprocesseur ni ordonnancement.
- **P3 — Espace utilisateur : non commencé.** Pas de syscall, chargeur, ABI ni processus isolés.
- **P4 — Stockage : non commencé.** Pas de pilote bloc ni de système de fichiers. Préparer une couche **VFS (Virtual File System)** indépendante du format de stockage, afin que le noyau et les programmes utilisent des interfaces stables plutôt que l’API d’un système de fichiers précis.
- **P4.1 — Préparation à OpenFS : à concevoir.** Définir le contrat d’intégration entre ArchiaOS et le dépôt séparé **Likounsee/OpenFS** : types et erreurs, opérations fichiers/répertoires, gestion des volumes, règles de propriété mémoire, synchronisation, limites de blocs et stratégie de version. Maintenir un adaptateur distinct afin de ne pas coupler le cœur du noyau à l’implémentation interne d’OpenFS. Cette préparation peut commencer avant qu’OpenFS soit terminé, mais ne doit pas bloquer les fondations P0/P1.
- **P4.2 — Intégration OpenFS : future, après stabilisation d’OpenFS.** Vérifier d’abord les exigences et garanties documentées d’OpenFS, puis l’intégrer derrière la VFS au moyen d’un adaptateur. Valider dans des tests isolés et en QEMU la détection/montage d’un volume, les lectures/écritures, les répertoires, les erreurs, les accès concurrents lorsque pris en charge, et les cas de corruption ou d’entrées malformées. N’activer l’intégration par défaut qu’après des tests reproductibles et une revue de sécurité; conserver une option de build sans OpenFS tant que l’intégration reste expérimentale.
- **P5 — Graphique : non commencé.** Pas de framebuffer ou de compositeur.
- **P6 — Réseau et performances : non commencé.** Pas de pile réseau ni de benchmarks.
- **P7 — Distribution : non commencé.** Pas de version distribuable ni d’audit de sécurité final.

## Piste parallèle — bootloader maison et ARM64

Cette piste est lancée sans prétendre que les fonctionnalités sont terminées. Le firmware UEFI standard de la machine reste utilisé ; ArchiaOS développe son propre chargeur et son noyau.

1. **B0 — Entrée UEFI : prototype ajouté.** Le crate `bootloader/` Rust sans dépendance externe définit l’entrée UEFI et affiche un diagnostic. La CI tente sa compilation pour x86-64 et AArch64. Ce jalon ne charge pas encore le noyau.
2. **B1 — Validation du prototype : à faire.** Vérifier les structures UEFI et démarrer une image EFI sous QEMU/OVMF pour chaque architecture.
3. **B2 — Chargement du noyau : à faire.** Lire le noyau depuis la partition EFI, valider les en-têtes ELF64 et les segments, et tester les entrées malformées.
4. **B3 — Handoff : à faire.** Définir une structure de démarrage stable et portable, collecter la carte mémoire et les informations plateforme, puis appeler `ExitBootServices` de façon robuste.
5. **B4 — Remplacement de Limine sur x86-64 : à faire.** Retirer progressivement la dépendance du noyau aux réponses Limine ; ne changer le chemin par défaut qu'après un test bout en bout reproductible.
6. **A0 — Préparation ARM64 : en cours.** La compilation de l'application UEFI AArch64 est visée par la CI. Le noyau lui-même reste x86-64 seulement.
7. **A1 — Noyau AArch64 minimal : à faire.** Ajouter une entrée propre à AArch64, une console de diagnostic, un contrat de mémoire indépendant de Limine et les premiers tests QEMU.
8. **A2 — Fondations matérielles ARM64 : à faire.** Exceptions, MMU/TLB, timer et contrôleur d'interruptions adaptés à la plateforme.
9. **A3 — Parité fonctionnelle : à faire.** Partager le cœur portable et tester séparément les implémentations spécifiques à chaque architecture.

La compilation du bootloader pour ARM64 ne compte pas comme compatibilité ARM64 du noyau. Cette dernière ne sera annoncée qu'après compilation et démarrage du noyau AArch64 avec tests automatisés.

## Conditions d’intégration d’OpenFS

OpenFS reste un projet distinct et n’est pas supposé être prêt tant que son propre dépôt et ses tests ne l’attestent pas. Avant de l’intégrer, confirmer son API publique, son modèle de licence, ses cibles d’architecture, ses dépendances, son modèle d’allocation mémoire, ses besoins de synchronisation et sa gestion des erreurs. Éviter de copier prématurément son code dans ArchiaOS ou de dépendre d’API instables. L’intégration doit être versionnée et reproductible dans la CI, avec tests de compatibilité et de non-régression.

## Critères de progression

Une phase ne passe à 100 % qu’après satisfaction de tous ses critères et tests documentés. L’intégration OpenFS est une étape future : elle ne compte pas comme réalisée avant que le dépôt OpenFS soit suffisamment stable et que les tests d’intégration d’ArchiaOS passent.
