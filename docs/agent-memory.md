# Mémoire de maintenance FabLabCAM

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
- Tests : dimensions finies, variantes, non-chevauchement, placement complet,
  sérialisation, imports invalides, copie, concurrence, historique, ZIP/CB/PDF,
  contrôle G-code et parcours CLI. Tests workspace et Clippy obligatoires.
- Guide utilisateur : https://fablabcam.github.io/docs/guide/meubles/.
  Distribution : https://github.com/FabLabCAM/core/releases/tag/v0.5.0.
  La publication effective doit être vérifiée avant de fermer l'issue.
- Régression découverte en QA : regrouper les frappes d'un champ en une seule
  étape d'annulation ; après sauvegarde, recharger le modèle courant avant de
  créer un meuble pour prendre la nouvelle révision.

## Livraison

Suivre [releasing.md](releasing.md) : PR core/docs, tests et Clippy, six cibles
natives et douze paquets, licences, empreintes, test d'installation et interface
réelle, tag annoté, release, puis publication du guide et clôture de l'issue.
Les captures du guide doivent provenir du binaire livré, avec données synthétiques.
Les captures du module sont référencées dans le guide et son index. Conserver
les provenances historiques. Pour une prochaine livraison, suivre ce processus
sans modifier implicitement les projets créés avec une ancienne révision.

## Améliorations futures

- TODO(agent) : optimiser le placement sans changer les repères ni accepter de
  résultat partiel ; ajouter des références indépendantes avant toute stratégie.
- TODO(agent) : distinguer de nouveaux types de construction pour les portes,
  tiroirs et assemblages usinés ; prévoir une migration explicite de schéma.
- TODO(agent) : améliorer l'accessibilité native des formulaires egui.
