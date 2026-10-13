# Politique de compatibilité CPU et extensions SIMD

## Contrat x86-64

**SSE4.1 est le minimum matériel obligatoire pour démarrer le noyau ArchiaOS x86-64.** Un processeur qui ne déclare pas SSE4.1 via CPUID est refusé explicitement au début de l'initialisation du noyau. Le contrat de compilation de base doit rester conservateur : le simple fait que SSE4.1 soit requis ne permet pas au compilateur d'émettre AVX/AVX2 partout.

Ce minimum est une décision de compatibilité x86-64. Il ne définit pas le minimum ARM64 : cette architecture doit avoir son propre contrat de fonctionnalités et ses propres vérifications.

## Niveaux d'optimisation

- **Baseline** : code qui peut s'exécuter sur tous les CPU x86-64 acceptés par ArchiaOS, au minimum SSE4.1.
- **SSE4.2, AES-NI, BMI1/BMI2, FMA** : fonctionnalités indépendantes, activées uniquement dans les chemins qui vérifient leur disponibilité et leurs prérequis.
- **AVX / AVX2** : la présence matérielle ne suffit pas. Il faut également que XSAVE et OSXSAVE soient annoncés et que XCR0 indique que l'état XMM/YMM est activé (XCR0 & 0x6 == 0x6).
- **AVX-512** : vérifier AVX-512F côté CPU et l'état OS requis dans XCR0 (XCR0 & 0xE6 == 0xE6) avant tout usage.
- **Extensions futures** : aucune extension ne devient une dépendance globale par simple présence dans une machine de développement.

Les informations « support matériel » et « utilisable par le noyau maintenant » doivent rester séparées. Le support AVX détecté sur le CPU ne signifie pas que le noyau a déjà configuré l'état étendu nécessaire. Le noyau devra sauvegarder/restaurer cet état correctement avant d'autoriser son utilisation dans les tâches, interruptions ou contextes concurrents.

## Dispatch et portabilité

Les optimisations doivent être isolées derrière des fonctions ou modules spécialisés, avec un chemin de repli baseline. La sélection se fait à partir des capacités utilisables réellement détectées, pas du nom commercial, du fabricant seul ou d'une hypothèse sur une génération de processeur. Sur un CPU hybride, les capacités ISA doivent être considérées avec prudence : un chemin d'instructions partagé entre cœurs ne peut être exécuté partout que si chaque CPU auquel une tâche peut migrer le prend en charge, ou si l'affinité garantit un sous-ensemble compatible.

Les optimisations facultatives ne doivent jamais faire planter le démarrage sur un CPU baseline valide. Des tests doivent couvrir les combinaisons synthétiques de fonctionnalités et les états OSXSAVE/XCR0, en plus de la validation sous QEMU et sur matériel.

## Implémentation actuelle

- Le noyau x86-64 vérifie SSE4.1 et s'arrête avec un diagnostic si cette exigence n'est pas satisfaite.
- Le détecteur CPUID rapporte SSE4.1/SSE4.2, AES-NI, FMA, BMI1/BMI2, AVX, AVX2 et AVX-512F.
- Les champs AVX/AVX2/FMA/AVX-512 distinguent le support matériel de l'état OS actuellement utilisable.
- **À valider par CI** : compilation, démarrage QEMU, refus contrôlé d'un profil CPU sans SSE4.1 et tests unitaires du décodage/dispatch.
- **Pas encore implémenté** : chemins de calcul réellement optimisés par AVX/AVX2/AVX-512 et sauvegarde/restauration générale de l'état SIMD pour les changements de contexte.

La détection actuelle est une première couche de compatibilité, pas une preuve que les optimisations SIMD sont déjà utilisées par des fonctions du système.
