# Mémoire de maintenance FabLabCAM

## Correctif SVG #26 — non publié

- `svgio` sépare parcours XML, grammaire `svgtypes`, transformations et export.
  Composer parent × local avant l'inversion Y ; exclure les sous-arbres de ressources.
- Préserver les arcs sous similitude et le sens sous miroir ; aplatir les arcs sous
  matrice non uniforme. Ne pas effacer le bulge du segment fermant un sous-chemin.
- Les erreurs de grammaire annulent l'import ; effets/références non supportés
  restent signalés. La convention historique des unités n'est pas modifiée.
- [Architecture et limites](svg-import.md), [QA](qa/issue-26.md). Le logo utilisateur
  reste hors dépôt ; utiliser les cas synthétiques pour la CI.
- Garder #26 ouverte jusqu'à la distribution vérifiée, suivant `releasing.md`.

## Version 0.7.0 : assistant natif, issue #20

- Architecture et contrats : [ADR 0005](adr/0005-assistant.md), [assistant](assistant.md),
  [QA](qa/0.7.0.md). Domaines dans `fablabcam-studio::assistant`, egui dans
  `fablabcam-view::assistant_ui`. Conserver les sept crates et les moteurs métier.
- Ask interdit les outils modificatifs ; Plan prépare sans appliquer ni exécuter
  Rhai ; Agent demande une validation par lot. Export séparé, destination native.
- Ne jamais appliquer un résultat obsolète : empreintes document/révision/contenu,
  page/sélection/filtres et identité fournisseur/modèle. Éditer un plan invalide
  son aperçu. Un lot dessin/script/meuble possède une annulation commune.
- QA UI : distinguer longueur totale/simulée ; comparer aussi le plan affiché
  au plan préparé avant installation. Interdire son édition pendant le worker.
- `PreparedProject` associe projet et caches calculés hors rendu ; ne pas les
  désynchroniser. Le runner Rhai manuel partage les limites et tourne en worker.
- Adaptateurs genai explicitement fixés ; continuer avec le protocole natif de
  résultat d’outil, y compris ses métadonnées de raisonnement non affichées.
  Ne pas déduire le fournisseur du nom de modèle ni rejouer un lot automatiquement.
- Historique sans clé ; coffre lié à l’endpoint ; variable seulement pour endpoint
  officiel. `.env` développeur ignoré, jamais chargé automatiquement par le Studio.
- Mistral `mistral-small-latest` testé réellement : caisson 7 panneaux, séparation
  8 panneaux, édition/exécution Rhai, outillage, simulation partielle, retour outil.
  Les trois autres services ont seulement des tests de protocole locaux à ce stade.
  Conserver #20 ouverte jusqu’à qualification réelle et compléter la QA.
- Rhai : arrêt coopératif, pas de quota global de mémoire ni préemption des appels
  natifs. Aucun shell, réseau arbitraire ou accès machine offert au modèle.
- TODO(agent) : renforcer la qualification des services réels et l’isolation des
  calculs lourds ; maintenir les fixtures SSE/NDJSON quand les API évoluent.

## Version 0.6.1 : validation des identifiants Rhai, issue #19

- Correction commune CLI/Studio dans `script::ids` : convertir les entiers par
  `u32::try_from`, jamais par `as`. `4294967297` et `-4294967295` visaient la
  forme 1 dans 0.6.0. Refuser également les membres hors plage d’un tableau.
- Plage numérique `0..=4294967295` ; l’existence de la forme est vérifiée dans
  le moteur métier. Les entiers seuls et tableaux gardent la même API ; les
  flottants, chaînes et booléens ne sont pas des identifiants.
- Régressions dans les deux fichiers `tests/script_ids.rs` des crates Studio et
  CLI : dix actions, dépassements et extrêmes, tableaux mixtes, bornes, projet
  et historique préservés, sortie absente/existante/identique à l’entrée.
- Les tests de régression ont échoué sur 0.6.0 avant de réussir avec le correctif.
  Préserver le journal d’erreur avec valeur et ligne ; ne pas accepter une
  conversion modulo pour tolérer un script incorrect.
- Références : [QA](qa/0.6.1.md), [issue #19](https://github.com/FabLabCAM/core/issues/19),
  [release](https://github.com/FabLabCAM/core/releases/tag/v0.6.1) et
  [guide Rhai](https://fablabcam.github.io/docs/reference/rhai/). Suivre
  `releasing.md` et vérifier la publication avant clôture.
- L’éditeur Rhai expose le dessin et les opérations CAO/FAO, pas la composition
  des meubles. Son exemple prérempli crée des formes sans opération d’usinage.

## Livraison 0.6.0 confirmée

- Release 0.6.0 publique : douze paquets et manifeste SHA-256 contrôlés.
  PR core #18 et docs #7 intégrées ; guide Pages, six captures et dix exemples
  vérifiés après publication ; issue #17 clôturée avec les preuves de validation.

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
