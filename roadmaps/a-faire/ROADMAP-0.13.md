# Feuille de route — Des mondes à la bonne échelle (0.13)

Objectif : des planètes **beaucoup plus grandes en voxels**, un **vrai relief** (montagnes, falaises,
vallées), une **vraie eau**, un ciel et un espace spectaculaires, puis les aliens et la terraformation.
La faune, la végétation vivante, les rivières, la géologie active, le son et les points d'intérêt
(bloc D de la 0.11) sont dans la **0.14** (`roadmaps/a-faire/ROADMAP-0.14.md`). **Aucune concession sur la qualité** : la performance s'obtient par la mesure,
le LOD et le travail en arrière-plan, jamais en retirant du détail.

Cette feuille de route rassemble :

| Source | Ce qui est repris |
|---|---|
| `roadmaps/fait/RAPPORT-generation-terrain.md` | Terrain plat, eau opaque : relief en voxels, vraie eau, couleurs, horizon, **planètes plus grandes** (décision du 04/10) |
| `IDEES-prochaine-version.md` (supprimé, tout est ici) | R1 détail du terrain, R2 brouillard, V1 skybox, V2 trous noirs, V3 voyage entre galaxies, V4 vue de la galaxie inclinée, A1 aliens, A2 terraformation |
| `roadmaps/a-faire/A-FAIRE-PLUS-TARD.md` (bloc D de la 0.11) | **Déplacé en 0.14** (`roadmaps/a-faire/ROADMAP-0.14.md`, 04/10/2026) |

Point de départ : **v0.12.0** (éditeur de modèles + correctifs C1 à C8). Les outils de mesure et de
réglage (panneau, benchmark) restent dans la **0.17** (`roadmaps/a-faire/ROADMAP-0.17-debug-opti.md`) : la 0.13 mesure
avec ce qui existe (bancs `bench_*`, FPS, captures).

Chaque phase = une branche `claude/roadmap-0-13-<phase>`, une PR non fusionnée : tu testes, puis tu dis
« push main ». Chaque phase a son **prompt prêt à coller** (§8).

---

## 1. Décisions

| Sujet | Décision |
|---|---|
| Taille des planètes | **Agrandir les planètes en voxels** (04/10/2026). |
| Qualité | **Aucune concession** : on ne retire pas de détail pour gagner des FPS ; on optimise. |
| Marcheur | Garde **~2 voxels** de haut (décision 0.12 E7) : c'est le voxel qui devient plus petit, pas le marcheur plus grand. |
| Vaisseau | Garde **4 voxels = 1 bloc** posé et en vol bas, icône dans l'espace (décision 0.12 E7). |
| Cubes | Le terrain reste en **cubes** (décision Q3 de la 0.11). |
| Tailles de l'espace | Rien ne réduit une distance de l'espace (étoiles, orbites, galaxies) : règle des 0.10 et 0.11. |
| Caméra 3e personne | 6 voxels, molette de 3 à 12 (correctif C4, v0.12.0). |

---

## 2. Point de départ (code actuel)

### 2.1 L'échelle aujourd'hui

| Grandeur | Valeur | Fichier |
|---|---|---|
| Rayon d'une planète type Terre | ~9 000 unités (5 500 à 13 800) | `settings.rs`, `StarConfig::scale() / 109` |
| Voxel | 5,5 à 11 unités (`MAX_VOXEL = 11`) | `terrain.rs:38`, `layout_for` |
| Rayon en voxels | **~1 300** | |
| Marcheur | 2 voxels, yeux à 1,8 | `surface.rs:411` |
| Horizon des yeux | **~70 voxels** | |
| Relief total | 2,5 à 5 % du rayon = 32 à 64 voxels | `planetgen/system.rs:366` |
| Profondeur du quadtree | ~6 niveaux, plafond 14 | `terrain.rs:192` |
| Tuiles fines | ~2 tuiles de 32 voxels autour du joueur | `SPLIT_FACTOR = 1,8`, `terrain.rs:291` |
| Plan proche de la caméra | 0,1 unité (Bevy par défaut) | `main.rs:410` |

### 2.2 Constantes en **unités** qui supposent le voxel actuel

Elles devront passer en **voxels** (ou en fraction du rayon) au changement d'échelle :

| Constante | Valeur | Fichier |
|---|---|---|
| `FLIGHT_ZOOM` (passage en vol bas) | 1 000 | `surface.rs:453` |
| Hauteur de stationnement, marges | `+150`, `+250`, `+1 500` | `surface.rs:171-173`, `:1117`, `:1423` |
| Vitesse en vol bas | `clamp(120, 4 000)`, montée 400 | `surface.rs:1411-1415` |
| Durées de descente / montée | `/8 000`, `/9 000`, `/12 000` | `surface.rs:840`, `:879` |
| Brume de l'horizon | 3 000 à 120 000 | `gas.rs:155` |
| Profondeur des grottes | `MAX_DEPTH = 2 000` | `caves.rs:22` |
| Phares | portée 1 000 | `surface.rs:1843` |
| Ombres (cascades) | en voxels déjà (`voxel * 0.5`) | `surface.rs:1625` ✅ |
| Marcheur, sauts, marées, grottes (régions), rochers, décor | en voxels déjà | `surface.rs`, `caves.rs`, `rocks.rs`, `decor.rs` ✅ |

