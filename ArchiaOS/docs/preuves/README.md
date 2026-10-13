# Preuves de validation physique

## Démarrage physique P1 — 13 octobre 2026

![Photo du démarrage physique ArchiaOS](Snapchat-76924188.jpg)

- **Machine :** Lenovo LOQ 15IAX9 (modèle indiqué par l'utilisateur).
- **CPU :** Intel Core i5-12450HX.
- **Mémoire :** 24 Go DDR5.
- **GPU utilisé :** NVIDIA GeForce RTX 4050 dédiée ; le GPU intégré au CPU n'est pas utilisé pour cet essai, selon l'utilisateur.
- **Résultats rapportés :** 5/5 démarrages à froid et 5/5 redémarrages à chaud atteignent la bande blanche finale ; retour au firmware puis au système habituel sans problème.
- **Commit ayant produit les binaires USB testés :** `cb5c676a9adbd0a9cf0e7a4ef69224b4a8729ecc`.
- **Artefact GitHub Actions :** [archiaos-p0-usb-x86_64](https://github.com/Likounsee/ArchiaOS/actions/runs/38432842691).
- **Version BIOS/UEFI :** dernière version publiée par Lenovo pour le LOQ 15IAX9, selon l'utilisateur ; numéro exact non relevé.

Les SHA-256 du noyau et du chargeur extraits de l'artefact correspondent à ceux présents sur la clé USB. Les résultats de démarrage sont rapportés par l'utilisateur. La bande blanche indique que le chemin d'auto-tests antérieurs a été atteint si elle provient bien de ce build ; elle ne prouve pas la correction exhaustive du noyau.

Cette image est archivée comme pièce de preuve visuelle et l'utilisateur confirme qu'elle est acceptable. L'utilisateur indique également que le BIOS/UEFI est à jour selon Lenovo pour ce modèle. Le numéro exact de version n'a pas été relevé ; les résultats physiques restent rapportés par l'utilisateur et ne sont pas présentés comme des tests exécutés indépendamment.
