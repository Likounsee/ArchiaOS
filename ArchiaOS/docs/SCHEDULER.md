# Ordonnanceur

Aucun ordonnanceur, thread ou changement de contexte n’est implémenté. Le noyau reste dans une boucle d’attente terminale après son initialisation. Avant de planifier des tâches, il faut disposer d’exceptions et interruptions fiables, d’une base de temporisation, d’un modèle de contexte x86-64 et de primitives de synchronisation testées.