# Feuille de route ArchiaOS

> **Règle de suivi :** les pourcentages sont des estimations de réalisation, pas des mesures automatiques. Une phase ne passe à 100 % qu'après implémentation, tests reproductibles et critères d'acceptation satisfaits. Une fonctionnalité décrite dans la documentation mais absente du code ne compte pas comme implémentée.

## Tableau de bord

| Priorité | Domaine | Avancement estimé | État / prochaine preuve attendue |
|---|---|---:|---|
| **P0** | Démarrage UEFI maison x86-64 par défaut | **98 %** | **Seul reste le test sur le PC physique** : démarrer depuis la clé USB avec l’artefact `archiaos-p0-usb-x86_64`, vérifier les diagnostics réels puis redémarrer à froid. Le workflow de packaging a réussi sur le commit `977fa0c5088f015f48f7f5a5261ffcc3c8cec7d8`; cela valide la compilation et l’artefact, pas encore le démarrage matériel. P0 reste ouvert jusqu’à ce test. |
| **P1** | Fiabilité du chargeur, BootInfo et mémoire | **74 %** | Autotests PMM étendus : invariants bitmap, refus des frames réservées, réutilisation après libération et vérification du compte libre. Prochain : audit complet des plages et scénarios d'échec du chargeur. |
| **P2** | Ordonnanceur et synchronisation | **0 %** | Non implémenté ; définir les primitives de tâches et le changement de contexte avant la préemption. |
| **P3** | Espace utilisateur et ABI système | **0 %** | Non commencé ; dépend notamment des exceptions, syscalls et mécanismes mémoire isolés. |
| **P4** | Stockage, VFS et préparation OpenFS | **0 %** | Pas de pilote bloc/VFS fonctionnel ; spécifier d'abord les interfaces. |
| **P5** | Affichage et pile graphique | **0 %** | Pas de pile graphique de production. |
| **P6** | Réseau et mesures de performances | **0 %** | Pas de pile réseau complète ni de bancs de performance stabilisés. |
| **P7** | Distribution, installation et sécurité | **0 %** | Pas de version distribuable validée. |
| **P8** | Fondations noyau pour le contrôle matériel | **12 %** | CPUID x86-64 intégré et inventaire PCI legacy en lecture seule sur le bus 0 ; ECAM/ACPI et tests dédiés restent à faire. |
| **P9** | Service privilégié de télémétrie/contrôle | **0 %** | Pas de service fonctionnel ; dépend d'interfaces noyau et de pilotes sûrs. |
| **P10** | Archia Control Center (interface tout-en-un) | **5 %** | Vision et modules documentés ; pas encore d'application fonctionnelle. |
| **B0–B9** | Chargeur UEFI maison | Voir jalons ci-dessous | Plusieurs fondations validées en CI ; la CI du HEAD courant est verte ; la validation sur matériel physique reste ouverte. |
| **A0–A4** | Démarrage et fondations ARM64 | Voir jalons ci-dessous | Prototype et tests ciblés ; parité avec x86-64 non atteinte. |

Les phases **P8–P10 sont une piste produit parallèle** : elles ne doivent pas conduire à contourner la fiabilité du démarrage, la mémoire, les permissions ou la sécurité. Le but est un logiciel intégré inspiré des usages de CPU-Z, HWiNFO, GPU-Z, Fan Control et OpenRGB, mais conçu pour ArchiaOS et son matériel pris en charge.

## Ordre de travail prioritaire

### P0 — Démarrage UEFI maison x86-64 — 98 %

**Objectif :** faire du chemin UEFI ArchiaOS le démarrage par défaut fiable.

