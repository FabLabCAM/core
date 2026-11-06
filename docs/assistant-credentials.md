# Clés de session et signature macOS — en préparation

Non inclus dans 0.8.0. Le cache est privé au panneau et ne fait pas partie de la
session sérialisée. Une lecture réussie du coffre/environnement est conservée
pour une seule identité fournisseur/adresse/modèle, y compris une absence de clé.
Les erreurs restent réessayables. Aucun nouvel accès au coffre pour un envoi ou
un retour d’outil sur cette connexion. Une clé saisie explicitement reste prioritaire.

Le changement de fournisseur, d’adresse ou de modèle, l’édition/suppression de clé,
et « Oublier la clé de cette session » invalident le cache. Fermer l’application
le détruit ; replier le panneau ne le détruit pas. Oublier ne supprime pas le coffre.
`Zeroizing` efface l’allocation du cache à sa destruction ; les copies nécessaires
au client HTTP ne permettent pas de promettre l’absence de secrets en mémoire ou
dans un core dump. Aucun secret n’est écrit dans la conversation ou les journaux.

## Signature de distribution

Les distributions actuelles utilisent une signature ad hoc ; ce cache évite les
lectures répétées pendant une session, mais ne rend pas cette signature stable.
Le packager accepte une identité **Developer ID Application** préinstallée :

```sh
export FABLABCAM_CODESIGN_IDENTITY='<empreinte SHA-1 du certificat>'
export FABLABCAM_CODESIGN_TEAM='<Team ID Apple, 10 caractères>'
export FABLABCAM_REQUIRE_DEVELOPER_ID=1
python3 packaging/package.py --target aarch64-apple-darwin --platform macos --arch arm64
```

Les trois exécutables sont signés de l’intérieur vers l’extérieur, puis le bundle,
avec identifiants stables, hardened runtime et timestamp. Le packager contrôle
l’autorité Developer ID, l’équipe et les identifiants, puis vérifie les signatures.
Une identité demandée mais absente/invalide ne retombe jamais silencieusement en
ad hoc. Aucun certificat ni mot de passe ne doit être commité.

Sans configuration, les builds de PR restent ad hoc, sans accès à un secret.
Le workflow de paquets ne provisionne pas encore de certificat : son activation
sur un runner de release de confiance reste à faire. La notarisation est distincte
et n’est pas annoncée comme réalisée. Les autorisations des anciens paquets ad hoc
peuvent demander une nouvelle confirmation lors du premier passage à Developer ID.

Le 6 novembre 2026, le blocage de création a été résolu : Xcode utilisait une
équipe différente du portail Apple Developer. Le certificat Developer ID de
l’équipe autorisée est créé et installé avec sa clé privée dans le trousseau.
La copie temporaire en clair de la clé générée pour le CSR a été retirée après
vérification d’une signature réelle. Aucun certificat ou secret n’est dans Git.

Une copie QA du bundle 0.8.0 ARM64 a été signée par le helper de packaging :
les trois exécutables et le bundle passent `codesign --verify --deep --strict`,
l’autorité Developer ID, l’équipe, les identifiants, le runtime et le timestamp
sont vérifiés. Sa CLI exécute `version` avec succès. Ni l’application installée
ni les assets de release publiés n’ont été remplacés. Le certificat expire le
6 décembre 2027 ; prévoir son renouvellement avec la même équipe.

[Exigences de signature Apple](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements/)
· [Certificats Developer ID](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/).

## Vérifications

Tests hors réseau du cache : lecture unique, oubli, changement de connexion,
erreur, absence et nouvelle session. Tests Python de la politique de signature :
refus de repli, identifiants stables, ordre et rejet de mauvaise équipe/autorité.
La signature réelle est maintenant vérifiée comme décrit ci-dessus. La
conservation de l’autorisation du trousseau entre deux versions distribuées
reste à qualifier, ainsi que la notarisation et le provisionnement CI.

Validation locale : 440 tests workspace réussis, un test réseau ignoré ; Clippy
strict réussi. Trois tests Python réussis et signature/vérification ad hoc réelle
d’un bundle synthétique réussies. Guide : MkDocs strict, 37 pages/liens/ancres et
rendu vérifiés. Aucun appel cloud ni lecture de clé API réelle dans ces essais.
