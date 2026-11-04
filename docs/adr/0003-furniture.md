# Module Meubles

Statut : accepté et implémenté pour 0.5.0, issue #14.

Le module métier `furniture` reste dans `fablabcam-studio`, sans dépendance egui.
Il utilise les types et exports déjà présents. Les sept crates sont conservées.
L'interface native est dans `fablabcam-view::furniture_ui`.

Les fichiers natifs ont un discriminant `document_type` et une version de schéma.
Un projet conserve une copie complète de son modèle et du profil machine choisi.
La révision du modèle évolue à l'enregistrement ; aucun projet n'est migré
implicitement. L'import analyse et prévisualise avant tout ajout. Une collision
d'identifiant impose une copie. Le catalogue est écrit atomiquement sous verrou,
avec comparaison de la version chargée pour refuser les écritures concurrentes.

Les gabarits utilisent uniquement des panneaux rectangulaires vissés, dessus,
dessous et tablettes entre les côtés. Le dos rapporté est inclus dans la profondeur
extérieure. Les identifiants de pièces sont stables et repris dans les exports.
Le calcul refuse les dimensions impossibles et limite la taille des entrées.

Le placement est une heuristique déterministe de rectangles libres disjoints.
Il contrôle marges, diamètre, espacement, rotations et dimensions utiles. Il ne
garantit pas le nombre minimal de dalles. Une pièce non plaçable bloque le dossier.
Les courses déclarées pour le projet sont distinctes des dimensions des dalles
après pré-débit. Les limites physiques absentes d'un profil restent inconnues.

Les résultats dérivés sont recalculés depuis le document courant ; aucun ancien
placement ni G-code n'est incorporé au fichier natif. Le dossier de fabrication
contient des géométries, une nomenclature et une notice. La préparation FAO,
le bridage, les trajectoires et l'analyse machine restent des étapes explicites.
Le profil est transmis avec les plans ; son nom ne sélectionne pas un dialecte.

Les bibliothèques ZIP et PDF remplacent une implémentation ad hoc des conteneurs :
`zip` sans compression optionnelle et `pdf-writer`. `tempfile` assure les écritures
atomiques multiplateformes. Le dossier local peut être isolé par
`FABLABCAM_FURNITURE_STORE` pour les tests et les captures publiques.

TODO(agent) : formes arbitraires, assemblages usinés, quincaillerie et gestion des
chutes exigent de nouveaux modèles métier ; ne pas les simuler par des paramètres
ignorés dans le format V1.