- [x] Remplacer l'ancien chemin Limine dans la configuration par défaut.
- [x] Utiliser le contrat UEFI du chargeur ArchiaOS.
- [x] Confirmer la CI complète verte sur le HEAD `1ec20483c4e699ae884a271d4b1613531d158070` : compilation du noyau x86-64, chargeurs UEFI x86-64/ARM64 et démarrages QEMU/OVMF + AAVMF.
- [x] Confirmer compilation et démarrage sous OVMF/QEMU et AAVMF/QEMU sur le commit `da556433`.
- [x] Vérifier en CI les signatures de sortie CPU, inventaire PCI, BootInfo et autotests PMM.
- [x] Valider un démarrage négatif OVMF/QEMU avec ELF invalide : diagnostic de validation ELF exigé, aucune sortie `ExitBootServices` ni exécution du noyau attendue (CI OVMF : `ELF64 validation failed`, test négatif passé).
- [x] Ajouter un démarrage négatif OVMF où l'ELF est valide mais sa plage physique demandée est indisponible : échec d'AllocateAddress attendu, sans ExitBootServices ni exécution du noyau.
- [x] Ajouter un démarrage négatif OVMF avec volume EFI valide mais noyau absent : diagnostic du fichier manquant attendu, sans validation ELF ni transfert au noyau.
- [x] Ajouter un test de démarrage négatif BootInfo invalide via une feature dédiée au build CI (`test-invalid-bootinfo`) ; le chargeur normal reste inchangé sans cette feature.
- [x] Documenter les configurations CI : x86-64 sous OVMF/QEMU (Q35, 512 MiB, firmware sans Secure Boot), ARM64 sous AAVMF/QEMU (`virt`, Cortex-A57, 512 MiB) et prototype ARM64 en démarrage direct QEMU.
- [x] Compiler et empaqueter les fichiers de test USB x86-64 dans GitHub Actions ; workflow `ArchiaOS P0 physical USB package` réussi sur le commit `977fa0c5088f015f48f7f5a5261ffcc3c8cec7d8`, artefact `archiaos-p0-usb-x86_64` disponible.
- [ ] **SEUL POINT RESTANT POUR TERMINER P0 — validation sur le PC physique** : démarrer depuis la clé USB sans toucher au disque interne, relever les messages réels BootInfo/PMM/entrée noyau, effectuer un redémarrage à froid et vérifier le retour au firmware. Les tests virtuels ne garantissent pas la compatibilité universelle. En attendant que le PC soit disponible, poursuivre P1 sans déclarer P0 terminé.

**Critère de sortie :** build reproductible et tests de démarrage verts sur le HEAD courant, sans déclarer les essais QEMU comme une validation de tout matériel physique.

### P1 — Fiabilité du chargeur et de la mémoire — 74 %

**État : EN COURS.** P1 peut avancer pendant que la validation physique P0 attend la disponibilité du PC ; P0 reste à 98 % et ne sera fermé qu’après le test USB réel.

**Objectif :** sécuriser le transfert UEFI → noyau et la gestion de la mémoire physique.

