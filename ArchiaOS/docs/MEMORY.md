# Mémoire

## État actuel

Le noyau s’appuie sur les structures de pagination initiales fournies par Limine. Le linker associe des adresses virtuelles hautes à des adresses physiques de chargement à partir de 1 Mio. Le noyau ne possède pas encore de gestionnaire de mémoire physique ou virtuelle propre.

## Limites

Il n’y a ni parsing de la carte mémoire, ni bitmap de frames, ni allocation/libération, ni tas, ni séparation noyau/utilisateur, ni gestion des fautes de page. Les garanties exactes du mapping initial doivent être vérifiées dans le contrat Limine et ne constituent pas une politique de protection finale.

## Prochaine étape

Valider les structures d’informations de démarrage avec des bornes et contrôles de débordement, puis construire une représentation explicite des régions utilisables et réservées avant d’implémenter un allocateur de pages physiques.
