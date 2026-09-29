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