- [x] Valider les en-têtes ELF64 et les tables de programmes.
- [x] Contrôler les tailles, alignements, débordements, recouvrements et l'entrée exécutable.
- [x] Refuser les segments ELF RWX.
- [x] Construire les mappings high-half x86-64 et appliquer les permissions ELF.
- [x] Activer NXE et CR0.WP dans le chemin testé.
- [x] Fournir un BootInfo versionné et récupérer la carte mémoire UEFI finale.
- [x] Réessayer ExitBootServices avec une clé de carte mémoire fraîche si nécessaire.
- [x] Ajouter un contrôle de la carte mémoire UEFI finale pour détecter les buffers BootInfo/carte mémoire, segments ELF, image du chargeur et arène de tables de pages incorrectement classés en RAM conventionnelle.
- [ ] Compléter l'audit du framebuffer et des autres plages plateforme ; BootInfo indique actuellement un framebuffer absent (base/taille à zéro).
- [x] Vérifier en CI les rejets de libérations invalides/doubles, la cohérence bitmap (toute page libre est gérée, mémoire basse réservée), la réutilisation après libération et la restauration du compte de pages.
- [x] Extraire la validation scalaire du contrat BootInfo en fonction testable sans déréférencer la carte mémoire, avec tests négatifs magic/version, taille de structure, pointeur absent/désaligné, taille de carte et métadonnées.
- [x] Valider aussi l'adresse d'entrée noyau non nulle et la cohérence/non-débordement de la plage framebuffer, avec tests unitaires (validation CI du HEAD courant en attente).
- [x] Ajouter une simulation d'épuisement mémoire isolée et déterministe sur des bitmaps locaux, incluant filtrage des frames réservées et réutilisation, sans consommer la mémoire firmware ni bloquer QEMU (validation CI en attente).
- [x] Vérifier avant ExitBootServices que les buffers BootInfo/carte mémoire, les segments noyau, l'image du chargeur et l'arène de tables de pages ne chevauchent aucune plage UEFI conventionnelle.
- [ ] Vérifier le framebuffer quand un vrai framebuffer sera exposé dans BootInfo et terminer l'audit des plages spécifiques à chaque plateforme.
- [ ] Confirmer ces nouveaux contrôles par la CI du HEAD courant.
- [x] Réinitialiser les bitmaps PMM après la prévalidation et avant chaque tentative de population, afin qu'une tentative échouée ne laisse pas d'état périmé pour une reprise.
- [x] Refuser l'allocation lorsque le bitmap ne contient aucune frame à la fois libre et gérée ; la simulation locale teste l'épuisement déterministe.
- [x] Empêcher la libération des frames qui ne sont pas gérées par le PMM ; tests de frames réservées et de double libération intégrés.
- [ ] Confirmer ces contrôles sur le HEAD courant en CI et achever l'audit des plages réservées.

**Critère de sortie :** invariants mémoire documentés et testés, sans chevauchement dangereux ni accès hors plage.

### P2 — Exceptions, ordonnanceur et synchronisation — 0 %

**Objectif :** exécuter plusieurs tâches de façon contrôlée avant de bâtir un espace utilisateur complet.

1. Stabiliser les handlers d'exceptions et les diagnostics de faute.
2. Définir le contexte CPU d'une tâche et une pile par tâche.
3. Implémenter un changement de contexte coopératif et le tester.
4. Ajouter états de tâche, file d'exécution et primitives de synchronisation.
5. Introduire une source d'interruption/timer adaptée à la plateforme.
6. Ajouter la préemption seulement après validation du changement de contexte.
7. Définir la stratégie SMP et protéger les structures partagées avant d'activer plusieurs cœurs.

**Critère de sortie :** tests reproductibles de création, exécution, blocage et reprise des tâches ; pas de préemption ni de SMP annoncés avant leur implémentation.

### P3 — Espace utilisateur et ABI — 0 %

- Définir les niveaux de privilège et les frontières noyau/utilisateur.
- Définir une ABI système versionnée et des codes d'erreur stables.
- Concevoir les syscalls minimaux avec validation stricte des pointeurs et tailles.
- Ajouter un chargeur de programmes et des espaces d'adressage isolés.
- Prévoir permissions, terminaison et diagnostic des processus.
- Ne pas exposer l'accès brut aux ports, registres ou périphériques aux applications ordinaires.

**Critère de sortie :** un programme utilisateur peut démarrer, appeler l'ABI documentée et échouer sans compromettre le noyau.

### P4 — Stockage, VFS et OpenFS — 0 %

**P4.1 — Contrat VFS**
- Définir les abstractions fichiers, répertoires, volumes, handles, erreurs et permissions.
- Définir la propriété mémoire, les limites de blocs, la synchronisation et la gestion des accès concurrents.
- Séparer le VFS des formats et pilotes de stockage.

**P4.2 — Pilote bloc et tests**
- Choisir un périphérique bloc initial pour QEMU.
- Implémenter les opérations de lecture/écriture et leur gestion d'erreurs.
- Tester les limites, les transferts partiels et les erreurs de périphérique.

**P4.3 — Adaptateur OpenFS**
- Vérifier dans le dépôt distinct Likounsee/OpenFS l'API publique, la licence, les cibles, les dépendances et l'état des tests.
- Créer un adaptateur distinct plutôt que coupler directement le noyau au code interne d'OpenFS.
- Tester montage, lecture, écriture, répertoires, corruption, erreurs et concurrence effectivement prise en charge.
- Épingler une version/révision stable et rendre l'intégration reproductible en CI.

