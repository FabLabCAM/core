# FabLabCAM

CAO/FAO libre en Rust pour les FabLabs : dessin 2D, préparation d'usinage 2,5D,
G-code, visualisation des trajectoires et simulation de matière.

Dépôt : https://github.com/FabLabCAM/core · Licence : [Apache-2.0](LICENSE).

[Documentation du produit](https://fablabcam.github.io/docs/) ·
[Téléchargements](https://github.com/FabLabCAM/core/releases)

## Architecture

| Crate | Responsabilité |
|---|---|
| `fablabcam-geom` | Polylignes, arcs, décalages et opérations géométriques |
| `fablabcam-model` | Documents, calques, entités et format XML `.cb` |
| `fablabcam-cam` | Profils, poches avec îlots, perçage et post-processeurs |
| `fablabcam-backplot` | Lecture G-code, scène 3D et exports SVG/HTML |
| `fablabcam-studio` | Édition, outils, opérations, simulation et scripts Rhai |
| `fablabcam-cli` | Exécutable `fablabcam` |
| `fablabcam-view` | Exécutables `fablabcam-view` et `fablabcam-studio-ui` (egui) |

Les moteurs de géométrie et d'usinage restent indépendants de l'interface.
Voir [les notes d'architecture](docs/architecture.md).

## Construire et essayer

Installer Rust stable avec Cargo, puis :

```sh
git clone https://github.com/FabLabCAM/core.git
cd core
cargo build --locked --workspace
cargo run --locked -p fablabcam-cli --bin fablabcam -- help
cargo run --locked -p fablabcam-view --bin fablabcam-studio-ui -- --demo
```

Sous Ubuntu, le viewer nécessite notamment `pkg-config`, `libx11-dev`,
`libxcursor-dev`, `libxi-dev`, `libxrandr-dev`, `libxkbcommon-dev`, `libwayland-dev`
et `libgl1-mesa-dev`. La CI décrit l'installation utilisée pour les tests.

Exemples de ligne de commande :

```sh
cargo run --locked -p fablabcam-cli -- info crates/fablabcam-cli/tests/fixtures/plaque.cb
cargo run --locked -p fablabcam-cli -- styles tests/fixtures/styles-demo.xml
cargo run --locked -p fablabcam-cli -- gcode crates/fablabcam-cli/tests/fixtures/plaque.cb --op profile-outside --layer Contour --tool 6 --depth -3 --step 1 -o plaque.nc
cargo run --locked -p fablabcam-cli -- plot plaque.nc -o trace.html
cargo run --locked -p fablabcam-cli -- web demo -o studio.html
```

Les valeurs des fixtures servent aux tests ; adapter les réglages à la machine,
au matériau et à l'outil avant tout usinage réel.

## Tests

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

La CI exécute ces vérifications sous Linux, Windows et macOS. Le fichier
`Cargo.lock` est conservé pour construire avec les versions de dépendances testées.

## Formats et distribution

- Dessins : `.cb` (XML), DXF et SVG ; programmes : G-code.
- Le format `.cb` du Studio ajoute des outils et opérations propres à FabLabCAM.
  Voir [les limites d'interopérabilité](docs/file-formats.md).
- [Construction des applications](docs/building.md) pour macOS, Windows et Linux.
- [Contribuer](CONTRIBUTING.md) et [signaler une vulnérabilité](SECURITY.md).

Le code du projet est distribué sous Apache-2.0. Les dépendances conservent leurs
licences ; voir [NOTICE](NOTICE). Les données de test synthétiques sont incluses
sous la licence du projet. Aucune bibliothèque propriétaire n'est distribuée.
