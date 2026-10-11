# Pilotes

Le seul périphérique adressé actuellement est le port série compatible UART 16550 à l’adresse I/O COM1 (0x3F8), utilisé pour les diagnostics. L’initialisation configure le port et attend le registre de transmission avec une boucle bornée. Ce chemin n’a pas encore été testé dans une machine virtuelle.

Aucun pilote clavier, stockage, réseau, graphique, timer ou contrôleur d’interruptions n’est implémenté. Le prochain travail devra définir des contrats de pilote et séparer les accès matériels des services génériques.