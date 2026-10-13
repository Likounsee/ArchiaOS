# Ordonnanceur ArchiaOS

## État actuel

Le modèle d'états est borné et sans allocation dynamique. Il gère les états `Empty`, `Runnable`, `Running`, `Blocked` et `Exited`, avec sélection round-robin. Le runtime x86-64 de validation utilise deux contextes et deux piles statiques distinctes. Son scénario réel vérifie désormais : A se bloque, B réveille A puis cède le processeur, A reprend et termine, puis B reprend et termine avant le retour à la pile de démarrage. La CI exige le marqueur correspondant. Chaque contexte préserve les registres callee-saved, RSP et l'état x87/SSE via FXSAVE64/FXRSTOR64. Les registres YMM/ZMM ne sont pas sauvegardés : AVX/AVX-512 restent hors du chemin du scheduler tant que l'état étendu n'est pas correctement géré et testé.

**Ce runtime n'est pas encore un ordonnanceur général de production.** Il n'a pas encore de temporisateur/préemption, de SMP actif, de politique de placement multi-cœur, ni de découverte complète de la topologie et des asymétries du processeur. Le round-robin actuel ne connaît pas les P-cores/E-cores ou les domaines de cache.

## Exigence d'architecture : processeurs homogènes et hétérogènes

Le scheduler doit fonctionner sur tout processeur pris en charge, sans hypothèse codée en dur selon laquelle tous les cœurs auraient les mêmes performances, la même efficacité ou le même cache. La politique de placement doit être séparée de la détection matérielle et des mécanismes d'architecture CPU.

Cas obligatoires à couvrir :

- **Intel hybride** : identifier les types de cœurs quand le matériel les expose (par exemple P-core/E-core), leurs capacités et les contraintes de placement.
- **AMD et cache asymétrique** : représenter les topologies CCD/CCX et les différences de caches ; sur les processeurs équipés de 3D V-Cache, ne pas supposer que tous les CCD/cœurs possèdent le même cache L3 empilé.
- **CPU homogènes** : conserver un comportement correct et simple lorsque les cœurs sont équivalents ou que les détails de topologie ne sont pas disponibles.
- **Autres CPU/architectures** : le cœur générique du scheduler ne doit pas dépendre d'identifiants de modèles Intel/AMD. Les détecteurs propres à une architecture ou un fournisseur alimentent un contrat commun ; la portabilité vers une autre ISA exige son backend de contexte/interruptions propre.

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
