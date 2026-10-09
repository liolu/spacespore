# SpaceSpore - Architecture detaillee

Notes techniques par systeme et par version, sorties de `CLAUDE.md` le 09/10/2026 pour l'alleger (ce fichier
n'est pas charge automatiquement : le lire quand on travaille sur le systeme concerne). Index court dans
`CLAUDE.md` § « Index des systemes ». Quand une phase est faite, ajouter ses notes ici (pas dans `CLAUDE.md`).

## Structure et systemes

- Origine flottante (`src/origin.rs`, `settings::origin/to_local/to_abs`) : les positions des entites sont des f32
  relatives a une origine absolue en f64 qui suit le vaisseau (recentrage au-dela de 100 000). Ne jamais garder une
  position « monde » en memoire : stocker l'absolu (`abs_center()`, `Wormhole.abs_a`, `Peer.abs`) et convertir.
  `StarSystemConfig::center()` / `GalaxyConfig::center()` = monde ; `abs_center` = absolu (generation, caches).
- Echelle : `GALAXY_SCALE` (settings.rs, 300 x `SPACE_STRETCH`) multiplie les distances entre etoiles/galaxies
  (rayon 13,5 G ; voisine la plus proche ~100 M en mediane). `SPACE_STRETCH` = 5 (0.11) : etirement visuel de
  toutes les distances (etoiles, galaxies, orbites des planetes et lunes, applique a la fin de `system.rs`) ; tailles
  des astres (`GALAXY_SIZE_SCALE` pour les trous noirs) et physique (UA, temperatures, marees) inchangees, orbites
  plus lentes (Kepler), lumiere des etoiles compensee (`lumens`, `light_range_for`), vaisseau x5. Systemes ~19 M
  en mediane, 52 M pour 99 %. Galaxies : 10 000 = la principale, 20 exterieures (`NUM_DISTANT_GALAXIES`) et 9 979 lointaines
  (`NUM_OUTER_GALAXIES`, grille de cellules de 1,3 a 10 fois la plus lointaine, ajoutees apres : rien de connu ne
  change), toutes de vraies galaxies. `settings.systems` = `systems::Systems` : les 21 premieres galaxies generees
  au depart (`dense()`, numeros d'avant), chaque lointaine a une plage de numeros fixe (`galaxy_range`) et n'est
  generee qu'au premier acces (`get`, `in_galaxy`, `load`) ; `iter()` = systemes deja generes AVEC leur numero.
  Galaxie generee -> `planet::FarGalaxyLoaded` (index spatial, factions `FAR_FACTION_BASE + g x 64 + k`, trous de
  ver `generate_galaxy`). Bras / trou noir / disque (`stream_galaxy_visuals`) et nuages (`stream_clouds`) crees a
  l'approche seulement. `/aller` cherche dans les 21 premieres galaxies. LOD (`planet.rs`) : etoiles chargees par galaxie a l'approche (`stream_galaxy_stars`, entites
  `FarStar` creees / retirees), eclaircies avec la distance (`star_keep`, toujours les memes : elles reviennent en
  s'approchant), galaxie en point au-dela de `POINT_START` (`GalaxyPoint`, bras / trou noir / disque effaces). Les etoiles lointaines sont groupees en
  secteurs de ~100 etoiles (`StarSectors`) affiches/mis a jour ensemble.
- Proportions d'un systeme (generees depuis la graine du monde, `settings.rs`) : echelle G du systeme
  600 000 a 1 500 000 (`StarConfig::scale()`) ; 1 R_terre = echelle/109. 1 a 8 planetes (`planetgen/system.rs`) :
  orbites en UA (zone habitable ~ racine(L)), affichees en echelle log (zone habitable a 3,2 echelles), rocheuses,
  mini-Neptunes, geantes de glace et gazeuses (masse -> rayon de Chen-Kipping, gravite reelle a pied) ; lunes
  <= planete/3. Geantes gazeuses : pas de sol, on y vole jusqu'au coeur (`GAS_CORE`), la pression retire des PV
  (`gas.rs`, `combat.rs`), destruction = retour en orbite. Valeurs affichees arrondies (empreinte reseau).
  Atmosphere et climat (`planetgen/atmosphere.rs`, `climate.rs`) : retention des gaz (Jeans, rayons X), pression,
  albedo, serre, nuages, vents, couleurs du ciel ; `PlanetConfig::climate().temperature(lat, alt, moment)` ;
  matieres de surface (neige, sable, herbe, mer gelee ou a sec) par `climate::land_material/sea_material`,
  partagees par `terrain.rs` et `mesher.rs`. Brume de l'horizon et brouillard des geantes : `gas.rs`.
  Eau et glace (`planetgen/hydrology.rs`) : etat de l'eau (diagramme de phase), couverture oceanique -> niveau de
  la mer (relief ~ N(0,5 ; 0,09)), mers d'eau, methane, ammoniac ou lave (`VoxelType` Ice/Methane/Ammonia/Lava),
  neige seulement s'il y a de l'eau (`Hydro::snow`) ou du givre de CO2.
  Geologie et relief (`planetgen/geology.rs`) : activite interne, tectonique, volcans, seismes, champ magnetique,
  erosion ; `ReliefField::offset` (plaques -> montagnes/rifts, volcans, canyons, plateaux, crateres) ajoute au bruit
  des continents, partage par `terrain.rs` et `mesher.rs`. Banc des tuiles : `cargo test --release bench_tiles --
  --ignored --nocapture` (1,07 ms/tuile avant la phase 5, 1,18 apres, 1,20 avec les biomes).
  Sols et biomes (`planetgen/biome.rs`) : `BiomeField::material` choisit la matiere du sol (une `VoxelType` par
  biome : toundra, taiga, foret, jungle, cristal, spores, verre, soufre, basalte, sel, rouille...) d'apres
  temperature, humidite (ceintures + bruit), altitude reelle, sol et radiation ; terrestres si O2, sinon
  extraterrestres. Partage par `terrain.rs` et `mesher.rs` (couleur vue de l'espace = sol).
  Phase 9 : habitabilite et dangers (`habitability.rs`), traits 98/1,5/0,4/0,1 % (`traits.rs`), lunes = toute la
  chaine (`system.rs::world_layers`, maree `tidal_heating`, `MoonConfig::as_planet` pour terrain/rendu/profil),
  anneaux et aurores (`planet.rs::spawn_ring_and_aurora`), panneau scanner de l'astre cible (`scanner.rs`, touche I).
  Phase 8 : ressources (`planetgen/resources.rs`) : composition globale (noyau de fer d'apres la densite
  decompressee, silicates, glaces, gaz) et gisements de minerais reels et fictifs (Xenium, Aetherite, Chronite,
  `Realism::Fictional`) : abondance, profondeur, distribution, rarete, difficulte, quantite (t). Chaque minerai =
  un bien du rayon « Ressources » (`Ore::good`, biens ajoutes a la fin de `economy::GOODS`, une faction n'en vend
  qu'une partie : `economy::sold_by`). Minage (0.15) : `BodyDelta::ores` = tonnes extraites. Scanner, profil, `/stats`.
  Phase 7 : vie independante de l'habitabilite (`planetgen/life.rs`, sans plantes les biomes verts restent nus),
  decor voxel des tuiles proches (`decor.rs` : `tile_decor` calcule avec la tuile, enfants de la tuile, maillages
  et materiaux partages). Banc : `cargo test --release bench_decor -- --ignored --nocapture`. L'etoile a un type (`planetgen/star.rs`, O..M, naine blanche/brune, sous-geante, geante rouge) :
  une G garde l'echelle, les autres types ont leur taille reelle (1 R_sol ~ 1 050 000 a l'ecran), compressee
  au-dela de 1,5 M (max 6,5 M) ; les planetes d'une geante sont repoussees hors d'elle. Aucun type impose (Sol compris). Planetes et lunes
  sont explorables : zoomer sous 1000 du vaisseau = navigation basse altitude (ZQSD, Maj, Espace/Ctrl, clic
  droit, molette), `V` = atterrir (sortir du vaisseau) puis marcher, `V` = redecoller. Le dessous du vaisseau reste
  parallele a la surface. `src/terrain.rs` = terrain voxel (champ de hauteur, quadtree de tuiles),
  `src/surface.rs` = vol, atterrissage, marche, lumiere. Saves : `saves/vX.Y.Z/` (settings, world, info).
- Generation 0.10 (`roadmaps/fait/ROADMAP-0.10.md`) : module `src/planetgen/`. Les planetes et lunes ne sont plus stockees :
  `sys.planets()` les recalcule depuis le genome du systeme (cache libere loin du vaisseau), `planets_mut()`
  pour l'editeur, `planets_uncached()` pour parcourir tous les systemes. Profils `StarProfile`/`PlanetProfile`,
  sous-graines par couche (`seeds.rs`, numeros figes), conversions dans `units.rs` uniquement, valeurs
  vivantes = depart + delta (`live.rs`, `body_deltas` de `world.json`). Tests du monde : `planetgen::tests`
  (reproductible, types d'etoiles) et `settings::tests::planets_follow_their_star_and_never_touch` ; changer la
  generation = augmenter `PROTOCOL` (`net.rs`). Chat : `/profil` exporte l'astre cible en JSON, `/graine` = code court. Tests : `/aller etoile|planete|lune <type>` et `/aller suivant`
  (`test_cmd.rs` : recherche en arriere-plan, teleporte au bord du systeme trouve, cible l'astre une fois charge).
  Chat : Tab complete (`chat_cmd::suggestions`, liste `COMMAND_HELP`), fleches = historique (`saves/.../chat_history.txt`),
  Ctrl+Retour arriere = efface un mot. Entites qui peuvent disparaitre dans la meme image (teleportation) : `try_insert`. `/stats [n|tout]` :
  comptes et pourcentages de tous les astres d'une galaxie (`stats.rs`, calcul en arriere-plan, panneau F3,
  fichier `saves/vX.Y.Z/stats/`). Option « Afficher zones » (menu Options) : zones chaude / habitable / froide du
  systeme charge (`zones.rs`, limites interpolees entre les vraies planetes).
- 0.11 (`roadmaps/fait/ROADMAP-0.11.md`) : A1 = horloge du monde `world_clock.rs` (`WorldClock`, secondes de jeu f64, `clock` de
  `world.json` ecrit toutes les 30 s, donnee par l'hote : `net::follow_host_clock`, PROTOCOL 16). Orbites
  (`kepler::position(t: f64)`) et rotation = f(horloge). 1 h de la planete = 1 min de jeu (jour <= 3 h), saisons
  1 h en moyenne (`season_secs`, `SEASON_REF_DAYS`). `Spin` : rotation autour de l'axe incline (l'axe penche vers
  l'etoile a l'ete du nord, il fait le tour en une annee des saisons), synchrone = face +X vers l'etoile (lunes :
  vers leur planete). La racine de l'astre porte la rotation (tuiles, anneaux, aurores, nuages suivent).
  Repere fixe de l'astre (regle 10) : `surface::Frame` (`to_world`/`to_local`) ; tout ce qui est pose ou vole bas
  (marcheur, vaisseau, camera, `hover_dir`) est stocke dans ce repere. Chat : `/heure`, `/temps <facteur>` (hote).
  A2 = jour / nuit (`surface.rs`) : `daylight` (ciel), nuit au sol `AMBIENT_NIGHT` + clair de lune (`moonlight` :
  phase et taille apparente des autres astres), brume moins opaque la nuit (`haze_opacity`, lunes et etoiles
  visibles), galaxie visible la nuit (`NIGHT_GALAXY`), lampe du marcheur / phares du vaisseau (touche N, allumes
  dans le noir, puissance calee sur la lumiere de l'etoile : `update_lamps`). Ligne jour/nuit = lumiere de l'etoile.
  A3 = temperature vivante : `Climate::season` (`Season` : declinaison avec retard, excentricite, longitude du
  soleil ; jamais sauvee), `Climate::at(season)`, `temperature(lat signee, alt, Some(Moment { hour }))` (max a
  14 h 30, `DAY_PEAK`), givre du matin (`Climate::frost_at`, biomes). `Spin::season` ; amplitude jour/nuit
  `atmosphere::diurnal_amplitude`. Tuiles reconstruites quand la saison arrondie change (`TileStore::generation`,
  `surface::update_season`), maillage lointain aussi (`PlanetChunk::season`). `world_clock::LocalWeather` :
  heure, saison, temperature, min/max du jour et de l'annee (HUD, scanner, `/heure`). PROTOCOL 17.
  B1 = voxels 3D (`terrain.rs`) : cellules = colonnes de la sphere-cube au niveau le plus fin x couches radiales
  d'un voxel (k = 0 au niveau de la mer). Une seule fonction `Terrain::kind_at` = delta, puis champ de hauteur
  (`base_column`), puis formes 3D (arche de test `Overhang`, `/surplomb`). Tuiles du niveau le plus fin maillees en
  3D (`build_voxel_tile_mesh`), les autres en champ de hauteur (`build_height_tile_mesh`) ; `column` = vue de dessus.
  Collisions 3D : `Terrain::floor(dir, r)` (sol sous un point) et `ceiling` (marcheur, vol bas, camera).
  Deltas : `voxel.rs` (`BlockKey` 32^3, `BodyVoxels`, `VoxelDeltas` dans `world.json`, message `VoxelEdit`, minage
  0.15). Banc : `cargo test --release bench_voxel_tiles -- --ignored --nocapture`. PROTOCOL 18.
  B2 = grottes (`caves.rs`) : regions cubiques de 40 voxels hachees (regle 12, cache partage `Arc<Caves>` entre
  les tuiles : `Terrain::with_caves`), salle + tunnels vers des portes partagees avec les voisines, puits d'entree
  pres de la surface ; sortes selon la geologie (`CaveStyle::of`) : tube de lave, karst (lacs, stalactites), glace,
  geode, faille ; jusqu'a 2 000 unites (`MAX_DEPTH`). `Terrain::kind_in` : roche profonde (`style.rock`) et
  filons `VoxelType::Ore` (`ore_chance`, plus riches en profondeur). Maillage : pieces filtrees par colonne + memo.
  Sous terre (`Surface::underground`) : lumiere de l'etoile eteinte (`dim_star_light`), lampe allumee.
  Champignons lumineux (`DecorKind::GlowShroom`, vie). Scanner : grotte la plus proche ; `/grotte`. PROTOCOL 19.
  B3 = montagnes (`geology.rs::ReliefField::sample`) : chaines en bruit de cretes (`ridged`), cols, erosion qui
  arrondit ; mesas symetriques alignees sur la mer (`with_sea` : la cote ne bouge pas) ; canyons a fond plat ;
  volcans avec cratere au sommet et coulees figees (basalte) ; eboulis au pied (`ReliefSample` -> matiere dans
  `base_column`). Arches et cheminees de fee (`rocks.rs`, cellules de 60 voxels hachees, `Piece::Add`, mondes avec
  air), partagees entre tuiles (`Terrain::with_rocks`). PROTOCOL 20.
  B4 = crateres (`geology.rs::ReliefField::craters`) : 7 classes `CRATER_CLASSES` (loi de puissance, cellules
  hachees), `CraterKind` simple / complexe (fond plat, terrasses, pic central) / bassin a anneaux selon le rayon
  angulaire, age (recents : ejectas et rayons clairs `ReliefSample::bright`, vieux : uses), fonds remplis
  (`flooded` : lave figee, glace sur monde froid), erosion qui efface les petits. Tuiles lointaines : pas de
  cratere plus petit qu'1,5 colonne (`Terrain::min_crater`). PROTOCOL 21.
  B5 = meteores (`meteors.rs`) : etoiles filantes et bolides = f(graine de l'astre, horloge) (`meteor_in_bin`,
  tranches d'une demi-seconde), la nuit avec de l'air ; 3 pluies par annee (`shower_strength`) ; impacts rares pres
  du joueur ou `/impact` : cratere en deltas voxel (`impact_crater`), sauve (`world.json`) et envoye
  (`net::Msg::Voxels`, l'hote relaie a tous ; `voxel_outbox`/`voxel_inbox`) ; `surface::VoxelsChanged` fait
  reconstruire tuiles et sol. Sans air : pas de trainee. PROTOCOL 22.
  A4 = survie a pied (`suit.rs`) : `Environment` (pression, O2, CO2, temperature locale de `LocalWeather` ou de
  la roche sous terre, radiation du sol, lave) -> `rates` (reserve d'O2 ~8 min, degats lents, alertes), `Suit`
  (O2, vie) : a 0, `Surface::request_rescue` ramene au vaisseau (abri : recharge et soins). HUD a pied.
  C1 = ceintures d'asteroides : `planetgen/belts.rs` (`sys.belts()`, couche `Layer::Belts`) : rocheuse avant la
  premiere geante froide (sans toucher les orbites), glacee type Kuiper apres la derniere planete ; masse, largeur,
  epaisseur, richesse, melange C / S / M / glace (`AsteroidClass::ores` -> minerais de la phase 8). `asteroids.rs` :
  pas de liste, cellules hachees dans le repere qui tourne avec chaque anneau (Kepler, `mu()` des planetes), 3 niveaux
  (`LEVELS` : cailloux, rochers, gros ou l'on se pose), champs denses (`field_density`), formes `AsteroidShape`
  (gravats, binaire de contact, allonge, metallique, fragment, crateres ; `radius_at` = maillage, collisions et
  terrain via `BodyParams::asteroid`), rotation sur le plus petit axe. `AsteroidField` : astéroides affiches autour de
  la camera (cible et astre visite toujours gardes), poses en PreUpdate, maillages des gros en asynchrone, bande de
  poussiere de loin. `TargetKind::Asteroid(AsteroidKey)` : vol bas, atterrissage, marche en microgravite. Chocs (Q6) :
  `ship_collisions` -> `AsteroidHit` (degats selon la vitesse, `combat.rs`), cailloux pousses sans degat, pilote
  automatique qui contourne. Chat : `/ceinture`. Banc : `cargo test --release bench_asteroids -- --ignored --nocapture`.
  PROTOCOL 25.
  C2 = anneaux, Troyens, cometes, planetes errantes. Anneaux (`rings.rs`) : `Ring` a `ice`, `gaps` (divisions),
  `profile(f)` (bandes, divisions vides) ; maillage polaire a couleurs de sommets, ombre de la planete sur l'anneau
  (`ring_light`) et de l'anneau sur la planete (coquille `ring_shadow`), dans un repere `RingFrame` qui garde
  l'etoile a l'azimut 0 (couleurs recalculees seulement quand sa hauteur change). Les petits corps passent par
  `asteroids.rs` (`Sources::of` : ceintures, essaims, anneaux, cometes ; `AsteroidKey::belt` = code de source,
  `SWARM_BASE`/`RING_BASE`/`COMET_SOURCE` ; `Path` : ceinture, point de Lagrange, comete, anneau ; `Elements` =
  Kepler f64, meme formule que `kepler.rs`). Troyens : `planetgen/belts.rs::trojans` (L4/L5 des geantes, cellules
  cubiques dans le repere du point de Lagrange). Particules d'anneau : cellules qui tournent avec l'anneau autour de
  la planete (`planet_mu`), absentes des divisions. Cometes : `planetgen/comets.rs` (famille de Jupiter / longue
  periode, `activity(r)`), toujours affichees dans le systeme charge, chevelure + queue de gaz (droite, opposee a
  l'etoile) + queue de poussiere (courbee, en retard) qui grandissent pres de l'etoile (`update_comet_tails`) ;
  `/comete`. Planete errante : 1 systeme sur 30, derniere de `planets()` (`PlanetConfig::rogue`), loin et hors du
  plan, immobile, physique sans etoile ; ignoree par ceintures, zones, orbites ; `/aller planete errante`.
  PROTOCOL 26.
  C3 = etoiles doubles et triples (`planetgen/multiple.rs`, ~1/3 des systemes) : `Multiplicity` Single / Close
  (paire serree au centre, planetes de type P au-dela de 3 fois l'ecart, periode 1 a 200 j) / Wide (compagnon
  lointain, planetes de type S en deca du quart de son passage au plus pres) / Triple. `sys.stellar()` (recalcule),
  `sys.lighting()` (luminosite et masse des etoiles du centre additionnees : zone habitable, periodes),
  `sys.star_physics_of(i)`. Compagnons stockes dans `sys.stars` avec `StarConfig::orbit` (`StarOrbit` :
  factor x Kepler relatif, `orbit_stars`). `OrbitLimits` (min_au, max_au, exclusion, outer) passe a `system.rs`,
  `belts.rs`, `comets.rs`. Surface : `sun_list` / `combined_sky` (ciel et jour de tous les soleils, double coucher),
  `SurfaceSun` = une lumiere directionnelle par soleil avec ombres (cascades ~200 voxels) qui remplace la lumiere
  ponctuelle des etoiles sur un astre solide (`dim_star_light`), le decor projette des ombres. `/aller etoile
  double|triple`. Etoile cliquee de loin (`Star(indice du systeme)`) : `promote_star_target` (main.rs) la change en
  `Star(systeme * 1000)` une fois le systeme charge (sinon le vaisseau reste au centre de masse, vide) ; `star_parts`.
  Tests : `SPACESPORE_TEST_CMD` (commande du chat a 6 s), `SPACESPORE_TEST_STAR=k|sys` (`test_cmd::dev_script`). Saisons recalees (`SEASON_REF_DAYS` 115 -> 300 : moyenne 1 h, Q2). PROTOCOL 27.
  C4 = phenomenes du ciel (`sky.rs`) : orages magnetiques `storm(seed, activite, t)` par tranches de 15 min
  (`Storms` : eruptions plus hautes dans `update_flare_voxels`, aurores avivees 2 min plus tard) ; eclipses :
  `occultation` des disques, `SunDim` (lumiere de chaque soleil a la camera, passee a `sun_list`), taches d'ombre
  des lunes sur leur planete (`shadow_spot`), voile sombre et rouge des lunes dans l'ombre de leur planete ;
  `/eclipse [lune]` (`next_eclipse`, memes orbites que `planet.rs`, l'hote avance l'horloge). Marees :
  `terrain::Tide` (renflement P2 vers chaque astre, 0 a 3 voxels, `tide_voxels` / `solar_tide_voxels`), mer
  et greve dans `base_column`, recalculee par `update_tides` quand le niveau change d'un voxel sous le joueur
  (tuiles reconstruites comme pour les saisons). Phases des lunes (`MoonPhases`, scanner). Aurores du sol :
  rideaux (`curtain_mesh`) la nuit, enfants de la planete ; les anneaux d'aurore vus de l'espace s'effacent.
  C5 = meteo (`weather.rs`), f(graine, horloge, lieu) : `WeatherParams::of` (air de la phase 3), vents zonaux
  (`zonal`), nuages advectes et qui se forment / se defont (`cloud_field`, deux champs fondus), `sample` : pluie,
  neige, grele, pluies exotiques (methane, acide, neige carbonique, verre, fer), orages + `lightning`, poussiere,
  brouillard du matin. Couche de nuages en cubes reconstruite en arriere-plan (`rebuild_clouds`, 2 s pour l'astre
  ou l'on est, 30 s sinon ; `rotate_clouds` ne fait plus deriver la couche). `WeatherNow` (lieu du joueur) :
  lumiere des soleils voilee (`SunDim`), ciel gris / brun, flash des eclairs, brouillard (`gas.rs`), particules
  autour de la camera (`particle_positions`), vent qui pousse le vaisseau en vol bas (`Surface::drift`), scanner.
  Banc : `cargo test --release bench_cloud_layer -- --ignored --nocapture`.
  Particules : `NoFrustumCulling` (sommets reecrits a chaque image) ; `SPACESPORE_TEST_PRECIP=<0..1>` force la pluie.
- 0.12 (`roadmaps/fait/ROADMAP-0.12-editeur.md`) : editeur de modeles voxel, module `src/editeur/`. E0 = fondations : etat
  `AppState` (Jeu / Editeur, `editeur::in_game` coupe le clavier et la souris du jeu), ouvert au premier lancement
  (`GameSettings::first_launch`, creation du personnage), par le bouton du menu (`EditorMenuButton`) et `/editeur`.
  Format `.ssvox` (`format.rs`) : archive zip (meta.json + voxels.bin + zones.bin), palette 255 couleurs +
  matiere (mate, metal, verre, lumineuse), `Sparse` = chunks 32^3 creux (absent / uniforme / plein, RLE), 10 Mo au
  plus (Q4), grilles : perso 16 x 32 x 32, vaisseau 64 a 1024 (`ShipCategory`), autre <= 64. Bibliotheque
  `saves/modeles/`, import Pixel World (`import.rs`, depuis `saves/import/`, couleurs de `BlockData.cs`). Test des
  vrais modeles : `cargo test --release real_pixel_world -- --ignored --nocapture`. PROTOCOL inchange.
  E1 = l'editeur : `edit.rs` (`Doc` : modele + historique, un trait de souris = un lot annulable, `Tool`
  ajouter / retirer / peindre / pipette, symetrie miroir en x par defaut pour les persos, `raycast` DDA, maillage des
  faces visibles), `view.rs` (la camera du jeu passe sur le calque `EDITOR_LAYER` = le monde disparait sans etre
  decharge ; lumiere et gizmos `EditorGizmos` sur ce calque ; camera orbitale ; raccourcis 1-4, X, G, F, Ctrl+Z (W ou
  Z physique), Ctrl+Y, Ctrl+S, fleches), `panels.rs` (interface reconstruite quand `ui_dirty`, fenetres Nouveau /
  Bibliotheque / Renommer / Etiquettes, interface du jeu masquee). Tests visuels : `SPACESPORE_CAPTURE=x.png`
  (+ `SPACESPORE_EDITOR_DEMO=1`, `SPACESPORE_CAPTURE_SECS`, `SPACESPORE_EDITOR_SCROLL`) fait une capture puis ferme le jeu.
  E2 = palette (`palette.rs`) : OKLCH (`oklch_to_rgb8` ramene la saturation dans l'ecran), grille 36 teintes x 12
  clartes x 4 `Saturation`, 16 gris, `THEMES` (peaux, cheveux, metaux, coques, militaire, neons, Pixel World),
  `parse_hex`, `sorted` (palette du modele : gris, teinte, clarte). Recentes (16), couleur libre (Hex...), matiere de
  la couleur ; clic droit sur une couleur du modele = `Doc::replace_color` (un lot annulable). Rendu par matiere :
  `edit::build_meshes` (4 maillages) et `view::material_of` (mate, metal, verre transparent, lumineuse sans ombre).
  A l'enregistrement, `Model::compact_palette` sur une copie (l'onglet garde ses index pour l'annulation).
  E4 = blocs de mouvement (`motion.rs`) : un JSON par bloc dans `assets/editeur/blocs/` (integres par `build.rs`,
  regle 4 ; le joueur peut en ajouter ou remplacer dans `saves/editeur/blocs/`) : parties (parent, pivot, boites de
  cases) et animations (`cles` : [t, rx, ry, rz] degres / [t, x, y, z] ; `onde` : axe, amplitude, periode, dephasage),
  « repos » obligatoire. `Placement` (quarts de tour en y, miroir x, taille) ; `Doc::place_block` = blocs blancs
  (`BLOCK_WHITE`) + une `format::Zone` par partie (bloc, partie, parent, pivot, orientation), symetrie = bloc
  reflete en face ; Ajouter contre une zone l'y fait entrer, Retirer l'en sort ; `remove_zone` (blocs gardes).
  Lots d'annulation avec zones (`Batch`). Rendu : une entite par (zone, matiere) (`RigPart`, faces cachees par la
  meme zone seulement), poses `motion::compose(zone_locals)` (aperçu P, choix de l'animation, melange 0,35 s),
  gabarit `Ghost` qui joue son repos (rouge s'il deborde ou recouvre), contours des zones, `motion::collisions`
  (zone qui traverse le corps fixe = rouge). Captures : `SPACESPORE_EDITOR_DEMO=blocs` (apercu) ou `gabarit`.
  E5 = races et animations : `races.rs` (`RaceDef` : os = parties nom / parent / pivot / boites, `sym` = decrit a
  gauche « _g » et reflete « _d », `fixed` = partie fixe qui suit son parent, `options` = membres optionnels avec
  `replaces`) ; 23 familles dans `assets/editeur/races/`, 49 animations (§3.3) dans `assets/editeur/anims/`
  (generes par un script, `build.rs` integre blocs / anims / races ; le joueur ajoute dans `saves/editeur/`).
  `motion::Library` (blocs + `LibAnim` + races) ; une animation vise des os par nom de zone (`bone` : espaces -> _),
  par motif de chaine (`queue_*`, `patte_*_g` : onde decalee de `step` par maillon), cote droit = gauche reflete
  decale de `mirror` ; `requires` (voler : os « aile ») ; groupe « Procedurales » toujours actif par-dessus ;
  pistes de taille (slime). Pour une zone : animation de son bloc, sinon de la bibliotheque, sinon repos ; vitesse
  de la race (golem 0,6). Repere : perso vers +z, membre en avant = rx negatif. Choix de la race : `race_view`
  anime (`Editor::shown`), options a cocher, choix de l'animation, camera decalee (`OrbitCam::shift`).
  Captures par race : `SPACESPORE_EDITOR_DEMO=race:<id>` (+ `SPACESPORE_RACE_ANIM`, `SPACESPORE_RACE_OPTIONS`).
  E3 = grandes grilles (jusqu'a 1024³) : `format::Chunk::Full(Arc<Vec<u8>>)` (copie a l'ecriture : copier un
  modele, garder un chunk pour annuler ou l'envoyer au maillage ne coute rien), `Sparse::data_mut/fold/put_chunk`.
  `Doc` : lots d'annulation par chunk (`Snap` avant / apres des voxels, zones, calques ; 768 Mo au plus),
  `dirty_chunks` (seuls ces chunks sont remailles), `revision`, calque courant `layer`, coupe `cut`.
  `Doc::edit_region` ecrit chunk par chunk (outils de volume `Shape` boite / sphere / cylindre / ligne et `Brush`
  ajouter (cases vides) / retirer / peindre, avec le miroir ; `flood` = pot de peinture ; `copy` / `clear` /
  `paste` / `transform_selection` = selection et `Clip`). Les cases cachees (calque masque, au-dela de la coupe)
  ne sont ni touchees ni visees (`raycast` prend la `mesh::Visibility`). Calques : `Model::layers` +
  `layer_map` (`layers.bin`). `mesh.rs` : maillage glouton d'un chunk (`ChunkJob` emporte le chunk et ses 26
  voisins), niveaux de detail 1 / 2 / 4 selon la distance ; `view::ChunkMeshes` : entites par chunk, les 6
  premiers chunks tout de suite, le reste hors du fil principal. Poids du fichier et nombre de blocs calcules en
  arriere-plan. Outils 5 a 0, molette pendant un trace = epaisseur, Ctrl+C/X/V, Suppr, R (Maj+R), C et Page
  prec./suiv. (coupe). Mesures (regle 8) : `cargo test --release bench_editor -- --ignored --nocapture` ;
  capture + images/s : `SPACESPORE_EDITOR_DEMO=croiseur` (+ `SPACESPORE_EDITOR_CUT`), ecrit `<capture>.txt`.
  Feuille de route : hangars des porte-vaisseaux (§5.1, E6 / E7, Q8).
  E6 = blocs de vaisseau (§5) dans `assets/editeur/blocs/` (generes par un script) : portes, rampe, verriere,
  ailes repliables / en X / a geometrie variable, train, propulseur et manoeuvre (flammes lumineuses), tuyere
  orientable, tourelle, radar, anneau, panneaux, bras minier, feux ; `PartDef::color/material` (couleur de depart).
  Pistes pilotees : `entree` (poussee / vitesse / manoeuvre -> rotation et taille, `flicker`), `clignote`,
  `vise` (`aim_angle` : lacet, tangage d'apres la hauteur ; « poussee » = tuyere a l'oppose) ; `motion::Inputs`
  (donnes par le jeu en E7, simules dans l'apercu : `Editor::inputs`, poussee reglable). Etats du vaisseau
  `SHIP_STATES` : `BlockDef::states` etat -> animation, apercu « etat:vol » (passage en 1,5 s : `blend_secs`) ;
  une animation du bloc sans piste pour une partie = immobile. Hangars (§5.1) : `BlockDef::hangar` (categorie,
  soute a cargos, porte, place, chemin), `format::Hangar` dans `Model::hangars` (meta.json, annulable),
  `edit::hangar_allowed` (Q8 : croiseur = chasseurs ; capital = chasseurs, corvettes, cargos jusqu'a la
  fregate), apercu `hangar:entree` / `hangar:sortie` (`HANGAR_SECS`, porte qui joue « ouverture »,
  `hangar_ship` = vaisseau fantome), chemin prolonge au clic (`extend_path`). Captures :
  `SPACESPORE_EDITOR_DEMO=vaisseau` (etat combat) ou `hangar`.
  E7 = les modeles en jeu (`models.rs`) : `ModelKey` (defaut `vaisseau:chasseur` / `perso:<race>` fabrique par
  `editeur::defaults`, ou empreinte d'un `.ssvox`), `GameModels` (charge et maille hors du fil principal, fichiers
  par empreinte, `saves/cache_modeles/`), `Rig` (enfant : une entite par zone et matiere, `RigPart`, anime par
  `zone_locals_with` + `Inputs`, `play` = passage en douceur), `Fit::Ship` (nez +z du modele vers -Z, longueur
  `icon_length` a l'echelle 1) / `Fit::Character` (hauteur du marcheur, ~2 blocs : decision du 03/10).
  Choix : `GameSettings::ship_model` / `character_model` (bouton « Utiliser comme mon vaisseau / personnage » ;
  le premier personnage enregistre est pris). Vaisseau : vraie taille pose et en vol bas (`surface::ShipDims`,
  4 voxels = 1 bloc), icone dans l'espace ; zoom de vol bas `Surface::flight_zoom` ; etat (pose, decollage,
  vol, combat via `combat::CombatState::last_shot`, detruit) et poussee pour les blocs (`models::drive_local`).
  A pied : F5 = 3e personne (par defaut), personnage anime (`walker_anim`), le marcheur ne traverse pas le
  vaisseau pose (`collision_boxes` par chunk, `push_out`). Reseau (`net_models.rs`, PROTOCOL 28) : `Looks` dans
  `PlayerState` (modeles, etat du vaisseau, marcheur `WalkState`, amarrage `DockState`), fichiers demandes par
  empreinte en morceaux de 16 Kio via l'hote (`Transfers`, `Msg::ModelWant/ModelPart`), autres joueurs : vaisseau
  avec leur modele (vraie taille pres de nous sur un astre), personnage a pied (`sync_remote_walkers`).
  Amarrage (`dock.rs`, touche H, dans l'espace) : hangar libre a notre taille d'un autre joueur, entree / amarre /
  sortie, portes animees chez tous (`carrier_sequence`). Tests : `SPACESPORE_TEST_LAND=<s>` (V), 
  `SPACESPORE_TEST_PEER=marcheur|croiseur|capital` (faux joueur), `SPACESPORE_TEST_DOCK=<s>`.
  E8 = mode avance : `vox.rs` (MagicaVoxel 150 : SIZE/XYZI, RGBA, MATL verre/metal/lumineux, graphe nTRN/nGRP/nSHP,
  morceaux de 256³, z du .vox = y ici ; import depuis `saves/import/`, export `saves/export/`). Animations du
  modele `format::ModelAnim` (`Model::anims`, cles de rotation par os, dans meta.json, prioritaires dans
  `zone_locals`), `Doc::set_pivot` / `edit_anims` (annulables), `custom.rs` (cles : `angles_at`, `set_key`,
  `remove_key` ; `block_json` = zone + filles -> bloc de mouvement dans `saves/editeur/blocs/`). Panneau « Mode
  avance » (zone choisie, pivot +- 0,5 ou au clic, animations, frise de 21 cases, angles +-15, cle, duree,
  lecture / pause : `Preview::paused`). Partage (`Msg::Share`, `Net::share_out`) : le modele arrive chez les
  autres par empreinte, dans `saves/modeles/partages/`. Capture : `SPACESPORE_EDITOR_DEMO=avance`.
- Correctifs apres 0.12 : nuages (`weather::cloud_field`) : chaque champ n'est pousse par le vent que depuis sa
  naissance (deux periodes de `MORPH_SECS` = 15 min) ; avant, le cisaillement depuis le debut de la partie
  faisait des bandes de Jupiter. `CLOUD_DRIFT` (x6) pour voir bouger les nuages, taille des nuages propre a chaque
  monde. Eclairs seulement sous un nuage d'orage (au-dessus du joueur et de l'impact). PROTOCOL 29.
  Embarquement (`surface.rs`) : `Phase::Boarding` / `Disembarking` : le cockpit (bloc verriere, rampe ou porte du
  modele : `cockpit`) s'ouvre (etat « pose »), le personnage saute dedans puis le vaisseau decolle ; a
  l'atterrissage il en descend. Les vaisseaux par defaut ont une verriere. `SPACESPORE_TEST_LAND=<s>,<s>` (V a
  plusieurs instants). L'editeur rend la souris (`Surface::release_cursor`).
  Cometes, ceintures, etoiles multiples : la lueur d'une comete s'efface quand la camera est dans sa chevelure
  (`Tails::near`) ; `asteroids::draw_trails` = trainees (gizmos) derriere cometes (5 % de la periode), gros
  asteroides proches et planetes des systemes charges (4 %). Bande des ceintures = cailloux icosaedres bosseles
  ombres par l'etoile (`band_mesh`, opaques) ; epaisseur des ceintures d'apres la largeur affichee (principale
  0,12 a 0,24 de la largeur, Kuiper plate). `StarOrbit::period` = vraie periode (paire serree 1 a 200 j =
  24 min a 80 h de jeu, compagnon lointain en siecles) ; paire serree a un quart de la premiere orbite permise.
  PROTOCOL 30.
- 0.11.4 correctifs (`roadmaps/fait/ROADMAP-0.12-correctifs.md`, une PR par phase C1..C8). C1 editeur : outil Ajouter =
  un clic un bloc, le trait reste sur le plan du premier bloc (`Editor::add_plane`, `view::plane_cell`) ;
  panneaux qui defilent jusqu'au bout du contenu des que la souris est dedans, barre `ScrollThumb` tirable ;
  `view::UiHover` (panneaux + boutons) ; lumiere d'atelier qui suit la camera + contre-jour (`follow_light`,
  touche L = lumiere du jeu) ; bibliotheque : « Modeles fournis » (`defaults::all_ids`, copie a enregistrer) ;
  `OrbitCam::focus` cadre le contenu (`Sparse::chunk_bounds`) ou toute la grille. Captures :
  `SPACESPORE_EDITOR_DEMO=vide:<categorie>` / `fourni:<k>` (+ `SPACESPORE_TEST_CMD=/editeur`).
  C2 animations : `RaceDef::anims` (variante par famille, groupe `motion::VARIANTS` jamais propose seul,
  `motion::anim_for`) et `RaceDef::alias` (os vise -> os de la race, `race_pose`) ; onde avec `base` (Euler :
  aile depliee, cape toujours derriere le dos). Cephalopode : saluer / dormir (pose au sol) / nager (tete devant) ;
  dragon et harpie : voler / planer propres ; mecha : `voler_reacteur`. Vol : battement autour de l'axe avant du
  monde vu du tronc penche (`flap_axis` du script). Cape : 4 segments jusqu'aux mollets, fente pour la queue et
  les jambes. Flotter : la tete oscille. Test `every_family_animation_moves_something`.
  C3 vaisseau : poussee = commandes (`Surface::pilot` / `pilot_turn` au sol et en vol bas, `ship::ShipThrust`
  du pilote automatique en croisiere) dans `models::drive_local` (tuyeres `Inputs::steer`, manoeuvre = virage
  demande ou mesure), jamais le deplacement monde. `main::fly_ship` + `surface::orient_ship` : nez vers la
  destination sans roulis, passage doux a la pose de stationnement a l'approche. Vent `weather::wind` : direction
  qui tourne (heures), force qui varie, rafales de quelques secondes (`Sample::gust`), turbulence d'orage ; plus
  fort en altitude, roulis / tangage (`Surface::tilt`) que le pilote corrige, vent au HUD (`compass`).
  C4 personnage : F5 = 1re personne -> de dos -> de face (`Surface::view`), molette = distance de la camera
  (3 a 12 voxels, 6 par defaut, `GameSettings::walker_cam`, `remember_walker_cam`) ; lampe tenue dans la main
  droite (`Rig::hand` = pivot de `main_d` a la pose de l'image), elle eclaire ou l'on regarde.
  C5 scanner : panneau en sections (`scanner::Section` : titre + lignes libelle / valeur en colonnes), « Ici et
  maintenant » en haut (`live_rows`, valeurs a largeur fixe, mises a jour a 1 Hz dans `LiveCell` sans
  reconstruire tant que les libelles ne changent pas). « Rotation et orbite » : jour et annee reels et en temps
  de jeu (`OrbitSection::day_game_s / year_game_s / orbit_game_s`, aussi dans `/profil`). Point de mesure de
  `LocalWeather` dans l'espace = point de stationnement (`Surface::hover_dir`), plus sous le vaisseau.
  C6 cercles : `main::clickables` = la seule regle « cliquable » (zoom permis, distances de clic, etoiles
  lointaines, trous de ver, galaxies...), utilisee par `select_world_target` et `draw_body_markers` (un cercle
  par astre cliquable, fondu 0,2 s, couleur par type `marker_color`, cible en jaune, 60 etoiles lointaines au
  plus, rien autour d'un astre deja grand a l'ecran ; groupe de gizmos par defaut, pas `IndicatorGizmos`).
  C7 chargement : `planet::system_to_load` = le systeme de la cible (`target_system`, trou de ver compris), charge
  tout de suite ou que soit le vaisseau, message « Systeme X charge » ; sans systeme (trou noir, galaxie) on garde
  celui ou l'on est tant qu'on y est. Plus de chargement au plus proche ni de recherche large. Le verrou du zoom 1
  (pas d'autre systeme) est dans `clickables` (plus de `lock_system_at_planet_zoom`, qui annulait `/aller`).
  C8 : comete qui tremble = `ship_collisions` (PostUpdate) repoussait le vaisseau stationne contre sa cible
  APRES le placement de la camera ; dans l'espace, plus de choc avec l'asteroide / la comete cible, et la camera
  suit toute poussee. Mesure : `SPACESPORE_COMET_LOG=1` (`asteroids::comet_log`, positions rendues par image) ;
  `SPACESPORE_TEST_CMD` accepte plusieurs commandes separees par « ; ». Trou noir central (`spawn_galactic_core`)
  cree au vrai centre de la galaxie, disque d'accretion = enfants du trou noir (avant : au point zero du monde =
  la ou l'on etait au lancement, visible dans le ciel au sol et decale du trou noir apres un recentrage) ; son
  eclat suit `GalaxyDim` (`dim_accretion_disk`).
  Portee : `Clickable::too_far` (meme regle pour le clic et les cercles : pas de cercle hors du cercle blanc,
  le clic dit « trop loin ») ; sauts entre galaxies a 5 tailles de galaxie (diametres) du centre au plus
  (`galaxy_jump_range`, sphere dessinee face a la camera en vue d'ensemble par `draw_travel_range`). Toutes les
  etoiles visibles a portee ont leur cercle (pas les cachees par l'eclaircissement) ; cercles de taille fixe dans
  l'espace (celui d'un trou noir galactique = son disque), 6 px pour un point.
- 0.13 (`roadmaps/fait/ROADMAP-0.13.md`) : E1 = etude d'echelle (`roadmaps/fait/RAPPORT-echelle-E1.md`) : `terrain::set_voxel_scale(k)` /
  `layout_scaled`, `/echelle k`, `SPACESPORE_SCALE=k` ; banc `cargo test --release bench_scale -- --ignored
  --nocapture --test-threads=1` ; mesures en jeu `SPACESPORE_PERF=<fichier>` (+ `_FROM` / `_TO` en s :
  images/s medianes, 1 % bas, > 33 ms), `SPACESPORE_TEST_WALK=1` (marche tout droit). Choix : k = 16.
  E2 = passage a l'echelle : `terrain::GROUND_SCALE = 16` (le voxel 16 fois plus petit, rayons en unites
  inchanges ; `/echelle k` reste pour les tests). `select_tiles(layout, ground_r, cam)` mesure au sol (sinon pas
  de tuiles fines sur un plateau). `Terrain::layer` tolere 0,01 voxel (precision f32). Grottes : toujours 2 000
  unites de profondeur (Q2) mais maillees seulement jusqu'a `NEAR_CAVE_VOXELS` (300) sous la surface + une tranche
  de +-`CAVE_WINDOW_VOXELS` autour du joueur sous terre (`Terrain::cave_window`, `caves::for_tile(windows)`,
  tuiles reconstruites quand la tranche change). Vol bas en voxels (`HOVER_VOXELS` 22, vitesse 1,5 x altitude
  entre 40 et 4 000 voxels/s, Maj x4), camera a quelques longueurs du vaisseau. Vol suborbital : `J` (`Hop`,
  point vise au centre de l'ecran, > 5 000 voxels, 6 a 25 s). Brume en voxels (`Surface::ground_scale`), plan
  proche 0,1 voxel au sol (`near_plane`), phares en voxels. Marcheur : sort de la roche (coins du cube, limite
  de colonnes) en fin de pas. Saves : `world.json` garde `ground_scale`, cellules d'une autre echelle oubliees.
  PROTOCOL 31.
  E3 = streaming : `update_tiles` pose les tuiles terminees dans un budget de `TILE_BUDGET_MS` (3 ms) par image,
  les grosses d'abord ; `max_tile_tasks` = 2 par coeur ; priorite devant la camera. Pas de cache disque : relire
  une tuile (3,0 ms, 375 Ko) coute plus que la generer (1,7 ms) (`bench_tile_cache`). Mesures : `TileStats`
  (tuiles a leur finesse, dans `SPACESPORE_PERF`), `SPACESPORE_TEST_FLY=1|climb` (vol bas plein gaz, en montant),
  `Surface::test_zoom_in` (comme un coup de molette).
- 0.13.1 dex des decouvertes (`dex.rs`) : chaque astre lu par le scanner (`scanner::update_scanner` remplit
  `dex::LastScan`) entre dans `Dex` (`dex.json` a cote de `world.json`) : sections du scanner, decouverte (date,
  horloge, joueur), visites, atterrissages (`count_landings`), note. Scanner : section « HISTORIQUE »
  (`Dex::history`, cache quand le dex est ouvert). Panneau : touche K ou bouton « Dex » (`DexUi`) : onglets
  `CATEGORIES`, recherche sans accents dans tout (`Dex::filtered`), listes qui defilent (`DexScroll`, blocs
  interieurs qui ne retrecissent pas), note editable (`Field::Dex` dans `net_ui` bloque les touches du jeu),
  Viser, Exporter (`export/dex-<joueur>-<date>.json`), Importer (`import/dex*.json`, meme graine du monde :
  `Dex::merge`). Test : `SPACESPORE_TEST_DEX=<s>` ouvre le dex.
- 0.13 T1 = relief en voxels (`planetgen/landforms.rs`, `Landforms::offset`, un seul code pour `terrain.rs`
  `raw_height_full` et `mesher.rs` `build_chunk_mesh`, regle 16) : deformation du domaine, collines (8 a 25 voxels,
  ~110 de large), massifs en cretes (120 a 400 voxels avec plaques, `ReliefSample::mountain` + massifs regionaux),
  bosses (2 a 6 voxels, pas vues de l'espace), vallees d'erosion (mondes a air et eau) ; masque des terres
  `land_mask` (un cinquieme du relief sous la mer). Remplace les anciens bruits `mid` / `fine`. Pentes :
  `cargo test --release slope_distribution -- --nocapture`. PROTOCOL 32.
- 0.13 T2 = formes 3D du relief (`rocks.rs`, cellules de 60 voxels hachees, `CellForms` : pieces + reperes
  `Feature`) sur tout astre solide : falaises (pente > 45 deg mesuree sur 3 points de la cellule) = corniche
  (`Piece::Add` dalle) avec la roche creusee dessous (surplomb), strates (rainures `Carve`), entree de grotte ;
  pitons (montagnes), chaos de blocs (pied des pentes, mondes nus) ; avec air : gorges etroites (`Carve`,
  noyees sous la mer si eau), ponts naturels (`Add` qui l'emporte sur `Carve`), arches (jusqu'a 40 voxels),
  cheminees de fee. `kind_in` evalue les pieces creusees des rocks meme sans grottes ; `has_3d` vrai avec
  rocks (collisions). Chat : `/relief [forme]` (`go_relief`), `SPACESPORE_TEST_CMD_SECS`. Tests
  `cliff_forms_are_real_voxels`, `relief_marks_point_at_real_forms`. PROTOCOL 33.
- 0.13 T3 = couleurs du sol (`terrain.rs`) : taches de 200 / 30 / 5 voxels (`value_noise`, seulement celles plus
  grandes que 3 fois le quantum de la tuile), `Column::raw` (hauteur avant arrondi) -> `tint_columns` (pente
  d'apres les 4 voisines, `ground_tint` : roche nue `rock_of` au-dela de 45 deg, sable des plages en pente douce
  seulement, neige jusqu'a ~60 deg, mousse au pied des parois de 3 voxels sur les mondes humides) ; parois sous la
  couche du dessus = strates (`strata_color`, bandes de 2 a 4 couches, ocre avec air). `mesher.rs` applique
  `ground_tint` avec sa pente (regle 16). Test `ground_colors_follow_slope_and_scale`. PROTOCOL inchange.
- 0.13 T4 = detail jusqu'a l'horizon : `GameSettings::terrain_detail` (0 Bas .. 3 Ultra par defaut, menu Options
  « Detail du sol », `graphics::TERRAIN_DETAIL` = facteur de decoupe 1,8 / 2,4 / 3,2 / 4,5 et niveaux de tuiles
  avec decor 1 / 2 / 2 / 3, `Terrain::with_decor_levels`) ; `select_tiles_with(split, relief)` ne decoupe pas une
  tuile cachee par l'horizon (`Terrain::relief_span`). Fondu : une tuile remplacee reste affichee
  `TILE_FADE_SECS` (0,35 s) avec un materiau transparent qui s'efface (`TileStore::fading`). Maillages des
  tuiles `RenderAssetUsages::RENDER_WORLD` (memoire en vol bas Ultra 3,2 Go -> 0,84 Go). Option « Ombres du
  relief » (`relief_shadows`, defaut non : ~40 % d'images/s) : tuiles ombrantes + 4e cascade jusqu'a 4 000
  voxels. Mesures : `SPACESPORE_PERF` avec `terrain_detail` / `relief_shadows` dans settings.json. Test
  `detail_reaches_the_horizon_but_not_beyond`.
- 0.13 T5 = geologie active (`geoactive.rs`), f(graine, horloge) : `BodyParams::geo` (`GeoActivity` : volcanisme,
  seismes, `cryo` = ocean sous la glace) ; events haches par cellule de 400 voxels (`cell_vent`) : lave
  (volcanisme > 0,45, eruptions de quelques heures puis refroidissement), geysers (eau liquide, 8 a 30 s toutes
  les 1,5 a 8 min), fumerolles (air), cryovolcans (panaches de 300 voxels) ; `strength(vent, t)`. Affichage :
  particules face camera (`GeoParticles`), coulee lumineuse sans eclairage le long de la plus grande pente
  (`lava_path`, `GeoLava`) + lumiere orange (`GeoLight`) ; maillages reecrits a chaque image =
  `NoFrustumCulling`. Seismes : `quake(seed, quakes, t)` (tranches de 4 min), camera qui tremble apres
  `SurfaceControl`, poussiere, message. Chat : `/geologie [geyser|fumerolle|cryovolcan|lave|seisme]` (evente
  actif le plus proche ; seisme = force, tests). Tests : `SPACESPORE_TEST_CMD2` (+ `_SECS`, 30 s) = seconde
  commande apres l'atterrissage. PROTOCOL inchange (rien de nouveau dans le monde partage).
- 0.13 bloc P (v0.13.4, `roadmaps/fait/ROADMAP-0.13.md`) = approche planetaire, un seul vol continu de l'orbite au sol :
  `surface.rs::Flying` (ceiling `ORBIT_CEILING` 4 rayons, vitesse ∝ altitude plafonnee a `ORBIT_SPEED_FRAC` du rayon / s,
  freinage de l'air `approche::DRAG_Q0`, vaisseau qui grossit avec l'altitude (`stretch`, jamais moins de 1/30 de
  l'altitude), camera `cam_dist` qui recule et se penche, entree par ZQSD depuis la vue espace a moins de
  `FLIGHT_ENTER_RADII` rayons, sortie en reculant la molette, `cam_blend` pour le fondu) ; V seulement sous
  `LAND_ALT_VOXELS` (200, Q9) et sur une pente <= `LAND_MAX_SLOPE` (25, Q10, `slope_deg` / `find_flat`) ; plus de descente
  automatique. Maillage lointain lisse au-dessus de 0,30 rayon d'altitude de la camera (`TileStore::far_mode`,
  `FAR_ABOVE` / `FAR_BELOW`). `approche.rs` = physique commune (`FlightInfo` rempli par `surface.rs::flight_info`,
  `air_density`, `sound_speed` (1 voxel ~ 1 m : 340 voxels/s), `heat` = densite x Mach^3 : lent = rien, rapide = flammes,
  Q11) ; `BodyParams::plasma` = couleur des flammes selon l'air. `approche_fx.rs` = effets : plasma / traine / cone de
  condensation / retrofusees / feux de position (enfants du `Ship`, `ShipFx`), ombre / poussiere / traines /
  onde du bang (enfants de la racine de l'astre, `BodyFx`), nuages qui s'ecartent (`CloudMaterial` = materiau standard +
  `cloud_clear.wgsl`, conversion des dalles de `planet.rs` par `convert_clouds`, sillage de 6 s), brouillard dans la
  dalle (`cloud_state`, `cloud_fog`), liseré de l'atmosphere (`RimMaterial`, `rim.wgsl`, coque a `relief + 0,35 x
  atmosphere_depth`), secousses, autres joueurs (`remote_fx`, `Looks::heat` / `mach`, PROTOCOL 35). `approche_ui.rs` =
  indicateurs (gizmos : trajectoire, point d'impact, portes du couloir, points plats), alertes, points d'atterrissage
  (scanner, touche L), bouclier thermique en option (`GameSettings::heat_shield`, `HeatDamage`), vitre du cockpit (F5).
  `sound.rs` = premier son du jeu, synthetise en WAV en memoire (feature bevy `wav`), boucles pilotees par la vitesse et la
  densite (`air_gain` : silence dans le vide), bang, tonnerre retarde, `/volume` (`GameSettings::sound_volume`). Tests :
  `/essai mer`, `/nuage`, `SPACESPORE_TEST_FLY=reentry` (voir `TESTS-JEU.md`), `cargo test --release approche sound`.
- 0.13 bloc C (`roadmaps/fait/ROADMAP-0.13.md`) : C1 brouillard = `fog.rs` : un `FogVolume` (banc plat aligne sur la
  verticale du lieu, pose sur le sol sous le joueur, texture de densite 3D, largeur = portee des ombres du soleil) + `VolumetricFog`
  sur la camera + `VolumetricLight` sur les `SurfaceSun` et les phares (les ombres sont forcees quand le banc existe : sans
  ombres le brouillard n'est pas eclaire et assombrit tout) ; densite = `fog_density(matin, temperature, matiere, vallee,
  humide)` ; `/brouillard`, `SPACESPORE_TEST_FOG` ; brouillard de grotte = `DistanceFog`. C2 skybox = `skybox.rs` : `render_cube`
  (pur, testable : tirages de `galaxy_shape::Shape` + etoiles reelles + galaxies voisines, 6 x 1 024^2, threads ; plus de bande de galaxie dessinee, premier ciel calcule avant la 1re image par `first_sky`) lance en tache
  de fond par `watch_system` quand le systeme le plus proche change, `SkyMaterial` + `sky_dome.wgsl` (sphere sans profondeur,
  `z = 0`) qui dessine aussi `ClearColor` (le jour efface les etoiles). Tests : `/meteo clair`, `bench_sky`.
- 0.13 bloc V (`roadmaps/fait/ROADMAP-0.13.md`) : `cinematic.rs` + `cinematic.wgsl` = sequences plein ecran (noeud d'interface
  `UiMaterial`, tout en shader) : `Dig` (foreuse a vise, dezoom, grille 2D puis 3D, espace qui tourne comme un trou noir, zoom, elle
  avance et le tunnel se forme ; `dig_params` = le scenario, 34 s), `Ride` (interieur d'un tunnel), `Galaxy` (saut entre galaxies,
  10 s, lance par `watch_jumps` quand le vaisseau saute de plus de 5 portees). Echap passe (`skip_with_escape`, `Last`).
  `tunnel.rs` (V4) : 4 foreuses = biens `FIRST_TUNNEL_GOOD..` (I 100 u, II 500, III 5000, clandestine 250 ; 1 u = 1 000 000), energie = 1 cellule
  de carburant par u, tunnels absolus f64 dans `tunnels.json` (1 a 3 voies 5 / 15 / 40 u/s, 3e a peage 3 cr/u, clandestin = 1 voie,
  ouvertures cachees), `WormholeTravel::external` bloque les commandes pendant creusement et vol. Chat `/tunnel`. Pas de partage
  multijoueur. Test : `SPACESPORE_TEST_CINE`.
- 0.13.6 : cinematiques sur le vrai ciel (`CineMaterial::sky` = cubemap de `skybox.rs`, `Cinematic::use_sky` + repere
  `bx/by/bz` : le creusement voit le ciel d'ici dans l'axe du tunnel ; le saut entre galaxies etire les vraies etoiles
  du depart puis, au flash (`GALAXY_FLASH`), la camera recule et s'approche de la vraie galaxie d'arrivee). Skybox
  visible a tous les zooms, option « Fond d'etoiles » (`show_skybox`). Zone d'influence des etoiles :
  `StarSystemConfig::influence` = 0,8 x distance a l'etoile voisine (`set_influence`, dans `dense` / `lazy`), limite
  `Stellar::outer_limit` (planetes, ceintures, cometes) ; PROTOCOL 36. Mode creatif (option, `settings::creative()` :
  pas de degats, soute et achats illimites). Mode photo `photo.rs` (F9 / F10 filtres `ColorGrading` / F11 PNG) : seuls les astres restent (`hide_for_photo` cache
  a chaque image interface, gizmos, vaisseau, joueurs, fumee, effets ; `SPACESPORE_TEST_PHOTO=<s>`).
  Fumee en cubes derriere le vaisseau (`smoke.rs`). Crashs gardes par session dans `crashes/` (30 derniers).
  Couleurs des cercles : lune blanc, planete orange, comete bleu, ceinture rouge clair. Etoiles lointaines : 13
  couleurs (`star_palette`), granulation de la meme teinte. Asteroides / cometes remailles plus fins de pres
  (`wanted_detail`). Le vaisseau est emporte par sa cible qui bouge (plus de tremblement). Tab complete `/tunnel` et
  les choix ecrits dans l'aide (`help_choices`). Test sans capture : `SPACESPORE_QUIT_SECS`.
- Correctifs 0.13.6 : saut entre galaxies a la portee de la plus grande des deux (`galaxy_reach` : le retour est
  toujours possible) ; deplacement et cercles des etoiles jusqu'a 2 500 M (`MAX_TRAVEL_RANGE`). Foreuse et saut entre
  galaxies sur le vrai rendu du jeu : la sequence est transparente sur le fond (rendu premultiplie dans
  `cinematic.wgsl`), `cinematic::steer_camera` (appele a la fin de `camera_controller`) oriente la camera du jeu comme
  la sequence (foreuse) ou lui fait vraiment traverser l'espace d'une galaxie a l'autre (saut, `galaxy_path`) ;
  interface, gizmos et vaisseau caches (`hide_during`). Finesse des asteroides / cometes d'apres leur taille a
  l'ecran (`wanted_detail`). Trainees sur une grille de temps fixe (`trail`) et vitesse des asteroides calculee
  sur l'orbite (plus de tremblement). Poussiere et trainees de condensation en cubes (`approche_fx.rs`). Scanner :
  « Temps de jeu » (`game_clock_text`) pour tous les astres, periode des etoiles doubles en vraie valeur + jeu.
  Skybox fixe : calculee une seule fois, toujours vue depuis le systeme 0 (`first_sky`), plus de recalcul quand
  on change de systeme ou de galaxie (plus de `watch_system`) : le fond est le meme partout.
- 0.13 V1 = trous noirs (`black_hole_fx.rs` + `black_hole.wgsl`, d'apres le modele `trou_noir` de Kerr) : chaque
  `GalacticCore` / `DistantGalaxyCore` recoit une sphere de lentille (`LENS_RADIUS` = 70 rayons de Schwarzschild,
  1 unite = `core_radius / 2,6`) dont le shader suit les geodesiques de Kerr (RK4, sortie interpolee sur le bord) :
  ombre sans horizon dessine, fond VRAI devie (texture de transmission de Bevy = image deja rendue, sinon skybox
  fixe), disque d'accretion (Doppler, decalage gravitationnel), jets, etoile aspiree en spirale (2 galaxies sur 3).
  Profondeur ecrite = celle du centre (derriere : cache et vu devie ; devant : reste devant). Plus d'ancienne
  sphere + anneaux. Stationnement a `CORE_HOVER_RADII` (9) rayons, hors du disque. Test : `SPACESPORE_TEST_ZOOM=
  <s>:<distance>[:<lacet>:<tangage>]` avec `/tp 0`. L'animation de saut ne part plus pour un saut dans la meme galaxie.
- 0.14.0 V3 = vue de la galaxie inclinee : `CameraController::frame` (repere de l'orbite de la camera) suit en douceur
  (`galaxy_frame`, 2,5 /s) le plan de la galaxie de la cible (`GalaxyConfig::tilt`, `current_galaxy`) aux zooms Galaxie,
  Cosmos et Espace profond, et revient au monde en dessous ; `look_at` avec le haut de ce repere. Test
  `galaxy_view_follows_the_galaxy_plane`.
- 0.14.1 skybox unique (`skybox.rs::render_cube`) : ne recopie plus les vraies etoiles ni les vraies galaxies (doublons
  avec les objets du jeu) ; fond propre a la graine du monde : nebuleuses de 3 couleurs tirees de `PALETTE` (bruit de
  gaz deforme, detail, veines de poussiere, demi-resolution lue en bilineaire), petites etoiles, brillantes a halo.
  ~24 % du ciel colore, 0,4 s (`bench_sky`).
- 0.14.2 bloc X / X0 = mondes exceptionnels (`planetgen/archetypes.rs`) : `Archetype` (numeros figes, ajout a la fin),
  `CATALOG` (rarete 0 naturel / 1-3 comme `traits::TIERS`, realisme, phase, mot de `/aller`, condition `allowed`),
  `roll` (couche `Layer::Archetype`, apres toute la chaine physique, dans `system.rs::generate`) et `apply` (modifie
  les couches par parametres). Zone calme : `StarSystemConfig::calm` = 50 systemes les plus proches du systeme 0
  (`CALM_SYSTEMS`, `dense()`), passee par `OrbitLimits::calm` ; les naturels (oeil) restent possibles partout.
  Effets : `Climate::eye` + `lat_of` / `sin_lat` (temperature selon l'angle au point sous l'etoile +X, partout ou
  l'on lisait la latitude : terrain, maillage lointain, biomes, `LocalWeather`), `BiomeParams::force` (terres dans
  deux biomes), `Relief::tier_step` / `cubic_step` + `landforms::sculpt_dir` / `sculpt_height` (meme code dans
  `terrain.rs::raw_height_full` et `mesher.rs`, regle 16), mer, nuages, aurores, anneaux, rotation. Scanner : section
  « ANOMALIE » ; `/stats` : « Mondes exceptionnels » ; `/aller planete <mot>` ; cercle magenta (`draw_body_markers`).
  Tests `archetypes::tests` (zone calme, frequence, oeil, etages). `bench_tiles` inchange (1,85 ms). PROTOCOL 37.
