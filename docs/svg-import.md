# Import SVG — issue #26

## Architecture et décisions

Le lecteur importe des contours CAO, pas une image rendue. `svgio` reste indépendant
de l'interface et commun à la CLI et au Studio. Les étapes sont séparées : parcours
XML, grammaire des chemins, transformation affine, création d'entités et export.

La grammaire est confiée à [svgtypes 0.16.2](https://docs.rs/svgtypes/0.16.2/),
sous Apache-2.0 ou MIT, compatible avec le Rust 1.89 du dépôt. `PathParser` accepte
les exposants, séparateurs compacts et commandes implicites ; `Transform` lit les
listes de matrices, translations, rotations autour d'un point, échelles et biais.
Le choix de `PathParser` plutôt que d'une normalisation complète par `usvg`
préserve les arcs circulaires exacts et la convention de coordonnées existante.

Une pile explicite porte le repère de chaque branche XML. Pour un point local,
`p_doc = flip_y(parent × local × p)`. Les transformations ne se propagent pas aux
frères. La racine peut aussi porter `transform`. Les sous-arbres `defs`, `clipPath`,
`mask`, `marker`, `pattern`, `symbol` et métadonnées ne produisent aucune entité.
Une ressource inutilisée n'est donc ni un contour usinable ni un avertissement.

Les similitudes conservent cercles et bulges ; le déterminant ajuste le sens des
arcs en cas de miroir, puis l'inversion Y l'ajuste une seconde fois. Une matrice
non uniforme ou un biais transforme les arcs circulaires en polylignes. Leur pas
est calculé à partir d'une majoration de l'étirement et de la tolérance 0,01 unité
du document. Au-delà de 100 000 subdivisions par arc, l'import échoue explicitement.
Les Bézier et ellipses conservent l'approximation historique (24 segments par
cubique et échantillonnage des ellipses) ; aucune tolérance globale garantie n'est
annoncée pour ces courbes après un agrandissement arbitraire.

La fermeture d'un chemin retire seulement son point final redondant, sans effacer
le bulge porté par le segment précédent. Deux arcs peuvent former un contour fermé
à deux sommets ; ils ne doivent pas être rejetés comme un polygone dégénéré.

## Erreurs et limites

- Un chemin, une transformation ou une liste de coordonnées invalide fait échouer
  l'import complet avec le type d'élément et son `id`. Aucun résultat partiel n'est
  rendu à l'appelant ; `convert` ne remplace pas le fichier de sortie en cas d'erreur.
- Les références `use`, textes non vectorisés, images, SVG imbriqués, masques,
  filtres et découpes effectivement utilisés restent non pris en charge et sont
  signalés. La géométrie de base d'un élément avec découpe/filtre est importée avec
  un avertissement d'import incomplet. `display:none` exclut un sous-arbre.
- Ce n'est pas un moteur CSS : feuilles de style, héritage de visibilité, effets
  et transformations CSS ne sont pas appliqués. La transformation CSS inline est
  signalée. Remplissage et épaisseur du trait ne deviennent pas des opérations FAO.
- Les valeurs restent dans les unités utilisateur SVG historiques : `viewBox`
  fournit l'origine d'inversion Y, sans conversion générale px/mm ni
  `preserveAspectRatio`. Le fichier de #26 a les mêmes dimensions en mm et unités
  utilisateur ; son échelle est donc directement exploitable. Contrôler les cotes
  des autres exports. `data-doc-ymax` préserve les aller-retours FabLabCAM.
- Aucun accès à une ressource externe n'est effectué.

## Validation

Les tests synthétiques sont dans `fablabcam-studio/tests/svg_import.rs` et
`fablabcam-cli/tests/svg_import.rs`. Ils couvrent les trois défauts initiaux,
l'ordre des matrices, les frères, les miroirs, les courbes non uniformes, les
exposants, les erreurs atomiques, les sous-chemins et la fermeture des arcs.
Les suites `j1` et `j8` conservent les références d'aller-retour existantes.

Le fichier utilisateur est vérifié localement depuis la pièce jointe de
[#26](https://github.com/FabLabCAM/core/issues/26), sans copie du logo ou de ses
métadonnées personnelles dans le dépôt. Voir [le rapport QA](qa/issue-26.md).

TODO(agent) : normaliser unités/viewBox avec une migration explicite, gérer les
ressources référencées et remplacer l'échantillonnage historique des Bézier et
ellipses par une tolérance mesurée dans le repère final.
