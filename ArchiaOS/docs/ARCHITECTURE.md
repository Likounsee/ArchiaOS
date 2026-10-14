# Architecture actuelle

## Démarrage

ArchiaOS utilise son chargeur UEFI propre et un noyau Rust x86-64 lié en espace virtuel haut. Le contrat BootInfo versionné transmet la carte mémoire UEFI et les informations nécessaires au noyau. La CI construit le chargeur et le noyau, vérifie les segments ELF et exécute les validations configurées. Les mentions historiques de Limine ne décrivent pas le chemin de démarrage courant.

## Mémoire

Le PMM expérimental suit les frames physiques sous 512 Gio à l'aide de deux bitmaps : pages gérées et pages libres. Ils occupent environ 32 Mio au total et sont placés dynamiquement dans de la RAM conventionnelle réservée au démarrage. L'allocation effective reste limitée à 4 Gio, car la RAM haute n'est pas encore mappée par le chargeur. Cette limite reste une fenêtre de suivi et ne garantit pas que chaque CPU puisse adresser toute la plage. Les plages utilisables sont appliquées par mots bitmap ; les plages réservées prennent priorité. Le test au démarrage traverse plusieurs mots, vérifie les allocations distinctes, les libérations, les adresses invalides et le double-free.

La fenêtre de suivi est de 512 Gio, mais le plafond d'allocation sûr reste 4 Gio jusqu'à l'extension des mappings physiques. Ce PMM ne remplace pas un VMM : les mappings virtuels, les permissions de pages, l'invalidation TLB, les espaces d'adressage isolés et un tas noyau général restent à compléter et à valider.

## CPU, interruptions et ordonnancement

L'IDT x86-64 de 256 entrées et les stubs ISR sont installés. Le chemin d'exception est actuellement fatal et non retournant ; le PIC legacy reste masqué. Le modèle d'ordonnanceur coopératif et le runtime de validation de changement de contexte existent, mais il ne s'agit pas encore d'un ordonnanceur préemptif de production : timer, SMP actif et synchronisation multi-CPU restent à implémenter/valider.

## Limites fonctionnelles connues

- VMM complet et espaces d'adressage isolés : non terminés.
- Processus utilisateur ring 3, chargement ELF utilisateur et appels système : non terminés.
- SMP, IPI, verrous adaptés au noyau et tests multi-CPU : non terminés.
- Chemins d'E/S de stockage complets et VFS : à vérifier/compléter ; une détection PCI/NVMe ne prouve pas l'existence d'E/S fonctionnelles.
- OpenFS Rust : portage séparé volontairement ; ne pas réintégrer automatiquement l'ancienne implémentation.
- Tests QEMU d'intégration et validation matérielle : à étendre au-delà des scénarios déjà configurés.

Les états ci-dessus décrivent le périmètre du code et des validations connues ; ils ne constituent pas une certification de tous les chemins d'exécution.
