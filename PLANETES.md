# SpaceSpore - Les planetes : types et generation

Document de reference sur **tous les types de planetes (et de lunes)** et sur **comment chacune est generee**.
Source : module `src/planetgen/` (surtout `system.rs`, `atmosphere.rs`, `hydrology.rs`, `geology.rs`, `biome.rs`,
`life.rs`, `resources.rs`, `habitability.rs`, `traits.rs`, `star.rs`, `belts.rs`) + rendu (`mesher.rs`, `terrain.rs`,
`planet.rs`, `gas.rs`).

Tout est **deterministe** : une graine du monde donne exactement le meme univers sur toutes les machines. Rien n'est
stocke : les planetes sont recalculees depuis le « genome » du systeme (`SystemGenome` : 2 nombres, la graine du
systeme et la base des graines de planetes). Les valeurs affichees sont arrondies (rayon a 1 unite, orbite a 10, etc.)
pour que le calcul soit identique partout (empreinte reseau).

---

## 1. Vue d'ensemble : la chaine de generation

Pour chaque monde (planete ou lune), les couches sont calculees **dans cet ordre**, chacune utilisant les resultats des
precedentes :

```
Etoile -> Orbite -> Physique (masse, rayon) -> Atmosphere -> Climat -> Hydrologie (eau/glace/mers)
       -> Geologie (tectonique, volcans) -> Relief -> Radiation -> Vie -> Biomes -> Ressources
       -> Habitabilite & dangers -> Traits / anomalies
```

### Sous-graines par couche (`seeds.rs`)

Chaque astre a **une graine**. Chaque couche en tire sa propre sous-graine (SplitMix64, entiers seulement) :

| N° | Couche | N° | Couche |
|---|---|---|---|
| 1 | etoile | 8 | geologie |
| 2 | orbite | 9 | relief |
| 3 | physique | 10 | biologie |
| 4 | composition | 11 | ressources |
| 5 | atmosphere | 12 | gameplay |
| 6 | climat | 13 | traits |
| 7 | hydrologie | 14 | ceintures |

Les numeros sont **figes** : ajouter une couche ne change jamais les tirages des autres.

- Graine d'une planete : `planet_base + indice de la planete`.
- Graine d'une lune : `planet_base + 500 + indice_planete * 8 + indice_lune`.

---

## 2. Les 4 types de planetes (`PlanetKind`)

| Type | Rayon | Masse | Sol ? | Ou |
|---|---|---|---|---|
| **Rocheuse** (`Rocky`) | 0,34 a ~2,4 R_terre | 0,02 a ~8 M_terre | oui | partout |
| **Mini-Neptune** (`MiniNeptune`) | 1,5 a 3,5 R_terre | 3 a 12 M_terre | non (enveloppe de gaz) | surtout pres de l'etoile |
| **Geante de glace** (`IceGiant`) | 3 a 5 R_terre | 10 a 30 M_terre | non | au-dela de la ligne des glaces |
| **Geante gazeuse** (`GasGiant`) | 8 a 14 R_terre | 40 a 4 000 M_terre | non | au-dela des glaces, ou « Jupiter chaud » |

`gaseous()` = tout sauf `Rocky` : **pas de sol**, on y vole jusqu'au coeur (`GAS_CORE` = 0,3 du rayon), la pression
retire des PV au vaisseau (`gas.rs`, `combat.rs`), destruction = retour en orbite.

### Classes de taille affichees (`size_class`)

| Type | Classe |
|---|---|
| Geante gazeuse chaude | « geante chaude » |
| Geante gazeuse | « geante gazeuse » |
| Geante de glace | « geante de glace » |
| Mini-Neptune | « mini-Neptune » |
| Rocheuse < 0,5 R_terre | « minuscule » |
| Rocheuse 0,5 - 0,8 | « petite » |
| Rocheuse 0,8 - 1,25 | « terrestre » |
| Rocheuse > 1,25 | « super-Terre » |

### Variantes particulieres

- **Jupiter chaud** (`hot = true`) : geante gazeuse sur une orbite tres proche (a < 0,12 x zone habitable) autour d'une
  etoile F/G/K, 3 % de chance ; rayon x1,2 (gonflee par la chaleur) ; texture sombre et rougeoyante.
- **Planete errante** (`rogue`) : 1 systeme sur 30 (`ROGUE_CHANCE`). Derniere de la liste, 5 000 UA fictives (aucune
  lumiere, aucune orbite), loin de tout et **hors du plan**, immobile. Type : 60 % rocheuse, 25 % geante de glace,
  15 % geante gazeuse. Ignoree par les ceintures, zones habitables et orbites. Rocheuse errante : masse 0,1 a 6,3 M_terre.
  `/aller planete errante`.
