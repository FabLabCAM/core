# Assistant natif (0.7.0)

Voir l'[ADR](adr/0005-assistant.md) et le [guide utilisateur](https://fablabcam.github.io/docs/guide/assistant/).
Le panneau egui reste disponible dans CAO, 3D, Simulation, Scripts, Outils,
Profils machine et Meubles. Il utilise les moteurs existants, sans WebView.

## Contrat des modes

- Ask : texte uniquement, aucun outil modificatif transmis ou accepté.
- Plan : proposition typée et aperçu sur copie ; aucune application et aucune
  exécution Rhai. Passer explicitement en Agent pour appliquer un aperçu.
- Agent : préparer, relire et appliquer chaque lot. Le résultat est renvoyé au
  modèle avec le contexte actualisé. Chaque nouvelle proposition exige sa propre
  validation ; pas de boucle exécutant des actions sans accord.

Le lot est lié au document, à sa révision, au script, au meuble, à la page,
à la sélection et aux filtres de contexte. Un changement impose un nouvel aperçu.
Annuler/rétablir le dernier lot restaure ensemble dessin, script et meuble.
Une édition intermédiaire invalide cette annulation globale ; les historiques
natifs restent disponibles. Les caches de parcours sont préparés hors rendu.

## Catalogue typé

`Action` dans `assistant/plan.rs` définit le schéma transmis au modèle :
rayon des cercles, déplacement, remplacement/exécution Rhai séparés, caisson,
dimensions de meuble, division de case vide, bac, assemblage existant,
création/duplication/modification d'outil, création/modification d'opération,
préparation d'export CB/SVG/DXF/G-code/JSON meuble/ZIP.
Les paramètres numériques et textuels sont éditables avant recalcul. Une étape
peut être retirée ; l’édition est verrouillée pendant une préparation. Le pont
vérifie aussi que le plan affiché correspond exactement à l’aperçu.
Les identifiants de formes sont affichés en lecture seule dans le plan.

Un export est un lot distinct et ouvre un choix natif de destination. Il ne
prouve pas qu'un fichier a été écrit. Les validations FAO et machine existantes
restent obligatoires. Aucun shell, accès arbitraire au disque ou commande CNC
n'est offert au modèle. Les dossiers de fabrication sont calculés dans un worker.

## Connexions et données

Choisir le fournisseur, son identifiant exact de modèle et son endpoint ; tester
la connexion et la réception d'un appel d'outil avant Agent. Un modèle qui répond
au chat n'est pas forcément capable d'appeler les outils.

| Fournisseur | Protocole | Variable de clé |
|---|---|---|
| OpenAI | Responses | `OPENAI_API_KEY` |
| Claude | Messages | `ANTHROPIC_API_KEY` |
| Mistral | Chat | `MISTRAL_API_KEY` |
| Ollama | Chat natif | `OLLAMA_API_KEY` (facultative) |

Clé de session, variable d'environnement sur endpoint officiel, ou coffre système
lié au fournisseur et à l'endpoint. Linux utilise Secret Service ; s'il n'est pas
disponible, utiliser l'environnement ou la clé de session. Ollama local peut
fonctionner sans clé. HTTPS vérifié, sauf loopback et service Ollama HTTP ; pas
de redirection ni repli vers un autre fournisseur.

Les filtres choisissent dessin/sélection, script, meuble, outils/opérations,
diagnostics/simulation/profil et historique. Script et meuble sont exclus par
défaut. Épingler conserve l'instantané ; consulter « Dernier contexte envoyé ».
L'historique local contient la conversation et sa provenance, jamais les clés.
`FABLABCAM_ASSISTANT_STORE` isole le dossier contenant `conversation.json`.
Le fichier est écrit atomiquement, limité à 4 Mo et 100 messages ; une version
illisible n'est pas écrasée. Effacer la conversation supprime les échanges locaux,
pas les données déjà reçues par un fournisseur. Ne pas saisir de secret dans une
question ; la suppression de la clé active dans les réponses n'est pas un filtre
universel de données privées.

`.env.example` sert aux essais développeur. Le Studio **ne charge pas** `.env`
automatiquement ; transmettre les variables au processus ou utiliser Connexion.
Ne jamais versionner `.env` (déjà ignoré), ni copier ses valeurs dans un ticket.

## Bornes et observabilité

Contexte 128 Ko, requête 256 Ko, réponse 1 Mo ; 12 étapes, 10 000 formes et
200 opérations par lot. Délai réseau 5–300 s, génération 128–16384 tokens,
1–8 tours Agent. Arrêter annule le transport ou abandonne le résultat du worker.
Une réponse tronquée ou un outil inconnu invalide la proposition.

Rhai : code 64 Ko, 5 millions d'opérations, délai coopératif de 10 s, profondeur
64, pile 32, 256 variables, chaînes 64 Ko, tableaux 10 000, maps 1 000,
texte vectorisé 1 000 caractères par appel, 1 000 outils, 200 opérations.
Imports désactivés. Les appels géométriques natifs ne sont pas interrompus en
plein calcul ; les limites de conteneurs ne constituent pas un quota global de
mémoire du processus. TODO(agent) : isolation de processus si des scripts hostiles
ou des calculs géométriques très coûteux doivent être acceptés.

Journaux JSON : composant, requête, fournisseur, modèle, mode, actions, statut et durée, sans
contenu du projet ni secret. Le détail UI montre modèle, tour et empreinte de
contexte. La simulation indique sa progression réelle, révision, brut et gouges ;
une simulation partielle ne devient jamais une validation machine.

## Tests

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
# Avec .env local rempli ; effectue de vraies requêtes facturables :
FABLABCAM_TEST_PROVIDER=Mistral cargo test --locked -p fablabcam-studio \
  --test assistant_live -- --ignored --nocapture
```

Les tests de protocole utilisent des serveurs locaux pour les quatre fournisseurs.
Le test réel charge `.env` sans en afficher les valeurs. Voir la [QA](qa/0.7.0.md)
pour distinguer les fournisseurs réellement essayés des simulations de protocole.