**Critère de sortie :** tests VFS et d'intégration verts. OpenFS ne doit pas être considéré intégré avant ces preuves.

### P5 — Affichage et pile graphique — 0 %

- Détecter et valider les informations framebuffer fournies par le firmware.
- Ajouter une sortie graphique minimale sans dépendance à une résolution particulière.
- Séparer pilote d'affichage, rendu et interface.
- Ajouter ensuite événements d'entrée et composition de fenêtres selon les besoins.

**Critère de sortie :** affichage contrôlé sur configurations documentées, sans supposer qu'un framebuffer UEFI est disponible partout.

### P6 — Réseau et performances — 0 %

- Définir les abstractions de périphérique réseau et de paquets.
- Commencer par un pilote et un environnement virtuel reproductible.
- Implémenter progressivement Ethernet, configuration IP, puis les protocoles nécessaires.
- Ajouter des benchmarks reproductibles pour mémoire, stockage, réseau et ordonnancement.
- Publier les méthodes et configurations de test avec les résultats.

**Critère de sortie :** tests de connectivité et de charge répétables ; chiffres de performance accompagnés de leur environnement de mesure.

### P7 — Distribution et sécurité — 0 %

- Définir le format de l'image, l'installation et la mise à jour.
- Établir la politique de versions, la provenance des artefacts et la reproductibilité.
- Auditer les entrées non fiables, les permissions, les débordements et les chemins d'erreur.
- Exécuter la suite CI sur chaque changement et ajouter des tests de démarrage/installation.
- Ne déclarer une version stable qu'après les critères de sortie documentés.

**Critère de sortie :** image installable et récupérable, procédures de mise à jour documentées et tests de sécurité ciblés réussis.

## P8 — Fondations noyau pour le logiciel de contrôle matériel — 12 %

**But produit :** fournir au futur logiciel une source de télémétrie cohérente et, à terme, un accès contrôlé aux ventilateurs, à l'éclairage RGB et aux fonctions prises en charge des cartes mères et périphériques.

### P8.1 — Identification et inventaire en lecture seule
- [x] Ajouter un module x86-64 CPUID sans allocation dynamique : fabricant, famille/modèle/stepping, marque CPU si disponible et fonctionnalités pertinentes (commit `bb157c7`).
- [x] Vérifier les feuilles CPUID maximales avant chaque interrogation et gérer proprement les champs indisponibles.
- [ ] Ajouter des tests unitaires pour le décodage des champs CPU.
- [ ] Découvrir les périphériques PCI en lecture seule, d'abord avec le mécanisme de configuration disponible sur la plateforme cible.
- [ ] Prévoir une abstraction pour PCIe ECAM/ACPI ; ne pas supposer que l'accès PCI legacy convient à toutes les machines.
- [ ] Exposer un inventaire typé (identifiants vendeur/périphérique, classe, bus/emplacement) sans écrire dans les registres de configuration des périphériques.
- [ ] Journaliser les résultats et tester sur QEMU ; distinguer clairement tests virtuels et matériel physique.

### P8.2 — Découverte de plateforme et capteurs
- [ ] Définir des types communs pour température, tension, puissance, vitesse de rotation et état de disponibilité.
- [ ] Inclure unité, source, précision/validité, horodatage et erreurs ; ne jamais inventer une mesure indisponible.
- [ ] Découvrir les tables ACPI et les informations SMBIOS/UEFI avec validation des bornes.
- [ ] Concevoir une interface de pilote de capteur indépendante du fabricant.
- [ ] Séparer les capteurs CPU, GPU, carte mère, stockage et périphériques ; chaque source peut être absente ou partiellement prise en charge.

