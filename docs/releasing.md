# Publier une version et maintenir le guide

Chaque issue livrée doit apparaître dans `CHANGELOG.md`, dans les notes GitHub et
sur la page de versions du [dépôt docs](https://github.com/FabLabCAM/docs).
Ne fermer une issue qu'après vérification de ses critères ; préciser les limites
et relier la PR, la version et le guide. Une PR liée par `Refs #n` conserve l'issue
ouverte pendant la validation de distribution ; la fermeture suit la publication.

1. Implémenter et tester sur une branche. Documenter les décisions dans `docs/`.
2. Modifier `workspace.package.version`, actualiser `Cargo.lock` et le changelog.
3. Mettre à jour les exemples et les pages utilisateur, l'index des fonctionnalités,
   le suivi des issues et `extra.product_version` dans `FabLabCAM/docs`. Conserver
   les anciennes notes de version et la provenance réelle des captures.
4. Exécuter tests workspace et Clippy. Exécuter les exemples du guide avec ce
   binaire. Construire MkDocs avec `--strict`, vérifier les liens et le rendu.
5. Scanner le contenu préparé, noms de fichiers et médias pour vérifier identité,
   absence de secrets et données personnelles. Ne pas importer l'ancien projet.
6. Ouvrir les PR core/docs ; laisser les contrôles requis réussir. Construire les
   six cibles natives avec `Desktop packages`. Vérifier licences et sommes,
   architectures, installation, CLI et échantillon d'interface réelle.
7. Intégrer core. Vérifier que l'arbre construit est identique à l'arbre intégré,
   sinon reconstruire. Créer le tag annoté et une release brouillon.
8. Joindre les douze paquets et `SHA256SUMS.txt`, vérifier les empreintes retournées
   par GitHub, puis publier. Le workflow de paquets n'a pas ce droit d'écriture.
9. Intégrer docs, attendre GitHub Pages, vérifier les liens de téléchargement.
   Mettre à jour/fermer les issues livrées avec les liens de release et du guide.

Le suivi est maintenu dans Git, sans publication automatique de fonctionnalités
non livrées. Les problèmes connus restent des issues ouvertes ; les tests ne
remplacent pas une qualification de machine réelle.
