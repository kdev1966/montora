# Le Brain de Montora

Conception du 21 octobre 2026. Le Brain est l'orchestrateur de montage de Montora : il
reçoit un objectif (« un Short TikTok de 60 s de cette interview, sous-titré »), le découpe
en étapes, les confie à des compétences et te montre le résultat avant de toucher au projet.

Il reprend le modèle Brain des autres projets de l'auteur (khaddouj-ai, think-together,
AuraNote, et Delest pour la partie Rust) et l'adapte au montage vidéo.

## Sa vraie capacité : s'adapter aux deux H

Ce qui distingue le Brain n'est pas le nombre de ses fonctions, c'est qu'il s'adapte :

- **au Hardware** : il tourne aussi bien sur un vieux Mac Intel de 2015 que sur une machine
  musclée, en choisissant à chaque fois les outils que la machine peut porter ;
- **au Human** : il apprend les habitudes de son utilisateur et s'ajuste à son niveau, du
  débutant guidé pas à pas au monteur expérimenté qui veut aller vite.

Le même Montora n'a donc ni le même comportement d'une machine à l'autre, ni d'une personne
à l'autre.

### H comme Hardware

**Sonder la machine.** Au démarrage, puis quand quelque chose change (batterie, autre
écran, carte graphique), le Brain prend un instantané de ce qui est disponible : cœurs et
instructions du processeur (AVX2…), mémoire, carte graphique et sa mémoire, moteur Metal ou
autre, place sur le disque, secteur ou batterie, chauffe. Montora a déjà une partie de ces
sondes (choix du GPU dans `gpu.rs`, version de macOS dans `machine.rs`).

**Mesurer plutôt que deviner.** Une fiche technique ne dit pas combien de temps Whisper met
vraiment sur cette machine. Le Brain mesure chaque tâche réelle et garde ces mesures, comme
le fait déjà `Learned` dans `panes/pace.rs` pour les barres de progression. Ses choix
suivants partent de ces mesures.

**Choisir ses outils d'après la machine.** Pour chaque tâche, il y a plusieurs voies, du plus
léger au plus lourd :

| Tâche | Petite machine | Machine musclée |
|---|---|---|
| Transcription | Whisper petit modèle, ou Groq en ligne | Whisper grand modèle en local |
| Choisir des passages, écrire un titre | IA en ligne, ou petit modèle local (1 à 4 milliards de paramètres) | Modèle local plus gros |
| Aperçu | Proxys, qualité réduite | Pleine résolution |
| Recadrage, suivi | Détecteur de visages léger | Détection de personnes complète |
| Génération d'images ou de vidéos | En ligne seulement, coût annoncé | En local possible |
| Export | Encodeur matériel | Encodeur logiciel de qualité maximale si le temps le permet |

**Ne jamais proposer ce que la machine ne tiendra pas.** Une tâche estimée trop longue est
proposée autrement (en ligne, plus petit modèle, ou plus tard), avec la raison. Si la machine
fatigue en cours de route (chauffe, batterie faible), le Brain le dit et propose de ralentir.

### H comme Human

**Les habitudes.** Le Brain retient ce que tu fais de ses propositions :
- tu refuses toujours les coupes de moins d'une demi-seconde → il ne les propose plus ;
- tu changes toujours le style des sous-titres pour le même → il le propose d'emblée ;
- tu exportes toujours en 1080p pour YouTube → c'est son premier choix.

Une habitude ne compte que quand elle est sûre (plusieurs fois, sans contre-exemple récent),
comme dans AuraNote. Une habitude ne lève jamais un interdit.

**Le niveau.** Le Brain s'ajuste aux compétences de l'utilisateur :

| | Débutant | Confirmé | Expert |
|---|---|---|---|
| Propositions | Plan complet, expliqué étape par étape | Plan résumé | Seulement ce qu'on lui demande |
| Confirmations | Avant chaque étape | Avant les suppressions | Seulement avant ce qui ne s'annule pas |
| Explications | Pourquoi chaque choix | Sur demande | Sur demande |
| Interface | Réglages essentiels | Réglages courants | Tout, raccourcis mis en avant |

