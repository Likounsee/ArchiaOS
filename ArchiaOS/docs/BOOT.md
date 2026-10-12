# Démarrage UEFI ArchiaOS

ArchiaOS utilise le firmware UEFI standard de la machine et son propre chargeur UEFI. Le chemin x86-64 par défaut ne dépend pas de Limine.

## Contrat de démarrage

1. Le firmware lance le chargeur EFI ArchiaOS depuis la partition système EFI.
2. Le chargeur ouvre `\\EFI\\ArchiaOS\\kernel.elf` sur le même volume.
3. Il borne la taille du fichier et valide l'ELF64, les segments chargeables, les alignements, les débordements, les recouvrements et l'entrée exécutable. Les segments RWX sont refusés.
4. Il réserve les pages physiques requises, copie les segments et met à zéro la partie BSS.
5. Sur x86-64, il prépare les tables de pages du noyau high-half et applique les permissions W^X/NX.
6. Il construit le BootInfo versionné `ARCHIABI`, récupère la carte mémoire finale, puis appelle `ExitBootServices` avec une nouvelle tentative si la clé de carte est périmée.
7. Le noyau valide le contrat de transfert avant d'utiliser la carte mémoire.

Le chemin ARM64 charge actuellement des segments identité ; le remappage high-half et une transition MMU propre au noyau ne sont pas encore implémentés pour cette architecture.

## Boot local

Sur Linux ou WSL2, avec QEMU, OVMF, `dosfstools` et `mtools` installés, lancer depuis ce dossier :

```sh
make boot-local
```

Le script compile le noyau de production et le chargeur, crée `build/local-uefi/archiaos-uefi.img`, démarre l'image sous OVMF/QEMU et vérifie les marqueurs série jusqu'à `ArchiaOS: UEFI kernel entry validated`. Le timeout QEMU est attendu, car le noyau se bloque volontairement après ses validations. Les logs sont enregistrés dans `build/local-uefi/serial.log`.

Pour un firmware OVMF installé ailleurs, définir `OVMF_CODE=/chemin/OVMF_CODE.fd` et `OVMF_VARS=/chemin/OVMF_VARS.fd` avant la commande.

Ce test confirme le démarrage virtuel x86-64, pas la compatibilité de tout matériel. Le premier démarrage physique et l’écriture dans le framebuffer ont été confirmés sur une machine réelle ; le motif bleu est un test d’affichage bas niveau, pas un bureau. Un redémarrage à froid et le retour au firmware restent à consigner pour compléter la matrice de validation matérielle.

## Compilation des composants

```sh
rustup target add x86_64-unknown-none x86_64-unknown-uefi aarch64-unknown-uefi
cargo build --release --target x86_64-unknown-none
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
```

La CI vérifie aussi les rejets d'ELF invalide, de plage physique indisponible, de noyau absent et de BootInfo invalide. Ces tests ne garantissent pas la compatibilité avec toutes les implémentations UEFI physiques.

Les limites connues incluent la fenêtre physique prise en charge sous 4 Gio, un fichier noyau plafonné à 64 Mio et une mémoire cumulée de segments plafonnée à 128 Mio.
