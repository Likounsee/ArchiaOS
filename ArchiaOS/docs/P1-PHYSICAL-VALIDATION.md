# Campagne de validation physique P1

## Objectif et limites

Cette procédure collecte des preuves répétables pour la fiabilité du démarrage UEFI maison, du contrat BootInfo, de l'initialisation PMM et de l'affichage framebuffer sur une machine physique. Les tests QEMU/OVMF et la réussite CI ne remplacent pas cette campagne.

**Révision de référence initiale :** `9ec5605f5aa0aa98f1c9ebb4d10c1af9d02efb26` (branche `ArchiaOS`). Le workflow `hardware-p0-usb.yml` produit un **ZIP contenant le dossier EFI**, pas une image disque brute. Extraire le ZIP et consigner le SHA du commit exact ayant produit le paquet. Ne pas mélanger des résultats obtenus avec des commits différents.

## Préparation

- [ ] Vérifier que le paquet ZIP a été produit par GitHub Actions pour le SHA consigné et conserver le ZIP ainsi que le lien du workflow.
- [ ] Utiliser une clé USB dédiée, sauvegarder son contenu puis la préparer en FAT32 si nécessaire (le formatage efface les données de la clé).
- [ ] Extraire le ZIP et copier le dossier `EFI` à la racine de la clé FAT32. **Ne pas écrire le ZIP comme une image disque** : ce workflow ne produit pas d'image brute.
- [ ] Vérifier le modèle de machine, le processeur, la quantité de RAM, la version UEFI/BIOS et le port USB utilisé.
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

Sur chaque démarrage réussi, vérifier les preuves accessibles dans la sortie série ou les diagnostics disponibles :
- [ ] Le chargeur UEFI maison ouvre et valide l'image noyau ELF.
- [ ] Le chargeur quitte les services de démarrage UEFI et transfère le contrôle au noyau.
- [ ] Le noyau confirme `UEFI BootInfo validated`.
- [ ] Le test PMM annonce `UEFI PMM allocation/free self-test passed`.
- [ ] Le diagnostic annonce `framebuffer probe=written` et le motif attendu est effectivement visible à l'écran.

Les marqueurs série prouvent que le chemin correspondant a été atteint, mais pas à eux seuls que toutes les couleurs, résolutions ou configurations framebuffer sont correctes. Si le PC expose un port COM, une capture série peut être réalisée avec un adaptateur série adapté ; ne pas connecter directement un signal TTL à un port RS-232. Si aucune sortie série n'est disponible, consigner explicitement cette limite : une photo de l'écran prouve le motif visible, mais ne prouve pas à elle seule les marqueurs BootInfo/PMM.

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
| Barre blanche finale | Présente sur les 5 essais | Indice que le dernier auto-test visuel atteint a été marqué ; ne prouve pas à lui seul tous les invariants du noyau |
| SHA exact du paquet USB testé | Non communiqué | À rattacher avant clôture |
| Modèle de machine / version UEFI | Non consignés dans ce compte rendu | À renseigner |
| Sortie série BootInfo + PMM | Non fournie | Non vérifiée par cette observation |
| 5 redémarrages à chaud | Non communiqués | À vérifier séparément |
| Retour firmware et démarrage du système habituel | Non communiqués | À vérifier séparément |

Cette observation fait progresser la campagne physique, mais **ne clôt pas encore P1** : l’artefact exact et les autres critères doivent être consignés. Ne pas interpréter la barre blanche comme un journal complet de tous les tests.

## Fiche de résultats

Copier cette fiche pour chaque session :

| Champ | Valeur |
|---|---|
| Date et heure | À renseigner |
| Commit exact ayant produit le paquet USB | À renseigner |
| Nom du ZIP / lien du workflow CI | À renseigner |
| Machine / carte mère | À renseigner |
| CPU / RAM | À renseigner |
| Version UEFI/BIOS | À renseigner |
| Port USB et clé | À renseigner |
| Démarrages à froid (réussis / 5) | À renseigner |
| Redémarrages à chaud (réussis / 5) | À renseigner |
| BootInfo + PMM observés | À renseigner |
| Motif framebuffer observé | À renseigner |
| Retour firmware + OS habituel | À renseigner |
| Photos / journaux conservés | À renseigner |
| Anomalies et reproduction | À renseigner |

## Décision d'acceptation P1

P1 ne passe à 100 % que lorsque les essais ci-dessus sont documentés sur une machine physique identifiée, tous les critères sont réussis, et les preuves sont rattachées au SHA exact ayant produit le paquet testé. Si un essai échoue, conserver les journaux, isoler le défaut, corriger sur la branche `ArchiaOS`, puis relancer la CI et répéter les essais pertinents. Ne pas convertir une réussite QEMU en réussite matérielle.
