# Architecture actuelle

Le noyau expérimental cible x86-64 et est chargé au format ELF64 par le protocole natif Limine. Le script de liaison place les segments du noyau dans la moitié haute de l’espace virtuel et leur associe des adresses physiques de chargement à partir de 1 Mio. Des segments ELF explicites séparent les requêtes Limine, le code, les données en lecture seule et les données modifiables.

Limine fournit l’entrée en mode long et la pagination de démarrage ; un petit stub assembleur aligne la pile de démarrage et appelle la fonction Rust selon l’ABI x86-64 SysV. Le noyau initialise COM1, émet des diagnostics puis attend avec les interruptions désactivées.

Non implémenté : validation des informations de démarrage, IDT, gestion des exceptions, allocateur physique, tas, gestionnaire mémoire virtuel général, ordonnanceur, processus, syscall, stockage, système de fichiers, réseau et espace utilisateur. La pagination initiale et les services du bootloader ne remplacent pas ces composants.
