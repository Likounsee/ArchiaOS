# Démarrage UEFI ArchiaOS

ArchiaOS utilise le firmware UEFI standard de la machine et son **propre chargeur UEFI**. Le démarrage par défaut du noyau x86-64 est le chemin ArchiaOS, pas Limine.

## Contrat de démarrage

1. Le firmware lance le chargeur EFI ArchiaOS depuis la partition système EFI.
2. Le chargeur ouvre `\\EFI\\ArchiaOS\\kernel.elf` sur le même volume.
3. Il borne la taille du fichier et valide l'ELF64, les segments chargeables, les alignements, les débordements, les recouvrements et l'entrée exécutable. Les segments RWX sont refusés.
4. Il réserve les pages physiques requises, copie les segments et met à zéro la partie BSS.
5. Sur x86-64, il prépare les tables de pages du noyau high-half et applique les permissions W^X/NX.
6. Il construit le BootInfo versionné `ARCHIABI`, récupère la carte mémoire finale, puis appelle `ExitBootServices` avec une nouvelle tentative si la clé de carte est périmée.
7. Le noyau valide le contrat de transfert avant d'utiliser la carte mémoire.

Le chemin ARM64 charge actuellement des segments identité ; le remappage high-half et une transition MMU propre au noyau ne sont pas encore implémentés pour cette architecture.

## Construire les composants

Depuis le dossier `ArchiaOS` :

```sh
rustup target add x86_64-unknown-none x86_64-unknown-uefi aarch64-unknown-uefi
cargo build --release --target x86_64-unknown-none
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```

Tests unitaires des parseurs partagés :

```sh
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
```

Ces commandes compilent les composants ; elles ne créent pas à elles seules une image EFI prête à démarrer. La CI fabrique les volumes FAT et les lance avec OVMF/QEMU (x86-64) et AAVMF/QEMU (ARM64).

## Validation et limites

Les tests CI couvrent le démarrage x86-64 et ARM64 sous des configurations virtuelles documentées, ainsi que les rejets d'ELF invalide, de plage physique indisponible, de noyau absent et de BootInfo invalide. Ils ne garantissent pas la compatibilité avec toutes les implémentations UEFI physiques.

Avant de considérer P0 terminé, tester sur au moins une machine physique UEFI x86-64 : désactiver Secure Boot si le firmware refuse les binaires de développement non signés, démarrer depuis une clé FAT32 préparée avec le chargeur et `EFI/ArchiaOS/kernel.elf`, conserver les logs série/console si disponibles, confirmer la validation BootInfo et l'initialisation PMM, puis vérifier le redémarrage à froid et le retour au firmware en cas d'échec. Noter le modèle de machine, la version du firmware et le résultat. Ne pas tester sur un disque contenant des données importantes sans sauvegarde.

Les limites connues incluent la fenêtre physique prise en charge sous 4 Gio pour le chemin de chargement testé, un fichier noyau plafonné à 64 Mio et une mémoire cumulée de segments plafonnée à 128 Mio.
