# Architecture actuelle

Le noyau expérimental cible x86-64 et est chargé au format ELF64 par le protocole natif Limine. Limine fournit l’entrée en mode long et la pagination de démarrage ; un très petit stub assembleur aligne la pile de démarrage et appelle la fonction Rust selon l’ABI x86-64 SysV. Le noyau initialise COM1, émet des diagnostics puis attend avec les interruptions désactivées.

Les marqueurs de requêtes Limine sont conservés dans des sections dédiées par le linker script. L’ISO embarque une configuration Limine, les fichiers BIOS/UEFI du bootloader et le noyau. La CI construit le noyau, crée l’ISO et lance QEMU pour rechercher le message série d’entrée.

Non implémenté : validation des informations de démarrage, IDT, gestion des exceptions, allocateur physique, tas, gestionnaire mémoire virtuel général, ordonnanceur, processus, syscall, stockage, système de fichiers, réseau et espace utilisateur. La pagination initiale et les services du bootloader ne remplacent pas ces composants.
