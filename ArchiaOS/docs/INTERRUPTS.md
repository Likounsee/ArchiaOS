# Interruptions et exceptions

Aucune IDT ni aucun gestionnaire d’exception n’est installé à ce stade. Les interruptions matérielles sont désactivées avant l’entrée Rust et le noyau se met en attente avec `cli; hlt`. Aucun traitement normal des exceptions ou IRQ n’est donc disponible.

Le prochain lot devra installer une IDT avec des stubs adaptés à l’ABI x86-64, distinguer les exceptions qui poussent un code d’erreur de celles qui n’en poussent pas, sauvegarder l’état requis, diagnostiquer les fautes de page et ne permettre les IRQ qu’après l’initialisation du contrôleur et des handlers.