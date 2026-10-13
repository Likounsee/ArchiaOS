# Feuille de route ArchiaOS

> **Règle de suivi :** les pourcentages sont des estimations de réalisation, pas des mesures automatiques. Une phase ne passe à 100 % qu'après implémentation, tests reproductibles et critères d'acceptation satisfaits. Une fonctionnalité décrite dans la documentation mais absente du code ne compte pas comme implémentée.

## Tableau de bord

| Priorité | Domaine | Avancement estimé | État / prochaine preuve attendue |
|---|---|---:|---|
| **P0** | Démarrage UEFI maison x86-64 par défaut | **100 % (jalon physique atteint)** | Démarrage depuis la clé USB et premier affichage framebuffer confirmés par l’utilisateur sur le PC physique. Le motif bleu prouve l’écriture dans le framebuffer après le handoff ; il ne constitue pas encore un bureau ni une pile graphique. Un redémarrage à froid n’est pas consigné. |
| **P1** | Fiabilité du chargeur, BootInfo et mémoire | **99 %** | Le HEAD courant `e18b1901e262f21838ab0484459d207ccf004442` passe la CI principale ([CI](https://github.com/Likounsee/ArchiaOS/actions/runs/38375116739)) et le démarrage négatif BootInfo ([workflow](https://github.com/Likounsee/ArchiaOS/actions/runs/38375116552)). Le package USB et le démarrage UEFI local ont également réussi sur le commit de code précédent `dfd67f2937b95afc8c8d798c0b77483c2f7a43f9`; ils doivent être relancés après toute modification pertinente du chargeur. L’audit ACPI couvre RSDT/XSDT, SDT enfants et pointeurs FADT DSDT/FACS, avec limites de 256 entrées et 64 Mio ; la capacité maximale de suivi a été corrigée. Les objets firmware effectivement consommés après `ExitBootServices` sont validés et protégés. Les autres entrées de configuration UEFI ne sont ni déréférencées ni transmises au noyau ; le PMM n’accepte comme RAM allocatable que le type UEFI Conventional Memory (7), laissant les autres types réservés. Le package USB est généré avec succès sur le commit de code `45fffbf9` (identique au code du HEAD, la modification ultérieure étant documentaire). **Seule la validation sur machine physique reste à faire** ; l’utilisateur possède l’image USB mais ne dispose pas actuellement d’une machine pour l’essayer. P1 reste donc à 99 % et doit être repris pour les démarrages à froid, le transfert BootInfo/PMM, le framebuffer et le retour firmware. QEMU ne prouve pas le comportement de tous les firmwares et PC. |
| **P2** | Exceptions, ordonnanceur et synchronisation | **21 %** | Modèle de tâches, IDT x86-64 et primitive de changement de contexte coopératif ajoutés ; CI principale et régression hôte du contexte vertes sur HEAD `e18b1901`. Le scheduler n'est pas encore relié aux contextes et ne commute pas encore entre tâches. |
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

## Audit de référence — état du dépôt au HEAD `e18b1901e262f21838ab0484459d207ccf004442`

Cet audit porte sur les sources présentes et leurs raccordements visibles, pas sur les seuls noms de fichiers ou les déclarations de la documentation. Les résultats CI sont ceux consultés au moment de cette révision ; une CI verte ne prouve que les scénarios qu'elle exécute.

### Méthode de statut et de progression

- **Non implémenté (0 %)** : aucun chemin de code opérationnel correspondant.
- **Partiellement implémenté** : un sous-ensemble existe, mais une partie structurante manque ou reste isolée.
- **Implémenté, non validé** : le code est raccordé mais les critères de test/plateforme ne sont pas encore satisfaits.
- **Validé** : critères d'acceptation passés par une preuve reproductible identifiée ; les limites de cette preuve restent écrites.
- **Bloqué** : prérequis externe ou technique empêche la suite ; nommer le blocage.
- **Remplacé/abandonné** : décision documentée et raison conservée.

Le pourcentage d'une tâche doit être fondé sur ses sous-critères : 0 % si aucun critère opérationnel n'est satisfait, 25 % pour le squelette et le contrat seulement, 50 % si le chemin nominal fonctionne mais que les erreurs/tests manquent, 75 % si l'intégration et les tests automatisés passent mais qu'une validation importante reste, 90 % si seuls des scénarios ciblés non automatisés restent, 100 % uniquement si tous les critères annoncés sont satisfaits. Ces paliers sont des repères, pas des bonus automatiques. Le pourcentage d'une phase est la moyenne pondérée de ses livrables techniques (poids documentés dans cette phase) ; un simple fichier, une API déclarée ou un test écrit non exécuté ne rapporte pas de validation.

**Progression globale estimée : 25 %** selon la pondération de pilotage suivante : P0 5 %, P1 15 %, P2 20 %, P3 15 %, P4 12 %, P5 8 %, P6 8 %, P7 7 %, P8 4 %, P9 3 %, P10 3 % (total 100 %). Avec les valeurs affichées, la somme pondérée est d'environ 24,7 %, arrondie à 25 %. Les jalons B0–B9 et A0–A4 sont des vues transversales du boot/ARM64 et **ne sont pas additionnés une seconde fois**. Cette estimation mesure l'avancement vers l'ensemble des objectifs de la roadmap, pas la proportion de code écrite. La pondération est une convention de pilotage ; elle doit être changée explicitement si le périmètre final change.

### Audit des chemins d'exécution de la version actuelle

| Domaine | Ce que le code fait réellement | Limite / statut à refléter |
|---|---|---|
| Boot x86-64 | Entrée ELF, pile noyau propre, console série, passage BootInfo, écriture framebuffer et chemin UEFI maison ; workflows OVMF/CI et package USB. | P0 : premier démarrage physique signalé ; P1 reste à 99 % car les essais physiques de répétition/redémarrage ne sont pas consignés. |
| Mémoire physique | `memory.rs` utilise deux bitmaps (frames gérées et libres), limite la fenêtre gérée à 4 Gio, filtre la carte UEFI et réserve les plages critiques ; auto-tests allocation/libération, doubles libérations et épuisement simulé. | Pas de VMM général, de gestion des espaces d'adressage par processus ni de mapping dynamique complet ; PMM actuel n'est pas un substitut à ces composants. |
| Pagination / isolation | Les mappings de boot et permissions ELF sont construits par le chargeur ; le noyau démarre avec son environnement de mapping initial. | Pas de gestionnaire de tables de pages général côté noyau, pas de séparation fiable noyau/utilisateur ni de changement de CR3 par processus démontré. |
| Exceptions / tâches | `interrupts.rs` fournit IDT x86-64 0–31 et handlers fatals ; `scheduler.rs` modélise les états ; `context.rs` sait faire un transfert x86-64 sur pile dédiée testé en hôte. | Le scheduler ne possède pas les contextes/piles et n'appelle pas le switch ; pas de timer, préemption, livraison d'exception aux tâches ou SMP activé. |
| Pilotes / bus | `cpu.rs` lit CPUID et `pci.rs` parcourt les fonctions PCI du bus 0 via mécanisme de configuration legacy, en lecture seule pour les registres des périphériques. | Pas de PCIe ECAM/MCFG, de découverte générale des bus, de pilote bloc/USB/stockage ni de capteurs en direct. L'écriture du sélecteur CF8 est un accès d'adressage, pas une preuve d'inventaire complet. |
| Système de fichiers | Aucun VFS ou pilote bloc connecté au chemin de démarrage actuel ; OpenFS est un projet distinct. | P4 reste à 0 % pour l'intégration actuelle ; ne pas confondre les sources OpenFS historiques dans ArchiaOS-old avec un FS utilisable par le noyau actuel. |
| Syscalls / espace utilisateur | Pas d'ABI syscall active, d'exécution de binaire utilisateur isolé ni de gestion de processus dans le noyau Rust actuel. | P3 reste à 0 % ; les composants de l'ancien dépôt sont une référence de conception, pas une fonctionnalité portée. |
| ARM64 | Un chemin de démarrage/prototype séparé et un PMM initial existent, avec tests de compilation/émulation selon workflows. | Pas de parité avec x86-64 : MMU/espaces d'adressage, interruptions, SMP et pilotes ne doivent pas être supposés équivalents. |
| Tests et CI | CI principale verte au HEAD ; test hôte du scheduler/context switch et workflows de boot négatif présents. | Vérifier les jobs individuellement après chaque modification ; la CI hôte ne remplace pas le boot QEMU, et QEMU ne remplace pas le matériel physique. |

### Comparaison du code avec `Likounsee/ArchiaOS-old`

L'ancien dépôt est une base C/C++ x86-64 plus large, mais plus complexe et non équivalente à une implémentation de production validée. Ses composants ne sont pas raccordés automatiquement à la version Rust actuelle.

- **Scheduler et SMP :** l'ancien `cpu/scheduler.cpp` contient des tables par CPU, des piles statiques par tâche, un bootstrap de tâches et une intégration au LAPIC ; `cpu/smp.cpp` contient le démarrage d'AP via trampoline, initialisation GDT/TSS/IDT et attente de mise en ligne. C'est une référence utile pour les structures par CPU et le cycle de vie, mais l'ancienne logique dépend de LAPIC, TSS, GDT, IDT et de détails d'assemblage fortement couplés. Réutiliser les invariants et tests, pas copier en bloc.
- **Mémoire :** l'ancien code sépare PMM, paging, VMM, heap et `AddressSpace`, avec tests PMM/paging dédiés et refus explicite de mappings utilisateur dans certaines API du VMM noyau. Il offre des pistes pour un futur VMM et les espaces d'adressage, mais les bitmaps/limites, l'API C++ et les conventions de mapping doivent être audités et adaptés à BootInfo et au modèle Rust actuel. La gestion mémoire actuelle est plus simple et mieux bornée, mais nettement moins complète.
- **Processus / IPC / ring 3 :** l'ancien `process.cpp` inclut tables de PID, chargement ELF, piles utilisateur et entrée ring 3 ; `ipc.cpp` offre une couche IPC. Cela indique un écart fonctionnel majeur avec la version actuelle, mais chaque chemin doit être ré-audité pour isolation, validations ELF, permissions de pages, durée de vie et gestion des erreurs avant portage.
- **VFS / OpenFS :** l'ancien code comprend un VFS, un adaptateur OpenFS, un test OpenFS et une pile OpenFS C (bitmap, allocation, extents, inode, répertoires, liens, transactions, journal et fsck), ainsi que des abstractions bloc/GPT. C'est la source historique la plus concrète pour préparer P4. Avant réutilisation : isoler les appels disque, tester les erreurs E/S et les corruptions, vérifier les limites d'intégrité/journal et les formats on-disk, et écrire un contrat bloc/VFS stable. La présence de ces sources ne prouve ni que tous les chemins sont exécutés ni que le FS est sûr sur un disque réel.
- **ACPI :** l'ancien `acpi.cpp` sait chercher/analyser des tables firmware, mais son commentaire indique un mapping identité initial très large et il lit des adresses physiques via pointeurs volatils. La version actuelle a un audit de plages firmware plus strict au boot. Ne pas importer l'ancien parseur sans reprendre les validations de taille, checksum, mapping et chevauchement.
- **Pilotes PCI :** l'ancien `drivers/pci.cpp` est une base de découverte plus développée que le scan bus 0 actuel, mais reste liée aux mécanismes de configuration et à l'architecture. Comparer sa couverture et ses tests avant d'extraire un backend ; ajouter ECAM/ACPI séparément.
- **Conclusion de portage :** ArchiaOS-old est surtout une **référence de conception et de tests**, pas une dépendance à réintégrer directement. Chaque portage doit être isolé, accompagné de tests hôte, intégré par petites interfaces, puis validé en QEMU avant tout essai matériel.

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
- [x] Compiler et empaqueter les fichiers de test USB x86-64 dans GitHub Actions ; workflow `ArchiaOS P0 physical USB package` réussi sur le commit `977fa0c5088f015f48f7f5a5261ffcc3c8cec7d8`, artefact `archiaos-p0-usb-x86_64` disponible.
- [x] **Démarrage physique et premier affichage confirmés** : l’utilisateur a démarré depuis la clé USB et observé un fond bleu foncé avec une bande bleu clair, preuve que le noyau écrit dans le framebuffer après le handoff UEFI. Le redémarrage à froid et le retour au firmware restent non consignés ; ils ne bloquent plus le jalon P0 de premier démarrage.

**Critère de sortie :** build reproductible et tests de démarrage verts sur le HEAD courant, sans déclarer les essais QEMU comme une validation de tout matériel physique.

### P1 — Fiabilité du chargeur et de la mémoire — 99 %

**État : EN COURS — 99 %.** Le chargeur utilise exactement le snapshot du tableau de configuration UEFI dont la plage complète vient d’être validée dans la carte mémoire finale : les recherches ACPI 2.0, ACPI 1.0 et Device Tree ne relisent plus le compteur ni le pointeur depuis la table système entre validation et lecture. Les tests couvrent la recherche GUID, les entrées nulles, les tableaux absents ou vides, la lecture d’entrées non alignées et le rejet des comptes hors limites. Avant de parcourir ses entrées, le chargeur vérifie que le tableau entier est borné, réservé, accessible en mapping identité pour le noyau high-half x86-64 et disjoint du framebuffer ; l’audit final rejette aussi son chevauchement avec les buffers de transfert et les segments ELF. Le tableau lui-même est ajouté aux plages protégées de l’audit ACPI, empêchant une SDT ou une table FADT indirecte de l’aliaser. Le RSDP et le Device Tree restent différés jusqu’à validation de leurs plages. L’audit ACPI couvre RSDT/XSDT, les SDT enfants et les pointeurs FADT vers DSDT/FACS, avec une limite de 256 entrées racine et un budget de 64 Mio. La CI complète est verte sur le HEAD `e563bab342056ee32e7d41da062664b2c0fa12b9` : [CI principale](https://github.com/Likounsee/ArchiaOS/actions/runs/38369633471), [démarrage UEFI local](https://github.com/Likounsee/ArchiaOS/actions/runs/38369633426) et [test négatif BootInfo](https://github.com/Likounsee/ArchiaOS/actions/runs/38369633506). Les tests couvrent les plages du chargeur, les métadonnées GOP, les types mémoire UEFI et les réservations qui rejettent les plages mixtes, les trous, les plages vides et les débordements. La capacité maximale du tableau de plages protégées ACPI a été corrigée pour le pire cas théorique : trois plages initiales, la racine, puis un SDT, un DSDT et un FACS par entrée racine. L’audit de périmètre est clos : seuls les objets firmware réellement consommés après `ExitBootServices` sont suivis et validés ; les entrées UEFI inconnues/non utilisées ne sont ni déréférencées ni transmises au noyau, et le PMM ne considère allocatable que le type UEFI Conventional Memory (7). Le package USB est disponible depuis le workflow réussi sur le commit de code `45fffbf9` (code identique au HEAD actuel, la modification ultérieure étant documentaire). **Il ne reste que la validation sur machine physique**, qui ne peut pas être prouvée par QEMU.

**Objectif :** sécuriser le transfert UEFI → noyau et la gestion de la mémoire physique.

- [x] Valider les en-têtes ELF64 et les tables de programmes.
- [x] Contrôler les tailles, alignements, débordements, recouvrements et l'entrée exécutable.
- [x] Refuser les segments ELF RWX.
- [x] Construire les mappings high-half x86-64 et appliquer les permissions ELF.
- [x] Activer NXE et CR0.WP dans le chemin testé.
- [x] Fournir un BootInfo versionné et récupérer la carte mémoire UEFI finale.
- [x] Réessayer ExitBootServices avec une clé de carte mémoire fraîche si nécessaire.
- [x] Ajouter un contrôle de la carte mémoire UEFI finale pour détecter les buffers BootInfo/carte mémoire, segments ELF, image du chargeur et arène de tables de pages incorrectement classés en RAM conventionnelle.
- [x] Découvrir le framebuffer actif via UEFI GOP, transmettre base/taille dans BootInfo et refuser le transfert si sa plage chevauche de la RAM conventionnelle ou traverse un trou de carte mémoire.
- [x] Auditer les plages du RSDP ACPI et du Device Tree : contrôler le préfixe avant lecture des métadonnées, borner les tailles annoncées, vérifier la réservation complète et, pour le noyau high-half x86-64, l'identité de mapping RAM sous 4 Gio (commits `3d3dae0`, `2c3058e`; CI BootInfo négatif, UEFI local, CI et paquet USB verts sur `2c3058e`).
- [x] Auditer explicitement l'accessibilité dans le mapping identité actif des plages critiques BootInfo, carte mémoire, segments ELF et arène de tables de pages ; le framebuffer GOP reçoit un mapping d'écriture uncached dédié. Les autres plages réservées spécifiques aux plateformes restent à examiner.
- [x] Vérifier en CI les rejets de libérations invalides/doubles, la cohérence bitmap (toute page libre est gérée, mémoire basse réservée), la réutilisation après libération et la restauration du compte de pages.
- [x] Extraire la validation scalaire du contrat BootInfo en fonction testable sans déréférencer la carte mémoire, avec tests négatifs magic/version, taille de structure, pointeur absent/désaligné, taille de carte et métadonnées.
- [x] Valider aussi l'adresse d'entrée noyau non nulle et la cohérence/non-débordement de la plage framebuffer, avec tests unitaires confirmés par la CI du HEAD `1ab767a`.
- [x] Ajouter une simulation d'épuisement mémoire isolée et déterministe sur des bitmaps locaux, incluant filtrage des frames réservées et réutilisation, sans consommer la mémoire firmware ni bloquer QEMU (tests et démarrages CI verts sur `1ab767a`).
- [x] Vérifier avant ExitBootServices que les buffers BootInfo/carte mémoire, les segments noyau, l'image du chargeur et l'arène de tables de pages ne chevauchent aucune plage UEFI conventionnelle.
- [x] Valider la cohérence et l'absence de débordement de la plage framebuffer dans BootInfo. Le framebuffer GOP est une plage MMIO : elle peut légitimement être absente des descripteurs UEFI ; elle est mappée séparément en uncached et exclue de la RAM allocatable. Les tests de couverture générique rejettent les trous sans imposer à tort que le MMIO soit décrit comme mémoire système.
- [x] Rejeter les tables ACPI RSDP et Device Tree qui chevauchent les pages physiques allouées aux segments ELF `PT_LOAD` ; l’audit couvre les pages arrondies réellement réservées et des cas de chevauchement partiel ainsi que des plages disjointes (HEAD `c864ae2`).
- [x] Auditer les pointeurs FADT indirects vers DSDT/FACS : priorité aux pointeurs étendus non nuls, repli sur les champs legacy, signature DSDT obligatoire, FACS contrôlé séparément (signature, longueur, réservation, mapping identité et absence de chevauchement), budget global conservé et capacité de suivi bornée ; tests unitaires de sélection des pointeurs ajoutés (HEAD `abe8d7f`, CI en attente).
- [x] Reporter le parsing du RSDP et du Device Tree jusqu'à ce que la carte mémoire finale valide leurs plages complètes : la découverte UEFI ne fait plus que capturer les adresses, et l'audit final vérifie réservation, mapping identité, tailles, checksums/contenu et chevauchements avant tout déréférencement (HEAD `359c8b8`).
- [x] Reporter la lecture du tableau de configuration UEFI jusqu’à la carte mémoire finale ; vérifier sa plage entière comme réservée et, pour x86-64 high-half, identité-mappée avant d’en lire les entrées, puis rejeter les chevauchements avec framebuffer et stockage de transfert. Inclure ce tableau dans les plages protégées de l’audit ACPI pour interdire les alias avec les SDT/DSDT/FACS (HEAD `e57125e`, CI en cours).
- [x] Étendre les tests de couverture mémoire aux sous-plages non alignées, aux frontières entre descripteurs de types différents et aux trous entre descripteurs ; CI verte sur `1ab767a`.
- [x] Durcir le calcul des plages PMM réservées : écrêtage à la fenêtre gérée sous 4 Gio avant l'arrondi, sans débordement inutile pour une plage MMIO située hors fenêtre ; auto-vérifications au démarrage pour les limites, les plages non alignées et les longueurs nulles (commit `a8654bf`).
- [x] Centraliser le calcul du chevauchement entre intervalles de frames et ajouter des auto-vérifications déterministes pour les plages libres, réservées, adjacentes, disjointes, incluses et vides ; validation de démarrage OVMF et tests négatifs BootInfo verts sur le code correspondant (commit `b22d509`).
- [x] Valider le framebuffer GOP linéaire avant de le transmettre : rejeter les modes BltOnly, les dimensions/strides invalides, les plages débordantes et les buffers trop petits ; tests unitaires pour RGB/BGR/bitmask et cas invalides, CI x86-64/ARM64 et démarrages OVMF/AAVMF verts (commits `5526ae4`, `d218075`).
- [x] Borner le tableau des tables de configuration UEFI avant toute lecture : vérifier le nombre d'entrées, le calcul de taille et l'absence de débordement d'adresse, puis lire les entrées de façon non alignée pour éviter un comportement indéfini sur une adresse mal alignée ; tests des bornes et débordements (commit `2f360f7` ; CI ultérieure verte sur le HEAD `2c3058e`).
- [x] Confirmer les contrôles de transfert, mappings et BootInfo par la CI du HEAD `00bcb8c` : compilation x86-64/ARM64, démarrage UEFI OVMF, BootInfo négatif et paquet USB tous verts.
- [x] Réinitialiser les bitmaps PMM après la prévalidation et avant chaque tentative de population, afin qu'une tentative échouée ne laisse pas d'état périmé pour une reprise.
- [x] Refuser l'allocation lorsque le bitmap ne contient aucune frame à la fois libre et gérée ; la simulation locale teste l'épuisement déterministe.
- [x] Empêcher la libération des frames qui ne sont pas gérées par le PMM ; tests de frames réservées et de double libération intégrés.
- [x] Confirmer les contrôles mémoire/PMM existants sur le HEAD `00bcb8c` en CI.
- [x] Clore l’audit des objets firmware : toutes les tables consommées après `ExitBootServices` sont validées et protégées ; les entrées inconnues non utilisées ne sont pas déréférencées/transmises, et les descripteurs non-Conventional Memory restent exclus du PMM.
- [ ] Valider sur machine physique : plusieurs démarrages à froid depuis l’USB, transfert BootInfo et initialisation PMM sans blocage, affichage framebuffer, puis redémarrage et retour firmware. Consigner les résultats réels ; ne pas déduire ces preuves de QEMU.

**Critère de sortie :** invariants mémoire documentés et testés, sans chevauchement dangereux ni accès hors plage.

### P2 — Exceptions, ordonnanceur et synchronisation — 21 %

**État : PARTIELLEMENT IMPLÉMENTÉ — 21 % (HEAD `e18b1901e262f21838ab0484459d207ccf004442`).** La CI principale et le test de démarrage négatif BootInfo sont verts sur ce HEAD. Le test de contexte CPU est désormais exécuté par le job hôte du scheduler. Cela valide un transfert x86-64 vers une pile de tâche synthétique et le retour vers le contexte appelant ; cela **ne valide pas encore un ordonnanceur qui commute entre plusieurs tâches réelles**. Le module `scheduler.rs` reste un modèle d'états isolé : les tâches ne possèdent ni pile ni contexte CPU et ses transitions ne déclenchent pas `context::switch`. L'IDT x86-64 couvre les vecteurs 0–31 avec stubs et diagnostic fatal, mais l'installation et les handlers ne disposent pas encore d'un test d'exception contrôlé démontré dans la CI. Les interruptions restent désactivées au démarrage ; pas de timer, de préemption ni de SMP activé. La synchronisation multi-cœur n'existe pas encore dans la version actuelle.

**Implémenté et vérifié**
- [x] Modèle d'ordonnanceur borné sans allocation dynamique : états `Empty`, `Runnable`, `Running`, `Blocked`, `Exited`, sélection round-robin, blocage/réveil, sortie et invariants.
- [x] Tests hôte des transitions et cas d'erreur du scheduler, exécutés par un job CI dédié.
- [x] Primitive x86-64 de sauvegarde/restauration des registres callee-saved et de RSP, avec pile initiale alignée et entrée de tâche.
- [x] Test hôte de transfert réel vers la pile dédiée et de retour au contexte initial ; CI principale verte sur le HEAD cité ci-dessus.
- [x] Première IDT x86-64 pour les exceptions 0–31, normalisation des codes d'erreur et diagnostic CR2 pour page fault ; le comportement fatal reste volontaire.

**À faire — ordre de dépendance**
- [ ] Tester explicitement l'IDT chargée, la validité des sélecteurs et le chemin de diagnostic par déclenchement contrôlé d'exceptions bénignes puis de fautes isolées sous QEMU.
- [ ] Définir l'API de cycle de vie des tâches (entrée, argument, sortie, destruction) et la propriété des piles/contexte ; garder les piles dans une réserve statique ou une allocation dédiée, pas dans un gros objet local sur la pile de démarrage.
- [ ] Relier les identifiants du scheduler aux contextes CPU et piles, avec gestion atomique de la tâche sortante/entrante et rollback si aucune tâche exécutable n'existe.
- [ ] Ajouter un test à deux puis plusieurs tâches qui cèdent réellement le CPU, reprennent après suspension et terminent sans réutilisation prématurée de pile.
- [ ] Définir la convention d'appel et l'état sauvegardé par architecture ; documenter explicitement que la primitive actuelle est x86-64/System V et ne constitue pas une abstraction ARM64.
- [ ] Ajouter une file d'attente/primitives de synchronisation avec tests d'interleavings, de blocage/réveil et de perte de réveil.
- [ ] Configurer un timer fiable et son acquittement, d'abord en mode coopératif puis avec préemption après validation de l'état complet sauvegardé.
- [ ] Ajouter GDT/TSS/IST et piles dédiées aux exceptions critiques ; tester double fault et stack fault sans dépendre de la pile fautive.
- [ ] Définir la stratégie SMP, les données par CPU, les verrous/atomiques et les règles d'ordre mémoire avant de réveiller un second cœur.
- [ ] Ajouter tests de stress et régression sous QEMU, puis validation matérielle pour les chemins d'interruptions/timer.

**Critères de sortie P2 :** plusieurs tâches indépendantes commutent réellement et de façon reproductible ; leurs piles/contextes restent valides ; exceptions attendues et fautes fatales produisent un diagnostic stable ; timer et synchronisation sont testés ; toute activation SMP est couverte par des tests multi-CPU. Une primitive de switch testée seule ne suffit pas.

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