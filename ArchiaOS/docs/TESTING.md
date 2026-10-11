# Stratégie de tests

La CI compile le noyau pour `x86_64-unknown-none`, crée une ISO Limine et démarre cette ISO dans QEMU. Le test de démarrage attend le message série `ArchiaOS: Limine entered x86_64 kernel`. Le build seul vérifie compilation et liaison, pas le démarrage ; seul le résultat de l’étape QEMU valide ce smoke test.

Vérification manuelle depuis `ArchiaOS` :

```sh
rustup target add x86_64-unknown-none
cargo build --release
scripts/make-iso.sh
qemu-system-x86_64 -cdrom build/archiaos.iso -boot d -serial stdio -display none -no-reboot -no-shutdown
```

Il n’existe pas encore de tests unitaires mémoire ou interruptions : ces composants ne sont pas implémentés. `cargo test` seul ne valide pas le démarrage bare-metal.
