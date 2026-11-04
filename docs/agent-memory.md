# Mémoire de maintenance FabLabCAM

## Version 0.6.0 : éditeur de composition et assemblages, issue #17

- Branche de livraison : `feat/17-furniture-composer`. Références :
  [QA sources](qa/0.6.0.md), [release](https://github.com/FabLabCAM/core/releases/tag/v0.6.0)
  et guide ci-dessous. Vérifier la publication et les paquets avant de fermer #17.
- Composer dans l'étape Meuble : arbre/cases sélectionnables, divisions, positions
  fixes/égales, grille, bacs ouverts, copie avec liaisons et historique.
- Épaisseurs individuelles et profondeur de séparation ; angles configurables
  par panneau continu. Dimensions utiles recalculées sans double paroi.
- Schéma 2 lit le schéma 1 sans modifier l'original ; sauvegarde explicite et
  copie de secours du catalogue ancien. Enregistrer comme modèle garde la composition.
- Huit familles initiales, variantes orthogonales décrites dans le guide ; les huit
  autres restent désactivées. Un type non pris en charge ne devient jamais bord à bord.
- Jeu TOTAL, faces et profondeurs conservés dans ZIP/CB ; dalles séparées par
  épaisseur ; verso miroir X autour de Y avant contours recto. Aucun G-code automatique.
- Retours de revue à préserver : dégager aussi la racine mâle des tenons ; dépasser
  le brut avec les poches de réduction ; interdire qu'un îlot satisfasse une poche
  obligatoire ; remapper les assemblages copiés et les changements d'orientation.
- Historique : grouper la frappe par champ, pas la frappe avec le bouton suivant.
  Garder le dernier aperçu pendant une saisie numérique invalide, sans l’utiliser
  pour exporter. Les grandes grilles masquent les étiquettes 3D superposées.
- Ne nettoyer les liaisons orphelines qu'après édition explicite, jamais à l'import.
  Les paramètres des pièces de bac viennent de Bin, pas de panel_thickness.
- Voir [architecture](adr/0004-furniture-composer.md), [formats](furniture.md) et
  [guide](https://fablabcam.github.io/docs/guide/composer-meubles/).

## Version 0.5.0 : module Meubles, issue #14

- Demande validée : module natif simple pour fabriquer du mobilier de FabLab,
  avec assistant en quatre étapes, bibliothèque de modèles et bibliothèque de
  projets ; création, édition, duplication et import/export des deux types.
- Référence : caisson extérieur 600 × 600 × 800 mm, OSB 20 mm, une tablette,
  dos rapporté inclus dans la profondeur, dalles 1 500 × 800 mm. Six panneaux
  et trois dalles pour les réglages de l'issue. Ne pas confondre ces valeurs
  d'exemple avec les limites physiques d'une CNC qualifiée.
- Maquettes validées et dix captures attachées à
  [l'issue #14](https://github.com/FabLabCAM/core/issues/14).
- Code métier : `crates/fablabcam-studio/src/furniture/` ; interface egui :
  `crates/fablabcam-view/src/furniture_ui/` ; CLI : `furniture help`.
- Modèles `.fcm-template.json`, projets `.fcm.json`, schéma 1. Les projets
  embarquent modèle/révision et profil machine. Aucun résultat dérivé n'est
  réutilisé après modification. Voir [ADR](adr/0003-furniture.md).
- La FAO conserve le profil dans l'extension XML `FurnitureMachine`. Les anciens
  `.cb` restent lisibles. L'export natif contrôlé refuse un rapport incomplet.
- Stockage isolable avec `FABLABCAM_FURNITURE_STORE` ; écritures atomiques sous
  verrou et comparaison de l'état chargé. Ne jamais remplacer un modèle à l'import.
- Références : [PR core #15](https://github.com/FabLabCAM/core/pull/15) et
  [PR docs #5](https://github.com/FabLabCAM/docs/pull/5).
- Tests : dimensions finies, variantes, non-chevauchement, placement complet,
  sérialisation, imports invalides, copie, concurrence, historique, ZIP/CB/PDF,
  contrôle G-code et parcours CLI. Tests workspace et Clippy obligatoires.
- Guide utilisateur : https://fablabcam.github.io/docs/guide/meubles/.
  Distribution : https://github.com/FabLabCAM/core/releases/tag/v0.5.0.
  La publication effective doit être vérifiée avant de fermer l'issue.
- Régression découverte en QA : regrouper les frappes d'un champ en une seule
  étape d'annulation ; après sauvegarde, recharger le modèle courant avant de
  créer un meuble pour prendre la nouvelle révision.
- Les noms d'outils apparaissent dans les commentaires G-code : conserver un
  nom initial ASCII pour le profil Mach3. Le test de bout en bout vérifie aussi
  qu'une configuration fictive complète peut exporter un parcours calculé.

## Livraison

Suivre [releasing.md](releasing.md) : PR core/docs, tests et Clippy, six cibles
natives et douze paquets, licences, empreintes, test d'installation et interface
réelle, tag annoté, release, puis publication du guide et clôture de l'issue.
Les captures du guide doivent provenir du binaire livré, avec données synthétiques.
Les captures du module sont référencées dans le guide et son index. Conserver
les provenances historiques. Pour une prochaine livraison, suivre ce processus
sans modifier implicitement les projets créés avec une ancienne révision.

## Vérification après publication du 4 novembre 2026

- Release 0.5.0 publique, douze paquets et SHA256SUMS vérifiés ; guide déployé
  et issue #14 close. Les PR initiales sont core #15 et docs #5.
- Le blocage dû au Mac verrouillé est levé. Les parcours graphiques du binaire
  du DMG Apple Silicon et la persistance après relance sont vérifiés. Les sept
  captures du guide proviennent désormais du binaire publié.
- Voir le [compte rendu et ses limites](qa/0.5.0.md), notamment la différence
  entre exports natifs/ZIP testés dans l'interface et dialogue système G-code.
- Pour les tests locaux, isoler stockage et préférences et utiliser un identifiant
  de bundle distinct lorsqu'une ancienne installation du Studio est déjà ouverte.
  Conserver le binaire livré inchangé et vérifier son empreinte.

## Améliorations futures

- TODO(agent) : optimiser le placement sans changer les repères ni accepter de
  résultat partiel ; ajouter des références indépendantes avant toute stratégie.
- TODO(agent) : portes, coulisses, encoche de préhension et familles d'assemblages
  non prises en charge ; prévoir une migration explicite si le schéma évolue.
- TODO(agent) : améliorer l'accessibilité native des formulaires egui.
