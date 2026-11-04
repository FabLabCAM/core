# Validation du correctif SVG — issue #26

Date : 5 novembre 2026. Base : `cc77380822f3ec42061d5a94c625537268b8bd29`
(0.7.0), branche `fix/26-svg-import`. Correctif non publié.

## Cas utilisateur

Le fichier `Logo_simple_noir.svg` joint à
[#26](https://github.com/FabLabCAM/core/issues/26) a été testé sans modification.
Empreinte SHA-256 :
`ea677a3fd0c39e57a58f5396175f321a216b3f789fecffc689be6ae84df8e98a`.
Le fichier et ses métadonnées ne sont pas ajoutés au dépôt public.

| Contrôle | Avant | Après |
| --- | --- | --- |
| Entités importées | 23 | 10 contours fermés |
| Encombrement X × Y | 3357,189321 × 3457,428410 | 39,500860 × 39,499991 |
| Position des éléments | Dents, lettres et petits traits dispersés | Logo assemblé et proportions conservées |
| Messages SVG | Transformations de groupes ignorées | Aucun avertissement d'import |

Les valeurs sont en unités du document, ici des millimètres : le `viewBox`
et les dimensions physiques du fichier utilisent la même échelle. L'écart avec
les dimensions déclarées (39,500950 × 39,500072 mm) reste inférieur à 0,0001 mm.
Cette mesure décrit ce fichier ; elle ne garantit pas une conversion générale
des unités SVG ni une tolérance universelle pour les courbes.

## Contrôles exécutés

- Les neuf premiers tests de régression ont échoué sur l'ancien importeur
  (0 réussi, 9 échoués), puis réussi après correction.
- La suite finale comporte 14 tests SVG dans Studio et un test de conversion CLI.
  Elle contrôle aussi les matrices imbriquées, miroirs, transformations non
  uniformes, sous-chemins, fermetures d'arcs, ressources inutilisées, erreurs
  explicites et conservation du fichier de sortie en cas d'échec.
- `cargo test --locked --workspace` : **402 réussis, 0 échec, 1 ignoré**.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` : réussi.
  Cargo signale séparément l'avertissement de compatibilité future préexistant
  de `block 0.1.6`.
- `cargo build --locked -p fablabcam-view --bin fablabcam-studio-ui` : réussi.
- Conversion réelle du SVG en SVG et CB : réussie, 10 formes, aucun avertissement.
- Comparaison visuelle dans Chrome du rendu SVG original et des contours
  réexportés : engrenage, hexagones, lettres et symbole intérieur alignés.
- Application native corrigée sur macOS 26.2 ARM64 : import réussi par
  **Fichiers par chemin (secours) → Importer**, cadrage automatique correct,
  panneau affichant **10 forme(s)** et message « importé » sans élément ignoré.
  Le test utilise une application distincte avec des préférences isolées et une
  copie du SVG vérifiée octet pour octet. L'accès direct à Téléchargements et
  le dialogue natif ont bloqué dans cette application de QA ; le menu Ouvrir
  n'est donc pas validé par ce contrôle.
- Guide associé : `mkdocs build --strict` réussi, page Fichiers et échanges
  inspectée dans le navigateur. La documentation indique que le correctif
  n'est pas encore dans la version publiée.

Les captures `comparison.png` et `studio-import.png` sont conservées localement
hors Git, car elles montrent le projet utilisateur. Aucun usinage machine,
paquet distribué ni autre système d'exploitation n'a été qualifié ici.

## Livraison

Les PR core/docs référencent #26. L'issue reste ouverte jusqu'à la validation
de distribution et la publication prévues dans [releasing.md](../releasing.md).
Les limites du lecteur sont détaillées dans [svg-import.md](../svg-import.md).
