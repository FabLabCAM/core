# Composition de meubles et assemblages

Statut : en développement pour 0.6.0, issue #17.

Le moteur reste dans `fablabcam-studio::furniture`. Une composition facultative
fait partie des paramètres dimensionnels de chaque modèle/projet. L'absence de
composition conserve exactement le générateur 0.5.0. L'activation de l'éditeur
convertit explicitement les tablettes existantes en séparations identifiées.
Le schéma 2 lit le schéma 1 ; les originaux restent intacts et l'enregistrement
est explicite. La bibliothèque conserve son verrou et sa comparaison de version.

Une case est une feuille ou une division ordonnée horizontale/verticale. Les
séparations appartiennent à la division, les cases enfants ont des identifiants
stables. La répartition égale et les positions fixes sont distinctes. Les panneaux
partagés sont générés une fois ; les divisions internes sont interrompues aux
limites de leur case. Les bacs ouverts sont cinq panneaux avec jeux explicites.
La suppression d'une division expose le nombre de descendants avant confirmation.

Les panneaux ont un repère local (U, V, normale) et une enveloppe assemblée.
Les contacts orthogonaux génèrent des liaisons identifiées par les deux panneaux
et leurs faces. Un défaut et des surcharges paramétriques déterminent les découpes
complémentaires. Le jeu est TOTAL, appliqué une fois à la pièce femelle. Les
contours, poches, profondeurs et faces sont des données dérivées ; aucun cache
sauvegardé ne peut survivre à une modification. Les booléens existants de
`fablabcam-geom` sont réutilisés ; aucune nouvelle bibliothèque de CAO.

Le placement utilise l'enveloppe complète de chaque pièce, tenons compris.
Le dossier comporte les contours réels et un manifeste d'opérations associant
pièces, liaisons, profondeurs et faces. Les montages particuliers sont signalés,
jamais assimilés à un parcours validé. Les familles non implémentées du catalogue
restent visibles avec leur statut. La tenue mécanique n'est pas calculée.

Vérification : grille 5 × 6 (178,4 × 179 mm), divisions imbriquées, bacs,
positions fixes/égales, identifiants, limites, historique, migration et aller-retour,
liaisons tournées/miroir, jeu total, géométrie complémentaire, outils, placement,
exports, chaîne FAO, tests workspace/Clippy puis interface du paquet installé.
