# ArchiaOS — guide technique

ArchiaOS est un système d’exploitation expérimental en Rust sans environnement standard (no_std). Le chemin x86-64 par défaut utilise le chargeur UEFI maison et le contrat BootInfo versionné ARCHIABI.

## État actuel

- Le chargeur UEFI x86-64 charge le noyau de production, valide l’ELF64 et prépare le transfert vers le noyau.
- Le noyau valide BootInfo et la carte mémoire UEFI, puis initialise le gestionnaire de mémoire physique (PMM).
- Le chemin x86-64 high-half utilise des tables de pages dédiées et les protections W^X/NX sur les segments du noyau.
- Le noyau a démarré sur un PC physique et a écrit un motif dans le framebuffer UEFI : fond bleu foncé avec une bande bleu clair.
- Les jalons P0 et P1 sont considérés atteints selon les preuves et confirmations physiques archivées dans [docs/preuves/](docs/preuves/README.md) : 5/5 démarrages à froid et 5/5 redémarrages à chaud rapportés, avec retour au firmware puis au système habituel.
- P2 est le chantier actif : le scheduler coopératif, les réveils ciblés/collectifs et la topologie CPU sont en cours de consolidation. Le commit `67d5240` ajoute une régression ciblée au test de réveil transactionnel.
- Le système n’a pas encore de bureau ni de pile graphique complète.

Les succès QEMU/OVMF et AAVMF/QEMU démontrent uniquement les configurations virtuelles couvertes. Ils ne prouvent pas la compatibilité avec toutes les machines ou tous les firmwares.

## Démarrage local UEFI (Linux/WSL2)

Sur Debian/Ubuntu, installer les outils :

~~~sh
sudo apt-get update
sudo apt-get install -y build-essential qemu-system-x86 ovmf dosfstools mtools
rustup target add x86_64-unknown-none x86_64-unknown-uefi
~~~

Depuis ce dossier, lancer le test virtuel :

~~~sh
make boot-local
~~~

Le script compile le noyau de production et le chargeur, crée une image FAT32, démarre l’image sous OVMF/QEMU et vérifie les marqueurs série attendus. Le timeout QEMU est normal si le noyau a atteint son point d’arrêt de test. L’image et le journal série sont enregistrés dans build/local-uefi/.

Si OVMF est installé ailleurs, définir OVMF_CODE et OVMF_VARS avant la commande.

## Préparer les fichiers EFI pour une clé USB

~~~sh
make uefi-files
~~~

Les fichiers sont générés dans build/uefi-files/ :

- EFI/BOOT/BOOTX64.EFI — chargeur UEFI de secours x86-64.
- EFI/ArchiaOS/kernel.elf — noyau chargé par ArchiaOS.
- README.txt — instructions de copie du paquet.

Copier le dossier EFI à la racine d’une partition USB FAT32. Le firmware doit être configuré pour démarrer en mode UEFI. Secure Boot peut refuser un binaire de développement non signé. Utiliser un support de test dédié et ne pas modifier le disque système par inadvertance.

## Compilation et tests

~~~sh
# Noyau x86-64
cargo build --release --features uefi-boot --target x86_64-unknown-none

# Chargeur UEFI x86-64
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi

# Chargeur UEFI AArch64
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi

# Tests unitaires du chargeur sur Linux
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
~~~

Pour le commit `67d5240`, les workflows dédiés au démarrage local UEFI, au test négatif BootInfo et au paquet USB sont réussis. La CI principale comporte plusieurs jobs réussis ; le job de démarrage x86-64 OVMF était encore en cours lors du dernier contrôle. Vérifier le statut du commit ciblé avant toute validation ultérieure.

## Documentation technique

- [Architecture](docs/ARCHITECTURE.md)
- [Contrat de démarrage UEFI](docs/BOOT.md)
- [Architecture du chargeur et AArch64](docs/BOOT_AND_ARCHITECTURES.md)
- [Gestion mémoire](docs/MEMORY.md)
- [Sécurité](docs/SECURITY.md)
- [Stratégie de tests](docs/TESTING.md)
- [Roadmap](docs/ROADMAP.md)
- [Chargeur UEFI](bootloader/README.md)
- [Prototype noyau AArch64](kernel/aarch64/README.md)

Le développement actif se fait sur la branche ArchiaOS.
