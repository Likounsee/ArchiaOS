# Démarrage

Le noyau cible le protocole natif **Limine** et un ELF64 x86-64 lié à l’adresse 1 Mio. Les marqueurs de requêtes Limine sont conservés par le linker script. Le bootloader fournit l’entrée en mode long et l’environnement de pagination initial ; le noyau n’effectue plus lui-même une transition 32→64 bits.

Le script `scripts/make-iso.sh` télécharge Limine v12.9.1, vérifie le SHA-256 de l’archive, crée une ISO amorçable BIOS/UEFI et exécute `limine bios-install`. La CI démarre cette ISO sous QEMU et exige le message série de `_start`.

Cette base reste expérimentale. Elle ne valide pas encore les informations de démarrage ni les régions mémoire, et elle ne doit pas être présentée comme prête pour du matériel réel avant des tests prolongés et un audit.
