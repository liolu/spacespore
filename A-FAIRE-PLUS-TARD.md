# À faire plus tard

> **04/10/2026 : tout le bloc D est repris dans `ROADMAP-0.14.md`** (D1 à D6, mêmes noms). Ce fichier reste pour l'historique.


Ce qui reste du bloc D de la 0.11 (`ROADMAP-0.11.md`), mis de côté le 03/10/2026 après la release
v0.11.2 (blocs A, B et C terminés). On y reviendra après la 0.12 (éditeur).

Ordre : **D1 → D2 → D3 → D4 → D5 → D6**.

| Phase | Contenu | Taille |
|---|---|---|
| **D1. Eau vivante** | Rivières (écoulement calculé grossièrement depuis le relief), lacs, **cascades**, glace qui fond / gèle selon la saison, banquise, vagues selon le vent. | L |
| **D2. Géologie active** | Coulées de lave lumineuses (visibles la nuit), **geysers**, **cryovolcans** (type Encelade), fumerolles, petits **séismes** (secousse) selon l'activité. | M |
| **D3. Faune visible** | Créatures procédurales depuis `Fauna` (corps, pattes, ailes, nageoires), troupeaux, volants, aquatiques, **diurnes / nocturnes** (lien jour/nuit), fuite devant le joueur. Pas de combat. | L |
| **D4. Végétation vivante** | Arbres et herbe qui bougent avec le vent, plantes qui s'ouvrent le jour, bioluminescence la nuit, feuillage selon la saison. | M |
| **D5. Son** | Vent, pluie, tonnerre, écho des grottes, lave, faune, silence dans le vide. | M |
| **D6. Points d'intérêt** | Lieux rares à trouver (scanner) : grottes géantes, arches, cratères géants, sources chaudes, épaves, ruines (fictif, étiqueté), marqueurs posés par les joueurs. | M |

## Ce qui existe déjà et servira

- **D1** : niveau de la mer et marées (`terrain::Tide`, C4), saisons (`Climate::season`, A3), vents (`weather.rs`, C5), mers et glaces (`planetgen/hydrology.rs`).
- **D2** : volcanisme et séismes de la géologie (`planetgen/geology.rs`), coulées figées (B3), lave (`VoxelType::Lava`), orages des étoiles (`sky.rs`).
- **D3** : `Life` et la faune décrite par `planetgen/life.rs` (phase 7), jour / nuit (A2), météo (C5).
- **D4** : décor voxel des tuiles (`decor.rs`), vent (`weather.rs`), saisons et givre (A3), lumière de nuit (A2).
- **D5** : rien encore (pas de son dans le jeu) : il faudra choisir une caisse audio (Bevy `bevy_audio`).
- **D6** : scanner (`scanner.rs`), grottes (`caves.rs`), arches (`rocks.rs`), cratères (B4).

## Prompts (à coller dans une nouvelle session)

Contexte commun : « Lis `ROADMAP-0.11.md`, `ROADMAP-0.10.md`, `A-FAIRE-PLUS-TARD.md` et `CLAUDE.md`. Règles 1 à 13.
`git pull origin main` avant de coder. Tests. PR non fusionnée (je dirai « push main »). Ne change ni les tailles
ni les décisions sans me demander. »

- **D1** — « [contexte commun] Phase D1. Rivières calculées depuis le relief, lacs, cascades, glace qui fond et
  gèle selon la saison, banquise, vagues selon le vent. »
- **D2** — « [contexte commun] Phase D2. Coulées de lave lumineuses, geysers, cryovolcans, fumerolles, petits
  séismes selon l'activité géologique. »
- **D3** — « [contexte commun] Phase D3. Faune visible depuis `Fauna` : corps, pattes, ailes, nageoires,
  troupeaux, volants, aquatiques, diurnes / nocturnes, fuite devant le joueur, sans combat. »
- **D4** — « [contexte commun] Phase D4. Végétation qui bouge avec le vent, plantes qui s'ouvrent le jour,
  bioluminescence la nuit, feuillage selon la saison. »
- **D5** — « [contexte commun] Phase D5. Sons : vent, pluie, tonnerre, écho des grottes, lave, faune, silence
  dans le vide. »
- **D6** — « [contexte commun] Phase D6. Points d'intérêt rares repérés au scanner : grottes géantes, arches,
  cratères géants, sources chaudes, épaves, ruines (fictif), marqueurs des joueurs. »

## Après le bloc D

La 0.15 (minage et destruction, avant prévue en 0.14) : creuser / poser des voxels avec les deltas de B1, ressources de la
phase 8, astéroïdes minables (C1), destruction de planètes (débris → anneaux, C2).

## Musique (noté le 04/10/2026)

Pas de musique dans la 0.14 : le son (D5) ne joue que les sons du monde. La **musique d'ambiance** (selon
le lieu : espace, planète, grottes, combat) viendra dans une version plus tard.
