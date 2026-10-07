# Rapport — Génération du terrain (04/10/2026, après la v0.11.3)

Constat en jeu : **le terrain est très plat**, **l'eau n'est pas transparente**, le détail s'arrête
près du joueur. Ce rapport explique **pourquoi** à partir du code (✅ = lu dans le code, 🔍 = à vérifier
en jeu) et propose des corrections. Rien n'est encore modifié.

---

## 1. Les chiffres d'une planète type Terre

Planète rocheuse de 1 R_terre = échelle du système / 109, soit **~5 500 à 13 800 unités** de rayon.
Exemple pris ici : **R = 9 000 unités**.

| Grandeur | Valeur | D'où ça vient |
|---|---|---|
| Taille d'un voxel | **~7 unités** (5,5 à 11 selon le rayon) | `terrain.rs:192`, `MAX_VOXEL = 11` |
| Taille du marcheur | **~2 voxels** (yeux à 1,8) | `surface.rs:411`, décision du 03/10 |
| Rayon de la planète **en voxels** | **~1 300 voxels** | 9 000 / 7 |
| Si 1 voxel ≈ 1 m (le marcheur fait 2 voxels) | **planète de ~1,3 km de rayon** | (la Terre : 6 371 km) |
| Horizon vu des yeux du marcheur | **~70 voxels** | √(2 × 1 300 × 1,8) |
| Amplitude totale du relief (`terrain_height`) | **2,5 à 5 % du rayon** = 225 à 450 unités = **32 à 64 voxels** | `planetgen/system.rs:366` |
| Hauteur des terres au-dessus de la mer (le plus souvent) | **0 à ~16 voxels** | continent ±0,25 × `terrain_height` |
| Montagnes (crêtes des plaques) | **~10 voxels** en moyenne, **20 au plus** | `geology.rs:187` et `:588` : `mountains` 0,12 à 0,24, × 0,3 à 1,3, × `terrain_height` |
| Où sont les montagnes | **seulement en bandes le long des limites de plaques** | `geology.rs:588` : poids `w²` de la limite |
| Crêtes | longueur d'onde **~90 voxels** | `ridged(..., 14.0)` = R / 14 |
| Continents | ±10 voxels en général, longueur d'onde **~500 voxels** | `noise_scale` 1,5 à 3,5 |
| Collines (bruit moyen) | **±5 à ±10 voxels** en général, longueur d'onde ~260 voxels | `terrain.rs:450`, `MID_WAVE = 1800` unités |
| Relief fin | **±1 voxel** sur ~30 voxels | `terrain.rs:451`, `FINE_WAVE = 200` unités |

**Pentes qui en résultent :** hors des chaînes de montagnes (la plus grande partie des terres), les
pentes font **3 à 5°**, avec des écarts de hauteur de quelques voxels sur des centaines. Les chaînes
sont plus raides (~20°) mais basses (~10 voxels) et rares. À pied, presque tout paraît plat.

---

## 2. Pourquoi c'est plat

### 2.1 Le relief est réglé en **pourcentage du rayon**, pas à l'échelle du marcheur ✅

`terrain_height` = 2,5 à 5 % du rayon (`planetgen/system.rs:366`). Par rapport à la Terre (l'Everest =
0,14 % du rayon), c'est déjà **20 à 35 fois exagéré**. Mais la planète ne fait que **~1 300 voxels de
rayon** : 5 % de 1 300 = 65 voxels pour **tout** le relief, des fosses aux sommets. Le marcheur mesure 2
voxels : une montagne de 10 voxels, c'est un immeuble de 3 étages.

C'est le cœur du problème : **la planète est petite en voxels**, et tout le relief est proportionnel à
sa taille.

### 2.2 Les longueurs d'onde sont trop grandes pour ces hauteurs ✅

