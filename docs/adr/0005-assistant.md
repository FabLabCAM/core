# ADR 0005 — Assistant natif et actions préparées

Date : 2026-11-04. Issue : #20. Statut : retenu pour 0.7.0.

## Architecture

Le panneau Atelier est un `egui::SidePanel` commun aux pages du Studio.
`fablabcam-view` présente la conversation, les réglages et les différences ;
`fablabcam-studio::assistant` possède les contrats, le contexte, les limites,
les actions et les adaptateurs. Aucune crate supplémentaire dans le workspace.
Les transports et les calculs préparatoires tournent dans un worker ; le fil
egui reçoit des événements et applique seulement un résultat entièrement préparé.

Ask n'expose aucune action modificatrice. Plan prépare une proposition sans
modifier le document. Agent présente un lot typé, ses paramètres et ses effets
avant application. Le plan peut être édité puis recalculé ; changer le plan
invalide l'aperçu. La validation porte sur le contenu exact de cet aperçu.
Un changement de document, de sélection ou de révision invalide son application.
Les commandes existantes restent la référence pour validation et annulation.

## Composants et choix de bibliothèques

- `genai` 0.7.3 (MIT/Apache-2.0) : adaptateurs explicites OpenAI Responses,
  Anthropic Messages, Mistral Chat et Ollama. Pas de sélection automatique de
  fournisseur selon le nom du modèle, ni de repli cloud. Les modèles sont saisis
  par l'utilisateur. Comparé à `rig-core` 0.46 (MSRV 1.97.1, orchestration plus
  étendue), et aux adaptateurs HTTP maison avec reqwest : genai fournit déjà
  streaming, appels d'outils et restitution des résultats. FabLabCAM conserve
  sa propre politique métier, indépendante de cette bibliothèque.
- `reqwest` : client avec délais et redirections interdites ; TLS vérifié.
- `tokio` : annulation des futures et délais bornés dans le worker, sans runtime
  dans la boucle de rendu. Les réponses tardives ne peuvent appliquer d'actions.
- `schemars` : schémas JSON dérivés des arguments Rust, puis validation métier.
- `keyring` 3.6 : coffre système (Keychain, Windows Credential Manager,
  Secret Service Linux). Les clés ne sont jamais sérialisées avec l'historique,
  les préférences, les projets ou les journaux. Variables d'environnement
  possibles pour les essais ; aucun fichier personnel n'est importé au dépôt.

## Contexte et historique

Un instantané décrit page, projet, sélection, script, meuble, outillage et
diagnostics effectivement disponibles. L'utilisateur limite le contexte et peut
l'épingler. Chaque échange conserve sa provenance. L'historique local effaçable
est distinct des fichiers de fabrication. Un document ouvert conserve son
identité de session, même si son contenu est identique à un autre document.

## Actions et limites

Les actions typées réutilisent dessin, Rhai, composition, outils et opérations.
Une modification du texte Rhai n'exécute jamais le script. Un lot préparé est
atomique ; erreur ou annulation abandonne sa copie. Les actions d'export restent
distinctes des modifications annulables et passent par les contrôles métier et
une destination choisie explicitement. Pas de shell, réseau arbitraire ou pilotage
de machine exposés au modèle. La simulation 2.5D ne qualifie pas une CNC.

Les plafonds couvrent durée, taille du contexte/réponse, nombre d'actions et
opérations/mémoire Rhai. Le transport ne rejoue pas automatiquement un lot.
Les journaux structurés décrivent identifiant de requête, fournisseur, statut et
durée ; ni secrets ni contenu complet des documents. Le mode debug reste borné.

## Vérification

Tests déterministes avec serveurs HTTP locaux pour les quatre protocoles,
permissions des modes, annulation, réponse partielle, erreur fournisseur,
propositions obsolètes, validation et rollback métier. Essais réels facultatifs
en CI, explicitement rapportés dans `docs/qa/0.7.0.md`. Ne pas confondre un essai
simulé avec une connexion réelle. Tester également la GUI et les six paquets
natifs conformément à `releasing.md`.

Références consultées :
- https://developers.openai.com/api/docs/guides/function-calling
- https://platform.claude.com/docs/en/build-with-claude/streaming
- https://docs.mistral.ai/api
- https://docs.ollama.com/api/chat
- https://docs.rs/genai/0.7.3/genai/
- https://docs.rs/keyring/3.6.3/keyring/

TODO(agent) : conserver des fixtures de protocole lorsque les fournisseurs
évoluent ; n'ajouter un outil qu'avec des tests d'autorisation et d'annulation.
