# Ordonnanceur ArchiaOS

## État actuel

Le modèle d'états est borné et sans allocation dynamique. Il gère les états `Empty`, `Runnable`, `Running`, `Blocked` et `Exited`, avec sélection round-robin. Le runtime x86-64 de validation utilise deux contextes et deux piles statiques distinctes. Son scénario réel vérifie désormais : A se bloque, B réveille A puis cède le processeur, A reprend et termine, puis B reprend et termine avant le retour à la pile de démarrage. La CI exige le marqueur correspondant. `dispatch_or_idle()` distingue explicitement le lancement d'une tâche (`Run(TaskId)`) de l'absence de tâche exécutable (`Idle`) lorsque toutes sont bloquées ou terminées ; le caller reste responsable de l'implémentation réelle de la boucle/contexte idle. Des tests couvrent le réveil après état idle et le cas où toutes les tâches ont terminé. Chaque contexte préserve les registres callee-saved, RSP et l'état x87/SSE via FXSAVE64/FXRSTOR64. Les registres YMM/ZMM ne sont pas sauvegardés : AVX/AVX-512 restent hors du chemin du scheduler tant que l'état étendu n'est pas correctement géré et testé.

**Ce runtime n'est pas encore un ordonnanceur général de production.** Il n'a pas encore de temporisateur/préemption, de SMP actif, de politique de placement multi-cœur, ni de découverte complète de la topologie et des asymétries du processeur. Le round-robin actuel ne connaît pas les P-cores/E-cores ou les domaines de cache.

## Exigence d'architecture : processeurs homogènes et hétérogènes

Le scheduler doit fonctionner sur tout processeur pris en charge, sans hypothèse codée en dur selon laquelle tous les cœurs auraient les mêmes performances, la même efficacité ou le même cache. La politique de placement doit être séparée de la détection matérielle et des mécanismes d'architecture CPU.

Cas obligatoires à couvrir :

- **Intel hybride** : identifier les types de cœurs quand le matériel les expose (par exemple P-core/E-core), leurs capacités et les contraintes de placement.
- **AMD et cache asymétrique** : représenter les topologies CCD/CCX et les différences de caches ; sur les processeurs équipés de 3D V-Cache, ne pas supposer que tous les CCD/cœurs possèdent le même cache L3 empilé.
- **CPU homogènes** : conserver un comportement correct et simple lorsque les cœurs sont équivalents ou que les détails de topologie ne sont pas disponibles.
- **Autres CPU/architectures** : le cœur générique du scheduler ne doit pas dépendre d'identifiants de modèles Intel/AMD. Les détecteurs propres à une architecture ou un fournisseur alimentent un contrat commun ; la portabilité vers une autre ISA exige son backend de contexte/interruptions propre.

## Contrat logiciel de topologie (première étape P2)

Le module `kernel/src/scheduler_topology.rs` fournit un instantané borné de 256 processeurs logiques. Chaque entrée utilise l'identifiant APIC comme identité et conserve séparément package, cœur, domaine de cache, nœud NUMA, classe et capacité. Les champs indisponibles restent `None`, la classe peut rester `Unknown`, les identifiants APIC en double et les capacités nulles sont rejetés sans mutation. Ce modèle est uniquement descriptif : il n'effectue pas encore la découverte CPUID/ACPI, ne démarre aucun processeur et ne choisit pas de CPU pour une tâche.

## Modèle matériel prévu

La couche de découverte devra construire une description bornée et vérifiable des processeurs logiques et des domaines de partage :

- identifiant du processeur logique et du cœur physique ;
- type/capacité de cœur et indice de performance/efficacité lorsque ces données sont exposées ;
- topologie package, die, CCD/CCX et relations de cache/partage ;
- tailles, niveaux et domaines de partage des caches, avec signalement des informations inconnues ;
- nœud NUMA et distances mémoire quand ACPI/plateforme les fournit ;
- capacités ISA utilisables et contraintes d'affinité pertinentes.

Sous x86, la découverte combinera les feuilles CPUID disponibles et validées avec les tables/indications firmware pertinentes (notamment ACPI). Les feuilles propres au fournisseur ne seront interrogées que si elles sont annoncées et comprises. Une caractéristique non exposée doit rester « inconnue » ; elle ne doit pas être inventée à partir du nom commercial du CPU. Les détails de 3D V-Cache ne seront utilisés comme indice fournisseur/plateforme uniquement s'ils sont détectés de façon fiable ; la politique doit rester correcte avec les seules informations de topologie et de cache réellement disponibles.

## Politique de placement visée

La politique doit arbitrer entre latence, débit, efficacité énergétique et localité mémoire/cache. Elle devra au minimum :

