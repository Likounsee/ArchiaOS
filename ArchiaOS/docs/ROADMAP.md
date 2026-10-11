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

## Conditions d’intégration d’OpenFS

OpenFS reste un projet distinct et n’est pas supposé être prêt tant que son propre dépôt et ses tests ne l’attestent pas. Avant de l’intégrer, confirmer son API publique, son modèle de licence, ses cibles d’architecture, ses dépendances, son modèle d’allocation mémoire, ses besoins de synchronisation et sa gestion des erreurs. Éviter de copier prématurément son code dans ArchiaOS ou de dépendre d’API instables. L’intégration doit être versionnée et reproductible dans la CI, avec tests de compatibilité et de non-régression.

## Critères de progression

Une phase ne passe à 100 % qu’après satisfaction de tous ses critères et tests documentés. L’intégration OpenFS est une étape future : elle ne compte pas comme réalisée avant que le dépôt OpenFS soit suffisamment stable et que les tests d’intégration d’ArchiaOS passent.
