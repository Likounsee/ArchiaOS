# Architecture actuelle

Le noyau x86-64 démarre via Limine dans un ELF lié en espace virtuel haut. Le stub assembleur aligne la pile avant l’appel Rust. La CI vérifie les segments ELF, crée l’ISO, démarre QEMU, valide la carte mémoire Limine et exécute l’auto-test du PMM.

Le PMM expérimental suit les frames physiques sous 4 Gio avec deux bitmaps : pages gérées et pages libres. Les plages utilisables sont appliquées par mots bitmap ; les plages réservées prennent priorité. Le test traverse plusieurs mots, vérifie les allocations distinctes, les libérations, les adresses invalides et le double-free.

Non implémenté : IDT, gestion des exceptions, synchronisation/SMP, ordonnanceur, processus, syscall, stockage, système de fichiers, réseau et espace utilisateur. L’allocateur n’est pas synchronisé et ne constitue pas encore un gestionnaire mémoire général.
