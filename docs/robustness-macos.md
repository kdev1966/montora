# Stabilité de Montora sur macOS — liste de travail

Établie le 21 octobre 2026, à partir de l'audit de Concat du 12 octobre
(`audit/AUDIT-2026-10-12.md`, problèmes encore ouverts qui touchent le Mac) et de ce
que Montora a trouvé lui-même. Classée par gravité puis par probabilité sur un Mac.
Chaque correction se fait sur sa branche, avec un test qui reproduit le problème.

## À corriger en priorité

- [x] **Deux enregistrements partagent un fichier temporaire** (audit n° 6). Repris de Concat
      (e75af9b, 9197340) : PR « fix/concurrent-saves ». ⌘S deux fois,
      ou un enregistrement pendant la sauvegarde automatique ou la fermeture, peut laisser
      un projet qui ne se rouvre plus. Perte de données. `concat-host/src/projects.rs:174`.
- [ ] **Carte AMD qui bloque le GPU** (Montora). Les vignettes d'effets bloquaient la
      R9 M370X jusqu'au redémarrage du GPU et à la fermeture de la session. Contourné par
      le réglage GPU (Automatique → GPU intégré) ; reste à trouver le shader fautif.
- [ ] **ONNX Runtime sur Mac Intel** (Montora, et audit n° 16). `ort` 2.0.0-rc.13 demande
      l'API 27 d'ONNX Runtime ; la dernière version pour Mac Intel (1.23.2) n'en sert que
      23 : le détourage et la voix Chatterbox échouent probablement sur un Mac Intel.
- [ ] **Un plantage de Whisper ferme toute l'application** (Montora, vu le 23 octobre 2026).
      whisper.cpp tourne dans le processus de Montora : son arrêt brutal (Metal sur la
      R9 M370X) a fermé l'application. Contourné en faisant tourner Whisper sur le processeur
      des Mac Intel (PR #19) ; il faudra isoler le moteur dans un processus à part.
- [ ] **Le test de lancement n'ouvre jamais la fenêtre** (audit n° 17). `--smoke` s'arrête
      avant `app.run()` : un échec de la fenêtre ou de la surface Metal passe inaperçu.
      `concat/src/lib.rs:1898`.

- [x] **Le .dmg doit démarrer sous macOS 12** (Concat #342, #372). libavdevice retiré (#7),
      FFmpeg 8.1.3 compilé par nous pour macOS 12 (#10). Vérifié le 22 octobre 2026 : le .dmg
      Intel installé dans Applications démarre sur le Mac de l'utilisateur (macOS 12.7.6),
      importe, lit et exporte en H.264 avec le FFmpeg embarqué.

## Gravité moyenne

- [ ] **Titres au-delà de 24 qui disparaissent de l'aperçu pendant un glisser** (audit n° 5).
      Touchera les timelines chargées en sous-titres. `concat-media/src/pool.rs:541`,
      `concat/src/studio.rs:2618`.
- [ ] **Le volet Sous-titres peut transcrire un autre clip que celui affiché** (audit n° 8).
      `concat/src/panes/captions.rs`.
- [ ] **Transition ou effet absent de l'export sans message** (audit n° 11) : un pipeline
      refusé par le pilote est ignoré, seul le log le dit. `concat-render/src/gpu.rs:2576`.
- [ ] **Erreur de validation GPU présentée comme une perte du GPU** (audit n° 10) : l'export
      s'arrête avec « the GPU device was lost 3 times ». `concat-export/src/lib.rs:1231`.

- [ ] **Noms de fichiers arabes mal ordonnés à l'affichage** (vu le 22 octobre 2026) :
      « توزر جوهرة الجنوب (1).mp4 » s'affiche « .mp4(1) توزر… » sur la timeline et dans la
      bibliothèque. L'ordre bidirectionnel d'un texte qui mêle arabe et latin n'est pas appliqué.
      Même défaut sur les clips de sous-titres en arabe de la timeline (vu le 23 octobre 2026).
- [ ] **L'export `.srt` peut écraser un fichier** : `nom.srt` est écrit sans écriture
      atomique et peut remplacer un fichier apparu après la boîte de dialogue. L'import lit
      un gros `.srt` sur le fil de l'interface.
- [x] **Les sous-titres ne suivent pas les coupes** : supprimer les silences, les tics ou des
      phrases coupe désormais aussi les sous-titres des pistes qui ne portent que du texte, et
      les décale avec le clip (PR #22). Une piste mêlant texte et image n'est pas touchée.

## Gravité faible

- [ ] Le chargement de la bibliothèque Metal a pris 26 s au premier lancement sur le Mac Intel.
- [ ] La liste de tics darija par défaut est courte (4 trouvés sur EP02) : l'enrichir.
- [ ] Le modèle Turbo n'est pas sur le miroir `models-v1` : il se télécharge depuis la source.
- [ ] Le menu « Générer les sous-titres » cache l'import et l'export SRT.
- [ ] Les réglages de la suppression des silences (seuil, durée) ne sont pas modifiables.
- [ ] Des textes de l'interface française ne sont pas encore traduits.

- [ ] Deux exports simultanés du même nom peuvent s'écraser. `concat-export/src/lib.rs:977`.
- [ ] Export et création par l'API qui peuvent écraser un fichier. `concat-api/src/lib.rs:519`.
- [ ] Un thread refusé coupe une connexion JSON sans message. `concat-server/src/json.rs:128`.
- [ ] Écriture d'environnement `unsafe` dont la justification est fausse.
      `concat/src/platform.rs:350`.
- [ ] `render` de la CLI écrit sur le fichier en place. `concat-cli/src/main.rs:419`.
- [ ] L'arrêt du serveur peut se bloquer sur un flux gRPC ouvert. `concat-server/src/lib.rs:263`.

## Méthode

- Chaque élément : reproduire (test automatique si possible), corriger, vérifier en CI,
  puis sur le Mac de l'utilisateur quand c'est visible à l'écran.
- Scénarios manuels à dérouler à chaque version : importer, lire, couper, déplacer,
  ajouter un titre et des sous-titres, exporter, rouvrir le projet, fermer pendant un
  enregistrement.
- [ ] Le projet s'enregistre sous `concat.json` : à renommer avec le passage concat → montora,
      en gardant la lecture des anciens projets.