| Couche | Hauteur | Largeur | Effet |
|---|---|---|---|
| Continents (`noise_scale` 1,5 à 3,5) | ±10 voxels | ~500 voxels | plaines immenses |
| Montagnes (`ridged` × 14) | ~10 voxels (20 au plus) | ~90 voxels, en bandes étroites | collines, rares |
| Collines (`MID_WAVE`) | ±5 à 10 voxels | ~260 voxels | ondulations |
| Détail fin (`FINE_WAVE`) | ±1 voxel | ~30 voxels | à peine visible |

Il manque une couche entre « collines » et « détail fin » (bosses de 3 à 10 voxels sur 30 à 80 voxels)
et des **pentes raides** (falaises, pics, ravins). L'érosion (`rugged = 1 − 0,5 × érosion`) adoucit
encore tout sur les mondes à air et à eau.

### 2.3 Les cubes transforment les pentes douces en grandes marches ✅

La hauteur est arrondie au voxel (`terrain.rs:516`, cubes = décision Q3 de la 0.11). Une pente de 4°
monte d'un voxel tous les ~14 voxels : on voit de **grands paliers plats** séparés par une marche d'un
bloc. Plus la pente est faible, plus le terrain paraît plat et en escalier.

### 2.4 L'horizon est très proche ✅

À ~70 voxels, l'horizon coupe la vue (planète de 1 300 voxels de rayon). Le relief lointain ne se voit
que si on est sur un sommet. Les tuiles les plus fines (3D) ne couvrent que ~2 tuiles de 32 voxels
autour du joueur (`SPLIT_FACTOR = 1,8`, `terrain.rs:291`) : au-delà, le relief est simplifié. Voir R1
dans `IDEES-prochaine-version.md`.

### 2.5 Peu de formes 3D ✅

Les tuiles fines sont en 3D, mais les seules formes 3D sont : grottes (`caves.rs`), arches et cheminées
de fée (`rocks.rs`, une cellule de 60 voxels sur plusieurs, mondes avec air) et l'arche de test
(`Overhang`). Pas de falaises en surplomb, de pitons, de blocs éboulés, de rochers posés hors du décor.

---

## 3. Pourquoi l'eau n'est pas transparente

✅ **L'eau est un bloc opaque posé à la place du fond.** Dans `terrain.rs:520-536` (`base_column`) :