Le niveau est d'abord demandé au premier lancement, puis ajusté d'après ce que fait
l'utilisateur (raccourcis employés, images clés, réglages fins, annulations). Il est toujours
affiché et modifiable : le Brain ne décide pas seul de ce que quelqu'un sait faire.

**La confiance dans l'IA, mesurée sur toi.** Comme `ConfianceQwen` dans AuraNote : si tu
corriges souvent ce que propose le modèle, les règles l'emportent plus facilement sur lui ;
s'il te réussit, il gagne du poids. Le même modèle n'a pas le même poids d'un utilisateur à
l'autre.

**Vie privée.** Tout ce profil (machine, habitudes, niveau, mesures) reste sur l'ordinateur,
dans le dossier de Montora. Il est consultable dans les Réglages, modifiable, et effaçable
d'un clic. Il n'est jamais envoyé à un fournisseur d'IA.

## Les règles du Brain

1. **Le code décide, le modèle propose.** Un enchaînement de phases fixe décide. Le modèle de
   langue n'est qu'un outil à l'intérieur. Le Brain fonctionne même sans aucune IA.
2. **Des compétences branchables.** Chaque compétence a un manifeste (ce qu'elle sait faire,
   ce dont elle a besoin, sur quelle machine elle tourne) et est vérifiée au démarrage. Un
   test garantit que le cœur ne contient aucun vocabulaire de montage.
3. **Un ordre d'arbitrage fixe** : interdits > ce que tu as décidé > habitudes sûres > modèle
   > règles par défaut. Le modèle peut rendre une décision plus prudente, jamais lever un
   interdit.
4. **La sortie du modèle est une candidate.** Format fermé, puis contrôle : un passage choisi
   doit exister dans la transcription, avec des timecodes valides et dans les limites du
   clip. Sinon, la proposition des règles est gardée.
5. **Rien sans ton accord.** Le Brain montre son plan et son coût avant d'agir. Chaque étape
   appliquée est un « annuler » à elle seule. Une proposition refusée n'est plus refaite.
6. **Il passe par l'API de Montora**, la même que l'interface, avec ses validations. Il ne
   touche jamais directement aux fichiers.
7. **Tout est tracé** : chaque phase note ce qu'elle a reçu, décidé et pourquoi, et combien
   de temps elle a pris. La trace est lisible dans l'application.
8. **Rejouer avant de changer** : toute nouvelle règle, compétence ou modèle est rejouée sur
   des cas réels enregistrés et refusée si une réponse se dégrade.

## Le déroulement d'une demande

> objectif → **percevoir** (projet, rushes, transcription, machine, profil) → **planifier**
> (quelles compétences, dans quel ordre, avec quels outils pour cette machine) →
> **proposer** (chaque compétence produit un brouillon de modification) → **contrôler** →
> **te montrer le plan** (étapes, durée estimée, coût) → **appliquer** (un « annuler » par
> étape) → **tracer** → **apprendre** (ce que tu as gardé, refusé, corrigé)

## Les compétences

| Compétence | Décide par règle | Le modèle sert à… |
|---|---|---|
| Transcription | choix du modèle selon la machine | Whisper (outil) |
| Coupe | silences, « euh » détectés | proposer les meilleurs passages |
| Sous-titres | placement hors des zones cachées, découpe en lignes, style habituel | rien |
| Audio | bruit, baisse de la musique sous la voix, volume final | rien |
| Cadrage | suivre le visage, lisser le cadre | rien |
| Plateforme | format, durée, préréglage | proposer titre, hashtags, chapitres |
| Génération | coût, consentement, en ligne ou en local selon la machine | créer images et vidéos |
| Contrôle | durée, trous, zones cachées, volume | rien |

De nouvelles compétences s'ajouteront au fil des fonctionnalités, sans toucher au cœur.

