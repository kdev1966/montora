# Pistes de fonctionnalités pour Montora

Recherche du 21 octobre 2026 : projets libres, modèles et services qui peuvent apporter à
Montora ce qui lui manque par rapport à CapCut, et de nouvelles pistes (IA générative, montage
selon la plateforme). Les licences du code ont été lues sur GitHub, celles des modèles sur
leur fiche Hugging Face ou leur fichier de licence. Les prix et certaines limites des
plateformes viennent de sources secondaires : les revérifier avant de s'en servir.

## Règles de licence

Montora est sous AGPL-3.0-or-later.

- **Compatible** : MIT, BSD, ISC, Apache-2.0, MPL-2.0, GPL-3.0, GPL-2.0-or-later, AGPL-3.0.
- **Compatible avec mention** : CC-BY (une ligne de crédit), CC-BY-SA 4.0.
- **Interdit à l'intégration** : toute licence « non commerciale » (NC) ou limitée par pays.
  Les utilisateurs de Montora font du contenu commercial.
- Les poids des modèles ne sont jamais livrés dans l'application : ils se téléchargent à la
  demande, après affichage de leur licence. Appeler un service en ligne ne pose aucun
  problème de licence.

Concat (jub0t/concat) reste une source : on reprend pièce par pièce ce qu'il a déjà fait
(correctif macOS 12 #342, import/export SRT, courbes de vitesse).

## 1. Ce qui manque par rapport à CapCut

| Fonctionnalité | Solution retenue | Licence | Sur le Mac Intel |
|---|---|---|---|
| Stabilisation | Filtres FFmpeg `vidstabdetect` / `vidstabtransform` (FFmpeg compilé avec `--enable-libvidstab`). Plus tard : Gyroflow pour les clips avec données gyroscopiques | LGPL-2.1+ ; Gyroflow GPL-3.0 | Oui |
| Suppression des silences | FFmpeg `silencedetect` ; s'inspirer des seuils d'auto-editor | — ; Unlicense | Oui |
| Baisse de la musique sous la voix | FFmpeg `sidechaincompress`, ou silero-vad + courbe de volume | — ; MIT | Oui |
| Changements de plan | FFmpeg `scdet` | — | Oui |
| Sous-titres animés mot par mot | Heure des mots de whisper.cpp (déjà utilisé) ; rendu par notre moteur de titres ; format de référence : balises karaoké de libass | MIT ; ISC | Oui |
| Réduction du bruit | nnnoiseless (RNNoise en Rust pur) ; plus tard DeepFilterNet | BSD-3 ; MIT/Apache | Oui |
| Séparation voix / musique | demucs | MIT | Lent (minutes par morceau) |
| Autocollants, emojis | Fluent Emoji de Microsoft ; Twemoji ; OpenMoji | MIT ; CC-BY ; CC-BY-SA | Oui |
| Animations Lottie | velato (Rust, sur wgpu) ou dotlottie-rs (CPU) | Apache-2.0 ; MIT | À tester avec le GPU AMD |
| Musique et bruitages | Recherche Openverse filtrée CC0 + pack Kenney livré | MIT ; CC0 | Oui |
| Recadrage auto 9:16, suivi | usls (même version d'`ort` que Montora) + détecteur de visages YuNet + RT-DETR ou D-FINE + ByteTrack ; lissage du cadrage d'après AutoFlip | MIT ; MIT ; Apache-2.0 | Oui (YuNet rapide) |
| Détection du rythme | beat_this exporté en ONNX ; sinon aubio | MIT (code et poids) ; GPL-3.0 | Oui |
| Mots de remplissage | Invite initiale de Whisper contenant des « euh », à tester | — | Oui |
| Montage par le texte | Étudier audapolis (même licence AGPL : code réutilisable) | AGPL-3.0 | — |
| Extraits forts | Démarche de FunClip : transcription → IA choisit les passages → découpe | MIT (étude) | Oui avec IA en ligne |
| Traduction des sous-titres | Opus-MT via ctranslate2-rs ; doublage avec Kokoro (déjà présent) | CC-BY ; MIT ; Apache-2.0 | Oui |
| IA locale | llama.cpp `llama-server` | MIT | Processeur seul, modèles de 1 à 4 milliards de paramètres |

**À ne pas intégrer** (licence non commerciale ou limitée) : NLLB et SeamlessM4T, F5-TTS,
CrisperWhisper, modèles de visage InsightFace, modèles madmom, contenu Pixabay, modèles de
la Freesound API sans accord.

## 2. Nouvelles pistes

### Montage selon la plateforme visée

| Plateforme | Format | Durée max | À retenir |
|---|---|---|---|
| YouTube | 16:9 | — | Officiel : H.264 High, 1080p à 8 Mbit/s (24–30 i/s) ou 12 Mbit/s (48–60 i/s), audio AAC 48 kHz |
| YouTube Shorts | 9:16 ou 1:1 | 3 min | — |
| TikTok | 9:16, 1080×1920 | ~10 min dans l'appli | Garder libres les 10 % du haut et du bas |
| Instagram Reels | 9:16, 1080×1920 | 3 min | Zones masquées : haut 14 %, bas 35 %, côtés 6 % |
| Facebook Reels | 9:16 | à vérifier | Mêmes zones qu'Instagram |
| X | 16:9, 1:1, 9:16 | 140 s (compte standard) | — |
| LinkedIn | 1:2,4 à 2,4:1 | à vérifier | — |

À faire :
- préréglages d'export par plateforme, avec alerte de durée ;
- affichage des zones masquées dans l'aperçu, sous-titres placés au-dessus des 35 % du bas ;
- volume sonore cible −14 LUFS / −1 dBTP à l'export (FFmpeg `loudnorm` en deux passes) ;
- titres, descriptions, hashtags et chapitres YouTube (0:00 d'abord, au moins 3 chapitres de
  10 s) générés par l'IA à partir de la transcription ;
- miniature : meilleure image (nette, avec un visage) + texte par le moteur de titres.

La **publication directe** est écartée pour l'instant : YouTube garde les vidéos en privé tant
que l'application n'a pas passé un audit, TikTok et Instagram exigent aussi une validation.
À la place : exporter avec le préréglage, puis ouvrir la page d'envoi de la plateforme avec
les métadonnées dans le presse-papiers.

### Génération d'images

Impossible en local sur le Mac de test (2 Go de mémoire graphique). En ligne d'abord :
- OpenAI gpt-image et xAI, par le même point d'accès `/v1/images/generations` que nos
  fournisseurs ; environ 0,006 à 0,21 $ l'image chez OpenAI ;
- plus tard, un service agrégateur (fal.ai ou Replicate) pour plus de modèles.

Plus tard en local, sur machine récente : stable-diffusion.cpp (MIT) en programme
annexe, avec FLUX.2 klein 4B ou Z-Image-Turbo (Apache-2.0, 12 à 16 Go de mémoire graphique).
À éviter : FLUX dev (non commercial).

### Génération de vidéos

En ligne seulement (aucun modèle ne tourne sur le Mac de test) : Grok Imagine (la clé xAI
existe déjà, ~0,05–0,08 $/s), Sora 2 (0,10 $/s en 720p), Veo, Runway, Kling, ou fal.ai. Ces
services fonctionnent par tâche (envoi, attente, téléchargement) : prévoir une « tâche de
génération » avec estimation du coût avant l'envoi. Commencer par animer une image de la
timeline. Modèle libre pour plus tard : Wan 2.2 5B (Apache-2.0, 24 Go). À éviter :
HunyuanVideo (licence qui exclut l'UE et le Royaume-Uni).

### Autres pistes

| Piste | Solution | Licence | Mac de test | Avis |
|---|---|---|---|---|
| Assistant de montage | L'IA pilote Montora par son API JSON-RPC/MCP existante, un « annuler » par tour, confirmation avant toute suppression | — | Oui (IA en ligne) | À faire, forte valeur |
| Recherche dans les rushes par description | SigLIP 2 en ONNX + recherche dans la transcription | Apache-2.0 | Oui | À faire |
| Accorder les couleurs entre clips | Transfert statistique LAB dans un shader wgpu, sans modèle | — | Oui | À faire |
| Ralenti fluide, changement d'images/s | RIFE en ONNX | MIT | Lent, rendu hors ligne | À faire |
| Musique générée | ACE-Step 1.5 en programme annexe | MIT (code et poids) | Lent | Optionnel |
| Effacer un objet dans une vidéo | SAM 2 + LaMa | Apache-2.0 | Lent, scintille | Étude ; en ligne pour la qualité |
| Synchronisation des lèvres | LatentSync | Apache-2.0 | Non | En ligne seulement |
| Changeur de voix | RVC | MIT | Possible hors ligne | Étude, avec garde-fous sur le consentement |
| Script → vidéo | IA → scènes → images/vidéos + voix + sous-titres | — | En partie | Après l'assistant et la génération |

À éviter : MusicGen (poids non commerciaux), Wav2Lip, ProPainter, outils basés sur InsightFace.

## 3. Éditeurs libres à étudier

| Éditeur | Licence | Ce qu'il fait mieux |
|---|---|---|
| OpenCut | MIT | Ergonomie à la CapCut, la plus proche de notre objectif |
| audapolis | AGPL-3.0 | Montage par le texte |
| Kdenlive | GPL-3.0 | Proxys, éditeur de sous-titres, mixeur audio |
| LosslessCut | GPL-2.0 | Coupes sans réencodage, défilement rapide |
| Shotcut | GPL-3.0 | Formats et filtres nombreux |
| OpenShot | GPL-3.0 | Prise en main simple, modèles de titres |
| Flowblade | GPL-3.0 | Montage rapide au clavier |

## 4. Ordre proposé

D'abord ce qui coûte peu et sert le plus :

1. Filtres FFmpeg déjà présents : stabilisation, silences, baisse de la musique, changements de plan.
2. Sous-titres animés mot par mot (heures des mots de Whisper).
3. Préréglages par plateforme : formats, zones masquées, −14 LUFS.
4. Titres, descriptions, hashtags et chapitres générés par l'IA.
5. Réduction du bruit (nnnoiseless) et autocollants (Fluent Emoji, Twemoji).
6. Bibliothèque de sons (Openverse CC0 + Kenney).
7. Assistant de montage par l'API existante.
8. Génération d'images en ligne (OpenAI, xAI).
9. Recadrage 9:16 et suivi (usls + YuNet), recherche dans les rushes (SigLIP 2).
10. Traduction locale (Opus-MT) et doublage (Kokoro), génération de vidéos en ligne.

Les points 1, 2 et 4 recoupent le plan 0.3.0 (silences, sous-titres, extraits forts).
