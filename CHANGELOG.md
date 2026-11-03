# Notes de version

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
