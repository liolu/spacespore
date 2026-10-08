# SpaceSpore - Instructions Claude

Jeu spatial voxel (Rust, Bevy 0.15). Plateforme : Windows, PowerShell, clavier AZERTY. GitHub CLI (`gh`)
authentifie comme `liolu`. Le detail technique de chaque systeme est dans **`ARCHITECTURE.md`** (a lire seulement
pour le systeme sur lequel on travaille) ; les feuilles de route dans `roadmaps/` (sommaire `roadmaps/README.md`).

## Avant de coder

```bash
git fetch origin
git pull origin main
```
Si des changements locaux non commites existent : les mettre de cote avant le pull, puis les reappliquer.

## Fin de chaque reponse : push sur main

**A la fin de chaque reponse qui a modifie le depot, toujours pousser sur `main`** (sans attendre que l'utilisateur
dise « push main ») :
1. `git status` : verifier ce qui part (jamais `doc info/Nouveau dossier/api.txt`, jamais de fichier hors sujet) ;
2. si du code a change : `cargo build --release` et les tests concernes doivent passer, sinon ne pas pousser et le dire ;
3. commit, `git fetch` + rebase sur `origin/main`, `git push origin HEAD:main`.

Cette regle remplace l'ancienne facon de faire des feuilles de route (« PR non fusionnee, attendre push main »).
Ne rien pousser si rien n'a change.

## Versions

Le numero est uniquement `[workspace.package] version` dans `Cargo.toml` (jamais `tools/common/src/lib.rs`, jamais
de tag `v*` a la main ; si la version existe deja, la CI augmente le dernier chiffre toute seule).

| Numero | Exemple | Ou ca part |
|---|---|---|
| **Version corrective ou de phase** : le 3e chiffre monte | 0.13.5, 0.13.6, 0.18.1, 0.18.2 | **`main` seulement** = version **instable** (pre-release `unstable`, un build par push, proposee par le launcher en mode « Instable ») |
| **Nouvelle version** : le 2e chiffre monte, 3e chiffre a 0 | 0.14.0, 0.15.0 | **stable** : PR `main` -> `stable` ; le workflow `stable.yml` cree la release `vX.Y.0` avec zips + installeurs et met a jour `docs/version.json` |

- Chaque push sur `main` compile Windows / Linux / Mac (`unstable.yml`) et publie un build instable.
- Une release stable contient `-windows.zip`, `-linux.zip`, `-macos.zip` (le selecteur de version du launcher les
  liste par ces noms). Le launcher fait partie de chaque version et reste toujours le plus recent.
- Apres une release stable, le bot commit `docs/version.json` sur main : `git pull` avant de continuer.
- Tout deploiement est une version numerotee : pas d'upload de zip ni de binaire « a part ».

## Dependances : toujours la derniere version

- **Bevy** : le jeu est encore sur **0.15** ; la derniere version est **0.20** (verifie le 09/10/2026). Migration
  une version a la fois, chaque passage = version du jeu + 0.0.1 : bloc M de
  `roadmaps/a-faire/ROADMAP-0.17-debug-opti.md` (§8.1). Ne pas ajouter de code qui depend d'une API retiree dans les
  versions suivantes quand on peut l'eviter.
- **Avant d'ajouter ou de mettre a jour une dependance** : `cargo search <crate>` pour connaitre sa derniere
  version, et prendre celle-la. Si on ne peut pas (compatibilite avec Bevy, casse du monde), le dire et le noter.
- Ne jamais supposer la derniere version de memoire : toujours verifier.

## Build

- Toujours en release : `cargo build --release` (`run.bat` lance `target\release\spacespore.exe`).
- Tuer le jeu avant de recompiler : `Stop-Process -Name "spacespore" -Force`.
- Supprimer `saves/settings.json` ET `saves/astres.json` quand on change les defauts de type d'astre ou la graine.

## Tests en jeu