1. On calcule la hauteur du sol `h`.
2. S'il est sous le niveau de la mer, la colonne prend **le type « eau »** et son sommet est remonté
   **au niveau de la mer**. La hauteur du fond est **jetée** : elle ne sert qu'à assombrir la couleur
   (`depth` jusqu'à 45 %).
3. La couleur de l'eau a une opacité de **1,0** (`planet.rs:496`, `[0.18, 0.42, 0.85, 1.0]`).
4. Les tuiles n'ont **qu'un seul matériau opaque** pour tout (`surface.rs:2302`, rugosité 0,95) :
   l'eau est dessinée comme de la roche bleue.

Conséquences : pas de fond marin, pas de transparence, pas de reflet, pas de vagues, pas de rivage qui
descend sous l'eau. De l'espace (`mesher.rs:403`), même chose : la mer est une couleur.

Même cas pour le méthane, l'ammoniac (opaques normaux) et la lave (opaque, ce qui est juste).

---

## 4. Autres points relevés

| Point | Constat | |
|---|---|---|
| Couleurs du sol | Variation de ±10 % seulement (`terrain.rs:525`, bruit à l'échelle 12) : grandes plaques uniformes | ✅ |
| Rivières et lacs | Inexistants (prévus en D1, `roadmaps/a-faire/A-FAIRE-PLUS-TARD.md`) | ✅ |
| Plages | Grève seulement à marée basse ; pas de pente de plage douce | ✅ |
| Sous l'eau | Le marcheur détecte l'eau (`surface.rs:357`) mais il n'y a pas de volume d'eau où plonger | 🔍 |
| Lunes | `terrain_height` = 3,5 à 5,5 % du rayon (`system.rs:298`), encore plus petites en voxels : encore plus plates | ✅ |

---

## 5. Propositions

### T1. Relief à l'échelle du marcheur (le plus important)

Garder la forme générale (continents, plaques, mers) en % du rayon, et ajouter des couches dont la
hauteur est en **voxels**, indépendante du rayon :

| Couche | Hauteur | Largeur | Où |
|---|---|---|---|
| Collines | 8 à 25 voxels | 60 à 150 voxels | partout sur terre, moins sur les plaines érodées |
| Massifs | 40 à 120 voxels | 200 à 500 voxels | le long des plaques (crêtes `ridged` plus serrées et plus hautes) |
| Pics et falaises | 15 à 60 voxels | 10 à 40 voxels | dans les massifs : bruit de crêtes avec **seuil** (pente > 45° = paroi verticale en 3D) |
| Bosses et rochers | 2 à 6 voxels | 6 à 20 voxels | partout (couche qui manque aujourd'hui) |
| Déformation du domaine | — | — | `domain warping` : les crêtes et vallées se tordent au lieu d'être régulières |

Contrainte : sur une planète de 1 300 voxels de rayon, un massif de 120 voxels = 9 % du rayon : il se
verra de l'espace (bosses sur le contour). C'est un choix de style (voir Q1). Les formes doivent passer
par la même fonction pour le sol proche et le maillage lointain (règle 11 de la 0.11 :
`raw_height_full`, partagée par `terrain.rs` et `mesher.rs`). Mesurer `bench_tiles` avant / après.

### T2. Vraie eau

- **Garder la hauteur du fond** sous la mer (fond marin réel, plages qui descendent, fosses).
- **L'eau = une surface à part** : maillage séparé par tuile, au niveau de la mer (marées comprises),
  avec son propre matériau **transparent** (`AlphaMode::Blend`), reflet selon l'angle (Fresnel),
  couleur et opacité qui augmentent avec la **profondeur** (on voit le fond près du bord).
- **Vagues** légères (déplacement des sommets ou normales animées selon le vent de `weather.rs`).
- **Sous l'eau** : brouillard bleu, lumière atténuée, nage. Même chose pour méthane et ammoniac (leur
  couleur), la lave reste opaque et lumineuse.
- De l'espace : mer plus sombre et brillante (reflet de l'étoile).

### T3. Couleurs et matières

Variation de couleur sur plusieurs échelles (taches de 5, 30, 200 voxels), couleur selon la pente
(roche nue sur les pentes fortes, herbe sur le plat), strates sur les falaises, neige sur les faces
tournées vers le haut seulement.

### T4. Horizon et détail

Voir R1 (`IDEES-prochaine-version.md`) : tuiles fines plus loin, budget en millisecondes. Avec T1, les
sommets lointains dépassent l'horizon : il faut qu'ils restent détaillés.

### Option à étudier : planètes plus grandes en voxels

Doubler le rayon des planètes (ou réduire le voxel sous le marcheur) recule l'horizon et rend le relief
plus crédible, mais **change toutes les distances**, le coût des tuiles et des décisions passées
(taille du marcheur ~2 blocs). Non proposé sans ton accord (voir Q2).

---

## 6. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Accepter des montagnes qui se voient sur le contour de la planète depuis l'espace (style « petite planète », comme *No Man's Sky*) ? | Oui, avec des massifs de 40 à 120 voxels ; montagnes plus basses sur les lunes. |
| Q2 | Faut-il agrandir les planètes en voxels (option à étudier) ? | Non pour l'instant : T1 suffit à casser l'impression de plat. |
| Q3 | Quand faire ces changements ? | Après les correctifs de la v0.11.4 : T1 et T2 s'ajoutent aux idées de la 0.12 (`IDEES-prochaine-version.md`), avant R1. Changer la génération = nouveau `PROTOCOL`. |
