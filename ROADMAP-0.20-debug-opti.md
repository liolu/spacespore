# Feuille de route — Débogage et optimisation (0.20)

Objectif : **reprendre le contrôle** du LOD et de la lumière (les deux plus gros problèmes actuels) et
**mesurer** au lieu de deviner. Trois outils, dans cet ordre :

1. un **panneau de réglages en direct** (LOD, lumière, ombres, terrain, streaming, ciel) ;
2. des **mesures** et des **vues de débogage** qui montrent ce que le moteur fait ;
3. un **benchmark automatique** : le jeu pilote seul le vaisseau sur un parcours fixe, change les
   réglages un par un et enregistre tout dans des fichiers que l'IA analyse.

Ensuite seulement, on **optimise** le LOD et la lumière à partir des chiffres.

Source : conversation avec ChatGPT (`prompt0.20.md`), qui ne connaissait pas le projet. Cette feuille
de route garde ses bonnes idées et les **adapte au code réel** (Bevy 0.15, origine flottante,
~12 500 systèmes, tuiles de terrain asynchrones).

Chaque phase se termine par un build instable jouable. Une PR par phase, non fusionnée : tu testes,
puis tu dis « push main ». Chaque phase a son **prompt prêt à coller** (section « Prompts »).

---

## 1. Ta demande, reformulée

### 1.1 Benchmark

- Le joueur **n'a pas la main** : le jeu emmène le vaisseau de planète en planète, plonge vers la
  surface, regarde autour de lui, regarde les galaxies, passe par tout ce qui est lourd, **toujours dans
  le même ordre**.
- On **change des variables** entre deux passages pour voir laquelle coûte le plus et laquelle fait
  gagner le plus. Exemple : `a=10 b=25 c=80` pendant 1 min, puis `c=50`, puis `c=30`, puis `a=15`…
- À chaque passage : FPS et toutes les autres mesures.

**Verdict :** la méthode est bonne. Pour qu'elle soit fiable, il faut :

| Problème | Solution |
|---|---|
| Trop de combinaisons (5 variables × 5 valeurs = 3 125 passages = 52 h) | **Une variable à la fois** d'abord (§5.3), puis seulement les paires qui comptent |
| Le monde est procédural : une autre planète fausse la comparaison | **Graine fixe, parcours fixe, heure fixe** (§5.1) |
| Un passage peut tomber sur un pic (compilation de shader, tâche lente) | **Préchauffage** de 20 s puis **3 répétitions**, on garde la **médiane** |
| Le FPS moyen cache les saccades | Mesurer les **temps d'image**, le **1 % bas**, le **0,1 % bas** et les **pics** (§4.1) |
| Savoir que c'est lent ne dit pas **pourquoi** | Mesurer **chaque système** (terrain, lumière, étoiles…) en ms (§4.2) |

### 1.2 Panneau de réglages

- Une **page en jeu** pour régler **en direct** le LOD et la lumière, sans recompiler.
- Ajouter ce qui manque (culling, ombres, streaming, terrain, vues de débogage, préréglages).

---

## 2. Point de départ (code actuel, v0.11.0)

Le problème principal : **les réglages sont des constantes éparpillées dans six fichiers**. Une IA qui
« optimise le LOD » ne sait pas lesquelles toucher, et rien ne se règle en jeu.

### 2.1 LOD : il y en a six, indépendants

