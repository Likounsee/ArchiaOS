# Roadmap et critères

- **P0 — Fondations : en cours.** Build bare-metal, ISO Limine, diagnostics, CI et smoke test QEMU réussis. Le test de validation de carte mémoire est passé ; le dernier lot ajoute un allocateur bitmap et un test d’allocation/libération.
- **P1 — Noyau et matériel : premières fondations.** Entrée Limine, sortie série et carte mémoire validées en QEMU. L’allocateur de pages physiques expérimental doit encore passer la CI. Restent IDT, exceptions, réservations complètes et synchronisation.
- **P2 — Ordonnanceur : non commencé.** Pas de contexte de tâche, synchronisation ni ordonnancement.
- **P3 — Espace utilisateur : non commencé.** Pas de syscall, chargeur, ABI ni processus isolés.
- **P4 — Stockage : non commencé.** Pas de pilote bloc ni de système de fichiers.
- **P5 — Graphique : non commencé.** Pas de framebuffer ou de compositeur.
- **P6 — Réseau et performances : non commencé.** Pas de pile réseau ni de benchmarks.
- **P7 — Distribution : non commencé.** Pas de version distribuable ni d’audit de sécurité final.

Une phase ne passe à 100 % qu’après satisfaction de tous ses critères et tests documentés.
