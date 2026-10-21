//! nwasabi — cœur d'émulation des applications externes (fichiers `.nwa`) des calculatrices NumWorks.
//!
//! Le cœur est une bibliothèque pure : pas de threads, pas d'horloge système, pas d'entrées/sorties.
//! L'hôte (un outil en ligne de commande, une page web…) lui fournit les entrées et lit ses sorties.

#![forbid(unsafe_code)]
