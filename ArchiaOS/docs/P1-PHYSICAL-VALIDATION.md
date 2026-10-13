# Campagne de validation physique P1

## Objectif et limites

Cette procédure collecte des preuves répétables pour la fiabilité du démarrage UEFI maison, du contrat BootInfo, de l'initialisation PMM et de l'affichage framebuffer sur une machine physique. Les tests QEMU/OVMF et la réussite CI ne remplacent pas cette campagne.

**Révision de référence initiale :** `9ec5605f5aa0aa98f1c9ebb4d10c1af9d02efb26` (branche `ArchiaOS`). Le workflow `hardware-p0-usb.yml` produit un **ZIP contenant le dossier EFI**, pas une image disque brute. Extraire le ZIP et consigner le SHA du commit exact ayant produit le paquet. Ne pas mélanger des résultats obtenus avec des commits différents.

## Préparation

- [ ] Vérifier que le paquet ZIP a été produit par GitHub Actions pour le SHA consigné et conserver le ZIP ainsi que le lien du workflow.
- [ ] Utiliser une clé USB dédiée, sauvegarder son contenu puis la préparer en FAT32 si nécessaire (le formatage efface les données de la clé).
- [ ] Extraire le ZIP et copier le dossier `EFI` à la racine de la clé FAT32. **Ne pas écrire le ZIP comme une image disque** : ce workflow ne produit pas d'image brute.
- [x] Modèle, processeur, RAM et état du firmware consignés d'après les informations de l'utilisateur : Lenovo LOQ 15IAX9, Intel Core i5-12450HX, 24 Go DDR5 ; GPU dédié RTX 4050 utilisé selon l'utilisateur. L'utilisateur confirme utiliser la dernière version BIOS/UEFI publiée par Lenovo pour ce modèle ; le numéro de version exact n'a pas été relevé. Le port USB n'est pas consigné.
- [ ] Désactiver temporairement le démarrage rapide si cela empêche de faire un vrai démarrage à froid ; ne pas changer d'autres paramètres firmware entre les essais.
- [ ] Prévoir un moyen de revenir au système habituel via le menu de démarrage UEFI.

## Essais à réaliser

Pour chaque essai, noter date/heure, SHA du commit qui a produit le paquet USB, configuration UEFI et résultat. En cas de panne, noter l'étape exacte et prendre une photo de l'écran avant de modifier la configuration.

### A. Démarrages à froid

Effectuer **5 essais indépendants** :
1. Arrêter complètement la machine.
2. Couper l'alimentation quelques secondes lorsque cela est possible sans risque, puis la rétablir.
3. Démarrer sur la clé ArchiaOS via le menu UEFI ponctuel.
4. Vérifier que le démarrage atteint le noyau et que le motif framebuffer attendu apparaît.
5. Redémarrer sur le système habituel après chaque essai.

Critère : 5 démarrages sur 5, sans blocage, écran noir persistant ni retour firmware inattendu.

### B. Redémarrages à chaud

Effectuer **5 essais** depuis la machine démarrée sur ArchiaOS, en redémarrant et en sélectionnant de nouveau la clé dans le menu UEFI.

Critère : 5 démarrages sur 5. Consigner séparément tout comportement différent des démarrages à froid.

### C. BootInfo, mémoire et framebuffer

Sur chaque démarrage réussi, recueillir la sortie série si une interface série matérielle est disponible. Le noyau initialise le port legacy COM1 (0x3F8) et y écrit les étapes de démarrage ; le fait que l'écran affiche uniquement des lignes colorées est normal, car aucun rendu de texte n'est implémenté dans le framebuffer à ce stade.

Les marqueurs série à rechercher sont :
- [ ] Le chargeur UEFI maison ouvre et valide l'image noyau ELF, puis transfère le contrôle au noyau.
- [ ] `UEFI BootInfo validated`.
- [ ] `UEFI PMM allocation/free self-test passed`.
- [ ] `framebuffer probe=written` et le motif attendu est effectivement visible à l'écran.

#### Alternative sans port série accessible : marqueurs framebuffer

