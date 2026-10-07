# Analyse de la génération planétaire de SpaceSpore

## 1. Cohérence scientifique

### Points forts

- **Ancrage réel** : Chen & Kipping (2017), Stefan-Boltzmann, corps noir CIE 1931, fuite de Jeans, Clausius-Clapeyron, ligne des glaces à 2,7×hz, chauffage par les marées (Io/Europe), régimes tectonique/couvercle stagnant/inactif — tout cela est à jour et bien calibré. La distribution stellaire (66 % M) est réaliste.
- **Validation croisée** avec le Système solaire (tests nommés : `mass_radius_matches_the_solar_system`, `greenhouse_is_calibrated_on_venus_earth_and_mars`) — c'est rare et précieux.
- **Diagramme de phase de l'eau** incluant le supercritique, distinction glace/eau/vapeur selon pression → très propre.

### Incohérences / points faibles

- **Le cas « Vénus » est probablement inatteignable.** La composition est choisie d'après `T_eq` (albedo 0,3). Or à l'orbite de Vénus (0,72 UA, L=1) : `T_eq ≈ 300 K`, jamais > 400 K. Le seuil >400 K n'est franchi que très près de l'étoile (<0,5 UA). Résultat : **aucun monde tempéré ne basculera vers un emballement à effet de serre** par rétroaction CO2/H2O. Un Vénus-analogue à 0,7 UA est impossible, alors que la doc se réclame de Vénus (+500 K). Il manque une **boucle de rétroaction runaway** (T_eq → composition → T_surface → évaporation → T_eq…).
- **Seuil 25 % sans atmosphère** : cohérent avec les données exoplanétaires (stripping par flares M), mais combiné aux 66 % de naines rouges, cela crée un biais énorme (cf. §3).
- **`T_eq > 400 K` → Vénus ou CO2/SO2 50/50** : la seconde branche (0,001–0,3 bar) est un choix arbitraire sans équivalent réel clair.
- **Verrouillage synchrone à `a < 0,4 × M_star^(1/3)`** : formule correcte en ordre de grandeur (limite de Roche/tidal), mais ignit la résonance spin-orbite 3:2 (Mercure).
- **Anneaux `glace = 0,7–1,0` au-delà des glaces, `0–0,3` en deçà** : Saturne réel a un ratio eau/roche ~0,9, ok ; mais les anneaux sombres de Jupiter sont plutôt organiques/rocheux — cohérent.
- **Champ magnétique** `activité > 0,12 × rotation` → OK qualitativement, mais il faudrait un seuil de taille de noyau (Mercure, Ganymède contredisent un modèle purement thermique).
- **`age_surface / 3,5 × bouclier_atmo × (1-erosion)`** pour les cratères : la normalisation à 3,5 Gyr est arbitraire ; le comptage réel suit une saturation exponentielle (pas linéaire) au-delà de ~3 Gyr.

## 2. Qualité de génération procédurale

### Points forts

- **Architecture en couches figées** (14 sous-graines numérotées) : extrêmement propre pour la compatibilité réseau et le versioning. Ajouter une couche ne casse rien — c'est un choix de design excellent.
- **Tout déterministe, rien de stocké**, valeurs arrondies pour l'empreinte réseau : modèle cohérent et efficace.
- **Un seul code terrain voxel/espace** (`mesher.rs` partage `terrain.rs`) : garantit qu'un sommet vu de loin = un sommet à pied. Gros gain de cohérence.
- **Richesse combinatoire réelle** : 4 types × 13 gaz × 21 biomes × 17 minerais × traits.

### Points faibles