### 2.3 Ce qui manque pour la qualité

Relief plat et en paliers, eau opaque sans fond marin (`terrain.rs:520`), brouillard seulement à
grande distance (`gas.rs:155`), pas de skybox, trous noirs en cubes, pas de faune ni de végétation
vivante, pas de son. Détails dans `roadmaps/fait/RAPPORT-generation-terrain.md`.

---

## 3. Règles d'architecture (0.13)

Les règles 1 à 13 des 0.10 et 0.11 restent valables (génome, sous-graines, horloge du monde, repère
fixe de l'astre, une seule fonction de densité, pas de liste globale, budget). La 0.13 en ajoute :

14. **Une seule échelle du sol.** Tout ce qui se mesure au sol (vitesses, hauteurs, portées, brume,
    profondeurs) s'exprime en **voxels** via `terrain::Layout::voxel` ou une structure `GroundScale`,
    jamais en unités écrites en dur. Changer l'échelle = changer **un** nombre.
15. **L'espace ne bouge pas.** Rayons des astres en unités, orbites, étoiles, galaxies : inchangés. Seul
    le voxel rétrécit. Vue de l'espace, rien ne change sauf le détail.
16. **Le loin et le près sont la même planète.** Toute nouvelle forme du relief passe par
    `raw_height_full` / `kind_at`, partagés par `terrain.rs` et `mesher.rs` (règle 11) : un sommet vu de
    l'espace est le même sommet vu à pied.
17. **Qualité de référence = Ultra.** Chaque phase est jugée en Ultra sur la machine de test. Les
    réglages plus bas existent pour les petites machines, mais une phase ne « gagne » jamais des FPS en
    baissant Ultra.
18. **Mesurer chaque phase.** Avant / après dans la PR : FPS médian, 1 % bas, pics > 33 ms, temps d'une
    tuile (`bench_tiles`), mémoire, captures aux mêmes endroits.
19. **Précision.** Toute position au sol reste dans le repère fixe de l'astre ; si un calcul dépasse la
    précision `f32` à la nouvelle échelle (noise, collisions, deltas), il passe en `f64` ou en
    coordonnées de tuile.

---

## 4. Les phases

### Bloc E — Échelle : agrandir les planètes en voxels

**Principe :** le rayon des planètes **en unités ne change pas** (l'espace reste identique, règle 15) ;
le **voxel devient plus petit** (`MAX_VOXEL` divisé par un facteur **k**). Le marcheur (2 voxels) et le
vaisseau posé (4 voxels = 1 bloc) rétrécissent avec lui. Vu du sol, la planète devient **k fois plus
grande**.

| k | Voxel | Rayon Terre en voxels | Horizon (yeux) | Profondeur du quadtree |
|---|---|---|---|---|
| 1 (aujourd'hui) | ~7 u | 1 300 | 70 voxels | ~6 |
| 8 | ~0,9 u | 10 000 | 190 voxels | ~9 |
| **16** | ~0,45 u | **21 000** | **270 voxels** | ~10 |
| 32 | ~0,22 u | 42 000 | 390 voxels | ~11 |
| 64 | ~0,11 u | 83 000 | 550 voxels | ~12 |

| Phase | Contenu | Taille |
|---|---|---|
| **E1. Étude d'échelle** | Prototype derrière un réglage (`/echelle k`, non sauvegardé) : k = 8, 16, 32, 64. Pour chacun : FPS (médian, 1 % bas, pics) sur un parcours fixe, `bench_tiles`, mémoire, nombre de tuiles pour couvrir l'horizon, précision (`f32` du repère de l'astre, bruit, collisions, plan proche), temps de descente de l'orbite au sol. Rapport + captures. **Tu choisis k** (Q1). | M |
| **E2. Passage à l'échelle** | Règle 14 : `GroundScale` (voxel, marcheur, vaisseau posé) ; toutes les constantes du §2.2 en voxels ; **plan proche dynamique** (0,1 voxel au sol, plus loin dans l'espace) ; plafond du quadtree relevé ; vitesses de vol bas et de marche revues pour traverser une planète k fois plus grande (vol bas rapide, Maj ×4, **vol suborbital** pour les longues distances) ; transitions orbite → sol plus longues mais fluides ; grottes, rochers, cratères, décor, météores, marées, nuages, particules de météo, lampes, astéroïdes où l'on se pose ; `f64` là où la précision manque (règle 19). Sauvegardes : deltas voxel et position du marcheur convertis (ou remis à zéro si impossible, à dire dans la PR). `PROTOCOL` +1. Tests : planètes reproductibles, marcheur et vaisseau qui se posent partout, `/aller` sur chaque type. | XL |
| **E3. Streaming à la nouvelle échelle** | Avec k fois plus de voxels sous l'horizon : génération limitée en **ms par image** (plus en nombre de tâches), priorité devant la caméra et au centre de l'écran, **cache disque** des tuiles générées (graine + génération → réutilisable), tuiles lointaines simplifiées mais **même silhouette** (règle 16), aucune tuile manquante en vol bas rapide. Critère : pas de trou ni de pic > 33 ms en vol bas à pleine vitesse. | L |

### Bloc T — Terrain (rapport §5 T1, T3, T4 + R1)

| Phase | Contenu | Taille |
|---|---|---|
| **T1. Relief en voxels** | Forme générale (continents, plaques, mers) toujours en % du rayon ; nouvelles couches **en voxels** : **collines** (8 à 25 voxels × k/16, sur 60 à 150), **massifs** le long des plaques (crêtes plus serrées et plus hautes, 40 à 400 voxels selon k), **bosses** (2 à 6 voxels, la couche qui manque), **déformation du domaine** (crêtes et vallées tordues), **vallées** creusées par l'érosion (mondes à air et à eau), pentes réelles 0 à 60°. Lunes et mondes sans air : plus de cratères, moins d'érosion. Même fonction de près et de loin (règle 16). Tests : distribution des pentes (part de terrain au-dessus de 15°, 30°, 45°), pas de pic isolé absurde ; `bench_tiles` avant / après. `PROTOCOL` +1. | L |
| **T2. Falaises et formes 3D** | Pentes > 45° = **parois verticales** en 3D (strates, surplombs, corniches), **pitons**, **chaos de blocs** éboulés, **arches** et **ponts naturels** généralisés (plus seulement l'arche de test), **gorges** étroites, entrées de grottes dans les falaises. Hachés par cellule (règle 12), passent par `kind_at`. Collisions 3D du marcheur et du vaisseau vérifiées. | L |
| **T3. Couleurs et matières** | Couleur sur plusieurs échelles (taches de 5, 30, 200 voxels), **roche nue sur les pentes fortes**, herbe et neige sur le plat (neige sur les faces tournées vers le haut), **strates** colorées sur les falaises, sable des plages en pente douce, mousse au pied des parois humides. Couleur vue de l'espace = sol (`mesher.rs`). | M |
| **T4. Détail jusqu'à l'horizon** | (R1) Distance de détail réglable (défaut Ultra = tuiles fines jusqu'à l'horizon), transitions douces entre niveaux (fondu ou morphing des hauteurs, plus de marche visible), coutures sans fissure, ombres des montagnes lointaines, décor lointain en **instances**. Mesures avant / après. | L |

### Bloc O — Eau (rapport §5 T2 ; rivières et lacs en 0.14 D1)

| Phase | Contenu | Taille |
|---|---|---|
| **O1. Vraie eau** | La hauteur du **fond** est gardée sous la mer (`terrain.rs:520`) : fond marin, fosses, plages qui descendent. **Surface d'eau à part** : maillage par tuile au niveau de la mer (marées comprises), matériau **transparent** (`AlphaMode::Blend` ou passe dédiée), couleur et opacité selon la **profondeur** (fond visible près du bord), reflet selon l'angle (**Fresnel**), reflet du ciel et du soleil, réfraction légère. Même chose pour méthane et ammoniac (leurs couleurs) ; lave opaque et lumineuse. De l'espace : mer sombre avec le reflet de l'étoile. `PROTOCOL` +1. | L |
| **O2. Vagues, rivage, sous l'eau** | **Vagues** selon le vent de `weather.rs` (normales animées + déplacement des sommets près du joueur), **écume** au rivage et sur les crêtes, **sous l'eau** : brouillard coloré, lumière atténuée avec la profondeur, **caustiques** au fond, rayons de lumière, **nage** (marcheur), le vaisseau peut plonger ; bulles. | L |

### Bloc P — Approche planétaire : entrer, sortir, se poser

Aujourd'hui (`surface.rs`) : depuis la vue espace, **V** lance une descente scriptée (`begin_descent`) vers le
point visé, de n'importe quelle distance ; zoomer sous `flight_zoom` (1000) fait **basculer d'un coup** en vol
bas (caméra replacée, terrain créé), dézoomer au-delà fait l'inverse (`surface.abort()`). Dans la vue espace, le
vaisseau reste garé au-dessus de l'astre : on ne peut pas tourner autour de la planète en pilotant. Rien ne
marque l'entrée dans l'atmosphère.

But : **un seul vol continu** de l'orbite au sol et retour, sans coupure ni téléportation ; on tourne autour
de la planète en pilotant ; on ne se pose que **près du sol** ; l'entrée dans l'atmosphère se voit et se sent.

| Phase | Contenu | Taille |
|---|---|---|
| **P1. Vol orbital** | Dans la vue espace près d'un astre, **ZQSD pilote le vaisseau autour de la planète** (vitesse proportionnelle à l'altitude, Espace / Ctrl = monter / descendre, Maj = plus vite), la caméra suit derrière lui ; trajectoire et point au sol sous le vaisseau affichés (`IndicatorGizmos`) ; altitude, vitesse et vitesse verticale au HUD. **V ne marche que près du sol** (sous 200 voxels, Q9) : plus haut, message « Trop haut pour atterrir : descendez (Ctrl) ». **Plus de descente automatique** depuis l'orbite (Q7) : on descend en pilotant, V sert seulement à se poser. Le vaisseau garé dans l'espace reste possible (aucune touche = il garde son orbite). | L |
| **P2. Transition sans coupure** | Plus de bascule à `flight_zoom` : altitude, vitesse, caméra et repère changent **en continu** de l'orbite au vol bas (une seule loi de vol, distance de caméra lissée, repère fixe de l'astre gardé du haut jusqu'au sol, `f64` là où il faut, règle 19). Maillage lointain → tuiles en fondu (T4), tuiles demandées **devant la trajectoire** avant d'y arriver (E3), ciel qui passe du noir à la couleur de l'atmosphère avec l'altitude (`daylight`, `atmosphere_depth`), brume et horizon qui se courbent progressivement. Même chose à la montée. Critère : de l'orbite au sol et retour sans saut d'image, sans pic > 33 ms, sans trou de terrain. | XL |
| **P3. Rentrée atmosphérique** | Sur un astre **avec air** : sous l'altitude de l'atmosphère, **flammes de plasma sous le vaisseau** (côté qui avance), lueur orange → blanche selon vitesse × densité de l'air (pression de `BodyParams`), traînée lumineuse derrière, secousses de caméra, coque qui rougeoit (matériau émissif des blocs), freinage par l'air (la vitesse maxi baisse avec la densité). **Tout selon la vitesse** (Q11) : lent = rien, rapide = flammes. **Pas de dégâts** (Q8). Couleur des flammes selon l'atmosphère (azote = orange, CO2 = rose, méthane = vert-bleu…). Sans air : **pas de flammes**, rétrofusées qui freinent. Géantes gazeuses : rentrée plus violente, puis la pression de `gas.rs`. Vu par les autres joueurs (réseau : état « rentrée » dans `Looks`). | L |
| **P4. Traverser les nuages** | Les nuages de `weather.rs` (cubes) **s'écartent autour du vaisseau** : les cubes proches s'effacent / sont repoussés, un **sillage** reste ouvert quelques secondes puis se referme ; à l'intérieur d'un nuage : brouillard blanc ou gris (C1), gouttes ou givre sur le cockpit (vue intérieure), turbulences, éclairs dans un nuage d'orage ; au-dessus : mer de nuages éclairée par le soleil, ombre du vaisseau sur les nuages. | L |
| **P5. Se poser et décoller** | Approche finale : **ombre du vaisseau** sur le sol qui grandit, train d'atterrissage (blocs de l'éditeur E6), **poussière / neige / sable / eau soulevés** par la poussée selon la matière du sol, flammes des tuyères orientées vers le bas ; pente maxi (Q10) et zone d'atterrissage proposée (point plat le plus proche) ; poser doux (vitesse verticale limitée près du sol). Décollage : même poussière, montée progressive, rentrée inverse (flammes plus faibles à la montée). | M |
| **P6. Vitesse et air** | Selon la **vitesse dans l'air** (Q11, vitesse du son d'après la température et les gaz de l'astre) : **bang supersonique** au passage du mur du son (onde visible, secousse, entendu par les joueurs proches), **cône de condensation** autour du vaisseau près de Mach 1 dans un air humide, **traînées de condensation** en haute altitude (visibles du sol, s'effacent avec le vent). **Aérofreinage** : passer dans la haute atmosphère ralentit et fait baisser l'orbite, plusieurs passages pour se poser sans fusées. **Bouclier thermique en option** (Q8, désactivé par défaut) : avec l'option, trop vite trop bas = surchauffe (jauge au HUD) puis dégâts. | M |
| **P7. Aide à l'approche** | **Indicateurs** : couloir de descente dessiné, point d'impact prévu au sol, temps avant le sol, alerte « trop vite » avant la rentrée et avant le sol, altitude au-dessus du relief (pas seulement du niveau de la mer). **Points d'atterrissage** au scanner : plateaux, plages, clairières plats sous 25° (Q10), classés par distance, cliquables comme cible ; plus tard spatioports, bases, villages aliens (L5). | M |
| **P8. Météo, lumière, cockpit** | **Météo qui gêne** l'approche : vent de travers (C3), turbulences d'orage, **givre** en altitude (vitre et ailes qui blanchissent), pluie et grêle sur le cockpit, **foudre** qui frappe le vaisseau dans un orage (éclair + secousse ; dégâts seulement avec l'option de P6). **Phares automatiques** de nuit et dans les nuages, feux de position clignotants vus de loin, reflet du vaisseau sur la mer (O1). **Vue cockpit** (1re personne) pendant la rentrée : vitre qui rougeoit, plasma devant, instruments qui tremblent. **Liseré bleu de l'atmosphère** vu de l'espace (couleur du ciel de l'astre) qui s'épaissit en descendant, aurores traversées près des pôles. | L |
| **P9. Son et autres joueurs** | **Son** (premier système de son du jeu, la musique reste pour plus tard) : sifflement de l'air qui monte avec la vitesse, grondement de la rentrée, bang, tonnerre, moteurs, **silence dans le vide** ; volume selon la densité de l'air. **Autres joueurs** : leur rentrée vue de loin comme une **étoile filante** (traînée lumineuse dans le ciel, de jour et de nuit), leur bang entendu avec le retard de la distance ; réseau : état « rentrée » et vitesse dans `Looks`. | L |

**État du bloc P : fait en v0.13.4** (`approche.rs`, `approche_fx.rs`, `approche_ui.rs`, `sound.rs`, shaders
`cloud_clear.wgsl` et `rim.wgsl`).

| Phase | Ce qui est fait | Limites connues |
|---|---|---|
| P1 | Vol orbital : ZQSD pilote depuis la vue espace (`FLIGHT_ENTER_RADII` 8 rayons), vitesse ∝ altitude (au plus 0,35 rayon / s), HUD altitude / vitesse / vertical / Mach, trajectoire + point au sol + point d'impact (gizmos), V seulement sous 200 voxels (Q9), plus de descente automatique (Q7) | Pas de vraie mécanique orbitale : sans touche, le vaisseau s'arrête (il garde sa position, pas sa vitesse) |
| P2 | Une seule loi du sol à l'orbite : ceiling 4 rayons, vaisseau qui grossit avec l'altitude (jamais moins de 1/30 de l'altitude), caméra qui recule et se penche vers la planète (`cam_blend`), ciel et brume selon `air`, maillage lointain lisse au-dessus de 0,30 rayon (hystérésis 0,20) | Bascule tuiles / maillage lointain instantanée (pas de fondu) ; tuiles pas encore demandées « devant la trajectoire » ; le maillage lointain garde ses terrasses (bug B1 de `ROADMAP-0.13.5-correctifs.md`). Mesure (`SPACESPORE_PERF`, montée puis rentrée puis pose, 27 s) : médiane 179 images/s, 1 % bas 54, aucune image > 33 ms, pire 29,5 ms |
| P3 | Plasma selon chaleur = f(densité x Mach^3) (`approche::heat`), couleur selon l'air (`BodyParams::plasma`), traînée, lueur (lumière ponctuelle), secousses, freinage de l'air (`DRAG_Q0`), géantes x1,6, rétrofusées dans le vide, flammes moitié moins fortes à la montée, état « rentrée » dans `Looks` (PROTOCOL 35) | Pas de matériau émissif sur les blocs de la coque (la lumière rougeoie à la place) |
| P4 | Nuages (`CloudMaterial`) tramés autour du vaisseau et de son sillage de 6 s ; brouillard gris / blanc dans la dalle, phares, turbulences, éclairs (secousse + message, sans dégâts), givre, gouttes sur la vitre (cockpit), ombre du vaisseau sur la mer de nuages | Les nuages sont des dalles : le brouillard n'est dedans que dans la dalle |
| P5 | Ombre projetée selon le soleil, train d'atterrissage (état « atterrissage » du modèle sous 240 voxels et 160 voxels/s), poussière / neige / sable / eau selon la matière (`dust_color`), pente maxi 25 deg (Q10) avec point plat proposé (cercle vert) et V pour s'y poser, poser et décoller avec les mêmes effets | Les flammes de tuyères restent celles du modèle |
| P6 | Bang (anneau + secousse + son), cône de condensation près de Mach 1, traînées en haute altitude (air humide), aérofreinage (la traînée freine), bouclier thermique en option (`/bouclier`, jauge SURCHAUFFE, dégâts `HeatDamage`) | Pas de succession de passages d'aérofreinage : la mécanique orbitale n'existe pas |
| P7 | Couloir de descente (portes vertes / rouges), point d'impact prévu, temps avant le sol, alertes « trop vite » (rentrée, sol), altitude au-dessus du relief, points d'atterrissage plats au scanner (plage, plateau, clairière, neige, plaine ; touche L = suivant, cercle vert) | Spatioports, bases, villages : avec L5 |
| P8 | Vue cockpit (F5 en vol : vitre rougeoyante, givre, gouttes, secousses x2,5), phares dans les nuages, feux de position et strobe, liseré bleu de l'atmosphère (`RimMaterial`), foudre, givre | Reflet du vaisseau sur la mer et aurores traversées non faits ; les instruments du HUD ne tremblent pas |
| P9 | Premier son du jeu, synthétisé (aucun fichier) : vent (hauteur et volume selon la vitesse), grondement de rentrée, moteurs, bang, tonnerre retardé par la distance, silence dans le vide ; `/volume` ; autres joueurs : leur rentrée est une étoile filante, leur bang arrive avec le retard de la distance | Pas de musique (plus tard) ; pas de son spatialisé (volume seulement) |

Commandes de test ajoutées : `/vol`, `/mer`, `/nuage`, `/essai mer`, `/jour`, `/nuit`, `/heure <h>`, `/volume`,
`/bouclier` ; variables `SPACESPORE_TEST_FLY=up|reentry[:s]|slow|climb`, `SPACESPORE_TEST_FLYKEY`,
`SPACESPORE_TEST_COCKPIT` (voir `TESTS-JEU.md`).

### Bloc C — Ciel et atmosphère (R2, V1 des idées)

| Phase | Contenu | Taille |
|---|---|---|
| **C1. Brouillard** | (R2) **Brouillard au ras du sol**, plus épais dans les vallées (T1) et au-dessus de l'eau, qui monte avec l'altitude ; **bancs de brouillard** de la météo (matin, mers froides, marécages) en volumes (`FogVolume`, `VolumetricFog` de Bevy 0.15) ; **rayons de soleil** dans le brouillard ; couleur selon le soleil ; phares et lampe éclairent le brouillard ; brouillard léger dans les grottes ; brume de l'horizon en voxels (règle 14). | L |
| **C2. Skybox** | (V1) **Skybox générée** depuis la vraie galaxie vue du système courant (étoiles avec leur couleur et leur éclat, bande de la galaxie, nébuleuses, autres galaxies), cubemap haute résolution rendue en arrière-plan, refaite au changement de système ; étoiles proches restent de vrais objets cliquables ; au sol : visible la nuit, effacée par le ciel de jour, voilée par les nuages. | M |

### Bloc V — Espace spectaculaire (V2, V3, V4 des idées)

| Phase | Contenu | Taille |
|---|---|---|
| **V1. Trous noirs** | **Horizon invisible** (seulement l'ombre et la déformation), **lentille gravitationnelle** en post-traitement plein écran (skybox de C2 et étoiles déformées, **anneau d'Einstein**, image dédoublée), **disque d'accrétion** en shader (plus chaud à l'intérieur, effet Doppler, arrière du disque visible au-dessus et en dessous de l'ombre), **étoile aspirée** en spirale qui trahit le trou noir. Remplace l'anneau photonique en cubes (`black_hole.rs`). | XL |
| **V2. Voyage entre galaxies** | Séquence de ~10 s (Q4), passable avec Échap : plongée dans la lentille de V1, **tunnel** (étoiles étirées, couleurs du bleu au rouge, distorsion), **flash**, nouvelle galaxie qui grandit jusqu'à la vue d'arrivée ; chargement pendant le tunnel ; en multijoueur, éclair de départ et d'arrivée. Remplace le saut direct (`HYPERJUMP_DIST`, `main.rs`). | L |
| **V3. Vue de la galaxie inclinée** | En vue galaxie, la caméra prend le **plan de la galaxie** (`tilt`, `settings.rs:708`) au lieu du haut du monde (`main.rs:1252`) ; orbite autour de l'axe de la galaxie, transition douce d'une galaxie à l'autre. | S |

### Bloc L — Aliens et terraformation (A1, A2 des idées ; faune, végétation, son et lieux en 0.14)

| Phase | Contenu | Taille |
|---|---|---|
| **L5. Aliens** | (A1) Espèce intelligente rare sur les mondes habitables, apparence avec les familles de l'éditeur, niveau (tribu → cités → spatial), **villages et bâtiments** voxel posés sur le relief de T1, habitants qui vivent selon le jour et la nuit, scanner (espèce, population, attitude), interaction de base (Q5). | XL |
| **L6. Terraformation** | (A2) Projets qui changent les **valeurs vivantes** (deltas) : réchauffer / refroidir, épaissir l'atmosphère, apporter de l'eau (comètes), semer la vie ; l'habitabilité, le climat, les biomes, l'eau (O1) et la couleur vue de l'espace suivent ; les aliens spatiaux terraforment aussi (Q6). Réseau par deltas. | XL |

---

## 5. Ordre et versions

```
v0.12.0 (éditeur + correctifs)
   │
   E1 (choix de k) → E2 → E3   l'échelle change tout : elle passe en premier
   │
   T1 → T2 → T3 → T4           le relief à la nouvelle échelle
   │
   O1 → O2                     l'eau sur ce relief
   │
   P1 → P2 → P3 → P4 → P5      entrer, sortir, se poser (P4 utilise C1 si fait avant)
   P6 → P7 → P8 → P9           vitesse et air, aide à l'approche, météo et cockpit, son
   │                           ── release 0.13.0 « Mondes » ──
   C1 → C2                     brouillard, skybox
   V1 → V2 → V3                trous noirs (utilise C2), voyage, vue inclinée
   │                           ── release 0.13.1 « Ciel » ──
   L5 → L6                     aliens, terraformation
                               ── release 0.13.2 « Aliens » ──
   │
   0.14 : le monde qui vit (bloc D) et les mondes exceptionnels
```

Pourquoi cet ordre : l'échelle (E) change toutes les distances du sol ; régler le relief (T) ou l'eau
(O) avant serait à refaire. Le trou noir (V1) déforme la skybox (C2). Les aliens (L5) ont besoin du
relief et de l'eau finis.

---

## 6. Risques

| Risque | Parade |
|---|---|
| **Coût** : k fois plus de voxels sous l'horizon | Mesures de E1 ; E3 (budget en ms, cache disque) ; décision de k sur mesures (E1) |
| **Précision** `f32` au sol avec des voxels de ~0,4 unité | Règle 19 ; tests de E1 (k = 64 pour voir la limite) |
| **Sauvegardes** : deltas voxel, positions | Conversion dans E2, sinon remise à zéro annoncée |
| **Réseau** : génération modifiée 3 fois (E2, T1, O1) | `PROTOCOL` +1 à chaque fois |
| **Temps de trajet** au sol (planète 16 fois plus grande) | Vol bas plus rapide, vol suborbital (E2) |
| **Taille de la 0.13** | Trois releases (§5) ; chaque phase jouable seule |
| **Transition continue** (P2) : tuiles et maillage lointain chargés en même temps à mi-altitude, précision du repère | Fondu de T4, priorité devant la trajectoire (E3), mesures en descente rapide, `f64` (règle 19) |
| **Nuages qui s'écartent** (P4) : couche reconstruite en arrière-plan (`rebuild_clouds`) | Écartement dans le shader / par masque autour du vaisseau, pas de reconstruction |

---

## 7. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Facteur **k** d'agrandissement (§4, bloc E) ? | Décider après E1 sur les mesures ; viser **k = 16** (horizon ×4, rayon Terre ~21 000 voxels), 32 si les mesures le permettent. |
| Q2 | Grottes : `MAX_DEPTH = 2 000` **unités** (décision 0.11 Q4). À la nouvelle échelle, garder 2 000 unités (= ~4 500 voxels de profondeur à k = 16) ou garder **285 voxels** comme aujourd'hui ? | Garder la **même profondeur en unités** (grottes très profondes, cohérent avec « aucune concession ») ; générées à la demande, donc sans coût en surface. |
| Q3 | Faut-il le **vol suborbital** (sauter d'un point à l'autre de la planète par une courbe haute) ? | Oui, au-delà de ~5 000 voxels de trajet. |
| Q4 | Durée du voyage entre galaxies ? | 10 s, passable avec Échap. |
| Q5 | Aliens : combat ? amicaux ? commerce ? | Pas de combat au début ; attitude tirée de la graine (amicale, méfiante, hostile = refuse le contact) ; commerce oui. |
| Q6 | Terraformation : le joueur, les aliens, ou les deux ? En combien de temps ? | Les deux ; plusieurs heures de jeu pour un changement net, plusieurs jours pour une planète entière. |
| Q7 | V depuis l'orbite : supprimer la descente automatique ou la garder en pilote automatique ? | **Décidé** : V sert seulement à se poser, **pas de pilote automatique**. |
| Q8 | Rentrée : **dégâts** si l'on arrive trop vite (bouclier thermique) ? | **Décidé** : pas de dégâts ; bouclier thermique en **option** (désactivée par défaut). |
| Q9 | À quelle hauteur peut-on atterrir (V) ? | **Décidé** : sous **200 voxels** au-dessus du sol. |
| Q10 | Pente maxi pour se poser ? | **Décidé** : **25°** ; au-delà, message et proposition du point plat le plus proche. |
| Q11 | Quelles idées d'amélioration garder ? | **Décidé** : toutes (P6 à P9), les effets **selon la vitesse**. |

---

## 8. Prompts (à coller dans une nouvelle session, un par phase)

Contexte commun : « Lis `roadmaps/a-faire/ROADMAP-0.13.md`, `roadmaps/fait/RAPPORT-generation-terrain.md` et `CLAUDE.md` (règles 1 à
19). `git pull origin main` avant de coder. Branche `claude/roadmap-0-13-<phase>`. Build release,
tests, mesures avant / après (règle 18) et captures. PR non fusionnée (je dirai « push main »). Aucune
concession sur la qualité ; ne change ni les tailles de l'espace ni les décisions sans me demander. »

- **E1** — « [contexte commun] Phase E1 : prototype du voxel plus petit (`/echelle k`, k = 8, 16, 32,
  64) ; pour chaque k : FPS sur un parcours fixe, `bench_tiles`, mémoire, tuiles jusqu'à l'horizon, précision, temps de
  descente. Rapport pour choisir k (Q1). Ne rien fusionner d'autre. »
- **E2** — « [contexte commun] Phase E2 : passage à l'échelle k = <k choisi> : `GroundScale`, constantes
  du §2.2 en voxels, plan proche dynamique, quadtree, vitesses et vol suborbital (Q3), grottes (Q2),
  sauvegardes, précision `f64`, PROTOCOL +1. »
- **E3** — « [contexte commun] Phase E3 : streaming des tuiles en ms par image, priorité, cache disque,
  lointain à la même silhouette ; aucun trou ni pic en vol bas à pleine vitesse. »
- **T1** — « [contexte commun] Phase T1 : relief en voxels (collines, massifs, bosses, déformation du
  domaine, vallées d'érosion) partagé par `terrain.rs` et `mesher.rs` ; tests de pentes ; PROTOCOL +1. »
- **T2** — « [contexte commun] Phase T2 : falaises verticales, surplombs, pitons, éboulis, arches et ponts
  naturels, gorges, entrées de grottes ; collisions 3D vérifiées. »
- **T3** — « [contexte commun] Phase T3 : couleurs multi-échelles, roche sur les pentes, strates, neige
  sur les faces du haut, plages ; couleur vue de l'espace cohérente. »
- **T4** — « [contexte commun] Phase T4 : tuiles fines jusqu'à l'horizon en Ultra, transitions douces,
  coutures, ombres lointaines, décor en instances. »
- **O1** — « [contexte commun] Phase O1 : fond marin gardé, surface d'eau transparente à part,
  profondeur, Fresnel, reflets, autres liquides ; vue de l'espace ; PROTOCOL +1. »
- **O2** — « [contexte commun] Phase O2 : vagues selon le vent, écume, sous l'eau (brouillard,
  caustiques, rayons), nage, plongée du vaisseau. »
- **P1** — « [contexte commun] Phase P1 : vol orbital autour de l'astre dans la vue espace (ZQSD, altitude,
  HUD), V seulement sous 200 voxels (Q9), plus de descente automatique (Q7). »
- **P2** — « [contexte commun] Phase P2 : transition continue orbite ↔ vol bas sans bascule ni saut (caméra,
  repère, tuiles devant la trajectoire, ciel) ; mesures en descente rapide. »
- **P3** — « [contexte commun] Phase P3 : rentrée atmosphérique (flammes de plasma selon vitesse et densité,
  couleur selon les gaz, coque qui rougeoit, freinage, rien sans air) ; vue par les autres joueurs. »
- **P4** — « [contexte commun] Phase P4 : traverser les nuages (ils s'écartent, sillage, brouillard dedans,
  turbulences, éclairs, mer de nuages au-dessus). »
- **P5** — « [contexte commun] Phase P5 : se poser et décoller (ombre, train, poussière selon le sol, pente
  maxi Q10, zone plate proposée). »
- **P6** — « [contexte commun] Phase P6 : bang supersonique, cône de condensation et traînées selon la vitesse
  dans l'air, aérofreinage, bouclier thermique en option (Q8). »
- **P7** — « [contexte commun] Phase P7 : indicateurs d'approche, points d'atterrissage au scanner (Q10). »
- **P8** — « [contexte commun] Phase P8 : météo qui gêne (vent, givre, foudre), phares automatiques, vue cockpit
  pendant la rentrée, liseré bleu de l'atmosphère. »
- **P9** — « [contexte commun] Phase P9 : premier système de son (air, rentrée, bang, silence dans le vide) et
  rentrée des autres joueurs vue comme une étoile filante. »
- **C1** — « [contexte commun] Phase C1 : brouillard au sol, bancs volumétriques de la météo, rayons de
  soleil, couleurs, phares, grottes. »
- **C2** — « [contexte commun] Phase C2 : skybox générée depuis la galaxie réelle, refaite au changement
  de système, nuit au sol. »
- **V1** — « [contexte commun] Phase V1 : trous noirs (horizon invisible, lentille en post-traitement,
  disque d'accrétion en shader, étoile aspirée) ; remplace les cubes. »
- **V2** — « [contexte commun] Phase V2 : séquence de voyage entre galaxies (Q4). »
- **V3** — « [contexte commun] Phase V3 : vue galaxie dans le plan de la galaxie. »
- **L5** — « [contexte commun] Phase L5 : aliens (Q5). »
- **L6** — « [contexte commun] Phase L6 : terraformation par deltas (Q6). »
