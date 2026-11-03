# Notes de version

## 0.4.0 — 2026-11-03

### Atelier des profils machine dans le Studio — [#11](https://github.com/FabLabCAM/core/issues/11)

- Espace **Profils machine · Atelier** : bibliothèque locale, édition, duplication,
  import/export JSON et récapitulatif des courses renseignées.
- Création guidée en quatre étapes : identité, axes et sécurité, broche et outils,
  vérification. Les limites inconnues restent absentes ; un profil peut être
  enregistré avec l'état « À configurer » pour être complété plus tard.
- Formulaires G-code et JSON complet pour conserver les paramètres avancés.
  Un JSON invalide n'altère pas les champs et empêche l'enregistrement/export.
- Confirmation des brouillons, import avec copie ou remplacement explicite,
  refus d'écrasement des exports et détection des modifications concurrentes.
- Protection des brouillons lors de la fermeture, y compris « Quitter » sur macOS.
- Guide illustré avec des captures réelles du Studio et suivi de l'issue.

Le schéma JSON reste en version 1 et le magasin est partagé avec la CLI.
Les états de configuration ne valident ni une machine physique ni un programme.
La validation, la conversion et les archives G-code restent accessibles en CLI/API.
Les profils ne sélectionnent pas automatiquement un post-processeur `.cbpp`.

## 0.3.0 — 2026-11-03

### Profils de machine et gestion du G-code — [#9](https://github.com/FabLabCAM/core/issues/9)

- Profils JSON versionnés : contrôleur, axes, courses, avances, broche, dialecte,
  formats, outils et contraintes atelier. Référence Mach3 Mill sourcée et exemple fictif.
- Nouvelle commande `fablabcam machine` : CRUD, analyse, validation, statistiques,
  conversion de formats/dialectes compatibles et archives par révision.
- Diagnostics ligne/colonne et statuts `valid`, `invalid`, `incomplete` ; limites
  XYZ avec extrema d'arcs, unités, modes, avances et broche contrôlés.
- Archives avec instantané du profil/rapport, auteur, date et SHA-256 ; écritures
  concurrentes sérialisées, intégrité vérifiée à la lecture.
- Conversion conservatrice : préserve les valeurs, explicite les modes, adapte
  commentaires/numérotation et alias G70/G71 ; liste les obstacles sans créer de sortie.
- `fablabcam version` / `--version`, profils inclus dans les distributions,
  guide de machine, suivi des issues et procédure de mise à jour documentaire.

La fonctionnalité est accessible en CLI et API Rust. Les cycles, macros,
compensations actives et transformations non interprétés restent explicitement
incomplets. La conversion n'étend pas ces constructions. Le Studio graphique,
le support `.cb` et la licence Apache-2.0 sont conservés. Compilation : Rust 1.89+
(verrouillage des archives), distributions construites avec Rust 1.99.1.

## 0.2.0 — 2026-11-02

Première release publique : Studio 2D/2.5D, outils et opérations, simulation,
scripts Rhai, post-processeurs, CLI et viewer. Installateurs et archives pour
macOS, Windows et Linux, en x64 et ARM64 ; notices tierces et SHA-256.

[Release 0.2.0](https://github.com/FabLabCAM/core/releases/tag/v0.2.0)
