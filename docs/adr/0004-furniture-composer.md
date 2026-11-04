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

## Construction, références et retours de revue

Les épaisseurs facultatives des panneaux du caisson et des séparations remplacent
l'épaisseur globale pour ces seules pièces. Les ouvertures sont calculées avec
la somme des épaisseurs réelles. Une séparation peut réduire sa profondeur ; sa
portée X/Z est celle de sa case. Changer les dimensions du caisson redimensionne
ses panneaux : aucun panneau libre détaché de la structure n'est introduit.

`top_covers_sides`, `bottom_covers_sides` et le fond continu d'un bac choisissent
le panneau continu aux angles et donc la pièce femelle. Les dimensions extérieures
et les ouvertures restent constantes. Une tablette intérieure reste mâle vers
la face du côté. Les IDs de liaison encodent les deux IDs de panneau, la normale
de la femelle (0=X, 1=Y, 2=Z) et l'extrémité hi/lo du mâle. La face `Front` est
la face de normale minimale ; `Back` la face maximale. Ces références sont
reconstruites sans ambiguïté depuis le schéma natif ; le manifeste les explicite.
Après une édition UI, les surcharges sont conservées par paire de panneaux lors
d'un changement de continuité et retirées si la liaison disparaît. Un import
avec référence orpheline reste refusé. La copie réattribue les IDs et les surcharges
puis valide la structure et les usinages avec la fraise active avant application.

Les racines concaves des tenons reçoivent aussi les dégagements de fraise. Les
poches de réduction d'une languette dépassent le brut aux trois bords libres :
une poche limitée au rectangle de la languette laisserait des coins non usinés.
Sans dégagement, les angles doivent être finis manuellement. Les chevauchements
de poches sur une même face ou traversant la matière entre faces sont refusés.

Les enveloppes de placement comprennent les contours d'usinage, y compris les
surfaçages débordant du brut. Les épaisseurs distinctes occupent des dalles distinctes.
Une rotation utilise (H−V,U), puis la translation de placement. Le verso applique
(X,Y) → (largeur_dalle−X,Y) et inverse le signe des bulges. Usiner le verso avant
la découpe extérieure du recto ; un montage et des origines repérées sont nécessaires.

Les exigences FAO sont transportées dans `FurnitureOperations` du CB. Une poche
obligatoire doit porter sur son seul contour, à la profondeur prescrite et sans
surépaisseur : inclure ce contour dans une poche à îlots ne garantit pas son
évidement. Les tests reconstruisent aussi la matière résiduelle d'un fraisage
rond autour des épaules, indépendamment du seul aperçu idéal.