- **Lunes** : voir §10 (toujours generees comme des mondes rocheux complets).

---

## 3. Le systeme : combien de planetes, ou, de quelle nature

### Nombre de planetes (`system.rs::generate`)

- Etoile normale : **1 a 8** planetes, poids `[6, 10, 15, 17, 17, 14, 11, 10] %` (surtout 3 a 6).
- Autour d'une **naine blanche ou brune** : **1 a 4**, poids `[35, 30, 20, 15] %`.

### Orbites (unites reelles, UA)

- `hz` = distance de la zone habitable = racine de la luminosite (`sqrt(L)`).
- `snow` = **ligne des glaces** = 2,7 x `hz`.
- 1re orbite : `hz x 10^U(-1,1 ; -0,35)` (de 8 % a 45 % de la zone habitable), jamais a moins de 3 rayons de l'etoile
  (une geante rouge a englouti ses planetes proches), et au-dela de 1,1 x la zone instable d'une paire serree.
- Chaque planete suivante : `a x U(1,4 ; 2,3)`.
- Compagnon lointain (etoile double de type « Wide ») : pas d'orbite stable au-dela, la liste s'arrete.

### Nature de la planete (tirage `roll` de la couche Physique)

| Zone | Condition | Resultat |
|---|---|---|
| Tres proche (F/G/K) | `a < 0,12 hz` et `roll < 3 %` | Jupiter chaud |
| En deca de la ligne des glaces (ou etoile morte) | `roll < 20 %` | Mini-Neptune |
|  | sinon | Rocheuse |
| Au-dela de la ligne des glaces | `roll < 35 %` | Geante gazeuse |
|  | `35 % <= roll < 65 %` | Geante de glace |
|  | sinon (35 %) | Rocheuse |

### Masse et rayon (`radius_from_mass`)

| Type | Masse (M_terre) | Rayon |
|---|---|---|
| Rocheuse | `10^(-1,7 + 2,6 u^1,2)` | `m^0,279` |
| Mini-Neptune | `3 x 4^u` | `1,22 (m/2,04)^0,589` |
| Geante de glace | `10 x 3^u` | idem |
| Geante gazeuse | `40 x 100^(u^1,5)` | `1,22 (m/2,04)^0,589` jusqu'a 130 M_terre, puis `12,1 (m/130)^-0,044` |

Relation de Chen & Kipping (2017). Valeurs calibrees sur le Systeme solaire (Terre, Mars, Neptune, Jupiter, Saturne a
15 % pres : test `mass_radius_matches_the_solar_system`).

