# ArchiaOS

**ArchiaOS est un projet de système d’exploitation en développement.** Son objectif est de construire progressivement un OS moderne, modulaire et vérifiable, en commençant par une plateforme matérielle documentée, puis en ajoutant les services noyau et l’environnement utilisateur au fur et à mesure que leurs dépendances sont prêtes.

> **État vérifié du dépôt :** à la date de cette mise à jour, la branche **ArchiaOS** ne contient qu’un commit initial et ce README. Aucun code de noyau, système de build ou test exécutable n’a pu être confirmé dans l’arborescence distante. Les fonctionnalités décrites ci-dessous sont donc des objectifs, pas des capacités annoncées comme déjà disponibles.

## Vision

ArchiaOS vise à évoluer vers un système capable de démarrer de façon reproductible, de gérer la mémoire et les interruptions, d’exécuter des tâches isolées, de fournir des pilotes et un stockage fiables, puis de proposer un environnement utilisateur et graphique.

Le développement doit privilégier des composants fonctionnels, des frontières explicites et des tests réels plutôt que des modules vides ou des démonstrations qui donnent une impression trompeuse d’avancement.

## Langages envisagés

Le choix définitif doit être confirmé par l’architecture et la chaîne de compilation réellement mises en place.

- **Rust** : langage privilégié pour les nouveaux composants noyau et services quand les contraintes bare-metal le permettent, avec no_std, allocateur et runtime minimal configurés explicitement.
- **C** : intégration bas niveau, ABI et composants matériels ou existants lorsque cela se justifie.
- **Assembleur** : uniquement aux frontières matérielles indispensables, telles que l’entrée du noyau, certaines interruptions et changements de contexte.
- **C++** : seulement pour un composant ciblé dont les avantages justifient le runtime et le coût d’intégration.
- **Python** : scripts de build, génération et analyse/test.
- **JavaScript/TypeScript, HTML et CSS** : éventuellement pour une couche utilisateur future, pas pour le noyau.

Aucun langage ni composant n’est considéré comme déjà intégré tant que les fichiers de source et les commandes de compilation ne le démontrent pas.

## Architecture cible

La trajectoire envisagée comprend le démarrage et l’initialisation, l’architecture CPU, la mémoire physique et virtuelle, les exceptions et interruptions, l’ordonnanceur, les processus et appels système, les pilotes, le stockage et le système de fichiers, puis les programmes utilisateur et l’environnement graphique. L’ordre réel sera ajusté aux dépendances et à l’audit du code existant.

## Plateformes

- **Architecture et firmware pris en charge : non encore vérifiés dans ce dépôt.**
- **Démarrage en machine virtuelle ou sur matériel réel : non vérifié.**
- **Image de démarrage reproductible : non vérifiée.**

x86-64 peut être retenu comme première cible si l’audit et la chaîne de démarrage le confirment.

## Fonctionnalités

### Implémentées et vérifiées

Aucune fonctionnalité noyau n’est actuellement déclarée ici comme implémentée et vérifiée : aucun code source ni test exécutable n’a été confirmé dans l’état distant inspecté.

### Expérimentales

Aucun composant expérimental n’a été confirmé dans l’arborescence inspectée.

### Non implémentées ou non vérifiées

Démarrage, noyau, mémoire, interruptions, ordonnanceur, processus isolés, appels système, pilotes, stockage, système de fichiers, réseau, interface graphique et environnement utilisateur restent à auditer et à implémenter avant toute déclaration de disponibilité.

## Compilation et lancement

Aucune commande de compilation ni de lancement ne peut être fournie de manière fiable pour l’instant : aucun Makefile, manifeste Cargo, script de build, linker script ou image de démarrage n’a été confirmé. Ces instructions seront ajoutées une fois la chaîne de compilation réellement présente et testée.

## Roadmap

- **P0 — Fondations :** audit du dépôt, chaîne de build reproductible, diagnostics, CI, tests et documentation alignés sur le code.
- **P1 — Noyau et matériel :** démarrage, exceptions, interruptions, mémoire et diagnostics fiables.
- **P2 — Exécution :** contextes, threads noyau, synchronisation et ordonnanceur.
- **P3 — Espace utilisateur :** protection, appels système, chargeur et premiers programmes isolés.
- **P4 — Stockage :** interface bloc, pilote virtuel, système de fichiers et tests de corruption.
- **P5 — Interface utilisateur :** entrées, rendu graphique, composition et applications.
- **P6 — Réseau et performances :** réseau, compatibilité et benchmarks reproductibles.
- **P7 — Stabilisation :** audit de sécurité, tests prolongés, documentation et procédure de distribution.

Les phases ne seront considérées comme terminées qu’après validation de leurs critères techniques et de tests.

## Développement

La documentation technique et les futures instructions de contribution seront maintenues dans [ArchiaOS/README.md](ArchiaOS/README.md) lorsque ce fichier et l’arborescence du projet auront été créés. Les décisions de conception devront être documentées dans ArchiaOS/docs/.

Les changements de code doivent être développés exclusivement sur la branche **ArchiaOS**. La branche **main** ne doit pas être modifiée.
