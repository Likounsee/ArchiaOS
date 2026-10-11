# Roadmap et critères

- **P0 — Fondations : en cours.** Documentation initiale, manifeste Cargo bare-metal, script de liaison, génération d’ISO et CI build/boot. La validation du démarrage dépend du résultat du test QEMU.
- **P1 — Noyau et matériel : premières fondations seulement.** Entrée Limine et sortie série écrites ; restent IDT, exceptions, validation des informations de démarrage et gestion mémoire.
- **P2 — Ordonnanceur : non commencé.** Pas de contexte de tâche, synchronisation ni ordonnancement.
- **P3 — Espace utilisateur : non commencé.** Pas de syscall, chargeur, ABI ni processus isolés.
- **P4 — Stockage : non commencé.** Pas de pilote bloc ni de système de fichiers.
- **P5 — Graphique : non commencé.** Pas de framebuffer ou de compositeur.
- **P6 — Réseau et performances : non commencé.** Pas de pile réseau ni de benchmarks.
- **P7 — Distribution : non commencé.** Pas de version distribuable ni d’audit de sécurité final.

Une phase ne passe à 100 % qu’après satisfaction de tous ses critères et tests documentés.
