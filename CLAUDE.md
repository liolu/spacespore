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
