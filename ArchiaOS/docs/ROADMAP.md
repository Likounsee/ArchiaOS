# Roadmap et critères

- **P0 — Fondations : en cours.** Build bare-metal, ISO Limine, sortie série et smoke test QEMU réussis sur un commit précédent. La nouvelle vérification de la carte mémoire et la vérification des segments ELF doivent passer dans la CI avant d’être considérées validées.
- **P1 — Noyau et matériel : premières fondations.** Entrée Limine et diagnostics série vérifiés ; parsing borné de la carte mémoire en cours. Restent IDT, exceptions, réservations mémoire et allocateur physique.
- **P2 — Ordonnanceur : non commencé.** Pas de contexte de tâche, synchronisation ni ordonnancement.
- **P3 — Espace utilisateur : non commencé.** Pas de syscall, chargeur, ABI ni processus isolés.
- **P4 — Stockage : non commencé.** Pas de pilote bloc ni de système de fichiers.
- **P5 — Graphique : non commencé.** Pas de framebuffer ou de compositeur.
- **P6 — Réseau et performances : non commencé.** Pas de pile réseau ni de benchmarks.
- **P7 — Distribution : non commencé.** Pas de version distribuable ni d’audit de sécurité final.

Une phase ne passe à 100 % qu’après satisfaction de tous ses critères et tests documentés.
