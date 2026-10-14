# Architecture actuelle

## Démarrage

ArchiaOS utilise son chargeur UEFI propre et un noyau Rust x86-64 lié en espace virtuel haut. Le contrat BootInfo versionné transmet la carte mémoire UEFI et les informations nécessaires au noyau. La CI construit le chargeur et le noyau, vérifie les segments ELF et exécute les validations configurées. Les mentions historiques de Limine ne décrivent pas le chemin de démarrage courant.

## Mémoire

Le PMM suit et alloue des frames physiques dans sa fenêtre logicielle de 512 Gio. Ses deux bitmaps occupent environ 32 Mio et sont réservés dynamiquement sous 4 Gio. Les plages utilisables sont appliquées par mots bitmap et les plages réservées prennent priorité. Les tests de démarrage vérifient allocations/libérations, erreurs de libération, fragmentation et restauration des compteurs.

Le VMM de bootstrap (kernel/src/vmm.rs) fournit des mappings noyau à la demande, des permissions configurables avec refus du W^X, l'invalidation TLB locale et la récupération des tables intermédiaires vides. Le PMM/VMM est testé sous OVMF/QEMU avec 5 Gio de RAM : le noyau écrit et relit une frame physique au-dessus de 4 Gio par mapping virtuel.

Le tas global (kernel/src/heap.rs) mappe des pages à la demande, découpe et fusionne les blocs libres, et est testé pour alignement, croissance et réutilisation. kernel/src/address_space.rs crée des racines PML4 indépendantes, partage les mappings noyau superviseur et isole des mappings utilisateur dans une fenêtre bootstrap démarrant à 512 Gio virtuels. Son test bascule CR3, vérifie l'isolation entre deux racines et restaure les compteurs de frames.

## CPU, interruptions et ordonnancement

L'IDT x86-64 de 256 entrées et les stubs ISR sont installés. Le chemin d'exception est actuellement fatal et non retournant ; le PIC legacy reste masqué. Le modèle d'ordonnanceur coopératif et le runtime de validation de changement de contexte existent, mais il ne s'agit pas encore d'un ordonnanceur préemptif de production : timer, SMP actif et synchronisation multi-CPU restent à implémenter/valider.

## Limites fonctionnelles connues

- VMM et racines d'adressage bootstrap : présents, mais pas encore sûrs pour le SMP (pas de shootdown TLB inter-CPU).
- Chargement ELF, placement/relocalisation dans la fenêtre utilisateur bootstrap, transition ring 3 et appels système : non terminés.
- SMP, IPI, verrous IRQ/SMP pour PMM/VMM/tas et tests multi-CPU : non terminés.
- Chemins d'E/S de stockage complets et VFS : à vérifier/compléter ; une détection PCI/NVMe ne prouve pas l'existence d'E/S fonctionnelles.
- OpenFS Rust : portage séparé volontairement ; ne pas réintégrer automatiquement l'ancienne implémentation.
- Tests QEMU d'intégration et validation matérielle : à étendre au-delà des scénarios déjà configurés.

Les états ci-dessus décrivent le périmètre du code et des validations connues ; ils ne constituent pas une certification de tous les chemins d'exécution.
