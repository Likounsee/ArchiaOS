# ArchiaOS — documentation technique

ArchiaOS est un OS expérimental en Rust `no_std`. Le chemin de démarrage par défaut est le chargeur UEFI ArchiaOS : le noyau reçoit BootInfo `ARCHIABI` et initialise le PMM depuis la carte mémoire UEFI.

## État P0

- **Démarrage UEFI x86-64 :** chargeur ArchiaOS, noyau high-half, BootInfo v1, pagination W^X/NX et auto-test PMM.
- **Boot local reproductible :** `make boot-local` compile le noyau et le chargeur, fabrique une image FAT32 et la démarre sous OVMF/QEMU. La commande exige les marqueurs série du noyau de production, jusqu'au marqueur final.
- **Limite :** le succès sous QEMU valide le chemin virtuel, pas le démarrage sur une machine physique. La validation matérielle reste distincte.

## Démarrage local UEFI (Linux/WSL2)

Sur Debian/Ubuntu, installer les outils :

```sh
sudo apt-get update
sudo apt-get install -y build-essential qemu-system-x86 ovmf dosfstools mtools
rustup target add x86_64-unknown-none x86_64-unknown-uefi
```

Depuis ce dossier, lancer :

```sh
make boot-local
```

L'image est écrite dans `build/local-uefi/archiaos-uefi.img` et les diagnostics série dans `build/local-uefi/serial.log`. Le timeout QEMU est normal : le noyau termine ses validations puis se bloque volontairement. Toute compilation échouée, sortie QEMU inattendue ou absence d'un marqueur fait échouer la commande. Si OVMF est installé ailleurs, définir `OVMF_CODE=/chemin/OVMF_CODE.fd` et `OVMF_VARS=/chemin/OVMF_VARS.fd`.

Ce test est virtuel et ne remplace pas un essai sur PC UEFI physique. Pour le matériel, utiliser un support USB dédié et sauvegarder les données importantes.

## Compilation

```sh
cargo build --release
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi --target-dir target
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```

Voir [le guide de démarrage](docs/BOOT.md), [la stratégie de tests](docs/TESTING.md), [la roadmap](docs/ROADMAP.md) et [le bootloader](bootloader/README.md).

Le développement se fait sur la branche `ArchiaOS`.
