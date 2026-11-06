# Ollama : division d’une case — issue #20

Correctif en préparation après 0.8.0, non inclus dans les installateurs 0.7.3 ou 0.8.0.

## Diagnostic et choix

Sur Ollama local, Qwen3 1.7B avec `num_ctx 16384` crée le caisson puis répond à
son retour d’outil, mais ne produit pas de plan typé pour la division avec le
catalogue `submit_plan` imbriqué (reproduction : 45,74 s). Ajouter un outil natif
avec les métadonnées de Plan en paramètres provoque encore un rejet : le modèle
émet seulement `cell_id`, `vertical` et `count`. Ces trois paramètres initiaux sont bien
ceux de la commande métier ; imposer au modèle de réécrire l’enveloppe complète
n’apporte pas de validation supplémentaire.

Une deuxième erreur a été isolée : avec `vertical: bool`, Qwen3 1.7B répond
`true` à une demande de cases superposées. Le transport utilise maintenant
`layout: side_by_side | stacked`, traduit explicitement vers le booléen métier.
L’énumération est développée dans le schéma, sans référence indirecte.

L’adaptateur Ollama annonce donc un outil natif `divide_cell` à schéma simple,
avec `cell_id`, `layout` et `count`, uniquement en Plan/Agent, avec outils activés et des cases feuilles explicitement
transmises. Les identifiants possibles viennent du contexte ; la case sélectionnée
est rappelée dans le schéma. L’appel devient un Plan à une étape, avec titre et
explication déterministes, puis suit la préparation transactionnelle existante.
Les autres fournisseurs et le catalogue `submit_plan` sont conservés. Les autres
opérations restent accessibles par leur plan habituel. Aucun nom de fonction
n’est deviné depuis la demande, aucune réponse textuelle n’est convertie en outil.

Ce choix réutilise les appels et retours natifs de genai/Ollama ; pas de client
HTTP alternatif, de contournement du parseur ni de relance automatique.
[Protocole officiel Ollama](https://docs.ollama.com/capabilities/tool-calling).

## Invariants

- Un appel natif par réponse, outil réellement annoncé, mode autorisé.
- Trois paramètres obligatoires et typés, champs supplémentaires refusés.
- Aucun document modifié à la réception de la proposition ou à sa préparation.
- Case absente, parent déjà divisé, paramètres hors limites : refus métier.
- Annulation, contexte modifié ou mode Plan : application refusée.
- Confirmation humaine et retour natif avec l’identifiant de l’appel conservés.
- Les erreurs et les réponses textuelles sans outil ne deviennent jamais un plan.

## Reproduction locale

Le modèle dédié reprend les poids Qwen3 1.7B avec un contexte de 16 384 tokens,
comme décrit dans le guide. Aucun appel cloud ni clé API dans le test ciblé :

```sh
OLLAMA_MODEL=fablabcam-qwen3:1.7b \
  cargo test --locked -p fablabcam-studio --test assistant_ollama_live -- --ignored --nocapture
```

Le test utilise un caisson public synthétique, des identifiants neufs, une demande
en français sur la case sélectionnée, deux divisions successives (verticale en
deux, horizontale en trois), l’aperçu, l’autorisation et le retour outil natif.
Il vérifie la case, le sens, le nombre de compartiments et de panneaux. Les tests
ignorés par défaut ne sont pas comptés comme réussis dans la suite hors réseau.

## Résultats du 6 novembre 2026

- Suite workspace après intégration de main 0.8.0 : **441 tests réussis**, 0 échec, 2 tests réseau ignorés par
  défaut. Clippy workspace/toutes cibles avec `-D warnings` réussi.
- Tests de protocole : **12 réussis**, dont 4 nouveaux sur la division native,
  les paramètres invalides, le catalogue/mode et le refus du texte libre.
- Test local ciblé final : **Qwen3 1.7B réussi en 51,63 s** et
  **Qwen2.5 3B réussi en 126,56 s**, tous deux avec contexte 16 384.
  Chaque modèle passe les deux demandes naturelles, la bonne case, le sens,
  les panneaux, l’autorisation et les deux retours d’outil.
- Ces essais appellent le fournisseur local et les couches métier réelles ;
  ce ne sont pas des essais manuels de l’interface ni des appels cloud.

Le parcours global final `assistant_live`, Qwen3 1.7B, échoue en **41,56 s** :
création, retour natif et division réussissent, puis `replace_script` en mode
Plan répond sans plan typé. Le script n’est pas appliqué ; les étapes suivantes
(exécution Rhai, outils, simulation) ne sont donc pas atteintes dans cet essai.
Ce test complet est **en échec**, pas ignoré ni déclaré réussi.

## Limites constatées pendant le développement

Le parcours global `assistant_live` franchit maintenant la division, puis échoue
sur la proposition d’édition Rhai (réponse vide avec Qwen3 1.7B). Cela ne qualifie
pas le catalogue complet ni tous les petits modèles. Les premières variantes du
schéma ont aussi produit une mauvaise division lors des demandes naturelles ;
les résultats finaux doivent être distingués de ces essais. Les demandes et
objets restent à relire dans l’aperçu. L’issue #20 reste ouverte ; pas de
qualification CNC ni d’essai avec Internet physiquement déconnecté.

## Synchronisation avec 0.8.0

Les PR #32 / docs #16 ont été intégrées pendant cette campagne. Leurs nouveautés
de copie de messages et leur documentation sont conservées. La suite workspace
et Clippy ont été entièrement rejoués après fusion de main. Les fichiers du
transport et du test local Ollama sont identiques à ceux des essais réels ci-dessus ;
ces essais n’ont pas été répétés pour les seuls changements de version et d’UI.
La documentation synchronisée passe MkDocs strict et le contrôle de ses 37 pages,
liens, ancres et version 0.8.0 ; le rendu du complément Ollama a été revérifié.
