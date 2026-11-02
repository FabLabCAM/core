# FabLabCAM/core

CAO/FAO en Rust, sous Apache-2.0. Dépôt : https://github.com/FabLabCAM/core.

- Conserver les sept crates `fablabcam-*` et séparer moteurs et interfaces.
- Documenter les choix, hypothèses et limites dans `docs/`.
- Préserver le support `.cb`, le schéma XML et les tests de géométrie.
- Le dépôt public ne doit contenir que l'identité FabLabCAM ; contrôler le texte,
  les noms de fichiers, les images et le contenu préparé avant chaque commit.
- Ne pas importer de code décompilé, de bibliothèques propriétaires, d'exécutables,
  de clés `.lic`, de secrets ni de fichiers de configuration personnels.
- Styles de test synthétiques : `tests/fixtures/styles-demo.xml`.
- Vérifications : `cargo test --locked --workspace` et
  `cargo clippy --locked --workspace --all-targets -- -D warnings`.
- Utiliser des commits ciblés et tenir la documentation à jour.
- Workflows : permissions minimales, actions épinglées par SHA, aucun secret
  accessible au code des pull requests externes.
- TODO(agent) : ajouter des cas de référence indépendants pour les poches,
  les perçages et les textes ; améliorer l'interopérabilité XML documentée.
