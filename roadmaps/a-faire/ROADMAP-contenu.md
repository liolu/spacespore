# Feuille de route — Q. Contenu (structure du jeu, histoires, défis)

Source : bloc **Q** de `roadmaps/a-faire/RAPPORT-ameliorations.md` (idées 215 à 224) + idées 1, 2, 9, 10, 13, 14, 82 à 84, 105 à 110.
Les phases s'appellent **CON-n**.

Objectif : donner au joueur **des raisons de voyager, de chercher et de revenir** : un début guidé, des
mystères, des objectifs, une histoire de l'univers découverte par fragments, des défis, des événements —
**sans casser** la liberté d'exploration ni le déterminisme (tout est généré depuis la graine ; rien n'est
scripté à la main sauf le monde de départ et les tutoriels).

---

## 1. Point de départ (code du 05/10/2026)

- **Boucle actuelle** : explorer (galaxies, systèmes, astres explorables, grottes), **scanner** (dex K), **économie**
  (`economy.rs` : biens, factions, `sold_by`, missions simples `mission_text`), **guildes** (`guild.rs`), **diplomatie**
  (`diplomacy.rs`), **combat** (`combat.rs`), **territoires** (`claims.rs`), **PNJ** (`npc_ui.rs`), **trous de ver**
  (`wormhole.rs`), **éditeur** (modèles), **multijoueur**. Pas de fil conducteur, pas de succès, pas de quêtes
  structurées, pas d'histoire.
- **Contenu généré** : galaxies, systèmes, planètes, lunes, ceintures, comètes, minerais (phase 8), vie (phase 7),
  traits rares (98 / 1,5 / 0,4 / 0,1 %), mondes exceptionnels (0.14 X), événements (0.14 N), points d'intérêt
  (0.14 D6 : épaves, ruines, monolithe).
- **Aliens et terraformation** prévus en 0.13 L5 / L6 (pas de combat au début, commerce oui, attitude tirée de la
  graine).
- **Dex** (`dex.rs`) : découvertes, notes, historique, import / export : base de l'histoire et des succès.
- **Premier lancement** : ouvre l'éditeur (création du personnage) puis rien d'autre : pas de tutoriel.

## 2. Règles

1. **Tout est généré depuis la graine** (histoire, quêtes, ruines, noms) : deux joueurs de la même graine vivent le
   **même** univers ; seul le monde de départ et les tutoriels sont écrits à la main.
2. **Pas de contenu bloquant** : aucune progression n'est verrouillée derrière une quête ; elles **récompensent**
   (objets, réputation, informations), elles ne gardent pas les portes.
3. **Rien n'est obligatoire** : mode « explorateur libre » sans objectifs ni message de mission.
4. **Cohérence du lore** : la même histoire alimente les ruines, les épaves, les textes du dex, les noms des
   factions et les missions (une seule source `lore.rs`).
5. **Texte généré = grammaire + gabarits + graine**, jamais de LLM à l'exécution (déterminisme, hors ligne).
6. **Les textes sont traduisibles** (localisation : idée 46) : identifiants de chaînes, pas de texte en dur.
7. **Combat optionnel** : une quête ne force jamais le combat ; alternatives (fuite, négociation, discrétion).
8. **Sécurité et réseau** : les quêtes d'équipe passent par l'hôte (validation des récompenses, anti-triche, TECH-6).

## 3. Les phases

