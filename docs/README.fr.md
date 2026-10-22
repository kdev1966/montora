<div align="center">

<img src="../assets/montora_logo_512.png" alt="Montora" width="120" />

# Montora

**Un éditeur vidéo libre et gratuit pour Windows, macOS, Linux et Android, avec une IA qui tourne sur votre machine ou chez le fournisseur de votre choix.**

[🇬🇧 English](../README.md) · **🇫🇷 Français**

</div>

> [!NOTE]
> Montora est un fork de [Concat](https://github.com/jub0t/concat), créé par Jareer et les contributeurs de Concat. Il en garde le moteur et y ajoute de nouvelles fonctions de montage et des outils d'IA. Le projet en est à ses débuts : aucune version de Montora n'est encore publiée.

## Ce que c'est

Montora est un éditeur vidéo pour ce qu'on fait d'habitude avec CapCut : vidéos courtes, montages YouTube, tutoriels, extraits de podcast. Son moteur, hérité de Concat, est écrit en Rust natif avec un compositeur GPU.

Grâce à Concat, il sait déjà faire :

- **Sous-titres automatiques** avec un modèle Whisper local.
- **Synthèse vocale et clonage de voix**, entièrement en local.
- **Détourage** : personnes, objets, ou masque peint à la main.
- **Images clés** avec éditeur de courbes : position, échelle, rotation, opacité, volume, paramètres d'effets.
- **Plus de 170 effets, filtres, transitions et animations de texte**, rendus sur le GPU.
- **Une timeline multipiste** : couper, rogner, décaler, figer une image, vitesse, modes de fusion.
- **Export** en H.264, HEVC et AV1, jusqu'en 4K 60 i/s et couleur 10 bits.
- **Une API** JSON-RPC, gRPC et MCP, pour que des scripts et des agents IA puissent monter aussi.
- **Ni filigrane, ni compte, ni abonnement.** Vos vidéos ne quittent jamais votre disque, sauf si vous choisissez un fournisseur d'IA en ligne.

## Où il va

1. **Des fournisseurs d'IA au choix.** D'abord les modèles locaux. Ensuite, en option, Ollama ou LM Studio, n'importe quelle API compatible OpenAI (OpenAI, Mistral, Groq, OpenRouter…) ou Anthropic. Vous apportez votre propre clé et choisissez votre budget.
2. **Monter par le texte.** Supprimez une phrase de la transcription, la vidéo est coupée au même endroit. Retirez les silences et les hésitations en une fois.
3. **Sous-titres en entrée et en sortie** : import et export SRT.
4. **Meilleurs passages** : transformer un long enregistrement en clips courts.
5. **Plus tard** : un assistant de montage, le recadrage automatique en 9:16, le doublage et la recherche dans vos rushs.

## Plateformes

Windows, macOS (Apple silicon et Intel), Linux et Android. Les versions seront publiées sur [GitHub Releases](https://github.com/kdev1966/montora/releases).

Montora est distribué hors des magasins d'applications. Sur macOS, ouvrez-le la première fois avec clic droit › **Ouvrir**. Sur un Mac Intel, le détourage, l'amélioration d'image et la voix Chatterbox demandent macOS 13.3 ou plus récent.

## Compiler depuis les sources

Il faut Rust (rustup installe la version figée par le projet), les bibliothèques de développement FFmpeg 7+, cmake et un compilateur C++. Sur macOS :

```sh
brew install ffmpeg cmake
cd src
FFMPEG_DIR="$(brew --prefix ffmpeg)" cargo dev
```

[CONTRIBUTING.md](CONTRIBUTING.md) couvre Windows, Linux et les vérifications à lancer (en anglais). [ARCHITECTURE.md](ARCHITECTURE.md) est une carte du moteur, écrite pour Concat et toujours valable pour Montora.

## Licence et crédits

Montora est un logiciel libre sous [GNU AGPL v3 ou ultérieure](../LICENSE), comme Concat, avec la même [exception pour les plugins](../LICENSE-EXCEPTIONS.md). Les composants tiers sont listés dans [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

« Concat » et son logo appartiennent à Jareer (voir [TRADEMARK.md](TRADEMARK.md)). Montora n'est ni affilié au projet Concat, ni approuvé par lui.
