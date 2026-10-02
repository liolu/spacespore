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
- Echelle : `GALAXY_SCALE` (settings.rs, les distances entre etoiles/galaxies x300 depuis la phase 2) multiplie les distances entre etoiles/galaxies
  (rayon 2,7 G ; voisine la plus proche a ~15 M en mediane dans la galaxie principale, ~4,5 M ailleurs) ; les
  systemes gardent leur taille (~4 M en mediane, 11 M pour 99 %). Les etoiles lointaines sont groupees en
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
