# Architecture cible — contrôle matériel bas niveau

## Objectif produit

ArchiaOS doit pouvoir évoluer vers un logiciel de contrôle matériel bas niveau : surveillance des températures et tensions disponibles, contrôle de ventilateurs, profils thermiques, pilotage RGB et intégration des fonctions exposées par les cartes mères, GPU, contrôleurs et périphériques compatibles.

Ce document fixe les contraintes d'architecture dès maintenant. **Il ne signifie pas que ces fonctions sont déjà implémentées.** La compatibilité réelle dépendra des contrôleurs, du firmware, des protocoles documentés et de tests sur du matériel physique.

## Architecture en couches

1. **Découverte de plateforme** — inventaire des périphériques et capacités à partir des informations firmware (ACPI/UEFI, SMBIOS et, sur les plateformes concernées, Device Tree). Les données de découverte sont des entrées non fiables à valider.
2. **Bus et primitives matérielles** — abstractions contrôlées pour PCI/PCIe, I²C/SMBus, USB, GPIO et interfaces firmware pertinentes. Chaque bus doit vérifier les plages, tailles, temporisations et droits avant d'accéder au matériel.
3. **Pilotes de capteurs et d'actionneurs** — interfaces communes pour températures, tachymètres, ventilateurs, contrôleurs RGB et autres fonctions. Les détails spécifiques aux fabricants restent dans des pilotes/adaptateurs séparés, avec détection de capacités plutôt que supposition fondée sur le seul nom du produit.
4. **Service privilégié de contrôle** — applique les politiques, les courbes ventilateur, les profils, les effets RGB et les règles de priorité. Il doit pouvoir fonctionner sans interface graphique.
5. **API et interface utilisateur** — clients non privilégiés capables de lire l'état et de demander des changements autorisés. Aucun client ordinaire ne reçoit un accès arbitraire aux ports d'E/S, registres, I²C/SMBus ou à la mémoire physique.

Le noyau doit fournir des mécanismes minimaux et sûrs ; les politiques complexes, profils utilisateurs, effets RGB et logique spécifique aux fabricants doivent rester hors du cœur du noyau lorsque c'est possible. L'API entre ces couches doit être versionnée, documentée et indépendante des pilotes concrets.

## Capacités à prévoir

- **Ventilation :** lire le régime (RPM) quand disponible, connaître les capacités du contrôleur, définir une consigne ou une courbe seulement si le matériel l'autorise, confirmer le résultat et détecter les commandes non prises en charge.
- **Thermique :** lire les capteurs CPU/GPU/carte mère accessibles, distinguer valeur absente d'une valeur nulle, détecter valeurs incohérentes et pertes de capteur.
- **RGB :** découvrir les zones et modes disponibles, représenter les couleurs et effets dans un modèle commun, puis convertir vers les capacités de chaque pilote ; ne pas supposer que tous les contrôleurs exposent les mêmes couleurs, fréquences ou effets.
- **Profils :** profils silencieux/équilibrés/performance, courbes personnalisables, limites, validation et possibilité de revenir au profil firmware par défaut.
- **Diagnostic :** journaliser le pilote, le périphérique, la capacité détectée et le résultat d'une commande sans exposer de données sensibles.

## Sécurité et comportement de repli

- La lecture des capteurs est séparée de l'écriture sur les actionneurs ; les opérations d'écriture sont explicitement autorisées.
- Refuser les adresses, tailles, valeurs et fréquences hors limites ; utiliser des accès bornés et des délais.
- Ne jamais écrire dans un registre inconnu simplement parce qu'il semble associé à un ventilateur ou au RGB.
- Une perte de capteur, un blocage du service ou une commande rejetée doit déclencher une politique documentée et sûre. Le firmware reste le filet de sécurité principal tant qu'ArchiaOS ne peut pas démontrer un contrôle thermique fiable.
- Le système ne doit jamais promettre qu'un ventilateur est pilotable si le contrôleur ne fournit pas une interface vérifiée. Le contrôle des ventilateurs critiques exige des essais physiques et des limites matérielles indépendantes.
- Le mode RGB doit être incapable de compromettre le refroidissement ou de contourner les protections thermiques.

## Plan d'implémentation

1. **Contrat abstrait :** définir les types de périphériques, capacités, unités, erreurs, identifiants stables, permissions et version d'API, avec des pilotes simulés pour les tests.
2. **Inventaire en lecture seule :** découverte plateforme et rapport des capteurs/capacités, sans aucune écriture matérielle.
3. **Premiers pilotes ciblés :** sélectionner le matériel de référence et implémenter des lectures contrôlées, testables sans matériel puis sur machine réelle.
4. **Ventilation contrôlée :** consignes bornées, tachymètre de retour, courbes, temporisation, watchdog et repli sûr ; aucun réglage automatique par défaut avant validation.
5. **RGB :** API de zones/effets et pilotes spécifiques, avec limites de fréquence et arrêt propre.
6. **Service/API et interface :** autorisations explicites, profils persistants, journalisation et compatibilité versionnée.
7. **Validation matérielle :** matrice carte mère/contrôleur/firmware, tests de charge et de panne, mesures thermiques et preuve que le contrôle firmware reste disponible en cas d'échec.

## Dépendances avec les autres jalons

- **P1 / pilotes :** définir les frontières sûres des accès matériels et les conventions d'erreur.
- **P2 / ordonnanceur et synchronisation :** nécessaires aux tâches de surveillance, aux temporisations et à la coordination concurrente ; ne pas introduire de boucle active de contrôle dans le noyau.
- **P3 / espace utilisateur :** nécessaire pour isoler le service de contrôle et l'interface du noyau.
- **P5 / graphique :** l'interface RGB et thermique n'a pas besoin d'attendre un compositeur natif ; une API stable doit précéder l'interface.
- **P6 / performances :** mesurer la charge CPU, la latence de commande et la fréquence d'échantillonnage sans sacrifier la sûreté thermique.

## État actuel

**Conception documentée ; aucun pilote de ventilateur/RGB ni service de contrôle n'est déclaré implémenté.** Les prochaines étapes concrètes sont le contrat d'API, l'inventaire en lecture seule et le choix d'une plateforme matérielle de référence. La prise en charge de chaque composant devra être prouvée individuellement.
