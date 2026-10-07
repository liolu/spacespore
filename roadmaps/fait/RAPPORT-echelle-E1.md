# Rapport — Étude d'échelle (0.13 E1, 04/10/2026)

But : choisir **k**, le facteur qui rend le voxel k fois plus petit (la planète k fois plus grande en
voxels, ses rayons en unités inchangés). Prototype : `/echelle k` ou `SPACESPORE_SCALE=k` (non
sauvegardé, au prochain atterrissage), `terrain::layout_scaled`.

## 1. Banc (`cargo test --release bench_scale -- --ignored --nocapture --test-threads=1`)

6 planètes rocheuses (0,5 à 2 R_terre, rayons 4 600 à 18 100 unités), marcheur posé (yeux à 1,8 voxel),
toutes les tuiles choisies par le quadtree autour de lui, construites une à une (un seul fil).

| k | Voxel | Rayon | Profondeur | Horizon | Tuiles fines jusqu'à | Tuiles / atterrissage | Temps (1 fil) | ms / tuile | Maillages | Précision f32 |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 6,82 u | 1 304 vx | 7 | 67 vx | 132 vx | 202 (47 fines) | 0,35 s | 1,72 | 65 Mo | 0,0003 vx |
| 8 | 0,85 u | 10 430 vx | 10 | 188 vx | 130 vx | 303 (39 fines) | 1,16 s | 3,81 | 185 Mo | 0,0022 vx |
| **16** | **0,43 u** | **20 861 vx** | **11** | **266 vx** | **89 vx** | **327 (32 fines)** | **2,06 s** | **6,31** | **264 Mo** | **0,0045 vx** |
| 32 | 0,21 u | 41 722 vx | 12 | 376 vx | 87 vx | 346 (26 fines) | 5,09 s | 14,70 | 370 Mo | 0,0090 vx |
| 64 | 0,11 u | 83 443 vx | 13 | 532 vx | 39 vx | 351 (8 fines) | 7,65 s | 21,80 | 340 Mo | 0,0180 vx |

- **Le coût par tuile monte avec k** : le relief reste réglé en % du rayon (règle T1 à venir), il fait donc
  k fois plus de voxels de haut ; les tuiles fines en 3D ont des parois bien plus hautes (beaucoup de
  faces et de cases évaluées). T1 (relief en voxels) changera ces chiffres.
- **Les tuiles fines ne couvrent pas l'horizon** dès k = 8 (89 voxels sur 266 à k = 16) : c'est E3 et T4.
- **Précision** : l'écart entre deux `f32` voisins au rayon de la planète reste sous 0,02 voxel même à
  k = 64 ; le repère fixe de l'astre en `f32` suffit au sol (règle 19 à surveiller pour le bruit fin).

## 2. En jeu (parcours fixe)

Sol 1, atterrissage à 8 s (`SPACESPORE_TEST_LAND=8`), puis marche tout droit en sautant
(`SPACESPORE_TEST_WALK=1`) ; mesure de 16 à 36 s (`SPACESPORE_PERF`), Ultra, écran limité à 180 images/s ;
mémoire = plus grand ensemble de travail du processus.

| k | Images/s médianes | 1 % bas | Images > 33 ms | Pire image | Mémoire max |
|---|---|---|---|---|---|
| 1 | 180 | 143 | 0 | 12,0 ms | 879 Mo |
| 8 | 180 | 159 | 0 | 11,3 ms | 1 105 Mo |
| **16** | **180** | **108** | **0** | **14,5 ms** | **1 258 Mo** |
| 32 | 180 | 106 | 0 | 19,7 ms | 1 304 Mo |
| 64 | 180 | 95 | 0 | 14,1 ms | 1 026 Mo |

Captures : k = 16 propre ; **k = 64 : sol presque noir et marcheur contre une paroi** (relief k fois plus
haut en voxels, ombres réglées en voxels sur des montagnes immenses) : la limite est atteinte.

**Temps de descente orbite → sol** : inchangé par k (les durées sont en **unités**, §2.2 de la feuille de
route) ; E2 les repasse en voxels et ajoute le vol suborbital (Q3).

## 3. Choix

**k = 16** (décision Q1 : « k = 16 sauf mesures mauvaises ») :
- aucune image au-dessus de 33 ms en jeu, 1 % bas à 108 images/s, +380 Mo ;
- horizon ×4 (266 voxels), rayon Terre ~21 000 voxels ;
- 2 s de génération par atterrissage sur un fil (≈ 0,3 s sur les 8 cœurs), à lisser par E3.

k = 32 tiendrait en images/s, mais chaque tuile coûte 2,3 fois plus qu'à k = 16 et les tuiles fines ne
couvrent qu'un quart de l'horizon : à reconsidérer **après T1 et E3** (relief en voxels, streaming), qui
changent ces coûts. k = 64 est trop loin (sol noir, parois géantes).