### P8.3 — Bus et pilotes
- [ ] Définir les abstractions nécessaires pour PCI/PCIe, I²C/SMBus et USB.
- [ ] Ajouter les pilotes un par un avec identifiants matériels, capacités déclarées et tests ciblés.
- [ ] Garder les protocoles propres à chaque fabricant dans des pilotes/adaptateurs isolés.
- [ ] Limiter les accès matériels à une liste de capacités explicitement autorisées.
- [ ] Ne pas activer une écriture de registre sur la seule base d'une identification approximative du périphérique.

### P8.4 — Contrôle matériel sûr
- [ ] Commencer par la télémétrie en lecture seule.
- [ ] Définir les préconditions, plages valides, unités et erreurs de chaque commande d'actionneur.
- [ ] Prévoir une protection thermique prioritaire, des limites minimales/maximales et un retour à un état sûr.
- [ ] Refuser les commandes non prises en charge ou ambiguës.
- [ ] Ajouter les tests simulés et les tests matériels documentés avant d'autoriser l'écriture.
- [ ] Ne jamais couper une ventilation essentielle ni appliquer un profil RGB/ventilateur non validé.

**Critère de sortie P8 :** inventaire et télémétrie lisibles via des interfaces stables, tests passants et contrôle matériel explicitement limité aux modèles validés. L'inventaire PCI seul ne signifie pas que les températures GPU ou carte mère sont disponibles.

## P9 — Service privilégié de télémétrie et de contrôle — 0 %

- [ ] Créer un service séparé du noyau qui collecte les sources prises en charge.
- [ ] Exposer une API versionnée, documentée et typée au centre de contrôle.
- [ ] Séparer les permissions de lecture de télémétrie des permissions d'écriture/actionneurs.
- [ ] Valider les profils et courbes avant application ; enregistrer l'origine des commandes et les erreurs.
- [ ] Ajouter une simulation sans effet matériel pour développer et tester l'interface.
- [ ] Garantir que l'arrêt du service ou une erreur de profil ne neutralise pas les protections firmware/thermiques.

**Critère de sortie :** service testable sans privilèges d'écriture par défaut, API stable et modes dégradés sûrs.

## P10 — Archia Control Center, application tout-en-un — 5 %

La vision est une seule application avec des modules cohérents, pas une simple liste de capteurs. L'interface ne doit jamais accéder directement aux ports, registres ou protocoles fabricants : elle passe par le service privilégié.

1. **Tableau de bord :** état général, températures, fréquences, charge et alertes réellement disponibles.
2. **CPU et carte mère :** identification, fonctionnalités CPU et capteurs pris en charge.
3. **GPU :** modèle, charge, mémoire et températures uniquement si le pilote/source les fournit.
4. **RAM et stockage :** capacité, état et statistiques accessibles ; diagnostics ensuite.
5. **Ventilation :** ventilateurs détectés, courbes, profils et contrôle uniquement sur modèles validés.
6. **RGB :** périphériques compatibles, zones et profils via pilotes dédiés.
7. **Profils :** sauvegarde, validation, application et retour arrière.
8. **Diagnostics/benchmarks :** tests séparés, résultats comparables et avertissements thermiques.
9. **Compatibilité :** matrice par matériel, version de pilote et fonctionnalité prise en charge.

**Étapes d'implémentation**
- [ ] Établir le contrat de données avec P9 et un mode de simulation.
- [ ] Construire une première vue qui affiche les données réelles de CPUID/inventaire, avec état « indisponible » quand la source n'existe pas.
- [ ] Ajouter les cartes capteurs et l'historique lorsque des mesures réelles sont fournies.
- [ ] Ajouter les profils de ventilation après validation de P8.4 et P9.
- [ ] Ajouter RGB périphérique par périphérique, sans promesse de compatibilité universelle.
- [ ] Ajouter tests d'API, tests UI et vérification des erreurs/pertes de connexion.

**Critère de sortie :** application intégrée, données traçables jusqu'à leur source et commandes limitées aux fonctions réellement validées. Les modules documentés ne comptent pas comme terminés tant qu'ils ne fonctionnent pas.

## Piste parallèle — chargeur UEFI maison et ARM64

