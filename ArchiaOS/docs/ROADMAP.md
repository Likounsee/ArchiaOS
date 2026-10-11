# Roadmap et critères

- **P0 — Fondations : en cours.** Build bare-metal, ISO Limine, diagnostics, CI, vérification ELF et smoke test QEMU réussis. La carte mémoire et l’auto-test PMM passent en QEMU ; restent des tests de régression plus larges et l’audit des réservations.
- **P1 — Noyau et matériel : premières fondations.** Entrée Limine, sortie série, validation de la carte mémoire et allocateur bitmap physique testés en QEMU. Restent IDT, exceptions, réservations complètes et synchronisation.
- **P2 — Ordonnanceur : non commencé.** Pas de contexte de tâche, synchronisation ni ordonnancement.
- **P3 — Espace utilisateur : non commencé.** Pas de syscall, chargeur, ABI ni processus isolés.
- **P4 — Stockage : non commencé.** Pas de pilote bloc ni de système de fichiers.
- **P5 — Graphique : non commencé.** Pas de framebuffer ou de compositeur.
- **P6 — Réseau et performances : non commencé.** Pas de pile réseau ni de benchmarks.
- **P7 — Distribution : non commencé.** Pas de version distribuable ni d’audit de sécurité final.

Une phase ne passe à 100 % qu’après satisfaction de tous ses critères et tests documentés.
