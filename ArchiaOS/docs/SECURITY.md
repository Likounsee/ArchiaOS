# Modèle de menace et état de sécurité

Le bootloader, les informations de démarrage et le matériel sont des entrées externes. Le code noyau et les accès aux ports I/O sont privilégiés. Il n’existe pas de processus utilisateur ni de frontière d’isolation.

Mesures actuelles : Rust `no_std`, petite surface de code, deux fonctions `unsafe` encapsulant les accès au port série avec préconditions documentées, polling série borné et interruptions désactivées jusqu’à l’installation d’une IDT.

Risques : les informations de démarrage ne sont pas analysées ; la carte mémoire n’est pas validée ; aucune isolation, protection de processus, gestion de faute de page ou audit de sécurité par exécution n’a été validé. Ce noyau ne doit pas être présenté comme sécurisé ou prêt pour du matériel réel.

Critères suivants : valider les adresses et longueurs des informations Limine sans débordement, ajouter une IDT et des diagnostics de faute, puis maintenir des tests QEMU.
