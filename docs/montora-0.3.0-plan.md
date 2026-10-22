# Montora 0.3.0 — plan de version

Décidé le 21 octobre 2026. Une branche et une PR par étape ; chaque fonction est
testée dans l'app avant son merge ; la version sort après une pré-version testée sur
le Mac (macOS 12, Intel).

**macOS d'abord, puis Windows.** La 0.3.0 vise le Mac seul : chaque fonction est faite
et testée sur macOS, et la version sort en `.dmg` Intel et Apple Silicon. Windows vient
ensuite (0.4). Linux et Android sont mis de côté pour l'instant : leurs builds (Nix,
téléphones) ne se lancent plus qu'à la main. La CI garde à chaque PR les tests du moteur
(sur Linux, le banc d'essai du projet), les tests Windows et la compilation macOS.

## Phase 1 — Base solide

- [ ] 1. GPU : réglage Automatique / Intégré / Dédié (Réglages › Général), Automatique =
      GPU intégré sur un Mac dont la carte dédiée est une AMD sous macOS 12 ;
      validation des appels indirects de wgpu désactivée (fait) ;
      `MONTORA_GPU` gardé comme dérogation. Reste : essai sur le Mac de l'utilisateur.
- [ ] 2. PR `feat/ai-providers` → `main` (#5, en brouillon : IA, Réglages › IA,
      sous-titres en ligne), merge. Les PR Dependabot (#1, #4) attendent la 0.4, sauf
      mise à jour nécessaire au Mac ou à Windows.
- [ ] 2b. Correctifs repris de Concat, un par branche (même licence AGPL, en-têtes
      gardés) : démarrage sous macOS 12 sans libavdevice (#342), puis les autres
      correctifs Mac de stabilité. Liste de stabilité : `docs/robustness-macos.md`.

## Phase 2 — Fonctions

- [ ] 3. Transcription conservée dans le projet (texte, segments, mots horodatés, par média).
- [ ] 4. Mots horodatés avec whisper local (hors ligne).
- [ ] 5. Export / import SRT et VTT (reprendre l'import/export SRT de Concat, #403, puis VTT).
- [ ] 6. Suppression des silences (seuil, durée minimale, marge).
- [ ] 7. Suppression des hésitations (« euh », « hum », « um »…) à partir des mots.
- [ ] 8. Moments forts par l'IA → nouvelle timeline.
- [ ] 9. Montage par le texte.
- [ ] 10. IA locale : llama.cpp (`llama-server` embarqué, protocole OpenAI).

## Phase 3 — Sortie

- [ ] 11. Checklist de tests manuels par fonction.
- [ ] 12. Pré-version `v0.3.0-rc.1` (brouillon) : `.dmg` macOS Intel et Apple Silicon
      seulement (release.yml réduit au Mac pour cette version).
- [ ] 13. Corrections, puis `v0.3.0`.

## Déjà fait (branche feat/ai-providers)

- Crate `montora-ai` : Groq, OpenCode Zen, xAI, OpenAI, serveur personnalisé ; clés dans
  le trousseau du système.
- Réglages › IA (testé avec Groq).
- Sous-titres en ligne (testé : 6 min 57 transcrites en 14 s, 120 sous-titres).
