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
- Proportions d'un systeme (generees depuis la graine du monde, `settings.rs`) : etoile 90 000 a 160 000 de
  rayon, planete <= etoile/100, lune <= planete/3, premiere orbite a 2,4 rayons d'etoile. Planetes et lunes
  sont explorables : zoomer sous 1000 du vaisseau = navigation basse altitude (ZQSD, Maj, Espace/Ctrl, clic
  droit, molette), `Entree` = atterrir puis marcher, `Entree` = redecoller. Le dessous du vaisseau reste
  parallele a la surface. `src/terrain.rs` = terrain voxel (champ de hauteur, quadtree de tuiles),
  `src/surface.rs` = vol, atterrissage, marche, lumiere. Saves : `saves/vX.Y.Z/` (settings, world, info).
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