- **Complexité en cascade** : 14 couches avec dépendances non triviales (T_eq → atmosphère → albédo → T_surface → hydrologie → érosion → cratères → biome). Difficile à tester exhaustivement. Les cas limites (T_eq proche d'un seuil) créent des discontinuités brutales.
- **Seuils durs** (25 %, 3 %, 98 %, 50/50) : donnent une distribution **multimodale** — de grosses grappes de mondes identiques plutôt qu'un continuum.
- **Choix de biome global** avant variation locale : la température/humidité varient en latitude, mais le biome est sélectionné selon une moyenne — risque de « biome dominant » uniforme sur toute la planète.
- **Traits purement additifs** : aucune interaction (ex. océan + bioluminescence, calotte + cryovolcanisme ne fusionnent pas en un type émergent).

## 3. Risques de répétition

C'est le point le plus préoccupant du doc.

1. **Biais « rocheuse + sans air + froide »**. En zone externe : 35 % rocheuse. Parmi elles, 25 % sans atmosphère. Si M-dwarf (66 % des systèmes) + orbite serrée → stripping → sans air. Ces mondes tombent presque tous dans les 4 biomes **minéraux** (régolithe, basalte, sel, rouille). Or les 17 autres biomes (les plus intéressants) sont derrière le filtre O2/vie/eau. **Le monde médian du jeu est un caillou gris.**
2. **Un seul tirage `roll` par planète pour le type** → pas de corrélation à l'échelle du système, mais aussi aucune structure (pas de « système riche en géantes » ou « système rocheux »). Chaque planète est tirée indépendamment — statistiquement correct, mais **perceptuellement plat**.
3. **Traits à 98 % ordinaires** : sur 100 mondes, ~1,5 peu commun, 0,4 rare, 0,1 légendaire. Un joueur peut traverser 50 systèmes sans jamais voir un monolithe. Les points d'intérêt sont **trop rares pour être moteurs d'exploration**.
4. **6 palettes de géantes** (Jupiter 60 %, Saturne 40 % / Neptune 50 %, Uranus 50 %). Une géante vue = 1 des 6 visuels. Pour les plus gros objets d'un système, c'est peu.
5. **Atmosphères effectivement 5 types** (aucune, Vénus, tempérée, Titan, Mars). Les mondes tempérés sans O2 sont visuellement proches.
6. **Lunes** : chaîne complète de rocheuse, mais petites → masse < 0,3 M_terre → pas de tectonique → pas de volcanisme → biome stérile. Une lune sur deux ressemblera à une autre. Seules les lunes de géantes (chauffage de marée) sortent du lot.
7. **Naines rouges partout** (66 %) → la majorité des systèmes ont une HZ à < 0,3 UA → beaucoup de mondes synchrones → vent jour→nuit, climats « partagés en deux ». Uniformise la sensation de jeu.
8. **Pas de sub-Neptune/planète naine** : tout est soit rocheuse, soit mini-Neptune, soit géante. Pas de Pluton, Cérès, Éris comme entité visitable (ils sont dans les ceintures).

## 4. Priorités d'amélioration

Classées par ratio effort/impact.

**P0 — Rétroaction runaway (correctif scientifique)**
Boucler T_eq ↔ composition ↔ T_surface 3-4 fois ; introduire une rétroaction H2O/CO2 qui pousse un monde tempéré vers un Vénus. Sans cela, le label « Vénus » n'est jamais produit pour de vrai.

**P1 — Casser la monoculture « rocheuse sans air »**
- Moduler le taux « sans atmosphère » par masse, âge et activité stellaire plutôt que 25 % fixe.
- Autoriser de la vie aérobie à 25 % des tempérés (déjà fait) mais aussi des **biomes primaires** hors O2 (toundra anaérobie, désert de soufre, plaine de méthane) pour élargir la variété sans vie.
- Faire varier la composition en **mode système** : tirer un « métallisme » et un « régime » par système qui biaise tous ses corps (crée une identité par système).

**P2 — Relever le plancher de rareté des traits**
Passer à ~92 / 5 / 2 / 1 %. Un trait par système visité minimum. Corrélé à un attribut local (cratère > 0,4, océan > 90 %, activité > 0,7) plutôt qu'un tirage aveugle — la découverte devient guidée par l'observation.

**P3 — Variabilité visuelle des géantes**
Exposer plus de paramètres de `GasLook` : nombre et taille des taches (0-3), courbure des bandes, contraste, palette dérivée de T_eq. Actuellement 6 looks, viser 30+ combinaisons émergentes.

**P4 — Diversifier les lunes**
- Ajouter un type **corps capturé** (astéroïde rocheux, pas de chaîne complète).
- Autoriser tectonique sur les grosses lunes sans chauffage de marée en abaissant le seuil masse.
- Répartir les lunes d'une même planète dans 2-3 « familles » (prograde/rétrograde, composition différente) pour créer des contrastes visibles dans une même visite.

**P5 — Dwarf planets et sub-Neptune explicites**
Introduire un 5ᵉ `PlanetKind` rocheux très petit (0,1-0,34 R_E, Pluton/Cérès) avec géologie à part (cryovolcanisme, azote, tholins). C'est un réservoir de variété « sans air » bien plus intéressant que le régolithe.

**P6 — Interactions de traits**
Autoriser 2 traits combinables sur les légendaires (« océan bioluminescent + cristaux chantants ») et des traits qui modifient les règles locales (ex. « écho temporel » : la graine du biome change à chaque heure de jeu).

**P7 — Tests de non-répétition**
Ajouter un test automatisé qui échantillonne N=10 000 mondes, calcule l'entropie des biomes/types/atmosphères, et échoue si l'entropie descend sous un seuil. La répétition est mesurable — autant la surveiller comme un KPI.

---

**Bilan** : la cohérence scientifique est très au-dessus de la moyenne du genre (Chen & Kipping, Jeans, marées, CK calibré), mais elle est **statique** : un monde ne peut pas basculer d'un régime à l'autre (pas de runaway). La qualité procédurale est excellente dans l'architecture (sous-graines figées, déterminisme réseau), plus faible dans la **distribution** (beaucoup de seaux, seuils durs). Le risque n°1 est la monoculture des rocheuses sans air, qui étouffe les 17 autres biomes ; le n°2 est la rareté extrême des traits, qui vide l'exploration de sa récompense.