Le noyau écrit six bandes de couleur dans un ordre fixe. Photographier l'écran complet pour que la séquence soit visible ; ne pas se contenter d'un recadrage d'une seule bande.

| Ordre | Couleur indicative | Étape marquée |
|---:|---|---|
| 1 | Bleu clair / cyan | Entrée dans le chemin de statut framebuffer |
| 2 | Vert | Validation initiale de BootInfo et sonde framebuffer terminées |
| 3 | Vert-jaune | Initialisation de la mémoire UEFI terminée |
| 4 | Bleu | Auto-test PMM d'allocation/libération réussi |
| 5 | Violet / magenta | Auto-test de contexte x86-64 réussi |
| 6 | Blanc | Auto-test de l'état du scheduler réussi |

**Interprétation importante :** dans le code noyau actuel, les étapes d'échec BootInfo/mémoire/PMM s'arrêtent avant de peindre les marqueurs des étapes suivantes. Par conséquent, la présence de la bande blanche finale, si elle est bien issue de ce build, indique que l'exécution a dépassé avec succès les auto-tests BootInfo, PMM, contexte et scheduler. Cette preuve visuelle est une preuve d'atteinte du chemin de code, pas un journal texte et pas une preuve exhaustive de correction du noyau.

Si l'écran montre seulement une bande blanche isolée et que les autres bandes ne sont pas visibles, conserver la photo et ne pas déduire la séquence complète sans examiner le framebuffer. Si une sortie série physique est inaccessible, noter cette limite et joindre la photo complète plutôt que d'inventer ou de reconstituer des lignes série.

Pour une capture série réelle, utiliser uniquement une interface électrique adaptée au port exposé par la machine. Ne pas relier directement un signal TTL à un port RS-232 ni connecter des broches au hasard. Un PC sans port série accessible peut nécessiter un adaptateur compatible et un câblage documenté ; une connexion USB seule ne rend pas automatiquement COM1 accessible à l'hôte.

### D. Retour au firmware et système habituel

- [ ] Redémarrer depuis ArchiaOS et confirmer que le firmware reste accessible.
- [ ] Démarrer le système habituel depuis son entrée UEFI.
- [ ] Confirmer que le système habituel démarre normalement.
- [ ] Vérifier qu'aucune entrée de démarrage existante n'a été modifiée par la campagne.

Critère : retour firmware et démarrage du système habituel réussis après les essais.

## Résultat matériel rapporté — 13 octobre 2026

L’utilisateur rapporte **5 démarrages à froid réussis sur 5**, avec le même résultat à chaque fois : l’écran atteint la **barre blanche finale**. Ce résultat est consigné comme une observation physique répétée, déclarée par l’utilisateur ; il n’a pas été exécuté ni observé directement par l’assistant.

| Vérification | Résultat rapporté | Statut de preuve |
|---|---|---|
| Démarrages à froid | **5/5** | Répétabilité visuelle rapportée |
| Barre blanche finale et photo | Présentes ; photo validée par l'utilisateur | Selon l'ordre des marqueurs dans le noyau, indique que les auto-tests précédents ont été franchis si elle provient bien du build testé ; la photo est archivée dans [docs/preuves](preuves/README.md) et considérée comme lisible/acceptable selon la confirmation de l'utilisateur |
| SHA exact du paquet USB testé | `cb5c676a9adbd0a9cf0e7a4ef69224b4a8729ecc` | Confirmé par `BUILD-INFO.txt` et par les SHA-256 identiques de `kernel.elf` et `BOOTX64.EFI` sur l'archive extraite et la clé USB |
| Modèle de machine / version UEFI | Lenovo LOQ 15IAX9 ; dernière version BIOS/UEFI Lenovo pour ce modèle, selon l'utilisateur | État du firmware confirmé par l'utilisateur ; numéro exact non relevé |
| Sortie série BootInfo + PMM | Non fournie | Non disponible dans les preuves actuelles |
| Marqueur framebuffer final | Barre blanche rapportée | Rapport utilisateur ; photo complète à conserver si disponible |
| 5 redémarrages à chaud | **5/5 rapportés** | Répétabilité visuelle rapportée |
| Retour firmware et démarrage du système habituel | Confirmés sans problème par l’utilisateur le 13 octobre 2026 | Rapporté par l’utilisateur |

