# Mémoire

## Carte mémoire UEFI

Le noyau reçoit le contrat versionné `ARCHIABI BootInfo` du chargeur UEFI ArchiaOS. Il valide l'en-tête avant d'utiliser ses champs, puis parse la carte mémoire UEFI en respectant la taille et le pas des descripteurs. Les plages sont contrôlées contre les débordements et les entrées incohérentes ; une carte vide ou trop grande est rejetée.

Seules les plages `EfiConventionalMemory` deviennent candidates à l'allocation. Les autres types UEFI (chargeur, runtime, ACPI, MMIO et autres réservations) restent indisponibles. Les pages sous 1 Mio sont réservées. Le PMM suit et alloue les frames physiques dans `[0, 512 Gio)`. Une adresse physique n'est pas un pointeur virtuel : les frames hautes sont accessibles via un alias noyau créé à la demande par le VMM. La limite de 512 Gio est une fenêtre logicielle, pas une garantie que toutes les machines disposent de cette quantité de RAM.

## Allocateur physique expérimental

L'allocateur bitmap gère des frames de 4 Kio sous 512 Gio. Un bitmap marque les frames gérées et un second les frames libres. Les deux bitmaps (32 Mio au total pour 512 Gio suivis) sont placés dynamiquement dans une plage de RAM conventionnelle réservée au démarrage, afin de ne pas gonfler le segment BSS fixe du noyau. Une frame n'est allouable que si elle est à la fois gérée et libre ; cela empêche de libérer une adresse arbitraire, une frame réservée, une adresse non alignée ou une frame déjà libérée.

Les plages sont appliquées par mots bitmap plutôt qu'en parcourant chaque page individuellement. Le calcul préliminaire vérifie les descripteurs avant de réinitialiser l'état global ; l'allocateur n'est marqué initialisé qu'après le remplissage et la vérification des invariants. Les pages couvrant le framebuffer sont réservées avec arrondi vers l'extérieur, même si le firmware les classait incorrectement comme mémoire conventionnelle.

Le test PMM au démarrage vérifie notamment les calculs de plages utilisables/réservées, les limites et chevauchements du framebuffer, les invariants des bitmaps, l'épuisement simulé, le rejet des libérations invalides, l'absence de libération de frames réservées, ainsi que l'allocation et la libération de 130 pages distinctes à travers plusieurs mots bitmap. Il contrôle que le nombre de pages libres revient à sa valeur initiale après le test.

Le workflow de démarrage local doit aussi exiger le marqueur série `ArchiaOS: UEFI PMM allocation/free self-test passed` et `ArchiaOS: framebuffer probe=written`. Ces marqueurs prouvent que le chemin de test correspondant a été atteint sous OVMF/QEMU ; ils ne remplacent pas la validation sur matériel physique.

## Premier VMM x86-64

`kernel/src/vmm.rs` ajoute `map_physical_page` et `unmap_physical_page`. Le mapping physique à la demande utilise la base virtuelle canonique `0xffff900000000000`, crée les tables intermédiaires manquantes à partir de frames allouées sous 4 Gio et installe une feuille inscriptible/NX. `map_kernel_page` permet aussi de mapper une frame à une adresse noyau choisie, avec des permissions configurables et un refus explicite des mappings inscriptibles et exécutables simultanément. Le retrait vérifie que la feuille pointe toujours vers la frame attendue, invalide le TLB local et récupère les tables intermédiaires vides possédées par le PMM. Le test d'intégration au démarrage écrit et relit un motif via l'alias, le retire puis libère la frame ; il teste aussi une frame au-dessus de 4 Gio si la carte UEFI en fournit.

## Tas noyau initial

`kernel/src/heap.rs` fournit un allocateur noyau à liste libre, avec extension page par page dans la région virtuelle `0xffffa00000000000` et fusion des blocs adjacents à la libération. Il est enregistré comme allocateur global mais doit être initialisé après le PMM/VMM. L'autotest vérifie les alignements, l'extension sur plusieurs pages, l'accès mémoire, la libération et la réutilisation. Le verrou est volontairement limité au démarrage mono-CPU : cet allocateur ne doit pas être utilisé depuis une IRQ ni concurremment depuis plusieurs processeurs tant que la synchronisation du noyau n'est pas implémentée.

## Limites importantes

Le PMM suit et alloue des frames jusqu'à 512 Gio ; les bitmaps et tables de pages restent accessibles sous 4 Gio. Le VMM est encore un mécanisme de bootstrap : pas d'espaces d'adressage par processus, pas de récupération des tables intermédiaires, pas de shootdown TLB SMP, et le PMM/VMM ne sont pas synchronisés pour SMP. Le nombre d'octets de mémoire conventionnelle indiqué dans les statistiques est le total géré après exclusion des pages basses et du framebuffer, et non la totalité de la RAM installée. Il n'existe pas encore de tas général ni de gestionnaire complet de mémoire virtuelle. La stabilité des réservations et du démarrage doit encore être confirmée par une campagne matérielle répétable.
