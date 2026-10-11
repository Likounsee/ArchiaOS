# Mémoire

## Carte mémoire

Le noyau demande la carte mémoire via Limine. Il vérifie la révision du protocole, la présence de la réponse, un nombre d’entrées compris entre 1 et 4096, les pointeurs non nuls, les longueurs non nulles et les débordements. Le total des longueurs utilisables est additionné avec contrôle de débordement.

## Allocateur physique expérimental

Un allocateur bitmap suit les frames de 4 Kio situées sous 4 Gio. Un bitmap marque les frames issues des régions utilisables ; un second marque celles qui sont libres. Cette distinction empêche de libérer une frame réservée ou une adresse arbitraire. Les pages sous 1 Mio sont toujours réservées. Les plages non utilisables ont priorité si les entrées de carte se chevauchent. Les plages sont appliquées par mots bitmap, plutôt qu’en parcourant chaque page une par une.

Le smoke test QEMU alloue 130 pages distinctes en traversant plusieurs mots bitmap, vérifie leur alignement et la limite physique, confirme la baisse du compteur libre, libère les pages, vérifie le compteur restauré et exige que les adresses invalides et le double-free soient rejetés.

## Limites importantes

L’allocateur est limité à 4 Gio, initialisé une seule fois et non synchronisé pour SMP. Le nombre total d’octets utilisables déclaré par Limine n’est pas égal au nombre de pages gérées : les régions hors limite, les pages partielles et les réservations sont exclues du bitmap. Les réservations et chevauchements doivent encore être audités ; aucun tas ou gestionnaire de mémoire virtuelle général n’existe encore.
