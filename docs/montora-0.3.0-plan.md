# Montora 0.3.0 — plan de version

Décidé le 21 octobre 2026. Une branche et une PR par étape ; chaque fonction est
testée dans l'app avant son merge ; la version sort après une pré-version testée sur
le Mac (macOS 12, Intel) et le téléphone Android.

## Phase 1 — Base solide

- [ ] 1. GPU : réglage Automatique / Intégré / Dédié (Réglages › Général), Automatique =
      GPU intégré sur un Mac dont la carte dédiée est une AMD sous macOS 12 ;
      validation des appels indirects de wgpu désactivée (fait, non commité) ;
      `MONTORA_GPU` gardé comme dérogation.
- [ ] 2. PR `feat/ai-providers` → `main` (IA, Réglages › IA, sous-titres en ligne), merge.
      PR Dependabot #4 examinée.

## Phase 2 — Fonctions

- [ ] 3. Transcription conservée dans le projet (texte, segments, mots horodatés, par média).
- [ ] 4. Mots horodatés avec whisper local (hors ligne).
- [ ] 5. Export / import SRT et VTT.
- [ ] 6. Suppression des silences (seuil, durée minimale, marge).
- [ ] 7. Suppression des hésitations (« euh », « hum », « um »…) à partir des mots.
- [ ] 8. Moments forts par l'IA → nouvelle timeline.
- [ ] 9. Montage par le texte.
- [ ] 10. IA locale : llama.cpp (`llama-server` embarqué, protocole OpenAI).

## Phase 3 — Sortie

- [ ] 11. Checklist de tests manuels par fonction.
- [ ] 12. Pré-version `v0.3.0-rc.1` (brouillon) : installeurs des 4 bureaux + APK.
- [ ] 13. Corrections, puis `v0.3.0`.

## Déjà fait (branche feat/ai-providers)

- Crate `montora-ai` : Groq, OpenCode Zen, xAI, OpenAI, serveur personnalisé ; clés dans
  le trousseau du système.
- Réglages › IA (testé avec Groq).
- Sous-titres en ligne (testé : 6 min 57 transcrites en 14 s, 120 sous-titres).