| LOD | Réglage aujourd'hui | Fichier |
|---|---|---|
| Sphère des planètes vue de loin | 6 niveaux, seuils fixes à **3 / 5 / 7 / 10 / 14 rayons**, résolutions **20 / 14 / 10 / 6 / 4 / 2**, multiplié par « Détail planètes » (`lod_quality` 0,5 à 1,5) | `lod.rs` |
| Tâches de LOD des astres | `MAX_LOD_TASKS_IN_FLIGHT = 8`, `STAR_DETAIL_RADII = 6`, `DETAIL_CUTOFF` | `planet.rs` |
| Terrain proche (quadtree de tuiles) | `SPLIT_FACTOR = 1,8`, `TILE_CELLS = 32`, `MAX_VOXEL = 11`, `MAX_TILE_TASKS = 10`, `MAX_TILES = 520`, `TILE_KEEP_SECS = 8` | `terrain.rs`, `surface.rs` |
| Décor des tuiles | `MAX_DENSITY = 0,1`, calculé avec la tuile | `decor.rs` |
| Étoiles lointaines (secteurs de ~100) | `LOD_STARS_END / GONE`, `LOD_CAPS_START / FULL`, `LOD_STEPS = 10` | `planet.rs` |
| Masquage des astres | `astre_lod_cull` (distance + direction caméra), recharge différée | `astre/mod.rs` |

### 2.2 Lumière : plusieurs sources qui se superposent

