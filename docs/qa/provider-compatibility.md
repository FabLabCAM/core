# Qualification des fournisseurs — correctif 0.7.2

Essais synthétiques du 5 novembre 2026, sur macOS Apple Silicon. Les clés sont
lues depuis le fichier développeur ignoré ; elles ne sont ni affichées ni incluses
aux projets ou aux rapports. Les paquets 0.7.0/0.7.1 publiés restent inchangés.
Les nombres ci-dessous concernent la campagne du 5 novembre, avant intégration
des corrections SVG/Mach3 ; voir [QA 0.7.2](0.7.2.md) pour la validation intégrée.

## Causes et corrections

- GPT-4.1 mini : HTTP 400 `unsupported_parameter` sur `reasoning.summary`, alors
  qu’une demande simple réussit. genai 0.7.3 transforme la capture de raisonnement
  en demande de résumé. Désactiver cette capture pour GPT-4.1/GPT-4o ; garder les
  signatures chiffrées des modèles de raisonnement et la continuation native.
- Ollama 0.34.4 : réserver la génération à la réponse et aux outils avec
  `think:false`, température 0 en Plan/Agent et enveloppe `submit_plan` explicitée
  (les types d’actions ne sont pas des noms de fonctions). Une réponse finale vide reste refusée, avec un diagnostic
  indiquant les capacités du modèle. Aucun appel n’est reconstruit depuis le
  texte libre, aucune application ni relance automatique n’est déclenchée.
- Ollama tronque le contexte par défaut de 4 096 tokens après création du meuble :
  les journaux montrent 5 381 tokens d’entrée ramenés à 2 050. Cela supprime une
  partie des consignes/catalogue. Qwen3 1.7B et Qwen2.5 3B passent le caisson et
  le retour outil, puis ne fournissent plus de plan pour la division. Qwen3 4B
  rencontre une erreur de flux lors de cette étape (324,44 s pour le parcours).
  Ces échecs avec contexte tronqué ne démontrent pas une incapacité intrinsèque
  des modèles.
- genai 0.7.3 ignore `extra_body` dans son adaptateur Ollama : un test réseau local
  l’a vérifié. L’essai d’injection de `num_ctx` a été retiré. Configurer le contexte
  dans un modèle Ollama dédié via un Modelfile ; l’interface rappelle ce prérequis.
  Un contexte plus grand consomme davantage de mémoire. Le test porte sur 16 384
  tokens ; cela ne garantit pas que toutes les conversations longues tiennent.
- Avec un contexte de 16 384 tokens, sans troncature observée : Qwen2.5 3B
  (`fablabcam-qwen2.5:3b`) passe le caisson et le retour outil, puis renvoie une
  réponse vide à la division (42,65 s). Qwen3 1.7B (`fablabcam-qwen3:1.7b`)
  passe les deux premières étapes mais répond sans plan à la division (22,60 s).
  Ces deux modèles ne sont donc pas qualifiés pour le parcours Agent complet.
- Diagnostic de la réponse Qwen3 1.7B avec 16k : le modèle écrit un objet
  `divide_cell` comme une fonction autonome, au lieu d’un appel natif à
  `submit_plan`. Aucun appel n’est reconstruit depuis ce texte. Énumérer tous
  les noms d’actions dans la consigne a fait régresser le premier plan : cet
  essai a été retiré. Le correctif conserve la consigne ayant permis le caisson.
- Avant la clarification de l’enveloppe, `think:false`, un contexte agrandi et
  le développement des références du schéma ne suffisaient pas à obtenir le
  premier plan. Le schéma original est conservé ; les corrections sont combinées.

## Vérifications

- 389 tests workspace réussis, 0 échec, 1 test réel ignoré par défaut. Le test
  réel est exécuté séparément et explicitement ; il n’est pas compté comme réussi
  dans la suite hors réseau. Clippy workspace/all-targets avec `-D warnings` réussi.
- Régressions réseau locales : absence de `reasoning`/`include` pour GPT-4.1,
  identifiant daté et GPT-4o ; conservation pour GPT-5 ; `think:false` et plafond
  de génération Ollama, température 0 et consigne d’enveloppe limitée à Plan/Agent ; refus de réponse Ollama vide sans plan applicable.
- Parcours réel : création 600 × 400 × 800 mm, 7 panneaux ; retour outil natif ;
  division en 8 panneaux ; Plan sans exécution ; exécution Rhai en Agent ;
  création/duplication d’outil sans opération implicite ; simulation partielle.
- GPT-4.1 mini : parcours complet réussi en 11,28 s.
- GPT-5 mini : parcours complet réussi en 67,98 s (continuation incluse).
- Claude Haiku 4.5 : parcours complet réussi en 19,71 s.

## Reproduction

Configurer uniquement les variables nécessaires dans le fichier développeur
ignoré, puis sélectionner explicitement un fournisseur et un modèle :

```sh
FABLABCAM_TEST_PROVIDER=OpenAI OPENAI_MODEL=gpt-4.1-mini \
  cargo test --locked -p fablabcam-studio --test assistant_live -- --ignored --nocapture
```

Les appels cloud peuvent être facturés. Le test travaille sur un projet synthétique
isolé. Le test Ollama cible explicitement `http://127.0.0.1:11434` ; aucune machine
CNC n’est pilotée. Ces essais ne remplacent ni une revue graphique des trois OS,
ni une qualification avec Internet physiquement déconnecté. L’issue #20 reste ouverte.
