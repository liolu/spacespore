# Feuille de route — L. Mondes et surfaces

Source : bloc **L** de `roadmaps/a-faire/RAPPORT-ameliorations.md` (idées 156 à 170) + idées 111 à 118 (feu, eau, érosion, neige,
effondrements, débris, radiations, température).
Les phases s'appellent **MON-n**.

Objectif : des **surfaces qui racontent leur monde** : chaque matière, chaque liquide, chaque ciel a un aspect
et un comportement propres, qui **changent avec l'horloge** (dunes, glaciers, marées) sans stocker de données
supplémentaires.

Cette feuille de route **complète** `roadmaps/a-faire/ROADMAP-0.14.md` : D1 (rivières, lacs, cascades, banquise), D2 (géologie
active), D3 / D4 (végétation, faune), X (mondes exceptionnels) y sont déjà. Ici : ce qui n'y est pas, ou en
profondeur (matières et liquides exotiques, optique du ciel, dynamique lente du sol, neige et traces).

---

## 1. Point de départ (code du 05/10/2026)

- **Matières** : `VoxelType` (Ice, Methane, Ammonia, Lava, Ore, biomes : toundra, taïga, forêt, jungle, cristal,
  spores, verre, soufre, basalte, sel, rouille…) ; `climate::land_material` / `sea_material`, `BiomeField::material`
  (partagé par `terrain.rs` et `mesher.rs`, règle 16).
- **Liquides** (`planetgen/hydrology.rs`) : eau, méthane, ammoniac, lave, selon le diagramme de phase ; niveau de mer
  et **marées** (`terrain::Tide`) ; **vraie eau** (0.13 O1 / O2 : surface transparente, vagues, écume, sous l'eau,
  caustiques, `water.rs` / `water.wgsl` / `caustics.wgsl`) ; liquides non aqueux : couleur ?, vagues ? (à vérifier).
- **Relief** : `landforms.rs` (collines, massifs, bosses, vallées d'érosion), `rocks.rs` (falaises, arches, ponts,
  gorges, pitons, chaos), `caves.rs`, cratères (`geology.rs::craters`), géologie active (`geoactive.rs` : lave,
  geysers, fumerolles, cryovolcans, séismes).
- **Couleurs** du sol (T3) : taches de 200 / 30 / 5 voxels, roche nue sur les pentes, strates, mousse, sable, neige.
- **Ciel et météo** : `sky.rs` (orages, éclipses), `weather.rs` (vents, nuages cubiques, pluie, neige, grêle, pluies
  exotiques, poussière, foudre, brouillard), `gas.rs` (brume, géantes).
- **Décor** : `decor.rs` (voxels de la vie, champignons lumineux), tuiles à fondu (T4).
- **Pas encore** : dunes qui bougent, glaciers, banquise vivante (D1), mirages, optique atmosphérique (arc-en-ciel,
  halos), flaques et sol mouillé, traces dans la neige, sables mouvants, feu.

## 2. Règles

1. **Tout dépend de la graine et de l'horloge** (pas de simulation stockée) : une dune « avance » parce que sa
   phase est f(horloge), pas parce qu'on l'a déplacée.
