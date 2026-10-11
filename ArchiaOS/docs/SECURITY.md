# Modèle de menace et état de sécurité

Le bootloader, ses informations de démarrage et le matériel sont des entrées externes. Le code noyau et les accès aux ports I/O sont privilégiés. Il n’existe pas de processus utilisateur ni de frontière d’isolation.

Mesures actuelles :
- Rust `no_std` et une surface de code encore réduite.
- Les accès au port série sont encapsulés dans des fonctions `unsafe` dont les préconditions sont documentées ; le polling est borné.
- Les interruptions restent désactivées jusqu’à l’installation d’une IDT.
- Les informations de carte mémoire Limine sont contrôlées : réponse présente, nombre d’entrées borné, pointeurs non nuls, longueurs non nulles et vérification des débordements.
- L’allocateur bitmap distingue les pages gérées des pages libres, rejette les adresses non alignées, les plages non gérées et les doubles libérations. Le test QEMU vérifie ces chemins.

Risques et limites :
- Les pointeurs de réponse sont fournis par le bootloader ; leur accessibilité repose encore sur son contrat.
- La réservation complète des segments noyau, tables de pages et structures du bootloader doit être auditée avant un usage général.
- L’allocateur est limité à 4 Gio, initialisé une seule fois et non synchronisé pour SMP.
- Aucune IDT, isolation utilisateur, protection de processus, gestion de faute de page ou audit de sécurité complet n’est implémenté.

Ne pas présenter le noyau comme sécurisé ou prêt pour du matériel réel.
