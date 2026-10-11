# Mémoire

## Carte mémoire

Le noyau demande la carte mémoire via Limine. Il vérifie la révision du protocole, la présence de la réponse, un nombre d’entrées compris entre 1 et 4096, les pointeurs non nuls, les longueurs non nulles et les débordements. Le total des longueurs utilisables est additionné avec contrôle de débordement.

## Allocateur physique expérimental

Un allocateur bitmap suit les frames de 4 Kio situées sous 4 Gio. Un bitmap marque les frames issues des régions utilisables ; un second marque celles qui sont libres. Cette distinction empêche de libérer une frame réservée ou une adresse arbitraire. Les pages sous 1 Mio sont toujours réservées. Les plages non utilisables ont priorité si les entrées de carte se chevauchent.

Le smoke test alloue une page, vérifie son alignement et la limite physique, vérifie la diminution du compteur libre, libère la page, vérifie le compteur restauré et exige que le double-free soit rejeté.

## Limites importantes

L’allocateur est limité à 4 Gio et n’est pas synchronisé pour SMP. Le nombre total de bytes utilisables déclaré par Limine n’est pas égal au nombre de pages gérées : les régions hors limite, les pages partielles et les réservations sont exclues du bitmap. La réservation complète des segments noyau, tables de pages et structures du bootloader doit encore être auditée. Aucun tas ou gestionnaire de mémoire virtuelle général n’existe encore.

## Prochaine étape

Ajouter des tests de fragmentation et de plages chevauchantes, réserver explicitement toutes les structures nécessaires, puis introduire les primitives de synchronisation avant tout usage concurrent.