Avant tout test visuel (capture, shader, rendu) : lire `TESTS-JEU.md` et le suivre. Verifier le monde (eau
liquide, jour, pas synchrone-nuit) AVANT de capturer ; jamais de release sans capture qui montre la chose ; ne
jamais relancer 2 fois le meme test sans rien changer.

## Securite

- `doc info/Nouveau dossier/api.txt` contient des cles API en clair : NE JAMAIS COMMIT ce fichier.
- Toujours verifier `git status` avant de pousser.

## Regles qui touchent tout le code

- **Origine flottante** (`src/origin.rs`) : positions des entites en f32 relatives a une origine absolue f64 qui suit
  le vaisseau. Ne jamais garder une position « monde » en memoire : stocker l'absolu (`abs_center()`...) et convertir.
- **Determinisme** : tout le monde genere = f(graine, cellule, horloge) ; meme graine + meme version = meme monde.
  Sous-graines par couche figees (`planetgen/seeds.rs`), jamais renumerotees.
- **Changer la generation partagee = `PROTOCOL` + 1** (`net.rs`).
- **Regle 16** : ce qui se voit de l'espace et du sol vient de la meme fonction (`terrain.rs` et `mesher.rs`).
- Entites qui peuvent disparaitre dans la meme image (teleportation) : `try_insert`, jamais `insert`.
- Valeurs affichees arrondies (empreinte reseau). Conversions d'unites seulement dans `planetgen/units.rs`.
- Mesurer avant / apres pour le terrain et le rendu (`SPACESPORE_PERF`, bancs `cargo test --release bench_* --
  --ignored --nocapture`).
- Quand une phase est faite : ses notes techniques vont dans `ARCHITECTURE.md` (pas ici), sa feuille de route
  passe de `roadmaps/a-faire/` a `roadmaps/fait/` quand elle est terminee.

## Index des systemes (detail : `ARCHITECTURE.md`)

| Systeme | Fichiers |
|---|---|
| Univers, galaxies, systemes, echelles, LOD des etoiles | `settings.rs`, `systems.rs`, `planet.rs`, `galaxy_shape.rs`, `galaxy_fx.rs` |
| Generation des etoiles et planetes (chaine 0.10) | `planetgen/` (star, system, atmosphere, climate, hydrology, geology, landforms, biome, life, resources, habitability, traits, belts, comets, multiple) |
| Horloge, rotation, saisons, jour / nuit | `world_clock.rs`, `kepler.rs`, `surface.rs` |
| Terrain voxel, tuiles, grottes, formes 3D, deltas | `terrain.rs`, `caves.rs`, `rocks.rs`, `voxel.rs`, `mesher.rs`, `lod.rs` |
| Vol, atterrissage, marche, approche planetaire, survie | `surface.rs`, `approche*.rs`, `suit.rs` |
| Eau, meteo, brouillard, ciel, phenomenes | `water.rs`, `weather.rs`, `fog.rs`, `skybox.rs`, `sky.rs`, `gas.rs` |
| Geologie active, meteores, decor | `geoactive.rs`, `meteors.rs`, `decor.rs` |
| Asteroides, cometes, anneaux | `asteroids.rs`, `rings.rs` |
| Trous noirs, cinematiques, tunnels | `black_hole_fx.rs`, `cinematic.rs`, `tunnel.rs` |
| Editeur de modeles, modeles en jeu, amarrage | `editeur/`, `models.rs`, `net_models.rs`, `dock.rs` |
| Interface, scanner, dex, chat, stats, photo | `ui.rs`, `scanner.rs`, `dex.rs`, `chat_cmd.rs`, `test_cmd.rs`, `stats.rs`, `photo.rs` |
| Reseau, guildes, combat, economie, PNJ | `net.rs`, `net_ui.rs`, `guild*.rs`, `claims.rs`, `combat.rs`, `economy.rs`, `npc_ui.rs` |
| Son | `sound.rs` |
| Outils (launcher, installeur, mise a jour) | `tools/` (workspace Cargo) |

Commandes de test et variables `SPACESPORE_*` : `TESTS-JEU.md` et `ARCHITECTURE.md`. Touches du jeu : `TOUCHES.md`.