| Phase | Contenu | Idées | Taille |
|---|---|---|---|
| **CON-1. Premier lancement guidé** | **Monde de départ écrit à la main** (région fixe autour du système 0, lieu de départ scénarisé) ; **tutoriel** guidé (invites contextuelles CTL-5 : se déplacer, cibler un astre, voler, se poser, marcher, scanner, ouvrir le dex, se poser dans un cockpit, voler en vol bas) qui se **saute** ; **mission d'accueil** (un signal faible à suivre) ; choix d'un **mode de jeu** (explorateur libre / aventure / survie) ; aide en jeu (CTL-6) ; **« mode solo vs multijoueur »** expliqué. | 215, 222, 223 | M |
| **CON-2. Succès et rangs** | **Succès** (explorateur : 1er atterrissage, 100 mondes, un monde de chaque type, grotte la plus profonde ; scientifique : scans niveau 3, échantillons ; survivant : 1 h sans retour au vaisseau ; voyageur : traverser une galaxie, un trou de ver) ; **rangs d'explorateur** (liés au dex : % de complétion, raretés trouvées) ; **titres** au-dessus du nom (PER-5) ; **records personnels et de serveur** (plus haute montagne vue, canyon le plus profond, monde le plus rare) ; panneau « Carrière » (touche dédiée). | 1, 81 | M |
| **CON-3. Lore procédural** | `lore.rs` : **histoire de l'univers** déduite de la graine : civilisations disparues (nombre, ère, destin), **factions** (origine, rivalités, `diplomacy.rs`), **événements du passé** (guerres, migrations, catastrophes comme AST-3 supernovæ et N2 impacts), **personnages historiques**, **artefacts** ; **noms** (grammaire phonétique par civilisation), **textes courts** (inscriptions, journaux de bord, légendes) par gabarits ; **lore dans le dex** (fiche par monde : histoire géologique, découvreur, ancienne présence). Même graine = même histoire. | 216, 217 | L |
| **CON-4. Quêtes procédurales** | `quests.rs` : **modèles de quêtes** (livrer, scanner un monde, rapporter un échantillon, retrouver une épave, enquêter sur une ruine, escorter, relever un phénomène rare, cartographier, localiser une balise, retrouver une personne / un vaisseau), générés par les **faits de l'univers** (si un volcan est proche, « prévoir l'éruption »), **donneurs** (PNJ de factions, stations, aliens L5), **récompenses** (biens, minerais, réputation, informations sur le dex, plans), **suivi** (journal de quêtes, boussole, balise de quête INS-7), **échecs doux** (quête abandonnable sans pénalité). Évolue le système de missions de `economy.rs` (même base : plafond de missions, versement). | 2, 13 | L |
| **CON-5. Mystères et énigmes** | **Mystères** à plusieurs étapes : un **signal** (radio, balise ancienne) → des **ruines** (D6 / X8) → des **énigmes** (plaques, cristaux à aligner, portes à séquence, miroirs de lumière, constellations à reproduire, temps local Q6) → un **artefact** ; **monolithe** (D6 : le trait « monolithe » devient réel) ; **cartes au trésor** générées (coordonnées vagues à recouper par scanner) ; **indices** dans le dex ; solutions **déterministes** (graine) donc partageables par les joueurs ; aucune énigme ne se résout à la chance. | 215, 219 | L |
| **CON-6. Artefacts et objets rares** | **Artefacts** ramassables (propriétés étranges : Chronite = temps local, Aetherite = propulsion, Xenium = scanner), **objets uniques** (nom, histoire, origine du lore CON-3), **collections** (INS-9), **musée / vitrine** (liste dans le dex, plus tard dans une base 0.17), **échange** entre joueurs. Rareté **affichée** (« 1 sur 12 000 »). | 218, 82 | M |
| **CON-7. Dangers et pièges naturels** | **Pièges** annoncés (sables mouvants MON-2, glace fine MON-3, geysers prévisibles, poches de gaz en grotte, effondrements MON-10), **zones dangereuses** (radiations AST-8, mondes brûlés), **alertes** du scanner (« signature volcanique 80 voxels devant »), **moyens de s'en sortir** (corde, grappin PER-9, fusée), **mort douce** (PER-11 : retour au vaisseau, pas de perte d'objets nécessaires). | 221 | M |
| **CON-8. Faune, aliens et sociétés (contenu)** | Lien avec **0.13 L5** (aliens) et **0.14 D4** (faune) : **dialogues** par gabarits, **langue alien** que l'on apprend (glyphes, vocabulaire dans le dex, traduction progressive), **réputation par espèce / faction**, **commerce** d'objets rares, **observation / apprivoisement** (nourrir, photographier, pas de combat), **migrations** saisonnières, **villages vus de l'espace** (lumières la nuit). | 105–110 | XL |
| **CON-9. Créatures géantes (optionnel)** | **Créatures géantes** (vrais dragons non jouables de l'éditeur, grands herbivores, **colosses** de roche, créatures de cavernes), **rencontres rares** annoncées, comportement **non agressif par défaut** (observer, éviter, fuir), version agressive **optionnelle** (mode difficile). Modèles de l'éditeur (grille grande) et races associées. | 220 | L |
| **CON-10. Défis et événements** | **Défis du jour / de la semaine** (déterministes par la graine du monde et la date : « photographier une aurore verte », « se poser sur un monde œil ») ; **événements communautaires** programmés (pluie de météores géante, passage d'une comète, éclipse totale visible d'un monde du serveur) ; **expéditions de guilde** (objectif commun, barre d'avancement partagée, récompense) ; **chasses aux anomalies** ; **classements** par serveur (non obligatoires). | 83, 10, 224 | M |
| **CON-11. Difficulté et modes** | **Modes** : *Contemplatif* (pas de danger ni de consommation), *Normal*, *Survie* (A4 complet, énergie, dangers) ; réglages fins (dégâts, rareté des ressources, fréquence des événements N) ; **mode explorateur** sans économie ni combat (idée 84) ; **mode créatif** (bac à sable) ; changement **en cours de partie**. | 223, 84, 96 | S |
| **CON-12. Narration et ambiance du texte** | **Voix du jeu** (ton sobre, scientifique, un peu poétique), **journal de bord** (entrées automatiques : « Atterrissage sur … »), **textes d'ambiance** à l'arrivée sur un monde rare, **écrans de chargement** avec anecdotes scientifiques et lore, **crédits** et mentions légales, **cinématiques** courtes en caméra libre (voyage entre galaxies 0.13 V2, rentrée P3). | 183 | M |

Ordre conseillé : **CON-1 → 2** (début et récompense), **CON-3** (le lore alimente tout), puis **CON-4, 5, 6**,
**CON-7** (les dangers vont avec MON / AST), **CON-10, 11**, puis **CON-8, 9** (dépendent de L5 / D4), **CON-12** en
continu.

## 4. Détails importants

### 4.1 Le lore (CON-3) : la source unique
`lore.rs` expose : `history(seed) -> History` (civilisations, ères, destins), `faction_origin(f)`,
`ruin_story(system)` (qui a bâti cela, quand, pourquoi c'est vide), `name(culture, seed, kind)`. Aucune donnée
stockée : tout est **recalculé** depuis la graine et les identifiants. Les **ruines** (X8, D6), **épaves**, **textes
du dex**, **missions** et **dialogues d'aliens** interrogent ce module, d'où une histoire cohérente.

### 4.2 Les quêtes comme **faits du monde**
Une quête n'est pas écrite : elle est **déduite** d'un fait (ex. « une comète passera dans 2 j » → « la
photographier ») ; donc 10 000 quêtes possibles sans stockage, et chacune est **vraie** (on peut la vérifier avec
les instruments). Les états (acceptée, étapes, terminée) sont sauvés dans `quests.json` à côté du dex.

### 4.3 Récompenses : pas d'inflation
Récompenses bornées par un **barème** (`economy::GOODS`) ; objets uniques seulement par mystères / artefacts ; pas
de monnaie qui explose ; plafonds par jour de jeu et par joueur (les quêtes en multijoueur sont validées par l'hôte).

### 4.4 Monde de départ (CON-1)
C'est la **seule** zone écrite à la main (ou tirée avec des paramètres forcés) : système 0 avec une planète
tempérée garantie (« Il faut du temps pour y arriver » : retour sur la décision de la 0.10 « départ aléatoire à
venir » du mémoire de roadmap), un guide, un premier signal. La règle de la 0.14 (Q4 : aucun monde exceptionnel
dans les 50 systèmes les plus proches) reste, pour que les extrêmes se **méritent**.

### 4.5 Localisation
Les textes de CON-1 à CON-12 passent par des **identifiants de chaînes** (`fr.json`, `en.json`) ; le lore est
construit par gabarits **traduisibles** avec accords (genre, nombre) gérés par langue. Voir l'idée 46 du rapport.

## 5. Mesures et tests

- **Tests** : même graine → même histoire (hachage), quêtes toujours **réalisables** (le test les « résout »
  par code : le but existe, est atteignable, n'est pas dans un lieu interdit), énigmes toujours **déterministes**.
- **Longueur** : une quête de base prend entre 5 et 20 minutes de jeu ; un mystère complet 1 h.
- **Stats** : `/stats` donne la répartition des types de quêtes possibles.
- **Playtest** : parcours du **tutoriel** à l'état « 3 succès » mesuré en temps.
- **Réseau** : récompenses invalides, quêtes forgées → ignorées par l'hôte (fuzz TECH-6).

## 6. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Un fil conducteur principal est-il voulu ou le jeu reste-t-il ouvert ? | Ouvert par défaut ; un **mystère principal optionnel** (signal → ruines → artefact) que l'on peut ignorer. |
| Q2 | Langue du jeu au lancement ? | Français et anglais dès CON-1 (identifiants de chaînes). |
| Q3 | Les créatures géantes sont-elles hostiles ? | Non par défaut ; optionnel en mode difficile. |
| Q4 | Quêtes de combat ? | Jamais obligatoires ; contrats de chasse en option dans les factions militaires. |
| Q5 | Les succès sont-ils partagés avec le multijoueur (classements) ? | Oui, **facultatifs** (option « Afficher mes succès aux autres »). |
| Q6 | Un monde de départ écrit à la main contredit-il « tout généré » ? | Oui, c'est la **seule** exception, assumée, pour guider le début. |
| Q7 | Qui écrit les gabarits de texte ? | Toi (ton, vocabulaire) avec moi pour la grammaire ; 200 à 400 gabarits au départ. |

## 7. Prompts

**CON-1** : « Lis `CLAUDE.md` et `roadmaps/a-faire/ROADMAP-contenu.md` §3 CON-1 et §4.4. Crée le tutoriel guidé et sautable avec
invites contextuelles, la mission d'accueil, le choix du mode de jeu, et le monde de départ scénarisé (système 0).
Capture de chaque étape. »

**CON-2** : « §3 CON-2 : succès, rangs liés au dex, titres au-dessus du nom, records, panneau « Carrière ».
Sauvegarde dans `dex.json`. Tests de déclenchement. »

**CON-3** : « §3 CON-3 et §4.1 : `lore.rs` (histoire, factions, noms, textes) déterministe depuis la graine, lore
dans le dex. Test : même graine → même histoire. »

**CON-4 à CON-12** : « Lis `roadmaps/a-faire/ROADMAP-contenu.md` §3 CON-n, implémente, teste (réalisable, déterministe), capture,
PR non fusionnée. »
