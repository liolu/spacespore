# SpaceSpore - Instructions Claude

## Auto-update obligatoire

Avant toute modification de code, toujours executer :
```bash
git fetch origin
git pull origin main
```
Si des changements locaux non commites existent, les stash avant le pull puis les reappliquer apres.

## Build

- Toujours build en release : `cargo build --release`
- `run.bat` utilise `target\release\spacespore.exe`
- Tuer le process avant rebuild : `Stop-Process -Name "spacespore" -Force`
- Supprimer `saves/settings.json` ET `saves/astres.json` quand on change les defaults de body type ou le seed

## Securite

- `doc info/Nouveau dossier/api.txt` contient des cles API en clair — NE JAMAIS COMMIT ce fichier
- Toujours verifier `git status` avant push

## Structure

- Cargo workspace : root = jeu, `tools/` = common/installer/updater/launcher
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
  qu'une partie : `economy::sold_by`). Minage (0.14) : `BodyDelta::ores` = tonnes extraites. Scanner, profil, `/stats`.
  Phase 7 : vie independante de l'habitabilite (`planetgen/life.rs`, sans plantes les biomes verts restent nus),
  decor voxel des tuiles proches (`decor.rs` : `tile_decor` calcule avec la tuile, enfants de la tuile, maillages
  et materiaux partages). Banc : `cargo test --release bench_decor -- --ignored --nocapture`. L'etoile a un type (`planetgen/star.rs`, O..M, naine blanche/brune, sous-geante, geante rouge) :
  une G garde l'echelle, les autres types ont leur taille reelle (1 R_sol ~ 1 050 000 a l'ecran), compressee
  au-dela de 1,5 M (max 6,5 M) ; les planetes d'une geante sont repoussees hors d'elle. Aucun type impose (Sol compris). Planetes et lunes
  sont explorables : zoomer sous 1000 du vaisseau = navigation basse altitude (ZQSD, Maj, Espace/Ctrl, clic
  droit, molette), `V` = atterrir (sortir du vaisseau) puis marcher, `V` = redecoller. Le dessous du vaisseau reste
  parallele a la surface. `src/terrain.rs` = terrain voxel (champ de hauteur, quadtree de tuiles),
  `src/surface.rs` = vol, atterrissage, marche, lumiere. Saves : `saves/vX.Y.Z/` (settings, world, info).
- Generation 0.10 (`ROADMAP-0.10.md`) : module `src/planetgen/`. Les planetes et lunes ne sont plus stockees :
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
- 0.11 (`ROADMAP-0.11.md`) : A1 = horloge du monde `world_clock.rs` (`WorldClock`, secondes de jeu f64, `clock` de
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
  0.14). Banc : `cargo test --release bench_voxel_tiles -- --ignored --nocapture`. PROTOCOL 18.
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
- 0.12 (`ROADMAP-0.12-editeur.md`) : editeur de modeles voxel, module `src/editeur/`. E0 = fondations : etat
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
- Plateforme : Windows, PowerShell, clavier AZERTY
- GitHub CLI (`gh`) installe et authentifie comme `liolu`

## Branches et versions

- `main` = version **instable**. On travaille toujours ici. Chaque push sur
  main compile automatiquement Windows/Linux/Mac (workflow `unstable.yml`)
  et publie la pre-release `unstable` : le launcher la propose en mode "Instable".
- `stable` = version **stable**. Pour publier : fusionner `main` dans `stable`
  (PR main -> stable). Le workflow `stable.yml` cree la release `vX.Y.Z`
  avec zips + installeurs et met a jour `docs/version.json`.
- Numero de version : uniquement `[workspace.package] version` dans
  `Cargo.toml`. Ne pas modifier `tools/common/src/lib.rs` pour ca. Si la
  version existe deja, la CI augmente le dernier chiffre toute seule.
- Ne plus creer de tags `v*` a la main.
- Apres une release stable, le bot commit `docs/version.json` sur main :
  faire `git pull` avant de continuer.

## Deploiements = toujours une version

Tout deploiement est une version numerotee, jamais un fichier ou un binaire
"a part" :
- Instable (push sur `main`) : pre-release `unstable` = `vX.Y.Z` + numero de
  **build** (`unstable.json`, champ `build`). Chaque push = un nouveau build.
- Stable (PR `main` -> `stable`) : release `vX.Y.Z` (ex. v0.1.0, v0.1.1, v0.2.0),
  avec zips + installeurs, et `docs/version.json` mis a jour.
- Le numero vient seulement de `[workspace.package] version` dans `Cargo.toml`
  (la CI augmente le dernier chiffre si la version existe deja). On ne
  deploie rien a la main : pas d'upload de zip, pas de tag `v*` manuel.
- Une release stable doit contenir les zips `-windows.zip`, `-linux.zip`,
  `-macos.zip` : le selecteur de version du launcher les liste par ces noms.
- Le launcher fait partie de chaque version et doit toujours etre le plus
  recent : un retour a une ancienne version du jeu ne remplace pas le launcher.
