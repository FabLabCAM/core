# Contribuer

Créer une branche et ouvrir une pull request vers `main`. Décrire le problème,
le comportement obtenu et les vérifications effectuées. Les contributions au
projet sont proposées sous Apache-2.0.

Avant soumission :

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Documenter les modifications de formats et d'usinage. Utiliser des données de
test originales et de petits commits. Ne pas ajouter de secrets, de réglages
personnels, de compilations ou de fichiers tiers sans droits de redistribution.

Chaque changement utilisateur référence son issue dans `CHANGELOG.md` et met à
jour le guide `FabLabCAM/docs` (procédure, limites, exemples et index). Voir
[le processus de release](docs/releasing.md).
