# Feuille de route ArchiaOS

> **Règle de suivi :** les pourcentages sont des estimations de réalisation, pas des mesures automatiques. Une phase ne passe à 100 % qu'après implémentation, tests reproductibles et critères d'acceptation satisfaits. Une fonctionnalité décrite dans la documentation mais absente du code ne compte pas comme implémentée.

## Tableau de bord

| Priorité | Domaine | Avancement estimé | État / prochaine preuve attendue |
|---|---|---:|---|
| **GLOBAL** | Ensemble des objectifs P0–P10 | **28 %** | Score pondéré recalculé après P1 clôturée, P2 à 33,5 % et P8 à 37 % : 28,18 %, arrondi à l’entier le plus proche. Maturité de validation suivie séparément ; les pistes B/A ne sont pas additionnées. |
| **P0** | Démarrage UEFI maison x86-64 par défaut | **100 % (jalon physique atteint)** | Démarrage depuis la clé USB et premier affichage framebuffer confirmés par l’utilisateur sur le PC physique. Le motif bleu prouve l’écriture dans le framebuffer après le handoff ; il ne constitue pas encore un bureau ni une pile graphique. L’utilisateur rapporte 5/5 démarrages à froid et 5/5 redémarrages à chaud, tous avec la même barre blanche finale (compte rendu dans [la campagne physique P1](P1-PHYSICAL-VALIDATION.md)); le retour au firmware et le démarrage du système habituel sont confirmés par l’utilisateur ; le SHA du paquet USB testé est confirmé pour `cb5c676a9adbd0a9cf0e7a4ef69224b4a8729ecc` par `BUILD-INFO.txt` et les SHA-256 des deux binaires ; la photo de preuve existante est désormais archivée dans `docs/preuves/` ; le modèle, CPU et RAM sont consignés ; l'utilisateur confirme que la photo est acceptable et que le BIOS/UEFI est à jour selon Lenovo pour ce modèle (numéro exact non relevé). |
| **P1** | Fiabilité du chargeur, BootInfo et mémoire | **100 % (campagne physique documentée)** | Les contrôles ELF, mappings/protections, BootInfo/ExitBootServices, audit firmware et PMM ont leurs tests automatisés documentés. Le dernier commit de code noyau audité reste `e18b1901e262f21838ab0484459d207ccf004442`. La révision documentaire immédiatement précédente est `7bb1b9d1dd553e5fda68ea206ba3f035197f5751`. Sur ce SHA, [UEFI local, run 38428246398](https://github.com/Likounsee/ArchiaOS/actions/runs/38428246398) et [BootInfo négatif, run 38428246551](https://github.com/Likounsee/ArchiaOS/actions/runs/38428246551) ont réussi ; la [CI principale, run 38428246486](https://github.com/Likounsee/ArchiaOS/actions/runs/38428246486) est encore en cours au dernier contrôle. Les succès automatisés ne prouvent pas une validation matérielle reproductible. La procédure de campagne physique est documentée dans [P1-PHYSICAL-VALIDATION.md](P1-PHYSICAL-VALIDATION.md). Le paquet USB effectivement testé sur matériel correspond au commit `cb5c676a9adbd0a9cf0e7a4ef69224b4a8729ecc` ([workflow](https://github.com/Likounsee/ArchiaOS/actions/runs/38432842691)) : `BUILD-INFO.txt` et les SHA-256 de `kernel.elf` et `BOOTX64.EFI` correspondent exactement à la clé USB. La campagne matérielle est considérée comme acceptée sur la base des confirmations de l’utilisateur : 5/5 démarrages à froid et 5/5 redémarrages à chaud avec barre blanche finale, ainsi que le retour au BIOS/UEFI puis au système habituel sans problème. La provenance du paquet est maintenant confirmée. La photo existante est archivée dans `docs/preuves/` et validée par l'utilisateur. L'utilisateur confirme également utiliser la dernière version BIOS/UEFI publiée par Lenovo pour ce modèle ; le numéro exact n'est pas relevé. La sortie série reste facultative si elle n'est pas matériellement accessible. Les observations physiques restent déclaratives. |
| **P2** | Exceptions, ordonnanceur et synchronisation | **34 %** | Le modèle d'états et le runtime x86-64 à trois tâches existent, avec blocage/réveil et commutations de contexte réelles ; un test déterministe de 10 000 transitions a été ajouté pour vérifier les invariants du scheduler après chaque opération. Un test de #BP contrôlé remplace temporairement la gate du vecteur 3, vérifie le frame normalisé et le retour IRETQ, puis restaure la gate fatale ; ce chemin a été validé par QEMU/CI. Le timer APIC possède déjà un self-test one-shot de livraison/EOI/retour ; un callback P2 peut maintenant compter les ticks et publier une demande de replanification différée sans commuter depuis la pile IRQ, mais le timer périodique calibré et la préemption réelle ne sont pas encore intégrés. La primitive `EventSequence` et six tests ont été ajoutés pour couvrir les notifications concurrentes, le débordement et l’observation par plusieurs waiters ; le parking atomique/réveil de tâches n’est pas encore intégré et ne rapporte donc aucun point. GDT/TSS/IST, topologie CPU, placement multi-cœur et SMP restent à faire. |
| **P3** | Espace utilisateur et ABI système | **0 %** | Non commencé ; dépend notamment des exceptions, syscalls et mécanismes mémoire isolés. |
| **P4** | Stockage, VFS et préparation OpenFS | **0 %** | Pas de pilote bloc/VFS fonctionnel ; spécifier d'abord les interfaces. |
| **P5** | Affichage et pile graphique | **0 %** | Pas de pile graphique de production. |
| **P6** | Réseau et mesures de performances | **0 %** | Pas de pile réseau complète ni de bancs de performance stabilisés. |
| **P7** | Distribution, installation et sécurité | **0 %** | Pas de version distribuable validée. |
| **P8** | Fondations noyau pour le contrôle matériel | **37 %** | CPUID est intégré au démarrage ; les décodeurs de fonctionnalités CPU sont testés en hôte, le noyau SSE4.1 vectorisé possède un repli scalaire, et la CI sur `033df1d` passe les boots QEMU sans SSE4.1 (refus attendu), pré-AVX et AVX. La couverture matérielle physique et le décodage CPU typé restent à compléter ; ECAM/ACPI, capteurs et pilotes contrôlés sont absents. Contrat détaillé dans [CPU-FEATURE-POLICY.md](CPU-FEATURE-POLICY.md). |
| **P9** | Service privilégié de télémétrie/contrôle | **0 %** | Pas de service fonctionnel ; dépend d'interfaces noyau et de pilotes sûrs. |
| **P10** | Archia Control Center (interface tout-en-un) | **0 %** | Vision documentée mais aucune application fonctionnelle ; la documentation seule ne compte pas dans le score. |
| **B0–B9** | Chargeur UEFI maison | Voir jalons ci-dessous | Vue de progression du chargeur ; les preuves doivent être lues avec le SHA du workflow, et la validation sur matériel physique reste ouverte. |
| **A0–A4** | Démarrage et fondations ARM64 | Voir jalons ci-dessous | Prototype et tests ciblés ; parité avec x86-64 non atteinte. |

Les phases **P8–P10 sont une piste produit parallèle** : elles ne doivent pas conduire à contourner la fiabilité du démarrage, la mémoire, les permissions ou la sécurité. Le but est un logiciel intégré inspiré des usages de CPU-Z, HWiNFO, GPU-Z, Fan Control et OpenRGB, mais conçu pour ArchiaOS et son matériel pris en charge.

## Audit de référence — code et preuves au 14 octobre 2026

Cet audit porte sur les sources présentes et leurs raccordements visibles, pas sur les seuls noms de fichiers ou les déclarations de la documentation. Référence de code et de documentation contrôlée : `29644ef8bd95faf8f9b992cdc254523dcf44002b`. Sur ce SHA, la CI principale, le boot UEFI local et le test BootInfo négatif ont tous réussi (runs 38580438801, 38580438520 et 38580438955). La campagne physique du démarrage/PMM est documentée séparément dans [P1-PHYSICAL-VALIDATION.md](P1-PHYSICAL-VALIDATION.md) ; ses résultats sont explicitement attribués au compte rendu de l'utilisateur.

### Méthode de statut, score d'implémentation et maturité de validation

**Deux mesures distinctes :**
- **Implémentation (%)** : part pondérée des livrables fonctionnels dont le code opérationnel et l'intégration sont démontrés.
- **Maturité de validation** : preuve obtenue (tests hôte, CI, QEMU, matériel). Elle est décrite séparément et n'est pas confondue avec l'implémentation.

**États uniformes**
- **Non commencé (0)** : aucun comportement utile correspondant.
- **Partiellement implémenté** : un chemin concret existe, mais l'interface, les erreurs ou une partie essentielle manque.
- **Implémenté, non validé** : chemin raccordé, preuve d'acceptation absente.
- **Validé** : critère d'acceptation passé par une preuve reproductible identifiée.
- **Bloqué** : prérequis nommé empêchant la suite.
- **Remplacé/abandonné** : décision et raison documentées.

**Calcul reproductible**
1. Chaque phase possède une grille de livrables pondérés totalisant exactement 100.
2. Chaque livrable est évalué avec des critères observables. Si les critères sont pondérés, score = poids des critères démontrés / poids total des critères. Sinon, utiliser seulement les paliers suivants : 0 % = aucun comportement utile ; 25 % = contrat ou squelette seulement ; 50 % = chemin nominal raccordé mais erreurs/tests essentiels manquants ; 75 % = chemin et tests automatisés présents, avec validation/cas essentiel restant ; 90 % = seule une validation explicitement pondérée reste ; 100 % = tous les critères d'acceptation sont satisfaits. Un score intermédiaire (par exemple 30 % ou 60 %) est interdit sans sous-critères nommés, cochables et pondérés dans la grille de la phase. Un fichier présent, une API déclarée ou un test seulement écrit ne suffisent pas à cocher un critère.
3. Score phase = somme(poids du livrable × score du livrable) / 100. Un test écrit mais non exécuté ne satisfait pas un critère de test.
4. Score global = somme(poids de phase × score de phase) / 100, avec poids de phases totalisant 100. Arrondi à l'entier le plus proche uniquement pour l'affichage.
5. Les poids ne changent pas après observation des résultats sans justification documentée. Les tâches de documentation seule ne donnent pas de points d'implémentation.
6. Les tableaux B0–B9 et A0–A4 sont des vues transversales : ils ne sont pas ajoutés au score global car les mêmes livrables sont couverts par P0–P10.
7. Chaque preuve doit identifier le commit exact testé. Une CI sur un commit documentaire prouve la réussite des workflows sur ce commit, mais ne change pas le SHA du dernier changement de code.

**Pondération globale P0–P10 (total = 100)**

| Phase | Poids global | Score courant | Contribution |
|---|---:|---:|---:|
| P0 | 5 | 100 | 5,00 |
| P1 | 15 | 100 | 15,00 |
| P2 | 20 | 33,5 | 6,70 |
| P3 | 15 | 0 | 0,00 |
| P4 | 12 | 0 | 0,00 |
| P5 | 8 | 0 | 0,00 |
| P6 | 8 | 0 | 0,00 |
| P7 | 7 | 0 | 0,00 |
| P8 | 4 | 37 | 1,48 |
| P9 | 3 | 0 | 0,00 |
| P10 | 3 | 0 | 0,00 |
| **Total** | **100** | — | **27,98** |

La somme des contributions est **28,18 %** (5 + 15 + 6,70 + 1,48 = 28,18), affichée **28 %** après arrondi. La pondération privilégie les fondations noyau et les livrables qui débloquent plusieurs phases ; les objectifs produit restent dans le dénominateur et ne sont pas supprimés.

**Preuves CI exactes du HEAD documentaire de référence `4c0bfa4f4c752c5f0385d3a0913eec4159e31757` :**
- CI principale : [run 38422476186](https://github.com/Likounsee/ArchiaOS/actions/runs/38422476186) — succès.
- Démarrage UEFI local : [run 38422476688](https://github.com/Likounsee/ArchiaOS/actions/runs/38422476688) — succès sous OVMF/QEMU.
- BootInfo négatif : [run 38422476325](https://github.com/Likounsee/ArchiaOS/actions/runs/38422476325) — scénario négatif attendu réussi.
- Package USB : [run 38375095036](https://github.com/Likounsee/ArchiaOS/actions/runs/38375095036) — succès sur `dfd67f2937b95afc8c8d798c0b77483c2f7a43f9`, sans preuve sur le HEAD courant.
- Campagne matérielle P1 : aucun journal vérifiable consulté ; reste ouverte.

**Runs historiques conservés à titre de comparaison :** CI/UEFI/BootInfo sur `8b5ff8dbcc91cded600c03a98157bba428b86d6f` (runs 38379995664, 38379995825, 38379995687) et sur `8e5cb6276395516685e3a7e8fb6662c362ace32b` (runs 38421778545, 38421778130, 38421778262).

### Grilles de calcul des phases courtes

**P0 — démarrage UEFI par défaut (100 %)**
| Livrable | Poids | Score |
|---|---:|---:|
| Chemin UEFI maison par défaut et build reproductible | 25 | 100 |
| Chargement ELF/BootInfo et transfert au noyau | 25 | 100 |
| Démarrage QEMU/OVMF et tests négatifs de chargement | 25 | 100 |
| Premier démarrage physique et affichage framebuffer observé | 25 | 100 |
| **Total** | **100** | **100 %** |

Le 100 % P0 porte sur le jalon limité « démarrage maison par défaut et premier affichage ». Les démarrages à froid répétés et le retour firmware sont des critères P1, pas implicitement validés par P0.

**P2 — exceptions, scheduler et synchronisation (33,5 %, affiché 34 %)**
| Livrable | Poids | Score courant | Preuve / manque |
|---|---:|---:|---|
| Modèle d'états scheduler et tests de transitions/erreurs | 10 | 100 | Tests hôte dans CI ; pas de dispatch réel entre tâches. |
| Primitive CPU context + pile initiale | 8 | 100 | Test hôte de transfert/retour réel sur pile dédiée x86-64. |
| IDT/stubs/diagnostic fatal | 12 | 25 | IDT/stubs et diagnostic fatal présents. Le test #BP contrôlé vérifie le vecteur, l'absence de code d'erreur, le frame normalisé et IRETQ ; le log est exigé et validé par le smoke test QEMU/CI. |
| API tâche, piles/contextes possédés et switch intégré | 25 | 50 | Runtime x86-64 à trois tâches avec piles/contextes statiques, blocage/réveil/sortie et changements de contexte réels dans `scheduler.rs`. Couverture limitée au banc de test et à trois tâches ; politique générale de durée de vie/allocation absente. |
| Timer, acquittement et préemption | 15 | 0 | Le chemin matériel APIC one-shot, handler, EOI et retour IRETQ est testé au boot. Une API de timer périodique masqué-puis-armé et sa validation des vecteurs/comptes sont ajoutées et couvertes par la CI, mais aucun tick calibré n’est encore relié au scheduler et aucune préemption n’est activée. Critère de préemption toujours à 0. |
| Synchronisation, réveils sans perte et données par CPU | 12 | 0 | `EventSequence` (génération atomique Release/Acquire) et tests de course présents ; pas encore de protocole atomique d’inscription/parking du scheduler, de primitive bloquante intégrée ni de données par CPU. Critère toujours à 0. |
| GDT/TSS/IST et piles d'exception critiques | 8 | 0 | Absent/non validé. |
| Tests multi-tâches, QEMU et SMP | 10 | 0 | Absents. |
| **Total** | **100** | **33,5 %** | 10 + 8 + (12 × 25 %) + (25 × 50 %) = 33,5. Arrondi d’affichage : 34 %. |

**P8 — contrôle matériel (32 %)**
| Livrable | Poids | Score courant |
|---|---:|---:|
| CPUID et décodage CPU | 20 | 100 |
| Découverte PCI legacy lecture seule | 20 | 60 |
| PCIe ECAM/ACPI et inventaire typé | 15 | 0 |
| Découverte ACPI/SMBIOS/capteurs | 15 | 0 |
| Interfaces pilotes et permissions | 10 | 0 |
| Contrôle sûr des actionneurs / état de repli | 10 | 0 |
| Tests QEMU et matériel par modèle | 10 | 50 |
| **Total** | **100** | **37 %** |

Scores P8 calculés par critères élémentaires explicites. Les tests sont évalués séparément par la ligne « Tests QEMU et matériel par modèle » de la grille P8, afin de ne pas les compter deux fois.

- **CPUID : 4/4 critères fonctionnels = 100 %** (`kernel/src/cpu.rs`, raccordé et journalisé par `kernel/src/main.rs`) : (1) vérification des feuilles maximales avant interrogation — démontrée ; (2) décodage fabricant et marque CPU — démontré ; (3) décodage famille/modèle/stepping — démontré ; (4) extraction des indicateurs de fonctionnalités avec vérification des feuilles — démontrée. Cela mesure la fonctionnalité implémentée, pas une couverture de tests dédiée.
- **PCI legacy : 6/10 critères fonctionnels = 60 %** (`kernel/src/pci.rs`, appelé par `kernel/src/main.rs`) : (1) accès de configuration legacy en lecture des périphériques — démontré ; (2) parcours des 32 numéros de périphérique du bus 0 — démontré ; (3) prise en compte du bit multifonction et parcours jusqu'à 8 fonctions — démontré ; (4) décodage vendeur/périphérique — démontré ; (5) décodage classe/sous-classe/interface et type d'en-tête — démontré ; (6) sortie de diagnostic bornée à 64 entrées — démontrée ; (7) découverte des autres bus — absente ; (8) backend PCIe ECAM/ACPI — absent ; (9) inventaire typé réutilisable — absent ; (10) parcours des capacités et validation fonctionnelle étendue — absents.

La ligne « Tests QEMU et matériel par modèle » vaut **50 % (5/10)** : les tests hôte et les profils QEMU sans SSE4.1, pré-AVX et AVX passent dans le run CI 38696224133 sur `033df1d`, mais la campagne sur matériel physique reste à faire. La formule P8 est donc `20 × 100 % + 20 × 60 % + 10 × 50 % = 37` points sur 100. Ces scores mesurent uniquement les critères listés. Aucun point n'est attribué aux capteurs, à ECAM ou au contrôle matériel.

**P9 — service privilégié (0 %) :** API télémétrie et autorisations (25), IPC/authentification et séparation lecture/écriture (20), collecte/normalisation des sources (15), validation des commandes et journalisation (15), simulation/erreurs et arrêt sûr (15), tests intégration/fuzzing (10). Aucun livrable opérationnel raccordé : 0/100.

**P10 — Control Center (0 %) :** contrat de données et simulation (15), application/coquille et navigation (15), tableau de bord avec données source (20), vues CPU/GPU/RAM/stockage selon disponibilité (15), profils ventilateurs/RGB via P9 (15), diagnostics/benchmarks (10), tests UI/API et reconnexion (10). Les modules listés sont des objectifs, pas des implémentations : 0/100.

### Audit des chemins d'exécution de la version actuelle

| Domaine | Ce que le code fait réellement | Limite / statut à refléter |
|---|---|---|
| Boot x86-64 | Entrée ELF, pile noyau propre, console série, passage BootInfo, écriture framebuffer et chemin UEFI maison ; workflows OVMF/CI et package USB. | P0 : premier démarrage physique signalé ; Les critères de la campagne physique P1 sont considérés comme satisfaits sur la base des éléments rapportés par l'utilisateur : 5/5 démarrages à froid, 5/5 redémarrages à chaud, barre blanche finale stable, retour au firmware puis au système habituel, photo de preuve validée et BIOS/UEFI annoncé à jour selon Lenovo pour le LOQ 15IAX9 (numéro exact non relevé). La provenance du paquet testé est confirmée par le SHA et les binaires. Les logs série ne sont pas exigés si COM1 n'est pas matériellement accessible. Cette clôture est fondée sur le compte rendu et les confirmations de l'utilisateur ; les essais n'ont pas été exécutés indépendamment par l'assistant. |
| Mémoire physique | `memory.rs` utilise deux bitmaps (frames gérées et libres), limite la fenêtre gérée à 4 Gio, filtre la carte UEFI et réserve les plages critiques ; auto-tests allocation/libération, doubles libérations et épuisement simulé. | Pas de VMM général, de gestion des espaces d'adressage par processus ni de mapping dynamique complet ; PMM actuel n'est pas un substitut à ces composants. |
| Pagination / isolation | Les mappings de boot et permissions ELF sont construits par le chargeur ; le noyau démarre avec son environnement de mapping initial. | Pas de gestionnaire de tables de pages général côté noyau, pas de séparation fiable noyau/utilisateur ni de changement de CR3 par processus démontré. |
| Exceptions / tâches | `interrupts.rs` fournit IDT x86-64 0–31 et handlers fatals ; `scheduler.rs` modélise les états ; `context.rs` sait faire un transfert x86-64 sur pile dédiée testé en hôte. | Le scheduler ne possède pas les contextes/piles et n'appelle pas le switch ; pas de timer, préemption, livraison d'exception aux tâches ou SMP activé. |
| Pilotes / bus | `cpu.rs` lit CPUID et `pci.rs` parcourt les fonctions PCI du bus 0 via mécanisme de configuration legacy, en lecture seule pour les registres des périphériques. | Pas de PCIe ECAM/MCFG, de découverte générale des bus, de pilote bloc/USB/stockage ni de capteurs en direct. L'écriture du sélecteur CF8 est un accès d'adressage, pas une preuve d'inventaire complet. |
| Système de fichiers | Aucun VFS ou pilote bloc connecté au chemin de démarrage actuel ; OpenFS est un projet distinct. | P4 reste à 0 % pour l'intégration actuelle ; ne pas confondre les sources OpenFS historiques dans ArchiaOS-old avec un FS utilisable par le noyau actuel. |
| Syscalls / espace utilisateur | Pas d'ABI syscall active, d'exécution de binaire utilisateur isolé ni de gestion de processus dans le noyau Rust actuel. | P3 reste à 0 % ; les composants de l'ancien dépôt sont une référence de conception, pas une fonctionnalité portée. |
| ARM64 | Un chemin de démarrage/prototype séparé et un PMM initial existent, avec tests de compilation/émulation selon workflows. | Pas de parité avec x86-64 : MMU/espaces d'adressage, interruptions, SMP et pilotes ne doivent pas être supposés équivalents. |
| Tests et CI | CI principale, démarrage UEFI local et BootInfo négatif réussis sur le HEAD documentaire `4c0bfa4f`; tests hôte du scheduler/context switch exécutés par la CI. | Ces résultats couvrent les jobs configurés ; ils ne prouvent pas le multitâche intégré ni le comportement sur toutes les machines physiques. |

### État des preuves automatisées et portée

| Vérification | Commit réellement testé | Résultat | Portée |
|---|---|---|---|
| CI principale (tests hôte ciblés, builds x86-64/UEFI/ARM64 et smoke test AArch64) | `4c0bfa4f4c752c5f0385d3a0913eec4159e31757` | **Réussie** — [run 38422476186](https://github.com/Likounsee/ArchiaOS/actions/runs/38422476186) | Dernier SHA dont la CI complète est terminée avant cette révision ; ne prouve pas les fonctions non couvertes ni le matériel physique. |
| Démarrage UEFI local du noyau de production | `4c0bfa4f4c752c5f0385d3a0913eec4159e31757` | **Réussi** — [run 38422476688](https://github.com/Likounsee/ArchiaOS/actions/runs/38422476688) | Démarrage sous OVMF/QEMU ; ne remplace pas une validation physique. |
| BootInfo invalide | `4c0bfa4f4c752c5f0385d3a0913eec4159e31757` | **Réussi** — [run 38422476325](https://github.com/Likounsee/ArchiaOS/actions/runs/38422476325) | Valide uniquement le scénario négatif configuré. |
| Référence historique : CI principale | `8b5ff8dbcc91cded600c03a98157bba428b86d6f` | **Réussie** — [run 38379995664](https://github.com/Likounsee/ArchiaOS/actions/runs/38379995664) | Résultat historique, pas une preuve du HEAD plus récent. |
| Référence historique : démarrage UEFI local | `8b5ff8dbcc91cded600c03a98157bba428b86d6f` | **Réussi** — [run 38379995825](https://github.com/Likounsee/ArchiaOS/actions/runs/38379995825) | Résultat historique sous OVMF/QEMU. |
| Référence historique : BootInfo invalide | `8b5ff8dbcc91cded600c03a98157bba428b86d6f` | **Réussi** — [run 38379995687](https://github.com/Likounsee/ArchiaOS/actions/runs/38379995687) | Résultat historique du scénario négatif. |
| Package USB x86-64 | `dfd67f2937b95afc8c8d798c0b77483c2f7a43f9` | **Réussi** — [run 38375095036](https://github.com/Likounsee/ArchiaOS/actions/runs/38375095036) | Plus ancien que le HEAD actuel ; package non revalidé sur le HEAD documentaire. |
| Campagne matérielle P1 | Aucun commit/résultat consigné | **Non validée** | Démarrages à froid, transfert BootInfo/PMM, framebuffer, redémarrage et retour firmware à consigner. |
| Tests hors des jobs ci-dessus | Selon le module | **Couverture partielle** | Absence de test CI ne signifie ni réussite ni échec. |

### Comparaison du code avec `Likounsee/ArchiaOS-old`

L'ancien dépôt est une base C/C++ x86-64 plus large, mais plus complexe et non équivalente à une implémentation de production validée. Ses composants ne sont pas raccordés automatiquement à la version Rust actuelle.

- **Scheduler et SMP :** l'ancien `cpu/scheduler.cpp` contient des tables par CPU, des piles statiques par tâche, un bootstrap de tâches et une intégration au LAPIC ; `cpu/smp.cpp` contient le démarrage d'AP via trampoline, initialisation GDT/TSS/IDT et attente de mise en ligne. C'est une référence utile pour les structures par CPU et le cycle de vie, mais l'ancienne logique dépend de LAPIC, TSS, GDT, IDT et de détails d'assemblage fortement couplés. Réutiliser les invariants et tests, pas copier en bloc.
- **Mémoire :** l'ancien code sépare PMM, paging, VMM, heap et `AddressSpace`, avec tests PMM/paging dédiés et refus explicite de mappings utilisateur dans certaines API du VMM noyau. Il offre des pistes pour un futur VMM et les espaces d'adressage, mais les bitmaps/limites, l'API C++ et les conventions de mapping doivent être audités et adaptés à BootInfo et au modèle Rust actuel. La gestion mémoire actuelle est plus simple et mieux bornée, mais nettement moins complète.
- **Processus / IPC / ring 3 :** l'ancien `process.cpp` inclut tables de PID, chargement ELF, piles utilisateur et entrée ring 3 ; `ipc.cpp` offre une couche IPC. Cela indique un écart fonctionnel majeur avec la version actuelle, mais chaque chemin doit être ré-audité pour isolation, validations ELF, permissions de pages, durée de vie et gestion des erreurs avant portage.
- **VFS / OpenFS :** l'ancien code comprend un VFS, un adaptateur OpenFS, un test OpenFS et une pile OpenFS C (bitmap, allocation, extents, inode, répertoires, liens, transactions, journal et fsck), ainsi que des abstractions bloc/GPT. C'est la source historique la plus concrète pour préparer P4. Avant réutilisation : isoler les appels disque, tester les erreurs E/S et les corruptions, vérifier les limites d'intégrité/journal et les formats on-disk, et écrire un contrat bloc/VFS stable. La présence de ces sources ne prouve ni que tous les chemins sont exécutés ni que le FS est sûr sur un disque réel.
- **ACPI :** l'ancien `acpi.cpp` sait chercher/analyser des tables firmware, mais son commentaire indique un mapping identité initial très large et il lit des adresses physiques via pointeurs volatils. La version actuelle a un audit de plages firmware plus strict au boot. Ne pas importer l'ancien parseur sans reprendre les validations de taille, checksum, mapping et chevauchement.
- **Pilotes PCI :** l'ancien `drivers/pci.cpp` est une base de découverte plus développée que le scan bus 0 actuel, mais reste liée aux mécanismes de configuration et à l'architecture. Comparer sa couverture et ses tests avant d'extraire un backend ; ajouter ECAM/ACPI séparément.
- **Conclusion de portage :** ArchiaOS-old est surtout une **référence de conception et de tests**, pas une dépendance à réintégrer directement. Chaque portage doit être isolé, accompagné de tests hôte, intégré par petites interfaces, puis validé en QEMU avant tout essai matériel.

## Carte des dépendances techniques

| Livrable | Prérequis indispensables | Peut avancer en parallèle | Blocage avant acceptation |
|---|---|---|---|
| P1 — BootInfo/PMM | Boot UEFI et carte mémoire stable | Tests hôte du parseur/PMM | Campagne matérielle répétable ; aucun point de validation physique sans journal d'essais. |
| P2 — scheduler | IDT/interruptions, pile et contexte CPU ; puis timer | Tests de transitions et API tâche | Le scheduler commute réellement entre deux tâches sur piles distinctes et le scénario runtime block/wake (A bloque, B réveille A, A reprend et termine, puis B termine) passe dans la CI. Pas de préemption/SMP avant validation du switch intégré et tests multi-tâches. |
| P3 — processus/utilisateur | VMM, permissions page par page, espace d'adressage, cycle de vie scheduler | Conception ABI et tests hôte ELF | Pas de ring 3/syscall avant séparation mémoire et erreurs démontrées. |
| P4 — VFS/OpenFS | Contrat bloc, mémoire et synchronisation | Tests de format OpenFS en hôte et outils d'image | OpenFS n'est pas intégré ; pas de montage annoncé avant tests d'E/S et corruption. |
| P5 — graphique | Framebuffer BootInfo ; pilotes d'entrée pour interaction | Rendu logiciel et UI en hôte | GOP absent/invalide doit laisser un mode console de secours. |
| P6 — réseau | Allocation DMA sûre, découverte PCI, interruptions/timer, modèle pilote | Parsing de paquets en hôte, fuzzing, benchmarks | Aucun pilote NIC opérationnel ; connectivité à prouver sous QEMU puis matériel. |
| P7 — distribution/sécurité | P0/P1 stables ; P3 pour isolation ; P4 si installation sur stockage interne | SBOM, reproductibilité, threat model, fuzzing | Parcours installation/mise à jour/récupération complet à tester. |
| P8 — contrôle matériel | PCI/ECAM, ACPI/SMBIOS, interfaces pilotes et permissions | CPUID/inventaire lecture seule et simulateurs | Aucun accès en écriture avant identification fiable, limites d'actionneur et état sûr. |
| P9 — service privilégié | API P8, séparation de privilèges P3, journalisation/IPC | Simulation et contrat API | Lecture seule par défaut et refus des commandes non autorisées. |
| P10 — Control Center | Contrat P9, UI/graphique ou plateforme cible, simulation | Maquettes/UI avec données simulées | Pas de donnée inventée ; traçabilité et perte de service testées. |
| ARM64 | BootInfo commun, primitives AArch64, MMU/interruptions propres | Tests hôte des contrats communs | Ne pas réutiliser l'assembleur x86-64 ; parité après validation AArch64 séparée. |

Dépendances structurantes : **VMM/MMU → espaces d'adressage → processus/syscalls** ; **IDT/interruptions → timer/acquittement → préemption → synchronisation SMP** ; **pilote bloc → contrat VFS → adaptateur OpenFS** ; **inventaire/permissions pilotes → télémétrie lecture seule → service privilégié → Control Center**. Le rendu logiciel, les tests de format en hôte, la conception ABI et les simulateurs peuvent avancer en parallèle, mais ne remplacent pas les validations de bout en bout.

## Ordre de travail prioritaire

### P0 — Démarrage UEFI maison x86-64 — 100 % (jalon physique atteint)

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
- [x] Jalon historique : compiler et empaqueter les fichiers USB x86-64 via GitHub Actions sur `977fa0c5088f015f48f7f5a5261ffcc3c8cec7d8` ; d'autres runs plus récents sont référencés dans le tableau de bord. Aucun package USB n'est démontré sur le HEAD documentaire courant.
- [x] **Démarrage physique et premier affichage confirmés** : l’utilisateur a démarré depuis la clé USB et observé un fond bleu foncé avec une bande bleu clair, preuve que le noyau écrit dans le framebuffer après le handoff UEFI. Le redémarrage à froid et le retour au firmware restent non consignés ; ils ne bloquent plus le jalon P0 de premier démarrage.

**Critère de sortie :** build reproductible et tests de démarrage verts sur le HEAD courant, sans déclarer les essais QEMU comme une validation de tout matériel physique.

### P1 — Fiabilité du chargeur et de la mémoire — 100 %

**État : IMPLÉMENTATION ET VALIDATION AUTOMATISÉE TERMINÉES — 100 %.** La campagne physique du chemin UEFI/BootInfo/PMM est documentée à partir des observations de l'utilisateur (5/5 démarrages à froid, 5/5 redémarrages à chaud, barre blanche finale et retour au système habituel). La provenance du paquet physique testé est le commit `cb5c676a9adbd0a9cf0e7a4ef69224b4a8729ecc`. Cette preuve matérielle porte sur le démarrage, BootInfo, PMM et l'atteinte du marqueur framebuffer ; elle ne prétend pas valider sur matériel chaque ajout VMM ultérieur.

Le code actuel de la branche `ArchiaOS` est `29644ef8bd95faf8f9b992cdc254523dcf44002b`. Les trois workflows sur ce SHA ont réussi : [CI principale](https://github.com/Likounsee/ArchiaOS/actions/runs/38580438801), [démarrage UEFI local](https://github.com/Likounsee/ArchiaOS/actions/runs/38580438520) et [BootInfo négatif](https://github.com/Likounsee/ArchiaOS/actions/runs/38580438955). La CI principale inclut un boot de production QEMU à 5 Gio qui exige la réussite du mapping et de l'accès mémoire au-dessus de 4 Gio, ainsi que les auto-tests du VMM, du tas noyau et des espaces d'adressage.

**Objectif :** sécuriser le transfert UEFI → noyau et les fondations mémoire physiques/virtuelles utilisées par la suite.

**Calcul P1 (poids = 100 ; score = somme(poids × score du livrable) / 100)**

| Livrable pondéré | Poids | Score | Preuve / limite |
|---|---:|---:|---|
| Validation ELF64, segments, alignements, débordements et refus RWX | 20 | 100 | Audits et tests de chargement, dont scénarios négatifs UEFI. |
| Mapping high-half, NXE/CR0.WP et permissions ELF | 20 | 100 | Boot de production et contrôles de permissions dans la CI courante. |
| BootInfo versionné, carte mémoire finale et ExitBootServices avec retry | 20 | 100 | Boot UEFI et workflow BootInfo négatif réussis sur le SHA courant. |
| Audit des plages firmware consommées, ACPI/DT, framebuffer et réservations | 15 | 100 | Contrôles de bornes, chevauchements, types UEFI et réservations. |
| PMM 512 Gio, VMM/NX/W^X, accès au-dessus de 4 Gio, tas et racines d'adressage bootstrap | 15 | 100 | Auto-tests de démarrage de production sur QEMU 5 Gio ; l'usage SMP reste explicitement hors périmètre. |
| Campagne physique du démarrage, BootInfo/PMM et framebuffer | 10 | 100 | 5/5 démarrages à froid et 5/5 redémarrages à chaud rapportés ; marqueur blanc final et retour firmware/système habituel confirmés. Le résultat physique reste un compte rendu utilisateur, pas une observation directe de l'assistant. |
| **Total** | **100** | **100 %** | **Tous les livrables de cette phase sont clôturés ; les limites multi-CPU et ring 3 sont suivies en P2/P3.** |

**Implémenté et validé**
- [x] Valider ELF64, en-têtes/program headers, tailles, alignements, débordements, recouvrements et entrée exécutable ; refuser RWX.
- [x] Construire les mappings high-half x86-64, activer NXE/CR0.WP et appliquer les permissions de segments.
- [x] Transmettre BootInfo versionné, carte mémoire finale et framebuffer validé ; réessayer `ExitBootServices` avec une clé fraîche.
- [x] Exclure du PMM les types UEFI réservés, la mémoire basse, le framebuffer et le stockage des bitmaps ; vérifier les invariants et les erreurs de libération.
- [x] Suivre jusqu'à 512 Gio et tester explicitement l'allocation et l'accès virtuel à de la RAM au-dessus de 4 Gio sur le boot QEMU 5 Gio.
- [x] Ajouter le VMM noyau à la demande, les mappings NX/W^X, le retrait et l'invalidation TLB locale ; tester collisions, accès et récupération des tables.
- [x] Ajouter le tas noyau page-backed et ses tests d'alignement, extension, fusion, libération et réutilisation.
- [x] Ajouter les racines d'adressage bootstrap, mappings utilisateur isolés, bascule CR3 et nettoyage ; l'exécution ring 3/les syscalls ne font pas partie de P1.
- [x] CI principale, boot UEFI local et BootInfo négatif verts sur le SHA courant : [CI](https://github.com/Likounsee/ArchiaOS/actions/runs/38580438801), [UEFI local](https://github.com/Likounsee/ArchiaOS/actions/runs/38580438520), [BootInfo négatif](https://github.com/Likounsee/ArchiaOS/actions/runs/38580438955).
- [x] Campagne physique du chemin de démarrage/PMM documentée dans [P1-PHYSICAL-VALIDATION.md](P1-PHYSICAL-VALIDATION.md), avec provenance du paquet et résultats rapportés.

**Limites transmises aux phases suivantes**
- Les tables de pages et le PMM ne sont pas synchronisés pour SMP ; pas de shootdown TLB inter-CPU.
- Le tas noyau est limité au démarrage mono-CPU et ne doit pas être utilisé depuis des IRQ.
- Les racines d'adressage sont un socle pour P3, pas encore un chargeur ELF utilisateur ni une transition ring 3.
- La validation physique rapportée concerne le paquet USB identifié dans la campagne ; les nouvelles voies VMM/adressage sont couvertes par QEMU/CI et n'ont pas été revendiquées comme testées séparément sur matériel.

**Critère de sortie :** atteint pour le périmètre P1 défini ci-dessus. Les interruptions/SMP relèvent de P2 ; ring 3, syscalls et cycle de vie des processus relèvent de P3.


### P2 — Exceptions, ordonnanceur et synchronisation — 34 %

**État : PARTIELLEMENT IMPLÉMENTÉ — 34 % (estimation).** Le modèle d'états et la primitive x86-64 sont intégrés entre `scheduler.rs` et `context.rs`. Le scénario runtime block/wake est validé en CI : A se bloque, B réveille A puis cède le processeur, A reprend et termine, puis B reprend et termine avant le retour à la pile de démarrage. Le correctif du CPU QEMU (`-cpu Haswell`) et le démarrage x86-64 sont validés par les runs UEFI/CI sur le SHA `e73bc1e`. Le test d'exception #BP contrôlé (vecteur, absence de code d'erreur, frame normalisé et retour IRETQ) est également validé. Les tests de timer périodique couvrent les diviseurs légaux/illégaux, la validation avant écriture des registres et le masquage après armement, mais aucun tick calibré n’est encore relié au scheduler et aucune préemption n’est activée. La primitive de contexte vérifie maintenant les canaris, l'appartenance et les bornes du frame de restauration ; la CI hôte/QEMU passe sur le commit `e73bc1e`. La synchronisation multi-cœur, GDT/TSS/IST dédiés, découverte de topologie, placement hétérogène, XSAVE/AVX et préemption restent à faire. Le scheduler actuel fait du round-robin sans découverte de topologie ni placement conscient des différences de performances/caches : cette exigence est spécifiée dans [SCHEDULER.md](SCHEDULER.md).

**Implémenté et vérifié**
- [x] Modèle d'ordonnanceur borné sans allocation dynamique : états `Empty`, `Runnable`, `Running`, `Blocked`, `Exited`, sélection round-robin, blocage/réveil, sortie et invariants.
- [x] Tests hôte des transitions et cas d'erreur du scheduler, exécutés par un job CI dédié.
- [x] Primitive x86-64 de sauvegarde/restauration des registres callee-saved et de RSP, avec pile initiale alignée et entrée de tâche.
- [x] Test hôte de transfert réel vers une pile dédiée et retour au contexte initial.
- [x] Première intégration scheduler/context : deux tâches ont chacune un contexte CPU et une pile statique ; elles cèdent réellement le CPU, reprennent après suspension et sortent sans être redispatchées. Test hôte et test de démarrage noyau raccordés ; scénario historique validé sur `087af782`.
- [x] Ajouter des tests hôte ciblés : un réveil seulement par tâche bloquée, le second réveil doit échouer sans mutation, le round-robin doit reprendre la tâche réveillée et les identifiants des tâches terminées ne sont pas réutilisés.
- [x] Ajouter `dispatch_or_idle()` pour signaler explicitement l'absence de tâche exécutable et permettre une reprise après réveil ; tests hôte pour tâches toutes bloquées et toutes terminées (la boucle idle d'architecture reste à implémenter).
- [x] Valider dans la CI le scénario de blocage/réveil : A bloque, B réveille A, A reprend et termine, puis B termine ; vérifier les invariants et le retour à la pile de démarrage (CI complète réussie sur `ee42f21`).
- [ ] Définir un modèle de topologie/capacités CPU commun au scheduler, avec données inconnues explicites et tests synthétiques.
- [ ] Découvrir les types de cœurs, domaines de cache, CCD/CCX et informations NUMA exposés par CPUID/ACPI, sans dépendre des noms de modèles.
- [ ] Ajouter un placement adapté aux CPU homogènes et hétérogènes : Intel P/E, AMD avec caches/3D V-Cache asymétriques, affinités et localité.
- [ ] Tester la politique sur topologies synthétiques (homogène, hybride, CCD/cache asymétriques, informations partielles) avant de l'activer avec SMP.
- [x] Première IDT x86-64 pour les exceptions 0–31, normalisation des codes d'erreur et diagnostic CR2 pour page fault ; le comportement fatal reste volontaire.

**À faire — ordre de dépendance**
- [ ] Tester explicitement l'IDT chargée, la validité des sélecteurs et le chemin de diagnostic par déclenchement contrôlé d'exceptions bénignes puis de fautes isolées sous QEMU.
- [ ] Étendre le runtime statique de deux à plusieurs tâches et définir une API générale de cycle de vie (entrée, argument, sortie, destruction) ; garder les piles hors de la pile de démarrage.
- [ ] Ajouter la gestion des tâches bloquées/réveillées au runtime réel, avec rollback propre quand aucune tâche n'est exécutable et sans perdre de réveil.
- [ ] Définir la politique de réclamation/réutilisation des piles après sortie ; aucune pile ne doit être réutilisée tant que son contexte n'est plus actif.
- [ ] Définir la convention d'appel et l'état sauvegardé par architecture ; documenter explicitement que la primitive actuelle est x86-64/System V et ne constitue pas une abstraction ARM64.
- [~] Ajouter une primitive de synchronisation FIFO (TicketLock), tests de contention multi-thread, ordre FIFO déterministe, débordement des compteurs et acquisition opportuniste `try_lock` sans dépassement de file ; l'intégration à une file d'attente du scheduler, au park/unpark et aux tests d'interleavings de réveil reste à faire.
- [ ] Configurer un timer fiable et son acquittement, d'abord en mode coopératif puis avec préemption après validation de l'état complet sauvegardé.
- [ ] Ajouter GDT/TSS/IST et piles dédiées aux exceptions critiques ; tester double fault et stack fault sans dépendre de la pile fautive.
- [ ] Définir la stratégie SMP, les données par CPU, les verrous/atomiques et les règles d'ordre mémoire avant de réveiller un second cœur.
- [ ] Ajouter tests de stress et régression sous QEMU, puis validation matérielle pour les chemins d'interruptions/timer.

**Critères de sortie P2 :** plusieurs tâches indépendantes commutent réellement et de façon reproductible ; leurs piles/contextes restent valides ; exceptions attendues et fautes fatales produisent un diagnostic stable ; timer et synchronisation sont testés ; toute activation SMP est couverte par des tests multi-CPU. Une primitive de switch testée seule ne suffit pas.

### P3 — Espace utilisateur et ABI — 0 %

**État : NON COMMENCÉ.** Prérequis : gestionnaire de tables de pages/VMM, permissions page par page, contexte CPU intégré au scheduler et gestion fiable des exceptions.

| Livrable | Poids | Critères d'acceptation / preuves exigées |
|---|---:|---|
| Contrat mémoire virtuelle et espaces d'adressage | 25 | Créer/détruire un espace, mapper/démapper des pages, permissions W^X, séparation noyau/utilisateur et invalidation TLB ; tests des limites, collisions et échecs d'allocation. |
| Passage ring 3 et retour contrôlé | 15 | GDT/TSS/IST et piles noyau par tâche configurées ; entrer en ring 3 puis revenir sans corruption ; test QEMU avec registres/flags vérifiés. |
| ABI syscall versionnée | 15 | Convention d'appel documentée, numéros stables, erreurs et chemin d'entrée sécurisé ; tests d'arguments valides/invalides. |
| Validation des pointeurs et copies user/kernel | 15 | Aucun déréférencement direct de pointeur utilisateur ; vérifier tailles, overflow, pages mappées et droits ; fuzzing des adresses limites. |
| Chargeur ELF utilisateur | 15 | Vérifier ELF, segments, permissions, entrée, pile et arguments ; rejeter fichiers tronqués, RWX, overflow et segments chevauchant les zones réservées. |
| Cycle de vie processus/threads | 10 | PID/état, lancement, yield/blocage, sortie, nettoyage et récupération d'erreurs ; test de plusieurs processus. |
| Tests d'isolation et régression | 5 | Un programme utilisateur ne peut lire/écrire la mémoire noyau ; faute utilisateur isolée et aucun impact sur un autre processus. |
| **Total** | **100** | Phase validée seulement quand un programme ring 3 utilise l'ABI et que les tests d'isolation passent. |

**Ordre :** VMM → espace d'adressage → transition ring 3 → syscall minimale → chargeur ELF → cycle de vie → isolation multi-processus. Le contrat ABI et les tests ELF peuvent être conçus en parallèle. Risque principal : confusion adresse virtuelle/physique ou pointeur utilisateur malveillant. Tests hôte pour parseur/copies, puis QEMU pour privilèges et fautes ; matériel après stabilisation.


### P4 — Stockage, VFS et OpenFS — 0 %

**État : NON COMMENCÉ dans le noyau Rust actuel.** Les sources historiques VFS/OpenFS d'ArchiaOS-old et le dépôt OpenFS distinct sont des références, pas une intégration active. Prérequis : PMM, synchronisation de base et contrat de périphérique bloc.

| Livrable | Poids | Critères d'acceptation / preuves exigées |
|---|---:|---|
| Contrat bloc + pilote initial QEMU | 20 | API bloc bornée, capacité/taille secteur, lecture/écriture, timeout/erreurs et transferts partiels ; tests sur image virtuelle et périphérique absent. |
| Cache de blocs et synchronisation | 10 | Règles de cohérence, verrouillage et vidage explicites ; tests d'accès concurrents, échec d'écriture et limites de cache. |
| Contrat VFS | 20 | Fichiers, répertoires, handles, volumes, permissions, erreurs et propriété mémoire ; tests indépendants du format on-disk. |
| Opérations fichiers et montage | 15 | Ouvrir/lire/écrire/seek/stat/readdir/close, chemins bornés et erreurs stables ; tests fichiers vides, gros fichiers, noms limites et espace insuffisant. |
| Adaptateur OpenFS | 15 | API/version/licence/dépendances vérifiées, adaptation isolée, format épinglé et build reproductible ; pas de couplage aux détails internes. |
| Robustesse et corruption | 15 | Images tronquées/corrompues, métadonnées invalides, allocations épuisées, erreurs I/O ; pas de panique ni écriture hors volume. Vérifier les garanties réelles du journal/transactions. |
| Intégration CI/QEMU et stockage | 5 | Image de test, formatage/montage/lecture/écriture/reboot et persistance ; tests d'intégration reproductibles. |
| **Total** | **100** | Montage, opérations VFS et régression corruption doivent réussir avant d'annoncer OpenFS intégré. |

**Ordre :** contrat bloc → pilote/erreurs → tests d'image → VFS → adaptateur OpenFS → robustesse/corruption → tests reboot/persistance. Les tests hôte du format et les outils d'image peuvent avancer en parallèle. Risques : incohérence après panne, corruption on-disk, blocage I/O et concurrence. Validation hôte + QEMU avec erreurs injectées ; stockage réel avant compatibilité large.


### P5 — Affichage et pile graphique — 0 %

**État : NON COMMENCÉ pour une pile graphique générale.** Le framebuffer UEFI est transmis et un motif de démarrage a été vu physiquement ; cela n'équivaut pas à un pilote graphique général, une gestion d'entrées ou un compositeur.

| Livrable | Poids | Critères d'acceptation / preuves exigées |
|---|---:|---|
| Contrat framebuffer et modes de secours | 15 | Valider format, stride, dimensions, taille et overflow ; console série utilisable si framebuffer absent/invalide. |
| Rendu 2D logiciel | 20 | Primitives, clipping, conversion RGB/BGR, limites et rendu sans dépassement ; tests d'images de référence en hôte. |
| Pilote et abstraction d'affichage | 15 | Séparer backend framebuffer, rendu et UI ; modes non pris en charge signalés explicitement. |
| Entrées clavier/souris | 15 | Contrat d'événements, files bornées, touches maintenues/perdues et périphériques absents ; tests virtuels QEMU. |
| Fenêtres/composition et événements | 20 | Cycle de vie des fenêtres, ordre de rendu, focus, événements et nettoyage ; test d'intégration multi-fenêtres. |
| Tests et validation plateforme | 15 | Tests hôte clipping/format, QEMU à plusieurs résolutions et appareil documenté ; chemin de secours si GOP manque. |
| **Total** | **100** | Rendu stable sur les modes déclarés et gestion propre de l'absence de périphérique. |

**Ordre :** contrat framebuffer → rendu logiciel → entrées → composition. Peut avancer en parallèle avec VFS via tests hôte ; les applications isolées finales dépendent de P3. Risques : stride incorrect, accès MMIO invalide, formats propriétaires et absence de GOP. Validation QEMU et matériel pour chaque backend annoncé.


### P6 — Réseau et performances — 0 %

**État : NON COMMENCÉ pour une pile réseau opérationnelle.** Prérequis : allocation DMA/mémoire sûre, découverte PCI, interruptions/timer et modèle de pilote.

| Livrable | Poids | Critères d'acceptation / preuves exigées |
|---|---:|---|
| Abstraction NIC, DMA et files de paquets | 20 | API bornée, propriété des buffers, alignement et synchronisation ; tests hôte des transitions et épuisement. |
| Pilote réseau initial sous QEMU | 20 | Choisir une NIC virtuelle supportée, initialisation/arrêt, RX/TX et erreurs ; trafic reproductible en réseau isolé QEMU. |
| Ethernet et ARP | 10 | Validation tailles/MAC, trames tronquées et tables ARP bornées ; tests de paquets malformés. |
| IPv4, ICMP et configuration | 15 | Parsing avec vérification longueurs/checksum, configuration documentée ; ping aller/retour virtuel. |
| UDP puis TCP selon besoins | 15 | États, timeouts, retransmission et fermeture ; tests pertes, duplications, paquets hors ordre et limites. |
| Sécurité de parsing et robustesse | 10 | Fuzzing paquets, limites mémoire et erreurs ; aucune panique sur données réseau non fiables. |
| Benchmarks reproductibles | 10 | Mesures scheduler/mémoire/stockage/réseau avec matériel, build, paramètres et dispersion documentés. |
| **Total** | **100** | Connectivité reproductible et tests de charge/sécurité passants sur le pilote explicitement pris en charge. |

**Ordre :** abstraction NIC/DMA → pilote QEMU → Ethernet/ARP → IP/ICMP → UDP/TCP → fuzzing et benchmarks. Le parsing peut être testé en hôte sans pilote. Risques : DMA hors plage, interruptions perdues, paquets hostiles et benchmarks non comparables. QEMU d'abord, matériel réseau ensuite.


### P7 — Distribution, installation et sécurité — 0 %

**État : NON COMMENCÉ comme produit distribuable stable.** La CI et le package USB de développement ne constituent pas un installateur, une mise à jour sûre ou une politique de sécurité complète.

| Livrable | Poids | Critères d'acceptation / preuves exigées |
|---|---:|---|
| Menaces, frontières de confiance et privilèges | 15 | Modèle de menace couvrant chargeur, fichiers, utilisateurs, pilotes, service privilégié et mises à jour ; revue des mitigations. |
| Build reproductible et provenance | 15 | Toolchain épinglée, artefacts identifiés par SHA, SBOM/inventaire des dépendances et procédure de reproduction. |
| Image de distribution et installation | 20 | Image versionnée, détection des cibles, confirmation avant destruction, échec sans perte de données non voulue ; test QEMU sur disque jetable. |
| Mise à jour et retour arrière | 15 | Validation des artefacts, interruption tolérée ou récupération documentée, version précédente restaurable ; test de panne simulée. |
| Sécurité du noyau et entrées non fiables | 15 | Fuzzing ELF/BootInfo/ACPI/VFS/paquets, tests overflow/permissions, erreurs sans panique ; suivi des vulnérabilités. |
| Tests d'intégration bout en bout | 10 | Installation → boot → configuration → mise à jour → retour arrière ; artefacts et logs conservés en CI. |
| Documentation support et récupération | 10 | Matrice matériel, limitations, sauvegarde, diagnostic et récupération d'un boot échoué testés. |
| **Total** | **100** | Une version stable exige un parcours reproductible et une revue des risques connus, pas seulement une CI verte. |

**Ordre :** threat model et reproductibilité tôt ; installateur après boot/stockage fiables ; mises à jour après versionnement/provenance ; tests de récupération avant publication. Audits parser/fuzzing en parallèle. Risques : écrasement disque, artefacts compromis, échec mise à jour et fuite de privilèges. Validation QEMU puis matériel sur configurations supportées.


## P8 — Fondations noyau pour le logiciel de contrôle matériel — 32 %

**But produit :** fournir au futur logiciel une source de télémétrie cohérente et, à terme, un accès contrôlé aux ventilateurs, à l'éclairage RGB et aux fonctions prises en charge des cartes mères et périphériques.

### P8.1 — Identification et inventaire en lecture seule
- [x] Ajouter un module x86-64 CPUID sans allocation dynamique : fabricant, famille/modèle/stepping, marque CPU si disponible et fonctionnalités pertinentes (commit `bb157c7`).
- [x] Définir et appliquer SSE4.1 comme minimum x86-64 du noyau ; détecter SSE4.2, AES-NI, FMA, BMI1/BMI2 et AVX/AVX2/AVX-512F avec séparation entre support matériel et état OS utilisable (boots QEMU de validation réussis sur `033df1d`, run CI 38696224133).
- [x] Ajouter des tests unitaires synthétiques pour les profils sans SSE4.1, SSE4.1 seul, AVX, OSXSAVE/XCR0 incomplets et AVX-512, plus le repli scalaire et la somme vectorisée SSE4.1 ; les tests hôte et les profils QEMU correspondants passent dans la CI 38696224133 ; la validation physique reste ouverte.\n- [x] Ajouter des boots QEMU du noyau de production sans SSE4.1 (refus attendu), SSE4.1 seul/pré-AVX et AVX ; workflow CI 38696224133 réussi pour les profils QEMU ; la validation matérielle reste ouverte.
- [x] Vérifier les feuilles CPUID maximales avant chaque interrogation et gérer proprement les champs indisponibles.
- [ ] Ajouter des tests unitaires pour le décodage des champs CPU.
- [x] Première découverte PCI lecture seule via mécanisme de configuration legacy, limitée au bus 0 et à 32 périphériques / 8 fonctions ; implémentation `kernel/src/pci.rs`, sans test dédié ni couverture de tous les bus.
- [ ] Ajouter des tests ciblés et l'inventaire typé ; prévoir ensuite PCIe ECAM/ACPI, car l'accès legacy ne couvre pas toutes les machines.
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

## P10 — Archia Control Center, application tout-en-un — 0 %

La vision est une seule application avec des modules cohérents, pas une simple liste de capteurs. Les objectifs documentés ne constituent pas une implémentation ; le score P10 reste donc à 0 % jusqu'à ce qu'une application fonctionnelle existe. L'interface ne doit jamais accéder directement aux ports, registres ou protocoles fabricants : elle passe par le service privilégié.

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
- **B9 — Démarrage maison par défaut : bascule du code effectuée ; `0094090` est une référence historique, pas la preuve de la CI courante.** Les runs actuels et leurs SHA sont indiqués dans le tableau « État des preuves automatisées ». Le premier affichage physique a été signalé par l'utilisateur ; la validation matérielle répétable reste distincte. Lié directement à P0.

### ARM64 — progression séparée estimée : 46 % (non ajoutée au global)

Grille A0–A4 : A0 entrée (10 %), A1 transfert UEFI (20 %), A2 mémoire/MMU (25 %), A3 exceptions/interruptions (20 %), A4 interfaces communes/parité (25 %). Scores courants estimés sur les preuves listées : A0 100, A1 100, A2 35, A3 25, A4 10, soit 46,25 % → **46 %**. A2/A3/A4 sont des prototypes partiels, pas une parité avec x86-64.

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

## Prochaine priorité recommandée — clôture de P1

**Priorité immédiate : terminer la validation restante de P1 avant de déclarer P1 achevée ou de faire passer officiellement P2 au rang de phase prioritaire.** Le dernier livrable de P1 vaut 10 points et exige une campagne sur machine physique. Les succès GitHub Actions, QEMU/OVMF et les tests hôte ne peuvent pas remplacer cette preuve matérielle.

### Lot P1 — campagne matérielle

1. **Identifier l’artefact exact :** utiliser le package USB x86-64 produit depuis le commit candidat, consigner son SHA et le commit source. Le succès historique du package sur un autre commit ne valide pas le candidat actuel.
2. **Réaliser plusieurs démarrages à froid :** consigner le modèle de machine et le firmware, le périphérique de démarrage, le résultat et les éventuels blocages ; conserver la sortie série quand elle est disponible.
3. **Vérifier le transfert UEFI → noyau :** confirmer que BootInfo et la carte mémoire sont cohérents, que le PMM s’initialise et que les réservations préservent le framebuffer.
4. **Vérifier la fin du cycle de démarrage :** confirmer l’affichage framebuffer, redémarrer et vérifier le retour au firmware.
5. **Archiver la preuve :** conserver les journaux, le résultat par essai, le SHA de l’artefact et le commit exact testé dans le dépôt.

**Critères de sortie P1 :** campagne complète et reproductible documentée, artefact traçable, démarrages à froid réussis et comportement BootInfo/PMM/framebuffer/retour firmware vérifié. Si un essai échoue, corriger le défaut et répéter les vérifications concernées ; ne pas attribuer les 10 points sur la seule base d’un premier démarrage.

**Clôture P1 :** l'utilisateur confirme que la photo est acceptable et que le BIOS/UEFI est à jour selon Lenovo pour ce modèle. Avec les résultats 5/5 à froid et 5/5 à chaud, le retour firmware/OS et la provenance vérifiée du paquet USB, la campagne P1 est considérée terminée (**100 %**). Le numéro exact de version BIOS/UEFI n'est pas relevé. Ce statut reflète les preuves et essais rapportés par l'utilisateur, et non une exécution indépendante par l'assistant.

### P2 — lot courant : renforcer le cycle de vie coopératif

- [x] Ajouter au runtime de test un vrai blocage d'A, un réveil d'A par B, la reprise sur la pile/context d'origine et la terminaison des deux tâches.
- [x] Exiger un marqueur de réussite distinct dans le journal UEFI/CI.
- [x] Vérifier que la CI hôte et QEMU passent sur le SHA exact (`e73bc1e`, run 38695471107 réussi). P2 reste à 34 % tant que les critères de timer/préemption, synchronisation multi-cœur et infrastructure CPU restants ne sont pas implémentés.
- [ ] Étendre ensuite les tests de transitions (toutes les erreurs sans mutation involontaire, réveils répétés, blocage quand aucune tâche n'est runnable).

### Étapes P2 suivantes — après validation du lot courant

Une fois P1 validée, reprendre le scheduler coopératif réellement raccordé :

1. Auditer `kernel/src/scheduler.rs`, `kernel/src/context.rs` et les interfaces d’interruptions avant toute modification.
2. Définir le cycle de vie des tâches, la propriété de leurs piles et contextes, ainsi que les règles de terminaison.
3. Relier les identifiants du scheduler aux contextes et piles et intégrer le changement de contexte x86-64.
4. Démontrer l’alternance déterministe d’au moins deux tâches, puis leur terminaison sans reprise sur une pile libérée.
5. Exécuter les tests de régression et une preuve QEMU ; rattacher les résultats au SHA exact.

**Critères d’acceptation P2 pour ce lot :** deux tâches indépendantes alternent réellement, les états restent cohérents après chaque commutation, les piles restent valides, les tests existants passent et le chemin intégré est démontré sous QEMU.

**Hors périmètre de ce premier lot :** préemption par timer, synchronisation concurrente et SMP. Ces éléments restent dépendants de la validation du switch coopératif.

### Règle de priorité

L’ordre officiel reste P0 → P1 → P2 : ne pas déclarer une phase prioritaire terminée sans ses preuves d’acceptation. La préparation ou l’audit de P2 peut être effectué sans modifier le code pendant l’attente de la campagne physique, mais l’avancement de P2 ne remplace pas la clôture de P1.