| Lumière | Aujourd'hui | Fichier |
|---|---|---|
| Étoile | `PointLight` (portée `light_range`, ombres **activées** sur l'étoile chargée) : une ombre de lumière ponctuelle = **6 rendus** (cubemap), très cher | `planet.rs` (~l.1363, ~l.2048) |
| Soleil au sol | `DirectionalLight` `SunLight` (éclairement réglé selon l'heure, pas d'ombres) | `main.rs`, `surface.rs` |
| Ambiante | Constantes `AMBIENT_SPACE = 300`, `AMBIENT_DAY = 1 500`, `AMBIENT_NIGHT = 35`, `MOONLIGHT_MAX = 260`, brumes `NIGHT_HAZE`, `DAY_HAZE`, `NIGHT_GALAXY` | `surface.rs` |
| Phares, lampe (N) | `PointLight` du vaisseau, lampe du joueur | `ship.rs`, `surface.rs` |
| Brouillard | `DistanceFog` des géantes gazeuses et de l'horizon | `gas.rs` |
| Nébuleuses | `PointLight` | `astre/Remnant_stellaire/nebula.rs` |
| Ombres | Un seul interrupteur (`settings.shadows`), pas de taille de carte, pas de cascades réglées | `graphics.rs` |

### 2.3 Mesures existantes

| Outil | Ce qu'il fait | Fichier |
|---|---|---|
| `FrameTimeDiagnosticsPlugin` | FPS et temps d'image lissés, affichés en haut à gauche | `main.rs` |
| F12 `ProfilingLog` | Journal texte : FPS min / max / moyenne, astres masqués et rechargés | `astre/mod.rs` |
| Préréglages Bas → Ultra | MSAA, ombres, détail planètes, nuages, éruptions | `graphics.rs` |
| `/aller etoile|planete|lune <type>` | Téléporte vers un astre d'un type donné (base du parcours de benchmark) | `test_cmd.rs` |
| `/heure`, `/temps` | Horloge du monde (0.11 A1) : permet de **figer l'heure** pendant un test | `world_clock.rs` |
| Bancs `bench_tiles`, `bench_decor` | Coût d'une tuile et du décor, hors jeu | `planetgen` (tests) |

Touches F déjà prises : F1 (vaisseau), F2 (réseau), F3 (statistiques), F12 (journal).

---

## 3. Règles d'architecture

1. **Un seul endroit pour les réglages.** Une ressource `Tuning` (module `src/tuning.rs`), rangée par
   groupes : `lod.planete`, `lod.terrain`, `lod.decor`, `lod.etoiles`, `lumiere.etoile`,
   `lumiere.soleil`, `lumiere.ambiante`, `ombres`, `brouillard`, `culling`, `streaming`. **Toutes** les
   constantes du §2 y passent. Valeurs par défaut = valeurs actuelles : la phase T1 ne change **rien** à
   l'image.
2. **Les réglages sont des données.** `Tuning` se lit et s'écrit en JSON (`saves/vX.Y.Z/tuning/*.json`).
   Le benchmark, le panneau et l'IA utilisent **le même fichier**.
3. **Réglage ≠ débogage.** Les réglages changent le jeu. Les vues de débogage (couleurs de LOD, fil de
   fer…) ne servent qu'à **voir** et ne sont jamais enregistrées.
4. **Distances séparées.** Pour chaque couche : distance de **rendu**, de **génération**, de
   **collision**, de **suppression**. Aujourd'hui elles sont confondues.
5. **Budgets en millisecondes, pas en nombre.** « 10 tâches de tuiles » devient « 2 ms de génération
   par image » : une grosse tuile ne bloque plus l'image.
6. **Mesurer ne doit rien coûter quand c'est éteint.** Les mesures fines sont des `info_span!` de
   `tracing` (Bevy) et des compteurs ; éteintes, elles ne coûtent rien.
7. **Benchmark reproductible.** Graine fixe, parcours fixe, heure figée, réseau coupé, VSync et limite de
   FPS désactivées, taille de fenêtre fixe, build **release**. Le fichier de résultat note la machine
   (CPU, GPU, RAM, pilote), la version et le commit.
8. **Pas de régression de taille.** Aucune optimisation ne réduit une distance, un rayon ou une échelle
   du monde (règle des feuilles 0.10 et 0.11). On optimise le **coût**, pas le **monde**.

---

## 4. Ce qu'on mesure

### 4.1 Mesures « joueur » (est-ce que ça tourne bien ?)

| Mesure | Pourquoi |
|---|---|
| FPS moyen et **médian** | Performance générale |
| **1 % bas**, **0,1 % bas** | Fluidité réelle |
| **Temps d'image max** et nombre de **pics > 33 ms** | Les saccades que le joueur sent |
| RAM du processus | Fuites, caches qui grossissent |
| Nombre d'entités, de maillages, de matériaux | Fuites d'assets (maillages jamais libérés) |

Repère : 60 FPS = 16,7 ms ; **120 FPS = 8,3 ms** (cible de la règle 13 de la 0.11) ; 144 FPS = 6,9 ms.

### 4.2 Mesures « moteur » (pourquoi c'est lent ?)

| Groupe | Mesures |
|---|---|
| CPU par système | LOD planètes, quadtree du terrain, réception des tuiles, décor, étoiles et secteurs, masquage des astres, lumière / jour-nuit, physique et collisions, interface, réseau |
| Tâches asynchrones | tuiles en cours / en attente, temps moyen et max d'une tuile, tâches de LOD des astres |
| GPU | temps total et **par passe** (ombres, opaque, transparent, post-traitement) avec `RenderDiagnosticsPlugin` de Bevy 0.15 si le GPU le permet |
| Rendu | triangles affichés, nombre d'objets dessinés, lumières actives, lumières avec ombres |
| Monde | tuiles chargées par niveau, astres visibles / masqués, systèmes chargés, secteurs d'étoiles |

**Profilage profond** (option) : build avec la fonctionnalité `trace_tracy` de Bevy pour ouvrir une
capture dans **Tracy** et voir chaque système image par image.

### 4.3 Budget de l'image

Panneau qui compare chaque groupe à son budget (cible 120 FPS = 8,3 ms) :

```
Terrain        2,1 / 2,0 ms  🟠
Lumière        3,8 / 1,5 ms  🔴
Ombres         3,2 / 1,5 ms  🔴
LOD planètes   0,4 / 1,0 ms  🟢
Étoiles        0,6 / 1,0 ms  🟢
```

---

## 5. Le benchmark automatique

### 5.1 Parcours fixe

Lancement : `spacespore.exe --bench <plan.json>` (ou `/bench <nom>` dans le chat). Le jeu charge une
**graine de benchmark** fixe dans une sauvegarde à part (`saves/bench/`), coupe le réseau, fige l'heure
(`/heure`), puis enchaîne les étapes sans intervention :

| # | Scénario | Ce qu'il charge | Ce qu'il mesure surtout |
|---|---|---|---|
| A | **Voyage** : étoile G → planète rocheuse → géante gazeuse → système suivant → trou de ver | streaming des systèmes, LOD des sphères, tâches de LOD | saccades de chargement, RAM |
| B | **Plongée** : orbite → descente jusqu'à `FLIGHT_ZOOM` → vol bas → atterrissage (V) → tour à 360° → horizon → regard vers le ciel | quadtree des tuiles, décor, brume, soleil | génération de tuiles, décor, lumière |
| C | **Nuit au sol** : même lieu, heure de nuit, ciel étoilé, galaxie, lune, lampe | étoiles, galaxie, ambiante de nuit, lumières locales | lumière, transparence |
| D | **Galaxie** : on recule jusqu'à voir la galaxie entière, puis les galaxies voisines | secteurs d'étoiles, effets de galaxie (`galaxy_fx.rs`) | nombre d'objets, LOD des étoiles |
| E | **Géante gazeuse** : on entre jusqu'au cœur | brouillard, `gas.rs` | brouillard, transparence |
| F | **Pire cas** : planète à vie dense (beaucoup de décor), au coucher du soleil, ombres activées, lune visible, phares allumés, vol rapide à basse altitude | tout en même temps | ce qui casse en premier |

Les astres du parcours sont trouvés **une fois** par la recherche de `/aller` puis **figés par leur
identifiant** dans le plan : le parcours ne change jamais. La caméra suit des **trajectoires écrites**
(points + durées), pas des commandes clavier.

### 5.2 Fichiers produits

Dossier `saves/bench/<date>-<nom>/` :

- `frames.csv` : une ligne par image (`t, scenario, etape, frame_ms, cpu_ms, gpu_ms, tuiles, taches,
  entites, triangles…`) ;
- `resume.csv` : une ligne par passage (`config, a, b, c, fps_median, low1, low01, max_ms, pics, ram…`) ;
- `rapport.md` : tableau lisible + les 5 réglages qui ont le plus d'effet ;
- `machine.json` : CPU, GPU, RAM, pilote, version, commit, plan utilisé.

### 5.3 Plan de balayage des variables

`plan.json` décrit les passages :

```json
{
  "graine": 483729,
  "scenarios": ["B", "C", "F"],
  "prechauffage_s": 20, "duree_s": 60, "repetitions": 3,
  "base": "tuning/base.json",
  "balayages": [
    { "var": "lod.terrain.split_factor", "valeurs": [1.2, 1.5, 1.8, 2.2, 2.6] },
    { "var": "ombres.taille_carte",      "valeurs": [512, 1024, 2048, 4096] },
    { "var": "lod.decor.densite_max",    "valeurs": [0.02, 0.05, 0.1] }
  ],
  "croisements": [ ["lod.terrain.split_factor", "ombres.taille_carte"] ]
}
```

1. **Base** : réglages actuels, 3 répétitions.
2. **Une variable à la fois** : les autres restent à la base. Courbe « coût selon la valeur ».
3. **Croisements** seulement pour les 2 ou 3 variables qui comptent : on vérifie si elles s'additionnent
   ou s'aggravent (A seul −15 %, C seul −12 %, A + C −40 % = elles interagissent).
4. Le rapport classe les variables : **effet sur le FPS** et **effet sur l'image** (à vérifier à l'œil
   sur les captures prises au même instant de chaque passage).

