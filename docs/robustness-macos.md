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
- [ ] **Le test de lancement n'ouvre jamais la fenêtre** (audit n° 17). `--smoke` s'arrête
      avant `app.run()` : un échec de la fenêtre ou de la surface Metal passe inaperçu.
      `concat/src/lib.rs:1898`.

- [ ] **Le .dmg doit démarrer sous macOS 12** (Concat #342, #372). libavdevice n'est plus
      lié (fait). Reste : le FFmpeg de Homebrew des serveurs GitHub et ses bibliothèques
      sont compilés pour macOS 15. La fabrication du .dmg le détecte désormais et échoue ;
      il faut compiler FFmpeg et ses dépendances pour macOS 12 avant la pré-version.

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

## Gravité faible

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