- **Gravite** (g) = masse / rayon².
- **Echelle d'affichage** : 1 R_terre = echelle G du systeme / 109 (echelle G de 600 000 a 1 500 000), rayon minimum 50.
- **Orbites affichees** : echelle **logarithmique** (chaque doublement de distance = meme ecart a l'ecran), la zone
  habitable tombant a 3,2 echelles. Les orbites sont ecartees pour que planetes et lunes ne se touchent jamais, puis
  tout est etire x5 (`SPACE_STRETCH`) a la fin.
- **Excentricite** : `0,3 x U^3` (surtout quasi circulaire), x0,2 si tres pres de l'etoile. Inclinaison +-0,05 rad.

### Rotation et inclinaison

- **Synchrone** (face toujours eclairee, « verrouillee par les marees ») : planete rocheuse a `a < 0,4 x cbrt(masse etoile)`.
  Rotation = periode orbitale, inclinaison 0, vents du cote jour vers le cote nuit.
- Sinon : jour de **10 a 40 h** (`10 x 4^u`) pour une rocheuse, **9 a 17 h** pour une geante.
- Inclinaison de l'axe : 5 % de chance d'etre « couchee » (60 a 180 deg, > 90 = rotation retrograde), sinon `35 x u^1,5` deg.
- Periode orbitale reelle (jours) = `365,25 x sqrt(a^3 / masse_etoile)`. Dans le jeu : 1 h de la planete = 1 min de jeu,
  saisons ~1 h en moyenne.

### Anneaux

Chance : geante gazeuse 60 %, geante de glace 40 %, mini-Neptune 10 %, rocheuse 2 %.
- Bord interieur 1,25 a 1,5 x rayon ; exterieur 1,8 a 2,6 x rayon (jamais jusqu'a la premiere lune) ; opacite 0,35 a 0,8.
- **Glace** : au-dela de la ligne des glaces 0,7 a 1,0 (claire, comme Saturne), en deca 0 a 0,3 (roche sombre).
- 1 a 3 **divisions** (comme Cassini), largeur 1,5 a 6 % ; profil en bandes (`Ring::profile`).
- Rendu : ombre de la planete sur l'anneau et inversement (`rings.rs`).

### Aurores

Une planete a des aurores si champ magnetique >= 0,2, de l'air (ou geante) et un vent stellaire suffisant.
Force = `0,3 x sqrt(champ) x vent^0,25` (>= 0,1). Couleur : **vert** (O2 > 5 %), **rose** (H2), **rouge** (CO2), sinon **violet**.
Latitude = 68 - 6 x force (deg).

---

## 4. L'etoile et son influence

Le type d'etoile (`star.rs`) fixe la lumiere, donc tout le climat des planetes.

| Type | Part des systemes | Masse (M_sol) |
|---|---|---|
| O | 0,003 % | 16 - 60 |
| B | 0,13 % | 2,1 - 16 |
| A | 0,6 % | 1,4 - 2,1 |
| F | 3 % | 1,04 - 1,4 |
| G | 7,5 % | 0,8 - 1,04 |
| K | 12 % | 0,45 - 0,8 |
| M (naine rouge) | 66 % | 0,08 - 0,45 |
| Naine blanche | 5 % | 0,5 - 1,1 |
| Naine brune | 4 % | 0,013 - 0,075 |
| Sous-geante | 1,2 % | 1 - 2,5 |
| Geante rouge | 0,57 % | 0,8 - 3 |

Grandeurs calculees : luminosite (relation masse-luminosite), rayon, temperature (Stefan-Boltzmann), couleur (corps noir
CIE 1931), age (max 13 Gyr), duree de vie, **activite magnetique**, **UV**, **rayons X** et **vent stellaire**.
Ces trois derniers pilotent la perte d'atmosphere (Jeans), la radiation au sol, les aurores et l'helium-3.

Multiplicite (`multiple.rs`, ~1/3 des systemes) : simple, **paire serree** (planetes de type P autour des deux), **compagnon
lointain** (type S), **triple**. Les luminosites des etoiles du centre s'additionnent pour la zone habitable.

---

## 5. Atmosphere (`atmosphere.rs`)

### Gaz (13)

| Formule | Nom | Serre (par bar) | Brume | Rigueur |
|---|---|---|---|---|
| N2 | diazote | 0,1 | - | realiste |
| O2 | dioxygene | 0,02 | - | realiste |
| CO2 | dioxyde de carbone | 0,39 | - | realiste |
| CH4 | methane | 10 | orange | realiste |
| H2 | dihydrogene | 0,5 | - | realiste |
| He | helium | 0 | - | realiste |
| Ar | argon | 0 | - | realiste |
| H2O | vapeur d'eau | 100 | blanche | realiste |
| NH3 | ammoniac | 20 | creme | realiste |
| SO2 | dioxyde de soufre | 5 | jaune | realiste |
| Ae | **aetherion** (lumineux) | 2 | violette | **fictif** |
| Sp | **sporogaz** | 1 | verte | **fictif** |
| Cx | **chromex** | 8 | rose | **fictif** |

### Composition de depart (monde rocheux) d'apres la temperature d'equilibre `T_eq` (albedo 0,3)

Selon un tirage : **25 % des mondes (ou masse < 0,02 M_terre) n'ont aucune atmosphere**. Sinon :

| Condition | Atmosphere | Pression |
|---|---|---|
| `T_eq > 400 K`, 50 % | **Venus** : CO2 96,5 %, N2, un peu de SO2 | 30 a 150 bar |
| `T_eq > 400 K`, 50 % | CO2 60 %, SO2 30 %, N2 10 % | 0,001 a 0,3 bar |
| `180 < T_eq <= 400 K` | **Temperee** : N2 78 %, Ar 1 %, CO2 0,04 - 2 %, **+ O2 21 % dans 25 % des cas** | `10^U(-1,3 ; 0,7) x masse^0,6` |
| `T_eq <= 180 K`, 50 % | **Titan** : N2 95 %, CH4 5 % | 0,5 a 3 bar |
| `T_eq <= 180 K`, 50 % | **Mars** : CO2 95 %, N2, Ar | 0,003 a 0,05 bar |

Geante : H2 86 %, He 13 %, + CH4 (si T_eq < 400 K : 0,3 % gazeuse, 2 % neptunienne), NH3 (si T_eq < 150 K), H2O. Pression de
reference 1 bar (niveau des nuages).

**Gaz fictifs** : 3 % des mondes avec air recoivent 1 a 15 % d'Ae, Sp ou Cx.

### Retention des gaz (fuite de Jeans)

Un gaz reste si `v_liberation > 5 x 0,158 x sqrt(T_exo / masse_molaire)`. `T_exo` = ~3 x T_eq, bien plus chaud sous les rayons X
d'une etoile active proche. Les gaz trop legers s'echappent et la pression baisse d'autant : une planete proche d'une
naine rouge active perd son air. Si la pression tombe sous 0,0001 bar : plus d'atmosphere.

### Effet de serre, albedo, temperature de surface

- `T_eq = 278,6 K x L^0,25 x (1 - A)^0,25 / sqrt(d)`.
- Serre : atmosphere grise d'epaisseur optique `tau = somme(serre_gaz x fraction x pression^1,3)` ; `T = T_eq (1 + 0,75 tau)^0,25`
  (plafond 1 500 K). Calibre sur la Terre (+33 K), Venus (+500 K), Mars (~0), Titan (+12 K).
- La **vapeur d'eau suit la temperature** (Clausius-Clapeyron, plafond 4 %) : 8 iterations pour converger.
- Albedo : `0,12 + 0,3 x couverture nuageuse + 0,3 (acide sulfurique) + 0,15 (monde glace)` ; geante 0,35.
- Geante : T = 1,3 x T_eq (chaleur interne).

### Nuages (`CloudKind`)

| Type | Condition | Couleur |
|---|---|---|
| Aucun | pression < 0,005 bar | - |
| Exotiques | gaz fictif > 1 % | couleur de la brume du gaz |
| Acide sulfurique | SO2 + pression > 5 bar (couverture 100 %) | jaune |
| Ammoniac | geante avec NH3 | creme |
| Methane | CH4 et T < 200 K | orange |
| Eau | monde « humide » et 230 < T < 400 K | blanc |
| Glace carbonique | CO2 et T < 220 K | blanc-bleu |

### Climat (`climate.rs`)

`T(lat, alt) = moyenne + ecart x (1/3 - sin²(lat)) - gradient x altitude + saison + jour/nuit`

- **Ecart equateur-poles** (`span`) = `0,35 x T / (1 + pression)`, x1,6 si synchrone : une atmosphere epaisse repartit la chaleur.
- **Gradient vertical** (`lapse`) : 0 sans air, `50 x pression^0,3 x sqrt(g)` sinon.
- **Amplitude jour/nuit** : enorme sans air (Lune ~±150 K), amortie par l'atmosphere (Terre ~±5 K, Venus ~0). Max a 14 h 30.
- **Saisons** : declinaison avec retard, excentricite, **givre du matin** quand la nuit est descendue sous 0 °C.
- Moyenne sur la sphere = `mean_c`.

### Vents

- Sans air : aucun. Geante : 100 a 150 m/s, « jets alternes en bandes ».
- Rocheuse : `8 x (1 + span/40) x (1 + pression^0,3) x 0,5` (max 120 m/s).
- Circulation : **synchrone** = du jour vers la nuit ; **> 30 bar** = superrotation ; **jour < 30 h** = 3 cellules par hemisphere
  (Hadley, Ferrel, polaire) ; sinon une grande cellule de Hadley.

### Couleurs du ciel

Diffusion de Rayleigh (1/λ⁴) de la lumiere de l'etoile (ciel bleu pour une etoile blanche, tout autre pour une M ou une
geante rouge), blanchie par une atmosphere epaisse, teintee par les brumes (methane orange, soufre jaune, poussiere ocre
sur les mondes minces et secs, gaz fictifs). Coucher de soleil : le bleu est diffuse en route, tout devient rouge.

---

## 6. Eau, glace et mers (`hydrology.rs`)

### Etat de l'eau (diagramme de phase reel)

| Etat | Condition |
|---|---|
| **Supercritique** | T > 647 K et P > 220,6 bar |
| **Glace** | T < 273,15 K |
| **Vapeur** | P < 6 mbar (point triple) ou T > point d'ebullition (Clausius-Clapeyron) |
| **Liquide** | sinon |
| Absente | reserve d'eau <= 2 % |

### Reserve d'eau (`inventory`, 0 a 1)

- En deca de la ligne des glaces : `0,65 x u²` (monde sec a humide, l'eau vient des impacts).
- Au-dela : `0,15 + 0,85 x u` (**mondes-oceans** possibles).

### Liquide des mers (par priorite)

| Liquide | Condition | Couverture oceanique |
|---|---|---|
| **Lave** | T moyenne > 1 300 K | 30 a 90 % |
| **Methane** (Titan) | eau gelee + methane liquide (91 K a son point d'ebullition, CH4 >= 1 %, P > 0,1 bar) | 2 a 32 % |
| **Eau** | eau liquide ou glace, reserve > 2 %, P >= 6 mbar | = reserve (max 97 %) |
| **Ammoniac** | 195 < T < 240 K, reserve > 40 %, 25 % de chance (remplace l'eau) | = reserve |
| Methane (hors glace) | methane liquide | 2 a 32 % |
| Aucun | sinon | 0 |

Points de gel / ebullition utilises : eau -2 °C / ebullition selon la pression ; ammoniac -78 °C ; methane -182 °C ; lave
1 000 °C / 3 000 °C.

### Niveau de la mer

Le relief suit une loi normale N(0,5 ; 0,09) : le niveau de la mer qui donne une couverture `f` vaut `0,5 + 0,09 x probit(f)`.
Un monde sans liquide garde des bassins a sec (niveau de mer pour 5 a 35 % de bassins).

### Glace, neige, sous-sol

- Neige, calottes et glaciers **seulement s'il y a de l'eau** (`snow`) ; givre de CO2 sous -78 °C (calottes de Mars).
- Calottes = part de la surface plus froide que -10 °C.
- Eau souterraine : `reserve x 300 x (1 - ocean x 0,5)` m d'eau.
- **Ocean sous la glace** (Europe, Encelade) : lune glacee chauffee par les marees (>0,05), avec de l'eau.
- Mer qui gele (banquise) sous son point de gel ; mer a sec au-dessus de son point d'ebullition.

---

## 7. Geologie et relief (`geology.rs`, `landforms.rs`, `rocks.rs`, `caves.rs`)

### Chaleur interne et tectonique

- Activite = `2,7 x sqrt(m) x exp(-age / (3 sqrt(m)))` (0 a 1) : masse forte et jeune = chaud. Terre ~0,6, Venus ~0,4, Mars/Lune : eteintes.
  Les marees d'une geante proche l'entretiennent (Io, Europe).

| Regime | Condition |
|---|---|
| **Tectonique des plaques** (6 a 13 plaques) | activite > 0,35 **et** eau liquide **et** masse > 0,3 M_terre |
| **Couvercle stagnant** (une plaque : volcans geants, peu de chaines) | activite > 0,08 |
| **Inactive** | sinon |

- **Volcanisme** = activite x U(0,6 ; 1,4). Nombre de volcans : plaques `volcanisme x 18` ; couvercle `4 + volcanisme x 26` ; inactif 0 a 2.
  Hauteur `(0,12 + 0,18u) / sqrt(g)` (plus hauts a faible gravite : Olympus Mons). Formes : **bouclier** 50 %, **cone** 30 %, **caldeira** 20 %.
- **Age de la surface** : plaques 0,1 - 0,5 Gyr ; couvercle jeune (coulees) ; inactif = presque l'age de la planete (cratere).
- **Seismes** (par rapport a la Terre) : plaques `activite/0,58` ; couvercle 5 % ; inactif 0.
- **Champ magnetique** : noyau encore liquide (activite > 0,12) x rotation (une face eclairee = 0,15). Geantes : dynamo d'hydrogene metallique (1 a 20).
- **Erosion** : pluie (eau liquide 0,8), vent (air epais), glace ; adoucit le relief et **efface les cratères**.

### Formes du relief

Chaines de montagnes (convergence de plaques), rifts et dorsales, volcans, canyons (hors plaques), **plateaux / mesas** (35 % hors plaques),
**cratères** (loi de puissance en 7 classes de taille) :

| Cratere | Rayon angulaire | Forme |
|---|---|---|
| Simple | < 0,012 | cuvette |
| Complexe | 0,012 a 0,08 | fond plat, parois en 3 terrasses, pic central |
| Bassin a anneaux | > 0,08 | fond plat + anneaux concentriques |

Cratères recents : ejectas et rayons clairs ; vieux : uses ; fonds remplis (lave figee, glace). Densite = `age_surface/3,5 x bouclier atmospherique x (1 - erosion)`.

### Relief a l'echelle du marcheur (0.13)

`Landforms::offset` : deformation du domaine, collines (8 a 25 voxels), massifs en cretes (120 a 400 voxels), bosses (2 a 6), vallees d'erosion
(mondes a air et a eau). Formes 3D (`rocks.rs`) : falaises avec surplomb, strates, pitons, chaos de blocs, gorges, ponts naturels, arches,
cheminees de fee. **Grottes** (`caves.rs`) : tube de lave, karst, glace, geode, faille ; jusqu'a 2 000 unites de profondeur.
Un meme code sert au terrain voxel (`terrain.rs`) et au maillage vu de l'espace (`mesher.rs`) : un sommet vu de l'espace est le meme a pied.

Hauteur du terrain : `rayon x (0,025 + u x 0,025)` (planete), `rayon x (0,035 + u x 0,02)` (lune).

### Geologie vivante (0.13 T5, `geoactive.rs`)

Laves (volcanisme > 0,45), geysers (eau liquide), fumerolles, cryovolcans, seismes : tous f(graine, horloge).

---

## 8. Radiation, vie et biomes

### Radiation au sol (`biome.rs::surface_radiation`, 0 a 1)

UV (arretes par l'air ; l'ozone d'un monde a O2 les filtre x0,1), rayons X (arretes des quelques dixiemes de bar), particules
d'eruptions (deviees par le champ magnetique). `dose / (dose + 5)`.

### Vie (`life.rs`) - independante de l'habitabilite

Il faut un **liquide** et du **temps**. Chances (microbes / simple / complexe) selon le milieu :

| Milieu | Microbes | Simple | Complexe |
|---|---|---|---|
| Eau liquide en surface avec air | 60 % | 30 % | 12 % |
| Methane / ammoniac avec air | 20 % | 6 % | 1,5 % |
| Ocean sous la glace | 25 % | 5 % | 1 % |
| Air sans liquide | 3 % | 0 | 0 |
| Rien | 1 % | 0 | 0 |

Multipliees par l'age (microbes des ~0,15 Gyr, simple ~0,5 Gyr, **complexe ~1,25 Gyr**) et, pour la surface, par `1 - radiation`.

Niveaux : aucune, **microbienne**, **simple** (tapis, algues, plantes), **complexe** (animaux). Chimie : « carbone et eau (realiste) »,
« azotosomes dans le methane (speculatif) », « carbone dans l'ammoniac (speculatif) », « silicium et aetherion (fictif) ».
**Plantes** (`flora`) = vie simple+ en surface : sinon les biomes verts restent **nus**.
Faune (vie complexe) : 1 000 a 10 millions d'especes, taille max 5 a 30 m / gravite^0,7 (x0,3 sans O2), locomotion marche / vol (air >= 0,5 bar et
g < 1,5) / nage / terriers, nocturne si T > 40 °C ou radiation > 0,3.

### Biomes (21, `biome.rs`)

Choisis par **temperature x humidite x altitude x sol x radiation**. Humidite = `(0,25 + 0,5 ocean + 0,4 nuages)` modulee par les ceintures
climatiques (equateur humide, deserts vers 25-35°, latitudes moyennes humides, poles secs) et du bruit.

| Famille | Biomes (matiere voxel) | Rigueur |
|---|---|---|
| **Terrestres** (avec O2) | inlandsis (neige), toundra, taiga, foret temperee, prairie, steppe, savane, foret tropicale, marais, desert, plage, haute montagne | realiste |
| **Mineraux** (sans vie) | plaine de regolithe, champ de basalte, desert de sel, plaine de rouille | realiste |
| **Extraterrestres** (sans O2) | foret de cristal, plaine de spores, jungle fongique | **fictif** |
| | desert de verre, marais de soufre | speculatif |

Regles principales :
- `T < -10 °C` : inlandsis (si neige) ; `T > 100 °C` : sol nu ; `T > 45 °C` ou humidite < 0,12 : desert (desert de **verre** si radiation > 0,6).
- Radiation > 0,75 : la vegetation fragile est grillee.
- Sans air ou sans eau liquide : sols nus ; marais de soufre là où les volcans degazent du SO2.
- Sous la mer mais a sec : **desert de sel** (une mer s'est evaporee). 0 a 0,008 de hauteur : plage. > 0,42 : haute montagne.
- Terrestre vs extraterrestre : seule difference = presence de O2 (`< 5 %` = extraterrestre). La temperature donne ensuite :
  < 2 °C toundra / cristal ; 2-9 °C taiga ; 9-22 °C foret / prairie / steppe (fongique / spores) ; > 22 °C jungle / savane / desert.
- Sols (`Soil`) : sable, argile, regolithe, volcanique (basalte), glace, sel, metal (rouille si densite > 6,2).

### Materiaux voxel (`VoxelType`)

Air, Eau, Sable, Herbe, Roche, Neige, Glace (banquise), Methane, Ammoniac, Lave, Toundra, Taiga, Foret, Steppe, Savane, Jungle, Marais,
Basalte, Sel, Rouille, Cristal, Spore, Champignon, Verre, Soufre, Minerai (filons de grotte).

---

## 9. Ressources, habitabilite et traits

### Composition globale (`resources.rs`)

- Geante : gaz (H2, He) 85 %, glaces 10 %, roche et fer (coeur) 5 %.
- Rocheuse : part de **fer du noyau** d'apres la densite decompressee (`(d - 3,3) / 3,2`, de 2 % a 80 % : Terre ~0,35, Mercure ~0,6, Lune ~0,02),
  **glaces** (jusqu'a 45 %) au-dela de la ligne des glaces, le reste en silicates.

### Minerais (17)

| Realistes | Fictifs (rares) |
|---|---|
| fer, nickel, cuivre, aluminium, titane, or, platine, uranium, terres rares, silicium, glace d'eau, helium-3, deuterium, hydrocarbures | **Xenium**, **Aetherite**, **Chronite** |

Conditions : fer et nickel du noyau ; cuivre, or, uranium via la tectonique et le volcanisme (uranium : plaques, decroit avec l'age) ; titane des basaltes ;
**helium-3 : regolithe sans air expose au vent stellaire** (Lune) ; **deuterium** : oceans et geantes ; hydrocarbures : mers de methane ; glace d'eau : mondes glaces.
Fictifs : Xenium (cratères > 0,4, 2 %), Aetherite (30 % si gaz aether, 0,5 % si biomes exotiques), Chronite (0,4 %, profondeur 4 000 m).
Chaque gisement : abondance, profondeur, distribution (filons, couches, placers, nodules, fluide, cristaux), rarete, difficulte d'extraction
(profondeur, gravite, chaleur/froid extremes, pression, durete), quantite en tonnes. Chaque minerai = un bien de l'economie (`economy::GOODS`).

### Habitabilite (`habitability.rs`)

Score 0 a 1 = produit de : **temperature** (gaussienne 15 °C ± 25), **eau liquide**, **pression** (log gaussienne autour de 1 bar), **radiation**
(`1 - 1,5 x radiation`), **gravite** (1 g ± 0,45), **air respirable** (O2 partiel 0,16 - 0,5 bar, CO2 < 5 %, pas de SO2 / NH3 / Cx).
Etiquettes : > 0,6 **habitable** ; > 0,3 **vivable avec equipement** ; > 0,05 **hostile** ; sinon **inhabitable** ; geante : toujours inhabitable.

Dangers (niveau 1 gênant a 3 mortel) : chaleur extreme, froid extreme, vide / air mince, pression ecrasante, radiation, gravite forte, air toxique,
volcans et seismes, tempetes, **mers de lave**, **pluies acides**.

### Traits et anomalies (`traits.rs`)

Raretes : **98 % ordinaire**, 1,5 % peu commun, 0,4 % rare, 0,1 % legendaire. Le trait est choisi parmi ceux que l'astre permet.

| Rarete | Traits (rigueur) |
|---|---|
| Peu commun | geysers geants, super-tempete permanente, lacs de lave actifs, cratere d'impact geant, arches et cheminees de fees, champ magnetique inverse (realistes) |
| Rare | pluie de diamants, ocean bioluminescent, foret petrifiee, glace superionique, noyau de fer expose (speculatifs) |
| Legendaire | ruines d'une civilisation disparue, monolithe parfait, cristaux chantants, anomalie gravitationnelle, echo temporel (fictifs) |

Traits qui decoulent des donnees (pas des tirages) : anneaux, aurores polaires, rotation synchrone, rotation retrograde (inclinaison > 90°),
monde-ocean (> 90 % d'eau), ocean de magma.

---

## 10. Les lunes

- **Nombre** : rocheuse < 0,5 R_terre : 0 a 1 ; rocheuse / mini-Neptune : 0 a 2 ; geante de glace : 1 a 4 ; geante gazeuse : 2 a 4.
- **Rayon** : autour d'une rocheuse 12 a 33 % de sa planete ; autour d'une geante 0,08 a 0,45 R_terre (Ganymede = 0,41), max rayon planete / 3.
- **Masse** : `0,9 x r^3,4` (x0,6 si glacee, au-dela de la ligne des glaces) ; Lune ~0,012 M_terre.
- **Orbite** : espacee a partir de `rayon x (1,8-2,4 + 1,1-1,7 x rang)`, jamais sur la precedente. Excentricite 0 a 0,03.
- **Chaine complete** : une lune a toute la chaine d'une rocheuse (atmosphere, eau, geologie, biomes, vie, ressources, traits).
  **Une lune peut avoir de l'air** (comme Titan) et meme de la vie.
- **Chauffage par les marees** (`tidal_heating`, 0 a 1) : `0,6 x (M_planete / 318) x (6 / distance)^5 x (excentricite / 0,01)` ; Io ≈ 0,4 a 0,9, Europe ≈ 0,05,
  negligeable autour d'une rocheuse. Il entretient la tectonique/volcanisme et cree des oceans sous glace.
- **Rotation** : tirage de 2 a 16 jours (la rotation synchrone d'une lune vers sa planete est geree par `world_clock.rs`).
- Une lune glacee chauffee par les marees avec de l'eau = **ocean souterrain** (Europe, Encelade).

---

## 11. Petits corps du systeme (non-planetes)

- **Ceintures d'asteroides** (`belts.rs`) : rocheuse avant la premiere geante froide, glacee (Kuiper) apres la derniere planete. Classes : **C** (carbone,
  1,4 g/cm³, sombre), **S** (silicates, 2,7), **M** (metal, 5,0, fer 85 %), **Glace** (0,9). Melange change avec la distance (S dedans, C dehors).
- **Troyens** : aux points de Lagrange L4/L5 des geantes.
- **Cometes** (`comets.rs`) : famille de Jupiter et longue periode, chevelure + queue de gaz et de poussiere qui grandissent pres de l'etoile.
- **Anneaux** : particules (cellules hachees) qui tournent avec l'anneau.

---

## 12. Rendu : a quoi ressemble chaque type

| Type | Vue de l'espace | Vue a pied |
|---|---|---|
| Rocheuse | maillage `build_chunk_mesh` : couleurs de biomes (sol), mers, nuages (cubes), calottes | terrain voxel 3D, decor, meteo, jour / nuit |
| Geante gazeuse / glace / mini-Neptune | sphere lisse a **bandes de latitude** deformees par des tourbillons (`GasLook`) | pas de sol : vol jusqu'au coeur, brouillard de la couleur des nuages |

Aspects des geantes (`planet.rs::gas_look`, palette + bandes + tourbillons + contraste) :

| Variante | Palette | Bandes |
|---|---|---|
| **Jupiter chaud** | rouges sombres et braise | 6-10 |
| **Jupiter** (60 % des geantes gazeuses) | ocres, oranges, bruns, creme | 8-16 |
| **Saturne** (40 %) | jaune pale, creme | 10-16 |
| **Neptune** (50 % des geantes de glace) | bleus profonds | 3-6 |
| **Uranus** (50 %) | cyan pale | 3-5 |
| **Mini-Neptune** | bleu-gris brumeux, presque uni | 2-4 |

Chaque planete a une legere teinte propre. Les noms de biome / liquide / matiere se retrouvent dans le scanner (touche I), le dex (touche K), `/profil`, `/stats`.

---

## 13. Outils pour tester

- `/profil` : exporte l'astre cible en JSON (toutes les sections ci-dessus). `/graine` : code court de la graine du monde.
- `/aller etoile|planete|lune <type>` et `/aller suivant` : teleporte vers un monde du type voulu ; `/aller planete errante`.
- `/stats [n|tout]` : comptes et pourcentages de tous les astres d'une galaxie (panneau F3, fichier `saves/vX.Y.Z/stats/`).
- `/ceinture`, `/comete`, `/geologie`, `/relief`, `/grotte`, `/impact`, `/eclipse` : se rendre a la forme voulue.
- Tests : `planetgen::tests` (monde reproductible, types d'etoiles, ressources), `settings::tests::planets_follow_their_star_and_never_touch`,
  `mass_radius_matches_the_solar_system`, `greenhouse_is_calibrated_on_venus_earth_and_mars`, `rarities_follow_98_1_5_0_4_0_1`.
- **Changer la generation = augmenter `PROTOCOL`** (`net.rs`), et supprimer `saves/settings.json` et `saves/astres.json`.
