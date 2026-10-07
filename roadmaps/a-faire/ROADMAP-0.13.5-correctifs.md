# Feuille de route — Correctifs et petites idées (0.13.5)

Objectif : corriger **tous les bugs ouverts** de `doctravail/LISTE.md` (tri du 07/10/2026 : 21 captures de la
v0.13.3.1 + tes notes) et faire les **petites idées** qui ne demandent pas une feuille de route à elles seules.
Chaque bug a sa **cause cherchée dans le code** (§2) : on corrige la cause, pas le symptôme.

| Source | Ce qui est repris |
|---|---|
| `C:\Users\thomr\Desktop\Nouveau dossier\doctravail\LISTE.md` | Bugs B1 à B20, idées 💡, anciens bugs ❓ |
| `doctravail\01-bugs\actuels-v0.13\` | Les captures de référence (une par bug) |
| `doctravail\02-notes-du-jeu\Nouveau Document texte (2).txt` | Tes retours en vrac |

Ce qui est **déjà prévu ailleurs** n'est pas repris ici (skybox, trous noirs, voyage entre galaxies, aliens :
0.13 C2, V1-V3, L5-L6 ; glaciers, végétation : 0.14 ; quantités de matière : 0.15 ; mondes variés : 0.16).

Chaque phase = une branche `claude/correctifs-0-13-5-<phase>`, une PR non fusionnée : tu testes, puis tu dis
« push main ». Chaque phase a son **prompt prêt à coller** (§6).

---

## 1. Règles des correctifs

1. **La cause d'abord.** Avant de toucher au code : reproduire le bug (outils de F0), écrire la cause trouvée dans
   la PR, puis corriger. Pas de « réglage au hasard » qui cache le symptôme.
2. **Une capture avant / après au même endroit**, même heure, même angle (outils de F0, `TESTS-JEU.md`). Jamais
   de PR de rendu sans capture qui montre le bug corrigé. Ne pas relancer deux fois le même test sans rien changer.
3. **Un test automatique quand c'est possible** (couleur de loin = couleur de près, forme non ronde, traînée
   alignée...) pour que le bug ne revienne pas.
4. **Règle 16 tenue** : ce qui se voit de l'espace et ce qui se voit au sol viennent de la même fonction.
5. **Coût mesuré** : `bench_tiles`, `bench_voxel_tiles`, `SPACESPORE_PERF` avant / après pour toute phase qui
   touche au terrain ou au rendu ; pas plus de 5 % de perte sans le dire.
6. **PROTOCOL +1** seulement si la génération partagée change (F4).

---

## 2. Les bugs et leur cause

| # | Bug (capture) | Cause trouvée dans le code | Phase |
|---|---|---|---|
| B1 | Planète **en escalier avec des fentes noires** vue de l'espace (`Capture 2026-10-06 170805`, déjà en v0.11) | `mesher.rs::build_chunk_mesh` empile **12 couches** (`layers = 12`) entre `r_min` et `r_max`, qui incluent maintenant les massifs en voxels (`forms.max_height()`, jusqu'à 400 voxels) : chaque marche est énorme. Les faces de côté sont **toujours** posées au bord du morceau (`get_voxel` hors du morceau = air) et regardent sur le côté : sombres, elles font les fentes noires | F1 |
| B2 | Rigel 1 **rouge vue de l'espace**, beige / gris au sol (`planete couleur rouge`, `planete rouge 2`) | Pas trouvé à la lecture : les biomes sont les mêmes (`biomes.material`). À reproduire (F0) : lumière de l'étoile ou matériau du maillage lointain (`planet.rs`), teinte de la pente (`ground_tint` avec des pentes faussées par les marches de B1) | F1 |
| B3 | **Lignes droites** : bandes rectangulaires, falaise toute droite, carré clair vu de l'espace, **plaques plates qui flottent** (`gen droite`, `gen droite 2` à `5`, `gen droite simple`) | Pistes : cellules **carrées de 60 voxels** de `rocks.rs` (corniches `Piece::Add` en dalle alignées sur la cellule), bords des **faces de la sphère-cube**, tuiles d'une autre finesse restées affichées. À trancher en F0 (afficher les bords de cellules et de tuiles) | F2 |
| B4 | **Grands pans plats** devant la caméra en vol bas (`clipping`) | Pistes : tuile grossière qui reste pendant le fondu (`TileStore::fading`, 0,35 s) ou qui recouvre la fine ; plan proche (`near_plane`) | F2 |
| B11 | Mares **coupées par une ligne droite**, blocs qui flottent (`bug visuel 2`) | Couture de tuile sur l'eau (`water.rs`) ; chaos de blocs (`rocks.rs`, `Boulders`) posé sur le sol d'une tuile plus grossière | F2 |
| B5 | Comète : **disque gris plat**, noyau rayé (moiré) avec un reflet en miroir, **traînée pas alignée** (`comete bug`, `comete moche`, `moche 2`, `non aligne comete flare`) | `asteroids.rs::update_comet_tails` : la chevelure est une **sphère ico(3) unie** en mélange additif (pas de dégradé vers le bord) = disque à facettes. La traînée (`draw_trails`) est recalculée depuis `clock.secs` en Update, alors que la comète est posée en PreUpdate : ancrage différent. Moiré : ombres du noyau sur lui-même (acné d'ombre) | F3 |
| B14 | `/ceinture` et `/comete` mènent **toujours au même endroit**, astres trop petits pour être cliqués | `go_comet` : « la comète la plus active » ; `/ceinture` : « le champ le plus dense » : toujours le même par construction | F3 |
| B6 | **Geysers sans trou ni cône**, toujours seuls (`pas de trou ou geyser`) | `geoactive.rs` ne fait que des **particules** : le terrain ne change pas ; une seule bouche par cellule de 400 voxels (`cell_vent`) | F4 |
| B8 | **Tous les trous sont ronds** (`tout les trou sont rond 2`) | `geology.rs::CraterKind::profile` est **purement radial** (cercle parfait, rebord régulier) ; `caves.rs::Shape` n'a que capsules, sphères et dalles **sans bruit** | F4 |
| B9 / B20 | **Faille en cylindre parfait**, jamais de faille très verticale (`faille cyclindre améiore`) | `caves.rs` : la faille (`CaveKind::Fault`) est faite de formes lisses (`Shape`) | F4 |
| B10 | **Canyon comme un trait** dessiné (`trait`) | Canyons de `geology.rs` (bruit lisse à bords nets) et gorges de `rocks.rs` (formes régulières) | F4 |
| B7 | Monde de lave : « mers 59 % (lave) » mais **aucune lave visible**, **pluie de verre dessinée comme de l'eau** (`Capture 2026-10-06 173108`) ; la ligne « Eau » du scanner affiche la lave | Particules de pluie d'une seule couleur (`weather.rs`) ; mers de lave à vérifier sur place (`/mer`) ; `scanner.rs` met la mer de lave dans la ligne « Eau » | F5 |
| B18 | **Pas plus chaud au-dessus de la lave** | `suit.rs` : dégâts seulement **dans** la lave (`Environment::lava`) | F5 |
| B17 | **La lumière traverse le sol** et éclaire des faces qui devraient être à l'ombre | Ombres seulement jusqu'à ~200 voxels (cascades) ; option « Ombres du relief » coupée par défaut (−40 % d'images/s) : au-delà, rien n'arrête le soleil | F6 |
| B12 | **Éditeur** : la fenêtre « Nouveau modèle » s'affiche par-dessus le panneau d'outils (`image-1791032083964`) | `editeur/panels.rs` : la fenêtre n'a pas de fond opaque au-dessus du panneau, ou le panneau n'est pas masqué | F7 |
| B15 | On peut **encore cliquer une étoile** sous 98 M de zoom | `main.rs::clickables` (0.11.4 C6) | F7 |
| B16 | « Les volcans n'ont pas l'air d'exister », pas de `/volcan` | Les volcans du relief existent (`ReliefField`) mais rien ne mène à eux ; `/geologie lave` cherche une coulée active, pas un volcan | F7 |
| B13 | « `/impact` n'est que visuel » | Le code fait un cratère en deltas voxel (`meteors.rs::impact_crater`) : à vérifier sur place, peut-être trop petit à l'échelle k = 16 | F7 |
| B19 | **Le jeu plante** sur une planète habitable | `main.rs` écrit `saves/crash.log` à côté du jeu. Les deux trouvés sur ce PC sont anciens : `%APPDATA%\spacespore\crash.log` (28/09, `GasPlanetRes`) et `target\release\saves\crash.log` (29/09, `LodTask` inséré sur une entité disparue, corrigé depuis par `try_insert`). Le même genre de plantage reste possible : **8 `.insert(` dans `planet.rs`** sur des entités qui peuvent disparaître. Le rapport du plantage récent est dans le dossier du jeu installé par le launcher | F0, F8 |

Anciens bugs à **revérifier** (❓, captures de la v0.11) : lignes d'orbite visibles depuis le sol, titre « Vega »
alors que le scanner montre « GJ-13806 A », marcheur à −18 m dans un brouillard blanc. Vérifiés en F8.

---

## 3. Les phases

### F0 — Outils pour reproduire (avant tout le reste)

- **`/astre <nom>`** : aller à un astre par son nom (« Rigel 1 », « NGC-2410 5 », « Capella 1 ») dans les galaxies
  générées, comme `/aller` (recherche en arrière-plan, téléportation au bord du système, cible chargée).
- **`/pos`** : copie la position exacte (astre, latitude, longitude, altitude, cap, heure du monde) ;
  **`/pos <code>`** et `SPACESPORE_TEST_POS=<code>` y ramènent : même endroit, même heure, même angle pour les
  captures avant / après.
- **Affichage de débogage** (`F3` déjà pris : option dans `/debug`) : bords des tuiles (couleur par finesse), bords
  des cellules de `rocks.rs`, bords des faces de la sphère-cube, morceaux du maillage lointain.
- **Rapport de plantage complet** : `crash.log` reçoit la version, la graine, l'astre, la position (`/pos`), les
  10 dernières commandes du chat ; au lancement suivant, message « le jeu a planté : rapport dans … » avec le
  chemin. Le launcher sait ouvrir le dossier.
- Une **liste de positions de référence** (une par bug, graine `7JBP-WQA8`) dans `TESTS-JEU.md`.

### F1 — Planètes vues de l'espace (B1, B2)

- Remplacer le maillage lointain en **12 couches** par un **champ de hauteur lisse** (un sommet par case, hauteurs
  réelles de `terrain_heights`, comme `build_height_tile_mesh` du terrain) avec des **jupes** courtes au bord des
  morceaux (pas de faces de côté pleine hauteur) : plus d'escalier, plus de fentes.
- Sommets **partagés** entre morceaux voisins (mêmes directions, mêmes hauteurs) : pas de couture.
- Mers : une surface au niveau de la mer, comme aujourd'hui.
- **B2** : reproduire Rigel 1 (`/astre Rigel 1`), trouver la cause (lumière, matériau, teinte), corriger.
- Test `far_mesh_matches_ground` : pour 200 directions d'une dizaine d'astres, la couleur et la hauteur du maillage
  lointain sont celles de la tuile la plus fine (à l'arrondi près).
- Mesures : temps de construction d'un morceau, nombre de triangles (il doit baisser).

### F2 — Terrain en vol bas (B3, B4, B11)

- Avec l'affichage de F0, trancher la cause des **lignes droites** (B3) : cellules carrées de `rocks.rs`, faces du
  cube, tuiles d'une autre finesse. Correction selon la cause : bords des corniches et des falaises **suivant le
  relief** (bruit, pas la cellule), fondu entre cellules, ou couture des tuiles.
- **Plaques qui flottent** : une forme (`Piece::Add`) ne se pose que si le sol de la tuile **où elle est
  affichée** est sous elle ; sinon elle attend la tuile fine.
- **Pans devant la caméra** (B4) : une tuile en fondu ne se dessine pas devant la tuile fine (profondeur, ordre) ;
  plan proche vérifié en vol bas rapide.
- **Mares coupées** (B11) : surface de l'eau continue entre tuiles ; chaos de blocs posé sur le vrai sol.
- Captures avant / après aux positions de référence de NGC-2410 5 et TYC-3852 4.

### F3 — Comètes (B5, B14)

- **Chevelure** : sphère à **bord doux** (opacité qui baisse vers le bord, matériau qui tient compte de l'angle de
  vue), plus de facettes ; couleur qui suit l'activité.
- **Noyau** : plus d'acné d'ombre (biais de profondeur ou pas d'ombre sur lui-même), normales lissées.
- **Traînée** : ancrée sur la position **affichée** de la comète (même instant, même origine), test
  `comet_trail_starts_at_comet`.
- `/comete` et `/ceinture` : **le suivant à chaque appel** (comète suivante par activité, champ suivant par
  densité) ; cercle de clic plus grand pour les petits corps du système chargé.

### F4 — Formes naturelles (B6, B8, B9, B10, B20)

Changement de la génération partagée : **PROTOCOL +1**.

- **Cratères** : rebord **irrégulier** (bruit sur le rayon), ~5 % **elliptiques** (impacts rasants), cratères
  **polygonaux** sur les vieilles surfaces, petits cratères **secondaires** autour des grands, recouvrements. Test :
  aucun cratère n'est un cercle parfait.
- **Grottes et failles** : `Shape` reçoit une **déformation par bruit** (parois irrégulières) ; vraie **faille
  verticale** = fente haute et étroite, en zigzag, avec des blocs coincés.
- **Canyons et gorges** : tracé **sinueux**, bords irréguliers, terrasses et éboulis.
- **Geysers** : **cône de geysérite** (dépôt clair) et **trou** dans le terrain (pièces dans `Terrain::kind_in`,
  règle 16), **champs** de 3 à 12 bouches (dont des mares chaudes) au lieu d'une seule ; même chose pour les
  fumerolles (dépôts de soufre).
- Tests de forme ; `bench_voxel_tiles` avant / après.

### F5 — Monde de lave et météo exotique (B7, B18)

- Aller sur Capella 1 (`/astre`, `/mer`) : vérifier les **mers de lave** (matière, lueur, vue de l'espace) et
  corriger ce qui manque.
- **Précipitations** de la bonne couleur et de la bonne forme : verre (éclats gris brillants), fer (gouttes sombres),
  méthane (orangé), acide (jaunâtre), neige carbonique (flocons blancs) (`weather.rs`).
- **Scanner** : ligne « Eau » (eau seulement) et ligne « Mers » (le liquide : lave, méthane, ammoniac) séparées.
- **Chaleur de la lave** (B18) : `suit.rs` ajoute la chaleur rayonnée par la lave proche (selon la distance), au
  HUD et dans la survie.

### F6 — Lumière qui traverse le sol (B17)

- **Visibilité du soleil précalculée** par sommet dans les tuiles (horizon du relief vers le soleil, recalculée
  quand le soleil bouge d'un pas) : les versants à l'ombre d'une montagne le restent même sans « Ombres du
  relief ».
- **Occlusion ambiante** légère par sommet (creux, pieds de parois, grottes).
- Mesure : coût par tuile et images/s ; capture d'une vallée au coucher du soleil avant / après.

### F7 — Interface et commandes (B12, B13, B15, B16)

- **Éditeur** : la fenêtre « Nouveau modèle » passe au-dessus d'un fond opaque, le panneau d'outils est masqué
  dessous.
- **Clic** : sous 98 M de zoom (vue système), plus aucune étoile d'un autre système n'est cliquable
  (`clickables`) ; test.
- **`/volcan`** : aller au volcan du relief le plus proche (ou le plus haut du système) ; le scanner donne le volcan
  le plus proche comme la grotte.
- **`/impact`** : vérifier le cratère (taille à l'échelle k = 16, deltas sauvés, partagés) et corriger.

### F8 — Plantage et anciens bugs (B19, ❓)

- Récupérer le `crash.log` du jeu installé (dossier du launcher), ou reproduire avec le rapport de F0 sur des mondes
  habitables (`/aller planete habitable`), corriger la cause, test.
- Passer en `try_insert` les `.insert(` restants sur des entités qui peuvent disparaître dans la même image
  (8 dans `planet.rs`, puis les autres fichiers : chercher `commands.entity(..).insert`).
- Revérifier les anciens bugs ❓ : lignes d'orbite au sol, nom du système dans le titre et le scanner, marcheur
  sous le sol. Corriger ceux qui sont encore là.

### F9 — Petites idées (💡)

- **Biome où l'on est** dans le HUD (à pied et en vol bas), avec la température.
- **Orbite des lunes** : traînée comme pour les planètes (système chargé).
- **Nuage transparent** quand il passe entre la caméra et le vaisseau (fondu de la couche de nuages près de la
  caméra).
- **Capture d'écran** : touche **F9** (F12 sert au profilage) → `saves/captures/<date>_<astre>.png`, et option de
  capture automatique toutes les N minutes.
- **Effets en option** (menu Options) : activer / couper séparément caustiques, écume, aurores, particules de météo,
  lueur des étoiles (« shaders en option »).
- Mise à jour de `TOUCHES.md` (F9, `/astre`, `/pos`, `/volcan`).

### F10 — Éditeur : petites idées

- **Vue à plat** du modèle (face, dos, côtés) avec une **lettre par partie** (tête, bras, jambes...) ; cliquer une
  lettre sélectionne la partie, le 3D se met à jour.
- **Choisir le côté** d'un membre : jambe droite / gauche, bras droit / gauche (zone renommée et reflétée).
- **Écailles** et autres motifs de peau : pinceau qui pose un motif (écailles, plumes, rayures, taches) avec 2 ou 3
  teintes de la palette.

---

## 4. Ordre et versions

```
v0.13.4 (bloc P de la 0.13)
   │
   F0                   outils (tout le reste s'en sert)
   F1 → F2              ce qu'on voit en premier : la planète de loin, puis le sol en vol bas
   F3                   comètes
   │                    ── release 0.13.5 « Correctifs : rendu » ──
   F4 → F5 → F6         formes naturelles (PROTOCOL +1), lave et météo, lumière
   │                    ── release 0.13.6 « Correctifs : mondes » ──
   F7 → F8 → F9 → F10   interface, plantage, petites idées, éditeur
                        ── release 0.13.7 « Correctifs : confort » ──
   │
   suite de la 0.13 (P, C, V, L) puis 0.14
```

---

## 5. Questions restantes

Aucune bloquante ; choix par défaut, à changer si tu veux :

| # | Sujet | Choix par défaut |
|---|---|---|
| Q1 | Touche de capture d'écran | **F9** (F12 = profilage, Impr. écran = Windows) |
| Q2 | Pollution (idée 💡) | **Pas ici** : trop grosse, notée dans `roadmaps/a-faire/A-FAIRE-PLUS-TARD.md` (avec les aliens et les constructions) |
| Q3 | Foreuse « trou noir » (idée à préciser) | **Pas ici** : reste dans `roadmaps/a-faire/IDEES-constructions.md` jusqu'à ce que tu la décrives |
| Q4 | Captures de référence | Restent sur le Bureau (`doctravail`, ~35 Mo, trop lourd pour le dépôt) ; les positions sont dans `TESTS-JEU.md` |

---

## 6. Prompts (à coller dans une nouvelle session, un par phase)

Contexte commun : « Lis `roadmaps/a-faire/ROADMAP-0.13.5-correctifs.md` (règles §1, causes §2), `CLAUDE.md`, `TESTS-JEU.md` et
`C:\Users\thomr\Desktop\Nouveau dossier\doctravail\LISTE.md` ; regarde les captures du bug dans
`doctravail\01-bugs\actuels-v0.13\`. `git pull origin main` avant de coder. Branche
`claude/correctifs-0-13-5-<phase>`. Reproduis d'abord, écris la cause dans la PR, puis corrige ; capture avant /
après au même endroit ; tests ; mesures si terrain ou rendu. PR non fusionnée (je dirai « push main »). »

- **F0** — « [contexte commun] Phase F0 : `/astre <nom>`, `/pos` et `SPACESPORE_TEST_POS`, affichage de débogage
  (tuiles, cellules de `rocks.rs`, faces du cube, morceaux lointains), rapport de plantage complet et message au
  lancement suivant, positions de référence dans `TESTS-JEU.md`. »
- **F1** — « [contexte commun] Phase F1 : maillage lointain en champ de hauteur lisse avec jupes et sommets partagés
  (B1), couleur de Rigel 1 vue de l'espace (B2), test `far_mesh_matches_ground`, mesures. »
- **F2** — « [contexte commun] Phase F2 : lignes droites et plaques flottantes (B3), pans devant la caméra (B4),
  mares coupées et blocs flottants (B11), d'après la cause trouvée avec l'affichage de F0. »
- **F3** — « [contexte commun] Phase F3 : chevelure à bord doux, noyau sans moiré, traînée ancrée sur la comète
  affichée (B5) ; `/comete` et `/ceinture` passent au suivant, clic plus facile (B14). »
- **F4** — « [contexte commun] Phase F4 : cratères irréguliers, elliptiques, polygonaux (B8) ; grottes et failles
  déformées, failles verticales (B9, B20) ; canyons sinueux (B10) ; geysers avec cône, trou et champs (B6) ;
  PROTOCOL +1, tests de forme, `bench_voxel_tiles`. »
- **F5** — « [contexte commun] Phase F5 : mers de lave de Capella 1 (B7), précipitations exotiques colorées, lignes
  Eau / Mers du scanner, chaleur rayonnée par la lave (B18). »
- **F6** — « [contexte commun] Phase F6 : visibilité du soleil et occlusion par sommet dans les tuiles (B17), mesures
  et capture d'une vallée au coucher du soleil. »
- **F7** — « [contexte commun] Phase F7 : fenêtre Nouveau modèle de l'éditeur (B12), clic des étoiles sous 98 M
  (B15), `/volcan` et volcan au scanner (B16), vérification de `/impact` (B13). »
- **F8** — « [contexte commun] Phase F8 : plantage sur une planète habitable (B19) avec le rapport de F0 ; anciens
  bugs ❓ revérifiés et corrigés. »
- **F9** — « [contexte commun] Phase F9 : biome au HUD, orbite des lunes, nuage transparent devant le vaisseau,
  capture F9 et capture automatique, effets en option, `TOUCHES.md`. »
- **F10** — « [contexte commun] Phase F10 : vue à plat avec lettres par partie, côté droit / gauche des membres,
  motifs de peau (écailles...) dans l'éditeur. »
