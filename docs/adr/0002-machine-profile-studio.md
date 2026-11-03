# Atelier et création guidée des profils machine

Statut : accepté pour 0.4.0.

Le Studio ajoute un espace Atelier à côté des vues de dessin. La bibliothèque
locale et les formulaires restent visibles ensemble ; un parcours en quatre
étapes crée un profil depuis la référence Mach3 sans inventer de limites physiques.

Le modèle `machine_profiles` dans `fablabcam-studio` gère les brouillons,
la détection de modifications et les échanges JSON. Le module `machine_ui` dans
`fablabcam-view` affiche ce modèle avec egui, déjà utilisé par le produit.
La validation structurelle reste celle de `Profile::check` et le stockage celui
de `gcode_store::Store`, commun au CLI. Aucune dépendance graphique supplémentaire.

Les champs absents restent `null`. Un profil structurellement correct peut être
enregistré « À configurer » ; ce statut n'est jamais une validation de machine ou
de G-code. Les paramètres non exposés dans les formulaires restent éditables dans
le JSON complet, sans perte lors d'un aller-retour. Un JSON non appliqué empêche
l'enregistrement/export et reste protégé par l'indicateur de modifications.

Les brouillons survivent au changement de vue. Changer de profil, abandonner une
création ou quitter demande de traiter les modifications en cours. L'import
propose de conserver une copie ou de remplacer explicitement un profil de même
identifiant. L'écriture vérifie sous verrou que le profil n'a pas changé depuis
son chargement ; elle refuse les conflits avec un autre Studio ou le CLI.

Sur macOS, AppKit peut envoyer Quitter sans demande de fermeture de fenêtre de
winit 0.29. Le délégué diffère cette terminaison vers egui, qui traite d'abord
le brouillon machine, puis celui du dessin. La fermeture effective ne survient
qu'après ces décisions ; les autres plateformes conservent le chemin winit.

Le magasin utilise le dossier de données de l'utilisateur, remplaçable par
`FABLABCAM_MACHINE_STORE` pour le partage avec le CLI, les tests et les captures.
Les dialogues natifs restent asynchrones ; un champ de chemin permet de travailler
lorsque les outils de dialogue de l'OS ne sont pas disponibles.

Limites : les profils ne sélectionnent pas automatiquement un post-processeur
`.cbpp`, ne pilotent pas de machine et ne rendent pas la simulation physique.
La qualification d'une machine et la validation d'un programme sont des actions
distinctes de l'édition de sa configuration.
