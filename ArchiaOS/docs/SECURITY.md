# Modèle de menace et état de sécurité

Le bootloader, les informations Multiboot et le matériel sont des entrées externes. L’assembleur de transition, les tables de pages, les accès aux ports I/O et le noyau Rust sont privilégiés. Il n’existe pas de processus utilisateur ni de frontière d’isolation.

Mesures actuelles : Rust `no_std`, petite surface de code, deux fonctions `unsafe` encapsulant les accès au port série avec préconditions documentées, polling série borné et interruptions désactivées jusqu’à l’installation d’une IDT.

Risques : les informations Multiboot ne sont pas analysées ; la carte mémoire n’est pas validée ; les mappings initiaux sont identité-mappés et permissifs ; aucune isolation, protection de processus, gestion de faute de page ou audit par exécution n’a été validé. Ce noyau ne doit pas être présenté comme sécurisé ou prêt pour du matériel réel.

Critères suivants : valider les adresses et longueurs Multiboot sans débordement, contrôler le placement des sections et tables, ajouter une IDT et des diagnostics de faute, puis établir des tests QEMU.