# Mémoire

## État actuel

L’entrée assembleur crée les structures de pagination minimales du mode long. Une PML4 et un PDPT pointent vers un répertoire de 512 entrées de 2 Mio, ce qui identité-map la plage 0–1 Gio. Les tables et la pile sont statiques et le linker place le noyau à 1 Mio.

## Limites

Ce n’est pas un gestionnaire de mémoire physique ou virtuelle complet. Il n’y a ni parsing de la carte mémoire, ni bitmap de frames, ni allocation/libération, ni séparation noyau/utilisateur, ni gestion des fautes de page. Le mapping initial est permissif et n’est pas une politique de protection finale.

## Prochaine étape

Valider les structures d’informations Multiboot avec des bornes et contrôles de débordement, puis construire une représentation des régions utilisables et réservées avant d’implémenter un allocateur de pages physiques.