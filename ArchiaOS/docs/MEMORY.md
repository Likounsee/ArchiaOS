# Mémoire

## Carte mémoire UEFI

Le noyau reçoit le contrat versionné `ARCHIABI BootInfo` du chargeur UEFI ArchiaOS. Il valide l'en-tête avant d'utiliser ses champs, puis parse la carte mémoire UEFI en respectant la taille et le pas des descripteurs. Les plages sont contrôlées contre les débordements et les entrées incohérentes ; une carte vide ou trop grande est rejetée.

Seules les plages `EfiConventionalMemory` deviennent candidates à l'allocation. Les autres types UEFI (chargeur, runtime, ACPI, MMIO et autres réservations) restent indisponibles. Les pages sous 1 Mio sont réservées. Les plages sont bornées à la fenêtre physique gérée par le PMM, actuellement `[0, 64 Gio)`. Cette limite est une capacité de suivi du PMM, pas une garantie que tous les processeurs prennent en charge toute cette plage.

## Allocateur physique expérimental

L'allocateur bitmap gère des frames de 4 Kio sous 4 Gio. Un bitmap marque les frames gérées et un second les frames libres. Une frame n'est allouable que si elle est à la fois gérée et libre ; cela empêche de libérer une adresse arbitraire, une frame réservée, une adresse non alignée ou une frame déjà libérée.

Les plages sont appliquées par mots bitmap plutôt qu'en parcourant chaque page individuellement. Le calcul préliminaire vérifie les descripteurs avant de réinitialiser l'état global ; l'allocateur n'est marqué initialisé qu'après le remplissage et la vérification des invariants. Les pages couvrant le framebuffer sont réservées avec arrondi vers l'extérieur, même si le firmware les classait incorrectement comme mémoire conventionnelle.

Le test PMM au démarrage vérifie notamment les calculs de plages utilisables/réservées, les limites et chevauchements du framebuffer, les invariants des bitmaps, l'épuisement simulé, le rejet des libérations invalides, l'absence de libération de frames réservées, ainsi que l'allocation et la libération de 130 pages distinctes à travers plusieurs mots bitmap. Il contrôle que le nombre de pages libres revient à sa valeur initiale après le test.

Le workflow de démarrage local doit aussi exiger le marqueur série `ArchiaOS: UEFI PMM allocation/free self-test passed` et `ArchiaOS: framebuffer probe=written`. Ces marqueurs prouvent que le chemin de test correspondant a été atteint sous OVMF/QEMU ; ils ne remplacent pas la validation sur matériel physique.

## Limites importantes

Le PMM est limité à 4 Gio, s'initialise une seule fois et n'est pas synchronisé pour SMP. Le nombre d'octets de mémoire conventionnelle indiqué dans les statistiques est le total géré après exclusion des pages basses et du framebuffer, et non la totalité de la RAM installée. Il n'existe pas encore de tas général ni de gestionnaire complet de mémoire virtuelle. La stabilité des réservations et du démarrage doit encore être confirmée par une campagne matérielle répétable.
