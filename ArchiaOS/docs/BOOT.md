# Démarrage

Le noyau cible actuellement **Multiboot 1** et un ELF x86-64 lié à l’adresse 1 Mio. L’en-tête est conservé dans la section `.multiboot` en tête de l’image.

Multiboot 1 démarre le processeur en mode protégé 32 bits. `kernel/src/boot.S` prépare une pile, construit les structures de pagination, active PAE, active `EFER.LME` et `CR0.PG`, charge une GDT minimale, puis bascule vers le code 64 bits. Les diagnostics initiaux sont envoyés à COM1.

L’implémentation est expérimentale. Elle ne valide pas encore les informations Multiboot ni toutes les fonctionnalités CPU requises ; elle suppose un processeur x86-64 avec support PAE et mode long. Le démarrage doit être confirmé dans QEMU avant de revendiquer la compatibilité.

Commande proposée depuis ce dossier : `make run`. Elle n’a pas encore été exécutée.