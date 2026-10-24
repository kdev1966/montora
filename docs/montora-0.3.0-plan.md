# Montora 0.3.0 — plan de version

Revu le 22 octobre 2026 : **approche MVP par éléments préfabriqués.** Au lieu de terminer
chaque étape avant de passer à la suivante, on monte vite une version qui marche de bout en
bout, on lui ajoute des fonctions une par une en version minimale, et l'on ne consolide qu'à
la fin, comme une tour faite de modules préfabriqués assemblés puis finis ensemble.

**macOS d'abord.** La 0.3.0 vise le Mac seul (Intel testé, Apple Silicon construit), en
`.dmg` à partir de macOS 12. Windows vient ensuite.

## Les règles

- **Une tranche MVP** = le chemin le plus simple qui marche vraiment dans l'application,
  testé sur le Mac de l'utilisateur, marqué « expérimental » dans l'interface. Pas de
  finitions ni de cas rares : ils attendent l'intégration finale.
- **Une PR courte par tranche.** L'utilisateur essaie la tranche sur son Mac avant la fusion.
- **Mode manuel.** Chaque tranche est proposée (quoi, temps estimé sur le Mac et en CI,
  risque) et commence avec l'accord de l'utilisateur ; une tranche à la fois.
- **Ce qui ne se saute jamais :** la CI complète (Clippy et tous les tests) avant chaque fusion
  dans `main`, les données de l'utilisateur protégées (rien n'est supprimé ou écrasé sans
  accord), et un `main` qui reste toujours utilisable.

## Étape A — le MVP de base : une boucle complète

> importer → transcrire → sous-titres → couper les silences → exporter (vidéo + SRT)

- [x] Base solide : sauvegardes sûres, réglage GPU, `.dmg` qui démarre sous macOS 12
      (FFmpeg compilé pour macOS 12), vérifié sur le Mac de l'utilisateur.
- [x] IA au choix (Groq, OpenCode Zen, xAI, OpenAI, adresse personnalisée), sous-titres en
      ligne.
- [x] Transcription gardée dans le projet, par média (PR #14).
- [x] Export des sous-titres en `.srt` à partir de la transcription (PR #16).
- [x] Suppression des silences : Montora repère les silences, propose les coupes,
      l'utilisateur valide (PR #17).

Étape A testée sur le Mac de l'utilisateur le 23 octobre 2026.

## Étape B — tranches MVP ajoutées une par une

Ordre de départ, modifiable par l'utilisateur :

- [x] Import SRT (code de Concat, #403 ; PR #16). Le VTT reste à faire.
- [x] Suppression des hésitations et tics : mots repérés dans la transcription, liste par
      défaut et mots ajoutés par l'utilisateur (PR #18).
- [x] Mots horodatés avec Whisper en local ; Whisper sur le processeur des Mac Intel, car
      Metal faisait planter l'application (PR #19).
- [x] Modèle Whisper Turbo et choix de la langue (PR #20). Mesuré sur le Mac de
      l'utilisateur : seul modèle local qui garde la darija mêlée d'anglais ; environ 152 s
      de calcul par minute d'audio.
- [ ] Extraits forts : l'IA choisit trois passages, Montora crée une timeline.
- [ ] Montage par le texte : supprimer une phrase coupe la vidéo.
- [ ] Brain MVP : un plan fixe de deux ou trois étapes à valider ([brain.md](brain.md)).
- [ ] Identité MVP : les couleurs Nocturne dans le thème ([identite/charte.md](identite/charte.md)).
- [ ] IA locale : llama.cpp (`llama-server`, protocole OpenAI).

## Étape C — intégration finale

- [ ] Chaque module consolidé : interface finie, cas rares, tests complets, performances.
- [ ] Points restants de la liste de stabilité ([robustness-macos.md](robustness-macos.md)).
- [ ] Renommage concat → montora (en gardant la lecture des anciens projets) et découpage du
      module de l'application, pour des compilations plus courtes.
- [ ] Liste de tests manuels par fonction.
- [ ] Pré-version `v0.3.0-rc.1` (`.dmg` Intel et Apple Silicon), corrections, puis `v0.3.0`.
