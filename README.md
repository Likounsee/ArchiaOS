# ArchiaOS

**Un système d’exploitation expérimental écrit en Rust, construit autour de son propre chargeur UEFI.**

ArchiaOS est un projet d’OS bare-metal en développement. Le dépôt contient un noyau sans environnement standard (`no_std`), un chargeur UEFI maintenu dans le projet et les tests qui accompagnent ces fondations. L’objectif actuel est de rendre le transfert firmware → chargeur → noyau prévisible et sûr avant d’ajouter les services d’un système d’exploitation complet.

> **État actuel :** le démarrage UEFI x86-64 a été essayé sur un PC physique et le noyau a produit un premier affichage framebuffer (fond bleu foncé avec une bande bleu clair). C’est une preuve de démarrage et d’écriture vidéo bas niveau, **pas encore un bureau ni une pile graphique complète**. La CI du HEAD courant doit encore être stabilisée après les derniers changements P1.

## Ce qui existe aujourd’hui

- **Chargeur UEFI maison** en Rust : ouvre le noyau sur le volume EFI, valide l’ELF64, contrôle les segments et transfère l’exécution au noyau.
- **Noyau x86-64 sans environnement standard** avec contrat `ARCHIABI`/BootInfo versionné et validation de la carte mémoire UEFI.
- **Pagination x86-64 high-half** et protections de pages `W^X/NX` sur le chemin pris en charge.
- **Gestionnaire de mémoire physique (PMM)** avec auto-tests et contrôles de libération.
- **Framebuffer UEFI/GOP** : détection de la plage et premier motif visible confirmé sur matériel physique.
- **Prototype AArch64** et chemins de test UEFI/virtuel dédiés ; la parité avec x86-64 n’est pas atteinte.
- **CI GitHub Actions** pour compiler et démarrer des configurations virtuelles et pour produire un paquet EFI destiné à une clé USB.

ArchiaOS reste un projet expérimental. Les fonctionnalités décrites dans la roadmap comme non commencées ne sont pas disponibles simplement parce que leur documentation existe.

## Démarrage rapide

### Préparer les fichiers EFI d’une clé USB

Prérequis : Linux ou WSL2, Rust via `rustup` et les cibles UEFI/noyau nécessaires. Depuis le dossier du projet :

~~~sh
cd ArchiaOS
make uefi-files
~~~

Le paquet est généré dans `ArchiaOS/build/uefi-files/` :

~~~text
EFI/
├── BOOT/
│   └── BOOTX64.EFI
└── ArchiaOS/
    └── kernel.elf
README.txt
~~~

Copie le dossier EFI à la racine d’une partition USB FAT32. Le firmware UEFI lance `EFI/BOOT/BOOTX64.EFI`, puis le chargeur ArchiaOS recherche `EFI/ArchiaOS/kernel.elf` sur le même volume.

**Précautions matérielles :** utilise une clé dédiée, vérifie soigneusement le lecteur ciblé et sauvegarde les données importantes. Secure Boot peut refuser un binaire de développement non signé. Le paquet ne remplace pas le firmware UEFI de la machine et ne doit pas être présenté comme une distribution prête à installer.

### Démarrage virtuel local (QEMU/OVMF)

Sur Debian/Ubuntu, les outils nécessaires peuvent être installés ainsi :

~~~sh
sudo apt-get update
sudo apt-get install -y build-essential qemu-system-x86 ovmf dosfstools mtools
rustup target add x86_64-unknown-none x86_64-unknown-uefi
~~~

Puis :

~~~sh
cd ArchiaOS
make boot-local
~~~

Ce test construit une image FAT32 temporaire, démarre le chemin UEFI sous QEMU/OVMF et vérifie les marqueurs série attendus. Le délai d’expiration QEMU est normal lorsque le noyau a atteint son point d’arrêt de test. Les journaux et l’image sont conservés dans `ArchiaOS/build/local-uefi/`.

Pour un OVMF installé dans un autre emplacement, fournis `OVMF_CODE` et `OVMF_VARS` à la commande. Le démarrage virtuel et le démarrage sur matériel physique sont deux validations distinctes.

## Compilation et tests

Depuis ArchiaOS/ :

~~~sh
# Noyau de production x86-64
cargo build --release --features uefi-boot --target x86_64-unknown-none

# Chargeur UEFI x86-64
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi

# Chargeur UEFI AArch64
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi

# Tests unitaires du chargeur sur l’hôte Linux
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
~~~

Pour produire les fichiers EFI autonomes, préfère `make uefi-files`. Pour le smoke test complet sous OVMF/QEMU, utilise `make boot-local`.

## Structure du dépôt

| Chemin | Rôle |
|---|---|
| `.github/workflows/` | CI, tests de démarrage et création de l’artefact USB |
| `ArchiaOS/kernel/src/` | Noyau x86-64 de production |
| `ArchiaOS/kernel/aarch64/` | Prototype noyau AArch64 |
| `ArchiaOS/bootloader/` | Chargeur UEFI ArchiaOS et validation des données de transfert |
| `ArchiaOS/kernel/boot-test-*/` | Noyaux minimaux destinés aux tests de démarrage |
| `ArchiaOS/scripts/` | Scripts de packaging EFI et de démarrage local |
| `ArchiaOS/docs/` | Architecture, mémoire, sécurité, tests et roadmap |

## Roadmap

Le travail suit les priorités de [la roadmap P0–P10](ArchiaOS/docs/ROADMAP.md). Le jalon P0 de premier démarrage physique et d’écriture framebuffer est considéré atteint. **P1 reste en cours** : l’audit des plages mémoire réservées et la validation CI des derniers contrôles doivent être achevés avant de le clôturer.

Priorités suivantes : fiabiliser les fondations mémoire, puis implémenter les exceptions et l’ordonnanceur, l’espace utilisateur, le stockage/VFS, l’affichage, le réseau et les outils système. Les pourcentages de la roadmap sont des estimations et non des mesures automatiques.

## Documentation

- [Documentation technique et compilation](ArchiaOS/README.md)
- [Roadmap](ArchiaOS/docs/ROADMAP.md)
- [Architecture](ArchiaOS/docs/ARCHITECTURE.md)
- [Démarrage UEFI](ArchiaOS/docs/BOOT.md)
- [Stratégie de tests](ArchiaOS/docs/TESTING.md)
- [Mémoire et PMM](ArchiaOS/docs/MEMORY.md)
- [Sécurité](ArchiaOS/docs/SECURITY.md)
- [Chargeur UEFI](ArchiaOS/bootloader/README.md)
- [Prototype AArch64](ArchiaOS/kernel/aarch64/README.md)

## Contribution et état du projet

Les changements actifs de développement sont réalisés sur la branche **ArchiaOS**. Les validations doivent être rapportées selon les résultats réellement observés : une compilation réussie, un démarrage QEMU et un démarrage matériel ne prouvent pas la même chose.

ArchiaOS est un projet expérimental en évolution ; les limites documentées dans la roadmap et les guides techniques restent applicables.