1. respecter les affinités et les CPU autorisés d'une tâche ;
2. favoriser un cœur adapté à la charge et à sa priorité, sans supposer qu'un P-core est toujours préférable ;
3. considérer la localité des caches et éviter les migrations inutiles ;
4. permettre des préférences de performance/efficacité sans les confondre avec des garanties ;
5. équilibrer la charge et éviter famine et oscillations de migration ;
6. fournir un repli déterministe et sûr si la topologie est partielle ou inconnue ;
7. fonctionner d'abord en mono-cœur, puis être étendue au SMP seulement après mise en place des primitives de synchronisation et des tests multi-CPU.

Les décisions de placement devront être testées avec des topologies synthétiques, indépendamment de la machine de développement : homogène, P/E, plusieurs CCD avec cache asymétrique, topologie partielle/inconnue et contraintes d'affinité. Les capacités ISA font aussi partie des contraintes : le noyau x86-64 exige SSE4.1, tandis que AVX/AVX2/AVX-512 restent optionnels et ne sont utilisables que si le CPU et l'état système les autorisent. Une tâche utilisant une extension optionnelle ne doit pas migrer vers un cœur qui ne peut pas l'exécuter, sauf si l'état SIMD est correctement géré et que la politique garantit la compatibilité de tous les CPU autorisés. Les tests sur du matériel réel et les mesures de performance serviront ensuite à calibrer la politique ; ils ne remplaceront pas les tests déterministes.

## Étapes prévues

1. Stabiliser le cycle de vie et le contexte des tâches.
2. Définir les types communs de topologie/capacité et leurs invariants.
3. Ajouter les détecteurs x86 (CPUID et firmware/ACPI), avec tests de parsing sur données synthétiques.
4. Ajouter une politique de placement testable sur topologies synthétiques, puis les affinités.
5. Intégrer temporisateur/préemption, synchronisation et SMP par étapes, sans activer le SMP avant validation.

La détection CPU déjà présente dans `kernel/src/cpu.rs` relève surtout l'identité et quelques fonctionnalités ISA CPUID ; **elle ne constitue pas encore la découverte de topologie nécessaire au scheduler**.

## Découverte initiale de topologie CPU (P2)

La fonction `cpu::detect_current_topology_entry()` interroge le CPU courant via CPUID `0x1F`, avec repli sur `0x0B`. Le décodeur pur construit les identifiants package/core à partir des niveaux SMT/core et rejette les niveaux incohérents. Cache, NUMA, classe performance/efficacité et capacité restent inconnus plutôt que d'être devinés. Cette API n'énumère pas les autres processeurs et n'active pas SMP ; l'étape suivante est le rapprochement prudent avec ACPI MADT et l'énumération AP.
## Import de la liste CPU ACPI/MADT

`scheduler_topology::from_madt_processors` convertit les enregistrements processeur MADT en instantané borné pour l'ordonnanceur. Il conserve les processeurs activés ou marqués « online-capable », refuse les doublons et les trous dans la tranche d'entrée, et laisse package/cœur/cache/NUMA/classe inconnus tant que l'identifiant CPUID correspondant n'a pas été rapproché. Cela prépare l'intégration sans démarrer de CPU secondaire ni activer SMP.
## Rapprochement CPUID ↔ MADT

`CpuTopology::merge_cpuid_entry` enrichit un processeur déjà présent dans la liste ACPI en utilisant son identifiant APIC comme clé. Une découverte CPUID dont l'identifiant n'existe pas dans MADT est rejetée : elle ne peut donc pas ajouter implicitement un CPU non annoncé par le firmware. Les métadonnées inconnues restent inconnues ; cette opération ne démarre pas de processeur secondaire et n'active pas le SMP.

## Intégration initiale de la topologie P2

Au démarrage, après validation de la MADT, le noyau construit maintenant un instantané
`CpuTopology` à partir des processeurs activés ou capables d'être activés dans
la liste firmware. Il tente ensuite de fusionner les champs CPUID du processeur courant,
uniquement si son APIC ID correspond à un processeur de la MADT. Les champs absents
ne remplacent pas les métadonnées connues ; un CPU absent de la MADT n'est jamais ajouté.

L'instantané est pour le moment local au bloc d'initialisation et sert à la validation
et au diagnostic série. Il n'est pas publié comme état global de l'ordonnanceur :
aucun AP n'est démarré, aucune affinité n'est appliquée et aucune préemption SMP n'est
activée par cette étape.


## Politique de placement (socle P2)

Le module de topologie expose maintenant un sélecteur déterministe de candidat :
il filtre d'abord sur la liste des APIC IDs confirmés en ligne par le mécanisme
de démarrage des processeurs, puis applique l'affinité stricte. Une entrée MADT
« online-capable » n'est pas considérée active par défaut. Le domaine cache
préféré, la classe CPU demandée et la capacité connue départagent les candidats.
Les données inconnues restent admissibles comme repli et ne sont jamais devinées.
À égalité, l'ordre MADT est conservé. Une liste en ligne vide donne aucun candidat.

Cette fonction est pour l'instant une politique pure testable, pas encore raccordée
au dispatch des tâches. Elle ne constitue ni un équilibrage de charge ni du SMP.
