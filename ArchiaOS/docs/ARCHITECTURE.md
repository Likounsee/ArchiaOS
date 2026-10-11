# Architecture actuelle

La tranche initiale est un noyau expérimental x86-64 : en-tête Multiboot 1, transition assembleur du mode protégé 32 bits au mode long, tables de pages d’amorçage et sortie série COM1. Rust `no_std` prend ensuite le relais et attend avec les interruptions désactivées.

L’entrée assembleur prépare une pile statique, construit un PML4, un PDPT et un répertoire de pages, active PAE et le mode long, puis appelle `rust_kernel_main`. Le mapping identité couvre les premiers 1 Gio avec des pages de 2 Mio. Le linker place le noyau à 1 Mio ; cette hypothèse de placement doit être contrôlée si l’image évolue.

Non implémenté : analyse des informations Multiboot, IDT, gestion des exceptions, allocateur physique, tas, gestionnaire mémoire virtuel général, ordonnanceur, processus, syscall, stockage, système de fichiers, réseau et espace utilisateur.

L’assembleur est limité à la transition CPU. Rust gère les diagnostics et le flux de contrôle. Les accès port I/O sont encapsulés dans des fonctions `unsafe` documentées.