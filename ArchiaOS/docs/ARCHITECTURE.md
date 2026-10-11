# Architecture actuelle

Le noyau expérimental cible x86-64 et est chargé au format ELF64 par le protocole natif Limine. Le script de liaison place les segments dans la moitié haute de l’espace virtuel et leur associe des adresses physiques de chargement à partir de 1 Mio. Le segment des requêtes Limine est lisible/inscriptible afin que le bootloader puisse y placer les pointeurs de réponse.

Limine fournit l’entrée en mode long et la pagination de démarrage ; un petit stub assembleur aligne la pile et appelle Rust selon l’ABI x86-64 SysV. Le noyau initialise COM1, vérifie que la révision de protocole demandée est prise en charge, valide la carte mémoire fournie par Limine, puis attend avec les interruptions désactivées.

Non implémenté : réservations mémoire durables, allocateur physique, tas, IDT, gestion des exceptions, ordonnanceur, processus, syscall, stockage, système de fichiers, réseau et espace utilisateur. La validation des entrées mémoire est une étape de vérification, pas encore un allocateur.
