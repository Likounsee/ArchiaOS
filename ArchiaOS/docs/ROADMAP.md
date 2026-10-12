# Roadmap et critères

- **P0 — Fondations : en cours.** Build bare-metal, ISO Limine, diagnostics, vérification ELF et smoke test QEMU existants. La CI vérifie aussi la carte mémoire et l'auto-test PMM sur 130 pages. Restent un contrôle plus large des réservations, le formatage automatisé et des tests supplémentaires.
- **P1 — Noyau et matériel : premières fondations.** Entrée Limine, sortie série, validation de la carte mémoire et allocateur bitmap physique testés en QEMU. Restent IDT, exceptions, réservations mémoire complètes, gestion des fautes et synchronisation.
- **P2 — Ordonnanceur : non commencé.** Pas de contexte de tâche, synchronisation multiprocesseur ni ordonnancement.
- **P3 — Espace utilisateur : non commencé.** Pas de syscall, chargeur, ABI ni processus isolés.
- **P4 — Stockage : non commencé.** Pas de pilote bloc ni de système de fichiers. Préparer une couche **VFS (Virtual File System)** indépendante du format de stockage, afin que le noyau et les programmes utilisent des interfaces stables plutôt que l’API d’un système de fichiers précis.
- **P4.1 — Préparation à OpenFS : à concevoir.** Définir le contrat d’intégration entre ArchiaOS et le dépôt séparé **Likounsee/OpenFS** : types et erreurs, opérations fichiers/répertoires, gestion des volumes, règles de propriété mémoire, synchronisation, limites de blocs et stratégie de version. Maintenir un adaptateur distinct afin de ne pas coupler le cœur du noyau à l’implémentation interne d’OpenFS.
- **P4.2 — Intégration OpenFS : future, après stabilisation d’OpenFS.** Vérifier les exigences et garanties du dépôt OpenFS, puis l’intégrer derrière la VFS. Tester montage, lectures/écritures, répertoires, erreurs, accès concurrents pris en charge et entrées malformées/corruption. Maintenir l'option de build sans OpenFS tant que l'intégration reste expérimentale.
- **P5 — Graphique : non commencé.** Pas de framebuffer ou de compositeur.
- **P6 — Réseau et performances : non commencé.** Pas de pile réseau ni de benchmarks.
- **P7 — Distribution : non commencé.** Pas de version distribuable ni d’audit de sécurité final.

## Piste parallèle — bootloader maison et ARM64

Le firmware UEFI standard reste fourni par la machine. ArchiaOS développe son propre chargeur et son noyau. Les nouveaux jalons sont distingués des phases P0–P7 : ils ne doivent pas artificiellement gonfler leur pourcentage d'achèvement.

1. **B0 — Entrée UEFI : implémentée.** Crate Rust sans dépendance externe pour x86-64 et AArch64.
2. **B1 — Lecture du noyau : implémentée.** Le chargeur ouvre `\\EFI\\ArchiaOS\\kernel.elf`, borne sa taille et lit le fichier depuis le volume associé à l'image EFI ; la lecture gère les retours partiels.
3. **B2 — Validation ELF64 : implémentée.** Contrôles des en-têtes, du nombre et des limites de table des programmes, des segments chargeables, des tailles fichier/mémoire, des alignements, des recouvrements, des débordements et de l'adresse d'entrée exécutable. Tests unitaires présents.
4. **B3 — Chargement identité : implémenté sous contraintes.** Réserve les pages aux adresses physiques ELF demandées, refuse les segments où l'adresse virtuelle diffère de la physique ou les plages hors de la limite prise en charge, copie les octets et met la partie BSS à zéro. La chaîne actuelle limite les adresses à moins de 4 Gio et la mémoire totale des segments à 128 Mio.
5. **B4 — Contrat BootInfo : implémenté.** ABI versionnée `ARCHIABI`, contenant l'entrée noyau, la carte mémoire UEFI finale et les pointeurs de plateforme disponibles. Le RSDP ACPI est vérifié par signature, longueur et sommes de contrôle ; le FDT est contrôlé par un parseur partagé de ses blocs, jetons, noms de propriétés et terminaison de la table de réservations avant de transmettre les pointeurs. Le pointeur est passé via `RDI` en x86-64 et `x0` en AArch64.
6. **B5 — Sortie UEFI : implémentée.** Collecte la carte mémoire juste avant `ExitBootServices`, réessaie si la clé est périmée et évite les appels de service supplémentaires après un échec d'une première tentative de sortie.
7. **B6 — Validation x86-64 : test QEMU/OVMF présent.** Un noyau de test identité vérifie BootInfo après la sortie des services. Cela ne démarre pas encore le noyau de production high-half.
8. **A0 — Entrée noyau ARM64 : implémentée.** Entrée AArch64, pile dédiée et console PL011 pour QEMU `virt`.
9. **A1 — Transfert UEFI ARM64 : code implémenté, test de bout en bout ajouté.** Le chargeur alloue/copie les segments identité, synchronise l'I-cache AArch64, valide l'en-tête et les jetons FDT avec le parseur commun, transmet BootInfo et branche vers l'entrée du noyau. Le noyau réutilise la même ABI et le même parseur pour vérifier BootInfo, le FDT et les descripteurs mémoire. Le résultat du job AAVMF/QEMU doit encore être confirmé.
10. **A2 — Fondations matérielles ARM64 : à faire.** Vecteurs d'exceptions, MMU/TLB, timer et contrôleur d'interruptions adaptés à la plateforme.
11. **A3 — Architecture portable : à faire.** Extraire les abstractions communes et tester séparément les implémentations x86-64/AArch64.
12. **B7 — Démarrage du noyau principal : à faire.** Le noyau de production est high-half lié, dépend encore de Limine et utilise ses réponses mémoire ; il faut établir les tables de pages/contrat de mémoire avant de retirer Limine du chemin de démarrage.

La prise en charge ARM64 ne sera annoncée comme validée qu'après un test UEFI AAVMF/QEMU de bout en bout qui confirme l'entrée du noyau, la validation de BootInfo et du FDT. La compilation seule ne suffit pas.

## Conditions d’intégration d’OpenFS

OpenFS reste un projet distinct et n’est pas supposé prêt tant que son propre dépôt et ses tests ne l’attestent pas. Avant l'intégration, confirmer son API publique, sa licence, ses cibles, ses dépendances, son allocation mémoire, sa synchronisation et sa gestion des erreurs. Ne pas copier prématurément son code ni dépendre d'API instables. L'intégration doit être versionnée, reproductible et testée en CI.

## Critères de progression

Une phase ne passe à 100 % qu’après satisfaction de tous ses critères et tests documentés. L'intégration OpenFS ne compte pas comme réalisée avant que le dépôt OpenFS soit suffisamment stable et que les tests d'intégration ArchiaOS passent.