Durée typique : 3 scénarios × 1 min × 3 répétitions = ~10 min par valeur ; un balayage complet se lance
le soir et se lit le lendemain.

### 5.4 Le travail avec l'IA

On ne demande plus « optimise le jeu ». On donne `rapport.md` et `resume.csv` :

> « `lod.terrain.split_factor` 1,8 → 2,6 : GPU 8,1 → 17,9 ms, 1 % bas 82 → 41. `decor.densite_max` :
> effet < 0,3 ms. Trouve dans le code ce qui explique la première courbe. »

---

## 6. Le panneau de réglages en direct

Touche **F6** (ou `/reglages`). Panneau à onglets, chaque valeur avec un curseur, sa valeur par défaut
et un bouton « remettre ».

| Onglet | Contenu |
|---|---|
| **Performance** | FPS, temps d'image (courbe des 5 dernières secondes), 1 % bas, CPU / GPU, budget par groupe (§4.3), compteurs du §4.2 |
| **LOD** | Sphères : 5 seuils (en rayons) et 6 résolutions, multiplicateur. Terrain : `split_factor`, profondeur max, taille max d'un voxel, tuiles max, durée de garde. Étoiles : distances des secteurs et des points. **Forcer un niveau** : AUTO / LOD0 … LOD5. **Geler le LOD** (le LOD ne bouge plus, on peut tourner autour pour voir ce qui est chargé). **Hystérésis** (marge pour éviter le clignotement entre deux niveaux). |
| **Terrain et décor** | Distances de rendu / génération / collision / suppression (règle 4), budget de génération en ms par image, tâches max, densité du décor, distance du décor |
| **Lumière** | Étoile : intensité, portée, couleur. Soleil au sol : éclairement jour, aube, crépuscule. Ambiante : espace, jour, nuit, teinte. Lune, brume de jour et de nuit, éclat de la galaxie la nuit. Phares et lampe : intensité, portée, angle. **Exposition** de la caméra. |
| **Ombres** | Activées (étoile ponctuelle / soleil directionnel), taille des cartes (ponctuelle et directionnelle), **cascades** (nombre, distances, recouvrement), biais de profondeur et de normale, distance max des ombres |
| **Brouillard et ciel** | Brouillard des géantes (visibilité, couleur), brume d'horizon, nuages, éruptions |
| **Culling** | Interrupteurs et distances : astres (`astre_lod_cull`), décor, étoiles, nébuleuses ; afficher ce qui est masqué |
| **Streaming** | Distances de chargement / déchargement des systèmes et des astres, chargements max par image, tâches de LOD en vol |
| **Préréglages** | Bas / Moyen / Haut / Ultra / **Patate** / Personnalisé ; **enregistrer**, **charger**, **exporter / importer JSON** ; **comparer A / B** (une touche bascule entre deux réglages pour voir la différence d'image et de FPS) |

---

## 7. Les vues de débogage

Touche **F7** pour passer d'une vue à l'autre (ou liste dans l'onglet). Rien n'est enregistré.

| Vue | Ce qu'elle montre |
|---|---|
| Normale | Le jeu |
| **Couleurs de LOD** | LOD0 rouge, LOD1 orange, LOD2 jaune, LOD3 vert, LOD4 bleu, LOD5 violet ; tuiles de terrain colorées par profondeur. « Pourquoi cette montagne est-elle encore en LOD0 à 4 km ? » |
| **Bords des tuiles** | Contour de chaque tuile et de chaque secteur d'étoiles, avec son niveau |
| **Fil de fer** | `WireframePlugin` de Bevy : densité réelle des maillages |
| **États du streaming** | 🟢 chargé, 🟡 en cours, 🔵 généré pas encore affiché, 🔴 en déchargement, ⚫ masqué |
| **Cascades d'ombres** | Rouge / jaune / vert / bleu par cascade |
| **Lumière seule** | Tout en gris, on ne voit que l'éclairage (trouver les trous et les excès) |
| **Normales** | Normales en couleur (faces à l'envers, coutures entre tuiles) |
| **Collisions** | Volumes de collision du marcheur et du vaisseau |
| **Masquage** | Ce que `astre_lod_cull` cache, vu depuis une caméra libre détachée |

**Caméra libre détachée** (option des vues) : on fige la caméra du jeu (LOD, masquage, ombres calculés
depuis elle) et on se promène avec une seconde caméra pour voir le monde « de l'extérieur ».

---

## 8. Phases

| Phase | Contenu | Taille |
|---|---|---|
| **T1. Réglages centralisés** | Module `src/tuning.rs` : ressource `Tuning` rangée par groupes (règle 1), **toutes** les constantes du §2 remplacées par des lectures de `Tuning`, chargement / enregistrement JSON (règle 2). Les préréglages de `graphics.rs` passent par `Tuning`. Valeurs par défaut identiques : **aucun changement visible** (captures avant / après dans la PR). | L |
| **T2. Mesures** | Spans `tracing` sur les systèmes du §4.2, compteurs (tuiles, tâches, entités, maillages, triangles, lumières), `RenderDiagnosticsPlugin` (GPU par passe si possible), RAM du processus, 1 % / 0,1 % bas sur une fenêtre glissante, pics. Petit affichage F4 (FPS, ms, CPU / GPU, budget). Le journal F12 utilise ces mesures et écrit un CSV. Option `trace_tracy`. | M |
| **T3. Panneau de réglages** | Le panneau F6 du §6, réglage en direct (chaque changement s'applique à l'image suivante, sans recharger le monde quand c'est possible, sinon bouton « reconstruire »), préréglages, export / import JSON, comparaison A / B. | L |
| **T4. Vues de débogage** | Les vues du §7, forcer et geler le LOD, caméra libre détachée. | M |
| **T5. Benchmark** | Mode `--bench` : sauvegarde de benchmark à graine fixe, réseau coupé, heure figée, trajectoires écrites, scénarios A à F (§5.1), fichiers du §5.2, captures d'écran aux mêmes instants. | L |
| **T6. Balayage** | `plan.json` (§5.3) : base, une variable à la fois, croisements, répétitions, médianes, `rapport.md` avec le classement des variables. Première campagne complète, résultats joints à la PR. | M |
| **T7. Optimiser le LOD** | À partir des résultats de T6. Pistes : **budget en ms** au lieu de 10 tâches (règle 5) ; **hystérésis** contre le clignotement ; distances séparées rendu / génération / collision (règle 4) ; tuiles prioritaires devant la caméra et au centre de l'écran ; **transition** douce entre niveaux (fondu ou morphing des hauteurs) ; seuils de sphère reliés à la taille **à l'écran** (pixels) plutôt qu'en rayons ; décor en **instances** partagées ; libération vérifiée des maillages. | L |
| **T8. Optimiser la lumière** | À partir des résultats de T6. Pistes : **plus d'ombre cubemap** de l'étoile près d'une planète (6 rendus) : le soleil directionnel à cascades fait les ombres au sol, la lumière ponctuelle seulement l'éclairage lointain ; cascades réglées sur l'altitude ; une seule lumière avec ombres à la fois ; portée des phares et de la lampe limitée ; ambiante et brume continues entre espace, orbite et sol (plus de saut) ; exposition automatique douce. | L |
| **T9. Préréglages finaux** | Bas / Moyen / Haut / Ultra / Patate recalculés depuis les mesures (chaque préréglage = un objectif de FPS sur la machine de test), proposés dans le menu Options. Benchmark rejoué : tableau avant / après 0.20 dans la PR. | S |

Ordre : T1 → T2 → T3 → T4 → T5 → T6 → (T7 et T8 dans l'ordre que donnent les mesures) → T9.

---

## 9. Ce qui manquait dans la liste de départ

- **Six LOD indépendants** (§2.1) : la conversation ChatGPT parle d'un seul LOD ; ici il faut régler la
  sphère, le terrain, le décor, les étoiles, le masquage et le streaming séparément.
- **Ombres d'une lumière ponctuelle** : c'est probablement le premier coût caché (6 rendus de la scène).
- **Hystérésis** du LOD : sans marge, une tuile au seuil change de niveau à chaque image.
- **Saccades de compilation des shaders** au premier affichage d'un matériau : d'où le préchauffage.
- **Fuites d'assets** : maillages et matériaux créés par tuile et jamais libérés (compteur T2).
- **Origine flottante** : le recentrage (au-delà de 100 000) peut provoquer un pic ; il est mesuré à part.
- **Captures d'écran** à chaque passage : un réglage qui double le FPS mais rend le jeu moche n'est pas
  un gain.
- **Machine notée** dans chaque résultat : les chiffres d'un autre PC ne se comparent pas.
- **VSync et limite de FPS coupées** pendant le benchmark, sinon tout plafonne à 60 ou 144.

---

## 10. Questions à trancher

| # | Question | Proposition |
|---|---|---|
| Q1 | Interface du panneau : **bevy_egui** (curseurs, onglets et courbes tout faits, une dépendance de plus) ou **bevy_ui** maison (comme le reste du jeu) ? | **bevy_egui** pour le panneau de débogage seulement : 5 fois moins de code, et il ne touche pas l'interface du jeu. |
| Q2 | Cible de FPS ? | **120 FPS** (8,3 ms) sur la machine de test, comme la règle 13 de la 0.11 ; préréglage Patate à 60 FPS sur une petite machine. |
| Q3 | Panneau et vues accessibles aux joueurs, ou réservés au développement ? | Dans tous les builds, mais **cachés** : `/reglages` et F6 seulement après `/debug`. |
| Q4 | Benchmark : durée maximale d'une campagne ? | Une nuit (~8 h) ; un passage rapide (base seule, 3 scénarios) en ~5 min pour vérifier une PR. |
| Q5 | Quelle est la machine de test (CPU, GPU, RAM) ? | À noter ici une fois pour toutes. |
| Q6 | Les PR des autres versions (0.11 B/C/D, 0.12 éditeur) doivent-elles passer le benchmark rapide ? | Oui à partir de T6 : tableau avant / après dans chaque PR qui touche au rendu. |

---

## 11. Prompts (à coller dans une nouvelle session, une par phase)

Chaque prompt suppose : « Lis `ROADMAP-0.20-debug-opti.md` et `CLAUDE.md`. Crée la branche
`claude/roadmap-0-20-tX` depuis `main`. Build release. Ouvre une PR non fusionnée avec mesures et
captures. »

- **T1** — « Phase T1 de `ROADMAP-0.20-debug-opti.md` : crée `src/tuning.rs` avec la ressource `Tuning`
  rangée par groupes (règle 1). Remplace toutes les constantes listées au §2 (`lod.rs`, `terrain.rs`,
  `surface.rs`, `planet.rs`, `decor.rs`, `gas.rs`, `graphics.rs`, `astre/mod.rs`) par des lectures de
  `Tuning`, avec les valeurs actuelles par défaut. JSON dans `saves/vX.Y.Z/tuning/`. Aucun changement
  visible : joins des captures avant / après au même endroit. »
- **T2** — « Phase T2 : ajoute les mesures du §4 (spans `tracing`, compteurs, `RenderDiagnosticsPlugin`,
  RAM, 1 % et 0,1 % bas, pics), l'affichage F4 et le CSV du journal F12. Option de build `trace_tracy`.
  Vérifie que les mesures éteintes ne coûtent rien (FPS avant / après). »
- **T3** — « Phase T3 : panneau F6 du §6 (selon la décision Q1), réglage en direct de tout `Tuning`,
  préréglages, export / import JSON, comparaison A / B. »
- **T4** — « Phase T4 : vues de débogage du §7 (F7), forcer et geler le LOD, caméra libre détachée. »
- **T5** — « Phase T5 : mode `--bench` du §5 : graine fixe, réseau coupé, heure figée, trajectoires
  écrites, scénarios A à F, fichiers `frames.csv`, `resume.csv`, `rapport.md`, `machine.json`,
  captures. »
- **T6** — « Phase T6 : balayage `plan.json` du §5.3 (base, une variable à la fois, croisements,
  3 répétitions, médianes, classement). Lance une première campagne sur les variables de LOD et
  d'ombres et joins `rapport.md` à la PR. »
- **T7** — « Phase T7 : optimise le LOD d'après le dernier `rapport.md` (pistes du §8). Chaque
  changement : benchmark avant / après et captures. Aucune distance du monde réduite (règle 8). »
- **T8** — « Phase T8 : optimise la lumière et les ombres d'après le dernier `rapport.md` (pistes du §8),
  benchmark avant / après et captures jour, nuit, coucher, espace. »
- **T9** — « Phase T9 : recalcule les préréglages depuis les mesures, mets-les dans le menu Options,
  rejoue le benchmark complet et joins le tableau avant / après 0.20. »