## Où il vit dans le code

- Le cœur, les profils machine et utilisateur et l'arbitrage : crate `montora-ai`, en Rust,
  sur le modèle de l'arbitre de Delest (`crates/delest-core/src/arbiter.rs`).
- Les compétences appellent les commandes de l'API de Montora (`concat-api`, renommée plus
  tard) ; chaque fonctionnalité de la 0.3.0 y est exposée comme une commande.
- Profil, habitudes, mesures et journal des décisions : une base SQLite locale dans le
  dossier de données de Montora.
- Les appels au modèle passent par la passerelle existante (`montora-ai` : fournisseurs,
  clés dans le trousseau), avec délai maximal, budget par demande et repli sans IA.

## Calendrier

- **0.3.0** : les compétences se construisent comme fonctionnalités, chacune exposée en
  commande de l'API (transcription conservée, silences, sous-titres, extraits forts).
- **0.4.0** : le Brain v1 : sonde de la machine et mesures, cœur et arbitrage, plan à
  valider, compétence Contrôle, habitudes de base, niveau choisi au premier lancement.
- **Ensuite** : ajustement automatique du niveau, confiance mesurée dans le modèle, nouvelles
  compétences (cadrage, génération, plateformes).

## Inspirations

Idées reprises d'autres projets, comme modèles de conception (pas de code).

### open-duet (Anil-matcha/open-duet, Apache-2.0, revu le 22 octobre 2026)

Une couche de contrôle autour d'agents IA externes. Contraire à notre doctrine sur un point
essentiel : chez lui, l'agent IA décide et planifie, open-duet ne fait que filtrer ses actions.
Cinq idées valent d'être reprises :

1. **La carte de proposition et son cycle de vie.** Chaque proposition est enregistrée avec son
   contenu complet et un état : en attente, acceptée, refusée, expirée, annulée. Un délai dépassé
   compte comme un refus, et tout est revérifié juste avant d'appliquer (le projet n'a pas
   changé entre-temps). C'est notre étape « montrer le plan → appliquer ».
   Source : `backend/app/db/models/approval.py`, `thread_runtime.py` (ThreadApprovalGate).
2. **Le journal des décisions.** Un fichier en ajout seul (une ligne JSON par décision) qui note
   l'événement, **qui a décidé** (toi, une habitude, une règle, le modèle), **en vertu de quelle
   règle**, le mode, un résumé sans secret, et une durée de conservation. Ses champs « qui » et
   « règle » portent directement notre ordre d'arbitrage, et rendent le Brain explicable
   (« coupé parce que tu coupes toujours les silences de plus d'une seconde »).
   Source : `decision_log.py`.
3. **Des habitudes proposées sans IA.** Des règles fixes repèrent une habitude possible et la
   proposent ; une habitude refusée n'est plus jamais reproposée ; chaque changement de la
   mémoire est journalisé et peut être annulé ; le journal est hors de portée de ce qui le
   remplit. C'est le « H comme Human » appliqué sans risque.
   Source : `memory_proposals.py`, `memory_journal.py`.
4. **Une liste noire au-dessus de tout.** Elle l'emporte sur les règles enregistrées, et ce qui
   ne peut pas être analysé attend la décision de l'utilisateur. Pour Montora : ne jamais
   supprimer un rush, ne jamais écraser un export ou un projet sans demander, ne jamais envoyer
   un fichier à un service en ligne sans accord.
   Source : `safety_floor.py`.
5. **Des compétences qui déclarent leurs capacités**, avec un test de disponibilité, et qui ne
   peuvent jamais élargir leurs droits à l'import. Rejoint le manifeste des compétences et le
   « H comme Hardware » : la compétence dit ce dont elle a besoin, le Brain vérifie si la machine
   le permet.
   Source : `provider_adapter.py` (Capabilities, probe), `agent_package.py`.

À ne pas reprendre : l'agent IA qui décide, les lectures et modifications autorisées sans
demander, l'absence de plafond de dépenses.
