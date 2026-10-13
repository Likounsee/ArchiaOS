# Campagne de validation physique P1

## Objectif et limites

Cette procédure collecte des preuves répétables pour la fiabilité du démarrage UEFI maison, du contrat BootInfo, de l'initialisation PMM et de l'affichage framebuffer sur une machine physique. Les tests QEMU/OVMF et la réussite CI ne remplacent pas cette campagne.

**Révision de référence initiale :** `9ec5605f5aa0aa98f1c9ebb4d10c1af9d02efb26` (branche `ArchiaOS`). Si une image USB est reconstruite depuis un autre commit, consigner son SHA exact à la place. Ne pas mélanger des résultats obtenus avec des images différentes.

## Préparation

- [ ] Vérifier que l'image USB a été produite par le workflow GitHub Actions correspondant au SHA consigné.
- [ ] Conserver l'image et le journal du workflow avec le dossier de campagne.
- [ ] Sauvegarder les données importantes et utiliser une clé USB dédiée ; l'écriture d'une image efface le contenu de la clé.
- [ ] Vérifier le modèle de machine, le processeur, la quantité de RAM, la version UEFI/BIOS et le port USB utilisé.
- [ ] Désactiver temporairement le démarrage rapide si cela empêche de faire un vrai démarrage à froid ; ne pas changer d'autres paramètres firmware entre les essais.
- [ ] Prévoir un moyen de revenir au système habituel via le menu de démarrage UEFI.

## Essais à réaliser

Pour chaque essai, noter date/heure, SHA de l'image, configuration UEFI et résultat. En cas de panne, noter l'étape exacte et prendre une photo de l'écran avant de modifier la configuration.

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

Les marqueurs série prouvent que le chemin correspondant a été atteint, mais pas à eux seuls que toutes les couleurs, résolutions ou configurations framebuffer sont correctes. Si la machine ne fournit pas de sortie série, consigner cette limite et photographier l'écran.

### D. Retour au firmware et système habituel

- [ ] Redémarrer depuis ArchiaOS et confirmer que le firmware reste accessible.
- [ ] Démarrer le système habituel depuis son entrée UEFI.
- [ ] Confirmer que le système habituel démarre normalement.
- [ ] Vérifier qu'aucune entrée de démarrage existante n'a été modifiée par la campagne.

Critère : retour firmware et démarrage du système habituel réussis après les essais.

## Fiche de résultats

Copier cette fiche pour chaque session :

| Champ | Valeur |
|---|---|
| Date et heure | À renseigner |
| Commit exact de l'image USB | À renseigner |
| SHA / nom de l'artifact CI | À renseigner |
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

P1 ne passe à 100 % que lorsque les essais ci-dessus sont documentés sur une machine physique identifiée, tous les critères sont réussis, et les preuves sont rattachées au SHA exact de l'image testée. Si un essai échoue, conserver les journaux, isoler le défaut, corriger sur la branche `ArchiaOS`, puis relancer la CI et répéter les essais pertinents. Ne pas convertir une réussite QEMU en réussite matérielle.
