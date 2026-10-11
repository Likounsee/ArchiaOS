# Mémoire

## État actuel

Le noyau demande la carte mémoire via Limine. Il vérifie la prise en charge de la révision de protocole, la présence de la réponse, un nombre d’entrées compris entre 1 et 4096, le pointeur de tableau et les pointeurs d’entrée. Chaque région doit avoir une longueur non nulle et une fin calculable sans débordement. Le total des longueurs marquées utilisables est additionné avec contrôle de débordement.

La validation échoue de manière contrôlée et laisse le noyau dans une boucle d’arrêt si la réponse manque ou si une entrée est mal formée. Les diagnostics indiquent le nombre d’entrées, le nombre de plages utilisables et la somme de leurs longueurs.

## Limites importantes

Le noyau ne possède pas encore d’allocateur physique. Le total rapporté ne retire pas encore les segments du noyau, la pile, les tables de pages, les structures Limine ou les autres plages réservées ; il ne doit donc pas servir directement à allouer des pages. Le code s’appuie encore sur le contrat de confiance du bootloader pour l’accessibilité des pointeurs de réponse.

## Prochaine étape

Construire une liste de régions réservées et utilisables, vérifier les chevauchements et les alignements, réserver explicitement toutes les plages du noyau et du bootloader, puis implémenter et tester l’allocateur de pages physiques.
