# Archia Control Center — vision produit

## Objectif

Créer un centre de contrôle matériel unifié pour ArchiaOS, réunissant les catégories de fonctions de CPU-Z, HWiNFO, GPU-Z, Fan Control et OpenRGB, avec une interface et une API cohérentes. Ces noms décrivent les inspirations fonctionnelles : aucun code de ces projets ne doit être copié et aucune compatibilité universelle n'est présumée.

**État : vision uniquement.** Aucun tableau de bord unifié, pilote universel ni service de contrôle n'est actuellement déclaré implémenté.

## Modules prévus

1. **Vue d'ensemble PC** — inventaire CPU, carte mère, firmware, RAM, GPU, disques et périphériques, avec capacités réellement détectées.
2. **Capteurs et télémétrie** — températures, fréquences, charge, tensions, puissance et RPM lorsque disponibles ; graphiques, historiques, minimum/maximum, alertes et export.
3. **CPU, carte mère et RAM** — caractéristiques CPU, cœurs/threads, cache et fréquences ; BIOS/UEFI, modules mémoire et timings seulement si accessibles et fiables.
4. **GPU** — modèle, VRAM, charge, fréquences, températures, puissance et pilote ; multi-GPU via adaptateurs.
5. **Ventilation** — RPM, courbes multicapteurs, hystérésis, rampes, profils silencieux/équilibrés/performance, retour tachymètre et détection d'échec.
6. **RGB** — découverte de zones, couleurs, luminosité, effets et synchronisation, avec adaptateurs par contrôleur et fabricant.
7. **Disques et diagnostic** — données SMART/NVMe disponibles, événements, rapports système et alertes.
8. **Benchmarks et stabilité** — tests facultatifs, durée et impact annoncés ; aucun test de stress silencieux.
9. **Profils et automatisation** — profils groupant ventilateurs, RGB et alertes ; règles explicites et restauration lorsque prise en charge.

## Architecture

- **Noyau :** primitives matérielles et mécanismes sûrs, pas de logique de profil, animations ou politique utilisateur.
- **Pilotes/adaptateurs :** interfaces communes avec capacités déclarées, unités, plages valides, erreurs et délais. Les particularités fabricants restent isolées.
- **Service système privilégié :** arbitre les accès et applique les limites ; les clients ordinaires n'accèdent jamais directement aux registres ou ports matériels.
- **API versionnée :** sépare lecture des capteurs et écriture des actionneurs, avec autorisations explicites.
- **Interface utilisateur :** tableaux de bord et profils via l'API, jamais directement vers le matériel.
- **Simulation et tests :** périphériques simulés, capteurs absents, valeurs incohérentes, timeouts et commandes rejetées.

Chaque mesure doit exposer unité, source, horodatage et validité. Une valeur inconnue ne doit pas être remplacée par zéro. « Détecté », « lecture disponible » et « écriture validée » sont trois états distincts.

## Sécurité thermique et matérielle

- Les commandes d'écriture restent désactivées par défaut jusqu'à validation du pilote et de la plateforme.
- Refuser les valeurs hors limites et les registres inconnus ; borner les accès et les délais.
- Afficher séparément consigne demandée et valeur mesurée.
- Une perte de capteur ou de service déclenche une politique de repli documentée ; préserver les protections du firmware.
- Le RGB ne doit jamais compromettre le refroidissement.
- L'overclocking et l'écriture de tensions/limites de puissance sont hors périmètre initial.

## Plan de livraison

A. Contrats communs, permissions, unités, erreurs et pilotes simulés.  
B. Inventaire matériel en lecture seule, d'abord à partir du firmware et de PCI.  
C. Premières métriques sûres et historique de télémétrie.  
D. Lecture RPM, puis contrôle de ventilation borné sur une plateforme de référence, avec tests de panne.  
E. RGB avec contrôleurs documentés et profils restaurables.  
F. Données de disques, rapports et benchmarks facultatifs.  
G. Interface unifiée, profils et automatisations.  
H. Matrice de compatibilité réelle, tests physiques, CI et documentation des limites.

La compatibilité doit être déclarée par modèle, contrôleur, firmware et opération. Commencer sur du matériel de référence, puis étendre le support après tests. La compatibilité multi-fabricants est une cible progressive, pas une garantie implicite.
