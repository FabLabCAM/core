# Réutiliser les messages de l’assistant

Chaque message propose **Copier** (texte complet avec le Markdown),
**Ajouter au prompt** (ajout après le brouillon existant, sans envoi), et
**Texte sélectionnable** (source en lecture seule pour sélectionner un extrait).
Pour un extrait, sélectionner puis Cmd+C sur macOS, Ctrl+C ailleurs ; cliquer
dans le prompt puis Cmd+V/Ctrl+V. Le rendu Markdown reste disponible sous les actions.

L’interface emploie le presse-papiers natif d’egui/eframe. Le tampon source
immuable permet sélection et copie sans modifier l’historique. Les actions ne
sollicitent pas le fournisseur et ne modifient pas le projet d’usinage.