2. **Terrain et maillage d'accord** (règle 16) : chaque matière / couleur nouvelle passe par la fonction partagée.
3. **Les changements durables passent par les deltas voxel** (cratères, effondrements) ; les cycles
   (marée, saison, neige qui s'accumule) sont **calculés**, pas stockés.
4. Les nouveaux phénomènes ont leur **conditions physiques** (température, pression, humidité, vent) pour ne pas
   apparaître n'importe où ; sinon, étiquetés « fictif ».
5. **Budget** : tout effet par pixel ou par particule a son coût mesuré (règle 8) et un réglage « Détail du sol ».
6. **Scanner** : tout nouveau phénomène a sa ligne dans « Ici et maintenant » ou « Environnement ».
7. Pas de bouton « magique » : un liquide se comporte selon ses propriétés (viscosité, couleur, reflet).

## 3. Les phases

| Phase | Contenu | Idées | Taille |
|---|---|---|---|
| **MON-1. Liquides et surfaces liquides** | Chaque liquide a **sa couleur, sa transparence, sa viscosité, ses vagues** (eau, méthane, ammoniac, hydrocarbures de type Titan, lave, mer de sel) via un `LiquidLook` passé au shader d'eau ; **lacs d'hydrocarbures** (brume orange, reflets mats, rivages) ; **lave en surface** (croûte qui craque, îles qui flottent, éclat de nuit) ; **plaines de sel** (blanches, polygonales, miroir mince après la pluie) ; mer de méthane gelée / à moitié gelée. | 157, 158, 170 | L |
| **MON-2. Sables, poussières, sols meubles** | **Mers de sable** (ergs) à **dunes qui avancent** (phase f(horloge, vent) : la crête se déplace de quelques voxels par heure de jeu, forme barkhane / linéaire / étoile selon le vent dominant) ; **sables mouvants** (on s'enfonce, danger lent) ; **mers de poussière** sur les mondes sans air (lien gravité faible) ; **sol meuble** : traces de pas et de vaisseau qui persistent un peu (voir MON-5). | 156, 162, 221 | L |
| **MON-3. Glace vivante** | **Glaciers** (langues qui descendent, crevasses, moraines, fronts qui avancent / reculent selon la saison), **grottes de glace bleue** (lumière filtrée, `caves.rs` style glace), **banquise** qui se fend et se refroidit (complète D1), **geysers de glace** (T5 existe), **glace transparente** / **noire** (X3 de la 0.14), neige qui s'accumule selon la météo. | 163, 114 | L |
| **MON-4. Roche et minéraux spectaculaires** | **Forêts de cristaux** (diffraction selon l'heure et la position du soleil, éclat, sons), **champs de météorites** au sol (mondes sans air, cratères récents, fer et fragments), **sources chaudes et bassins colorés** (D6 de la 0.14), **dépôts de soufre**, **cheminées hydrothermales** sous-marines avec vie (lien MON-1 sous l'eau), roches **filtrant la lumière** (obsidienne, quartz, verre). | 159, 161, 164, 165 | L |
| **MON-5. Neige, sol mouillé et traces** | **Neige accumulée** (épaisseur = f(précipitation, température, pente, saison) jusqu'à fondre au printemps), **empreintes** dans la neige, le sable et la boue (anneau de 200 empreintes par joueur, effacées par le vent / la pluie / le temps), **sol mouillé** après la pluie (plus sombre, brillant, flaques qui se remplissent et sèchent), **boue**, **rosée** au matin (lien givre du matin A3), humidité de l'air. | 114 | M |
| **MON-6. Ciel : optique et astres multiples** | **Arcs-en-ciel** (pluie + soleil bas, angle réel 42°), **halos, parhélies, piliers de lumière** (cristaux de glace), **couronnes** autour de la lune, **éclipses vues du sol** avec ombre mobile, **plusieurs lunes de couleurs différentes** (reflet de leur sol : lien AST-9), **aurores colorées selon les gaz** (vert O, rouge O haut, bleu N₂, violet, méthane…), crépuscule à la teinte de l'atmosphère, **ciel qui noircit** avec l'altitude sans coupure (0.13 P2). | 160, 166, 167 | L |
| **MON-7. Mirages et chaleur** | **Mirages** (ondulations de l'horizon et lacs fantômes sur sol brûlant, f(gradient de température, hauteur de l'œil)), **ondulation de l'air** au-dessus de la lave et du sable chaud, **brume de chaleur**, **vapeur** qui monte des sources chaudes, **vagues de chaleur** et de **froid** (journée extrême) liées à `LocalWeather`. Post-traitement léger limité à l'horizon. | 168 | M |
| **MON-8. Marées intérieures, rivières lentes et eaux souterraines** | **Lacs et grottes à marée** (la mer qui entre dans les grottes), **niveau des nappes** (puits, sources, cenotes) qui monte avec les pluies, **crues** (lien D1 rivières), **inondations** locales (eau qui monte en quelques heures de jeu puis redescend), **marais** et **zones humides** (sol saturé, brume, moustiques en bioluminescence nocturne si vie). | 169, 112 | L |
| **MON-9. Feu, cendres et repousse** | **Incendies** (foudre, lave, impacts : la végétation brûle de proche en proche selon le vent et l'humidité), fumée visible de loin, **cendres** qui retombent (sol sombre, ciel voilé), **repousse** sur des semaines de jeu (herbe → arbustes → arbres), mondes sans oxygène sans feu. Les brûlures sont des **deltas de couleur** peu nombreux (pas de simulation continue). | 111 | M |
| **MON-10. Érosion et stabilité du sol** | **Effondrements** de grottes sous les séismes (T5) et le minage (0.15), **éboulements** sur pentes fortes, **glissements de terrain** après de fortes pluies, **érosion visible** à longue durée (falaise qui recule de quelques voxels par année de jeu, delta qui avance), **sinkholes**. Tout via **deltas voxel**, bornés, annulables, avec message au joueur. | 115, 113 | L |
| **MON-11. Physique des débris** | Blocs minés ou tombés qui **roulent et s'arrêtent** (petit nombre de corps, durée de vie courte), poussières soulevées, poussière au pied des falaises ; limites strictes (≤ 200 corps), désactivable. Pas de physique continue des liquides. | 116 | M |
| **MON-12. Danger et ressenti du sol** | **Radiations** locales (sol, roches, ciel) lisibles au scanner et au HUD (lien A4), **atmosphères toxiques**, **poches de gaz** en grotte, **température ressentie** (vent, humidité, tenue, abri, feu de camp), **pièges naturels** (glace fine, sables mouvants, geysers prévisibles). Dangers toujours **annoncés** avant de blesser. | 117, 118, 221 | M |
| **MON-13. Détail, optimisation, options** | Réglage « Détail du sol » étendu (dunes, glaciers, optique), budget de particules, **LOD de la neige et des traces**, mémoire, mesures (`SPACESPORE_PERF`) sur chaque phase ; options pour **réduire flashs et secousses** (lien CTL-7). | — | M |

Ordre conseillé : **MON-1 → 2 → 3** (le plus visible depuis l'orbite et la marche), puis **5, 6**, puis
**4, 7, 8**, puis **9, 10, 11, 12** ; **13** en continu.

## 4. Détails importants

### 4.1 MON-1 : un seul shader de liquide
`water.wgsl` reçoit un `LiquidLook` (couleur de fond, couleur d'absorption, rugosité, indice de réfraction, viscosité
qui amortit les vagues, mousse oui / non, émissif pour la lave). Un monde de méthane n'a plus « de l'eau bleue » ;
la lave est **opaque**, émissive et lente ; les hydrocarbures sont sombres et mats avec une brume orange.

### 4.2 MON-2 : dunes sans simulation
Une dune est une **hauteur supplémentaire** `dune(x, y, t) = A·profil((x − v·t) / L)` ajoutée au champ de hauteur
par `landforms.rs` sur les mondes sableux (le vent dominant vient de `weather::zonal`). Terrain et mesher utilisent
**la même fonction** ; les tuiles proches sont reconstruites quand la phase a avancé d'un voxel (comme
`update_tides` / `update_season`). Le joueur voit les dunes bouger sur des heures de jeu.

### 4.3 MON-3 : glaciers
Langue de glace = tube de hauteur le long de la plus grande pente (`lava_path` de `geoactive.rs` est une base),
largeur et longueur f(précipitation cumulée), surface avec crevasses (bruit à crêtes). Front qui bouge de
quelques voxels par année de jeu. Intérieur creusé en grotte bleue (`CaveStyle::Ice` existe déjà).

### 4.4 Empreintes
Un `Vec<Footprint>` borné par astre et par joueur (200), dans l'état local (pas sauvé par défaut, option « garder
mes traces » en deltas). Le vent / la pluie les effacent par opacité qui décroît.

### 4.5 Fictif étiqueté
Forêts de cristaux chantants, dunes qui chantent, mers de lave sur monde froid : tous marqués « fictif » au scanner
comme les minerais de la phase 8.

## 5. Mesures et tests

- Chaque phase : un monde de test dédié via `/aller planete <type>` (règle 2 de `TESTS-JEU.md` : monde adapté),
  conditions vérifiées avant capture (jour, liquide, vent).
- Tests unitaires : fonctions pures (`dune`, `snow_depth`, `mirage`, `glacier_profile`) bornées, déterministes.
- Banc de reconstruction de tuiles quand une phase change (dunes, neige) : pas de pic > 33 ms.
- Terrain / maillage d'accord (TECH-2) : les nouvelles matières apparaissent à l'identique des deux côtés.

## 6. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Les traces des joueurs persistent-elles en solo entre deux sessions ? | Non par défaut ; option pour les garder en deltas. |
| Q2 | Les incendies peuvent-ils détruire une base ? | Non tant qu'il n'y a pas de bases (0.17) ; alors, assurance. |
| Q3 | Les glaciers sont-ils des voxels réels ? | Oui (glace) pour la collision, avec un déplacement lent par phase. |
| Q4 | Effondrement de grotte : danger pour le joueur ? | Annoncé (secousse, poussière 3 s avant), dégâts légers, jamais mortel d'un coup. |
| Q5 | Mirages : gadget visuel ou aussi trompeur (faux lac au scanner) ? | Visuel seulement ; le scanner dit la vérité. |

## 7. Prompts

**MON-1** : « Lis `CLAUDE.md`, `TESTS-JEU.md` et `roadmaps/a-faire/ROADMAP-mondes-surfaces.md` §3 MON-1 et §4.1. Ajoute `LiquidLook`
au shader d'eau, les lacs d'hydrocarbures, la lave en surface et la plaine de sel. Capture sur un monde adapté
(jour vérifié) pour chaque liquide. »

**MON-2** : « §3 MON-2 et §4.2 : dunes f(horloge, vent) partagées terrain / mesher, sables mouvants, mers de
poussière, reconstruction de tuile à chaque voxel de phase. Banc de tuiles avant / après. »

**MON-3 à MON-13** : « Lis `roadmaps/a-faire/ROADMAP-mondes-surfaces.md` §3 MON-n, implémente, teste, capture, mesure, PR non
fusionnée. »
