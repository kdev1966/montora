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

## Montora et Concat

Montora est parti du moteur de Concat, dont il garde l'historique et les auteurs. Depuis la séparation, c'est un projet à part entière, avec sa propre direction :

- **L'IA là où vous la choisissez.** Des modèles locaux, ou le fournisseur de votre choix (Groq, OpenCode Zen, xAI, OpenAI, ou toute adresse compatible OpenAI comme Ollama) avec votre propre clé, rangée dans le trousseau du système. Les sous-titres viennent de Whisper sur votre machine ou de Groq en quelques secondes, avec l'heure de chaque mot.
- **La solidité d'abord, une plateforme à la fois.** macOS en premier, testé sur d'anciens Mac Intel comme sur les récents, puis Windows. Une plateforme sort quand elle tient, pas avant.
- **Le Brain.** Un orchestrateur de montage qui s'adapte à la machine (il choisit des outils plus légers ou plus lourds selon ce que votre ordinateur peut porter) et à la personne (il apprend vos habitudes et votre niveau). Le modèle propose, le code décide, et rien ne change dans votre projet sans votre accord. Voir [brain.md](brain.md).
- **Ce que Concat fait bien est repris, pas réécrit.** Les correctifs et fonctions que Concat a déjà sont rapportés un par un, avec leurs auteurs.

## Où il va

1. **Des fournisseurs d'IA au choix.** D'abord les modèles locaux. Ensuite, en option, Ollama ou LM Studio, n'importe quelle API compatible OpenAI (OpenAI, Mistral, Groq, OpenRouter…) ou Anthropic. Vous apportez votre propre clé et choisissez votre budget.
2. **Monter par le texte.** Supprimez une phrase de la transcription, la vidéo est coupée au même endroit. Retirez les silences et les hésitations en une fois.
3. **Sous-titres en entrée et en sortie** : import et export SRT.
4. **Meilleurs passages** : transformer un long enregistrement en clips courts.
5. **Plus tard** : un assistant de montage, le recadrage automatique en 9:16, le doublage et la recherche dans vos rushs.

## Plateformes

macOS d'abord (Apple silicon et Intel, macOS 12 ou plus récent), puis Windows. Linux et Android viendront plus tard. Les versions seront publiées sur [GitHub Releases](https://github.com/kdev1966/montora/releases).

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