La provenance exacte des binaires USB est maintenant rattachée au commit indiqué ci-dessus. Les résultats matériels et l'état du firmware restent des observations déclarées par l'utilisateur. L'utilisateur confirme que la photo archivée est acceptable et que le BIOS/UEFI est à jour selon Lenovo pour ce modèle. Le numéro exact de version n'est pas enregistré ; il n'est pas nécessaire de le redemander pour refléter ces confirmations.

### Résultat matériel rapporté — redémarrages à chaud (13 octobre 2026)

L’utilisateur rapporte **5 redémarrages à chaud réussis sur 5**, chacun atteignant le même résultat visuel : la barre blanche finale. Il s’agit d’une observation rapportée par l’utilisateur, et non d’un test exécuté directement par l’assistant.

| Vérification | Résultat rapporté | Statut de preuve |
|---|---|---|
| Démarrages à froid | 5/5, barre blanche finale | Rapporté par l’utilisateur |
| Redémarrages à chaud | **5/5, barre blanche finale** | Rapporté par l’utilisateur |
| SHA exact du paquet USB testé | `cb5c676a9adbd0a9cf0e7a4ef69224b4a8729ecc` | Confirmé par BUILD-INFO et concordance SHA-256 des deux binaires |
| Marqueurs série BootInfo et PMM | Non fournis | La preuve série reste indisponible ; la séquence visuelle constitue une preuve alternative de progression |
| Retour firmware et démarrage du système habituel | Confirmés sans problème par l’utilisateur le 13 octobre 2026 | Rapporté par l’utilisateur |

## Fiche de résultats

Copier cette fiche pour chaque session :

| Champ | Valeur |
|---|---|
| Date et heure | À renseigner |
| Commit exact ayant produit le paquet USB | `cb5c676a9adbd0a9cf0e7a4ef69224b4a8729ecc` (SHA confirmé pour les binaires USB testés) |
| Nom du ZIP / lien du workflow CI | À renseigner |
| Machine / carte mère | Lenovo LOQ 15IAX9 (modèle communiqué par l'utilisateur) |
| CPU / RAM | Intel Core i5-12450HX / 24 Go DDR5 |
| Version UEFI/BIOS | Dernière version publiée par Lenovo pour le LOQ 15IAX9, selon l'utilisateur ; numéro exact non relevé |
| Port USB et clé | À renseigner |
| Démarrages à froid (réussis / 5) | **5/5 rapportés, barre blanche finale** (13 octobre 2026) |
| Redémarrages à chaud (réussis / 5) | **5/5 rapportés, barre blanche finale** (13 octobre 2026) |
| BootInfo + PMM observés | Indirectement indiqués par la bande blanche finale rapportée ; conserver la photo complète |
| Motif framebuffer observé | Barre blanche finale rapportée |
| Retour firmware + OS habituel | Confirmé par l’utilisateur |
| Photos / journaux conservés | [Photo archivée dans docs/preuves](preuves/README.md) | Photo archivée et validée par l'utilisateur |
| Anomalies et reproduction | À renseigner |

## Décision d'acceptation P1

P1 ne passe à 100 % que lorsque les essais ci-dessus sont documentés sur une machine physique identifiée, tous les critères sont réussis, et les preuves sont rattachées au SHA exact ayant produit le paquet testé. Les preuves acceptées peuvent être des logs série ou, si aucune interface série n'est disponible, des photos complètes et lisibles de la séquence des marqueurs framebuffer, interprétées selon l'ordre réel du code. Les observations utilisateur doivent rester étiquetées comme rapportées et ne pas être présentées comme des tests exécutés indépendamment. Si un essai échoue, conserver les journaux, isoler le défaut, corriger sur la branche `ArchiaOS`, puis relancer la CI et répéter les essais pertinents. Ne pas convertir une réussite QEMU en réussite matérielle.