### Chargeur UEFI

- **B0 — Entrée UEFI : implémentée.** Crate Rust sans dépendance externe pour x86-64 et AArch64.
- **B1 — Lecture du noyau : implémentée.** Ouvre `\\EFI\\ArchiaOS\\kernel.elf`, borne sa taille et gère les lectures partielles.
- **B2 — Validation ELF64 : implémentée.** Vérifications des en-têtes, segments, tailles, alignements, débordements, recouvrements et entrée exécutable ; segments RWX refusés et tests unitaires présents.
- **B3 — Chargement identité : implémenté sous contraintes.** Réserve les pages physiques ELF, copie les octets et met le BSS à zéro ; prototype surtout utilisé par le chemin AArch64.
- **B4 — Chargement high-half x86-64 : validé en CI antérieure.** Tables 4 niveaux, fenêtre high-half, alias physiques NX et permissions ELF.
- **B5 — Protection x86-64 : validée en CI antérieure.** Vérification NX, activation EFER.NXE et CR0.WP ; test des permissions effectives.
- **B6 — Contrat BootInfo : implémenté.** ABI versionnée `ARCHIABI`, entrée noyau, carte mémoire UEFI finale et pointeurs de plateforme disponibles.
- **B7 — Sortie UEFI : implémentée.** Collecte de la carte mémoire juste avant `ExitBootServices`, nouvelle tentative avec clé fraîche en cas de `EFI_INVALID_PARAMETER`.
- **B8 — Noyau x86-64 de production via le chargeur maison : validé en CI antérieure.** OVMF/QEMU, validation BootInfo et test PMM de 130 allocations/libérations.
- **B9 — Démarrage maison par défaut : bascule du code effectuée ; CI complète verte sur `0094090`, validation sur matériel physique encore en attente.** Lié directement à P0.

### ARM64

- **A0 — Entrée noyau ARM64 : implémentée.** Entrée AArch64, pile dédiée et console PL011 pour QEMU `virt`.
- **A1 — Transfert UEFI ARM64 : validé sur AAVMF/QEMU.** Copie des segments identité, synchronisation I-cache, BootInfo et découverte console via ACPI SPCR sur le firmware testé.
- **A2 — Gestion mémoire ARM64 : prototype.** PMM bitmap sous 4 Gio avec tests d'allocations/libérations ; MMU, réservations exhaustives, SMP et mémoire au-delà de 4 Gio restent à faire.
- **A3 — Exceptions ARM64 : première base implémentée.** Table alignée et test d'exception synchrone ; handlers différenciés, retours contrôlés et interruptions asynchrones restent à faire.
- **A4 — Parité des architectures : conception en cours.** Extraire les interfaces communes sans mélanger les primitives propres à chaque architecture.

## Limites et règles de sécurité

- Les validations CI historiques ne prouvent pas que le HEAD courant est vert ni que tous les PC physiques fonctionnent. Vérifier la CI après chaque changement pertinent.
- Les contraintes actuellement documentées du chargeur comprennent l'accès identité sous 4 Gio, un total de segments limité à 128 Mio et un fichier noyau limité à 64 Mio ; les étendre seulement avec tests.
- Les résultats ARM64 actuels concernent le chemin identité testé ; la pagination high-half décrite ici concerne x86-64.
- Les fonctions capteur et contrôle varient selon le matériel, le firmware et les pilotes. Ne pas annoncer de compatibilité universelle.
- **Lecture seule avant écriture.** Les identifiants matériels ne suffisent pas à autoriser l'écriture dans un registre.
- La sécurité thermique et les mécanismes de protection firmware priment sur les profils utilisateur.
- OpenFS reste un dépôt distinct : vérifier son état et son API réelle avant toute intégration.

## Règle de mise à jour de la feuille de route

Après chaque lot de travail, actualiser les pourcentages seulement si du code ou des tests ont réellement avancé, indiquer la preuve (commit, test ou CI) et noter les blocages. Ne pas augmenter un pourcentage pour une intention ou une documentation seule.