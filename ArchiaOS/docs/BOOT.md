# Démarrage

Le noyau cible le protocole natif **Limine** et un ELF64 x86-64. Il est lié dans la moitié haute de l’espace virtuel, avec une adresse physique de chargement à partir de 1 Mio. Le script de liaison déclare explicitement les segments ELF et conserve les marqueurs de requêtes Limine.

Limine fournit l’entrée en mode long et les tables de pagination initiales ; un petit stub assembleur aligne la pile avant l’appel Rust. Le script `scripts/make-iso.sh` télécharge Limine v12.9.1, vérifie le SHA-256 de l’archive, crée une ISO amorçable BIOS/UEFI et exécute `limine bios-install`. La CI démarre cette ISO sous QEMU et exige le message série de l’entrée noyau.

Le premier test a détecté que Limine refusait les segments ELF liés en espace virtuel bas. Le linker a été modifié pour séparer VMA haute et LMA basse. Le résultat du test après cette correction doit être confirmé avant de revendiquer un démarrage fonctionnel.
