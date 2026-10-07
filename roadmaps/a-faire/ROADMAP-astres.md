# Feuille de route — K. Astres et espace

Source : bloc **K** de `roadmaps/a-faire/RAPPORT-ameliorations.md` (idées 141 à 155) + idées 12, 73, 74, 144 à 155.
Les phases s'appellent **AST-n**.

Objectif : que l'espace entre les mondes soit aussi riche que leur surface : des astres qui **évoluent**, des
phénomènes **rares et reconnaissables**, des dangers et des ressources, tout en restant **déterministe**
(`f(graine, horloge)`, règles 12 / 20 de la 0.14) et sans toucher à ce qui existe (les nouveaux tirages sont
**ajoutés après**, comme `NUM_OUTER_GALAXIES`).

---

## 1. Point de départ (code du 05/10/2026)

- **Étoiles** (`planetgen/star.rs`) : types O à M, naine blanche / brune, sous-géante, géante rouge. Compagnons
  (doubles / triples, `multiple.rs`). Orages magnétiques (`sky.rs::storm`). Pas de variabilité, pas d'évolution.
- **Modules hérités `src/astre/`** (~10 700 lignes, `#![allow(dead_code)]`, **plugins désactivés** « la galaxie gère
  tout ») : `Remnant_stellaire/` (trou noir 835 l., **étoile à neutrons** 1 311, **pulsar** 1 127, **magnétar** 960,
  **supernova** 715, **nébuleuse** 583), `planete/` (géante gazeuse, comète, météoroïde), `etoile/` (protoétoile,
  naine, séquence principale, géante, supergéante, hypergéante). C'est du code **déjà écrit** de rendu voxel / effets,
  à réactiver et brancher sur la vraie génération au lieu de repartir de zéro.
- **Galaxies** (`galaxy_shape.rs`, `galaxy_fx.rs`) : bras, trou noir central, disque, nuages (`stream_clouds`),
  10 000 galaxies, LOD par secteurs. Pas d'amas, pas de collision de galaxies.
- **Petits corps** : ceintures, Troyens, anneaux, comètes (`asteroids.rs`, `belts.rs`, `comets.rs`), planète errante
  (1 système sur 30).
- Le trou noir central existe (`spawn_galactic_core`) ; l'effet de lentille est prévu en 0.13 V1.
- Nommage : `Remnant_stellaire` avec majuscule (Linux sensible à la casse : voir `roadmaps/a-faire/ROADMAP-technique.md` TECH-7).

## 2. Règles

1. **Tout est f(graine, horloge)** : aucun état caché ; deux joueurs voient le même astre au même instant.
2. **Rien de connu ne change** : un astre déjà généré garde ses valeurs ; les nouveautés sont de **nouveaux tirages**
   (sous-graines nouvelles dans `seeds.rs`, numéros figés) ou des **états** qui dépendent de l'horloge.
3. **Rareté d'abord** : chaque phénomène a un pourcentage fixé, noté dans `/stats` (`stats.rs`) et dans le scanner.
4. **Réalisme avec une version fictive étiquetée** (`Realism::Fictional`), comme les minerais.
5. **Un astre = un profil** (`StarProfile` / `PlanetProfile`) lisible par le scanner, `/profil`, le dex, `/aller <type>`.
6. **LOD** : étoile lointaine = point ; l'effet détaillé n'existe que dans le système chargé (règle d'économie).
7. Toute nouvelle couche = test d'empreinte (voir `roadmaps/a-faire/ROADMAP-technique.md` TECH-2) et `PROTOCOL` incrémenté si le
   réseau en dépend.
8. Pas de modèle « événement mondial » qui change le terrain sans passer par les **deltas** (règle 7).

## 3. Les phases

| Phase | Contenu | Idées | Taille |
|---|---|---|---|
| **AST-0. Réveil des modules hérités** | Inventorier `src/astre/` : quoi est réutilisable (matériaux, maillages, shaders, paramètres) ; renommer `Remnant_stellaire` → `remnant_stellaire` ; retirer `allow(dead_code)` module par module en branchant ou supprimant ; **un banc** (`/aller etoile pulsar` etc.) pour voir chaque objet isolé ; documenter ce qui sert (étoile à neutrons, pulsar, magnétar, supernova, nébuleuse). Décision par module : réutiliser / réécrire / supprimer. | 12 | M |
| **AST-1. Objets compacts** | **Étoile à neutrons, pulsar, magnétar** comme types d'étoile **rares** (`planetgen/star.rs` : `StarKind::Compact`), avec leur système (planètes de pulsar très rares, planètes de type « survivantes »), rayonnement fort (radiation du scanner, A4), **faisceaux** du pulsar qui balaient (période = vraie rotation, effet visible à distance), champ magnétique du magnétar (aurores extrêmes, pannes du N3 de la 0.14), disque de débris. Nouveaux types dans `/aller etoile <type>`, `/stats`, scanner, dex. | 12, 145 | L |
| **AST-2. Étoiles variables et évolution** | **Variables** : Céphéides (luminosité en dents de scie, période = f(luminosité)), étoiles à flares, binaires à éclipses (vue depuis un compagnon), novæ récurrentes (flash tous les N jours de jeu) ; **évolution** : âge de l'étoile (déjà dans le profil ?) → stade (séquence, sous-géante, géante rouge, naine blanche) avec la zone habitable qui **avance** ; l'étoile s'éclaire de 1 % par « milliard d'années » (jamais vu en jeu, affiché au scanner). La lumière variable passe par `lumens` / `SunDim` (déjà là pour les éclipses). | 142, 73 | L |
| **AST-3. Supernovæ et restes** | **Supernova** : événement très rare (cf. N3 de la 0.14), 1 étoile massive sur N, état f(horloge) : précurseur → explosion (quelques minutes de jeu, lumière aveuglante, onde de souffle qui grandit) → **rémanent** (nébuleuse en expansion, étoile à neutrons ou trou noir au centre) ; planètes du système stérilisées / détruites **par les deltas** (monde mort X8) ; `src/astre/Remnant_stellaire/supernova.rs` + `nebula.rs` comme base. Restes **anciens** (stables) beaucoup plus communs que l'explosion elle-même. Kilonova et magnétar-éruption en phase suivante. | 144, 145, 151 | L |
| **AST-4. Nébuleuses et nuages à traverser** | Nébuleuses (émission, réflexion, planétaire autour d'une naine blanche, restes de supernova) comme **volumes** : couleur, taille, **pouponnières** avec étoiles jeunes (protoétoiles, `protostar.rs`) ; effets en les traversant : visibilité réduite, vitesse limitée (traînée), scan brouillé, **courants** qui poussent, rayonnement ; passage visible depuis le sol (ciel nocturne coloré, Voie lactée plus riche). `nebula.rs` comme source, intégré à `galaxy_fx::stream_clouds` (qui existe déjà pour les nuages galactiques). | 147, 143 | L |
| **AST-5. Systèmes jeunes et exotiques** | **Disques protoplanétaires** (systèmes très jeunes : disque de gaz et de poussière, planètes en cours d'accrétion, impacts fréquents) ; **planètes jumelles** (binaire planétaire : deux corps qui orbitent l'un autour de l'autre, marées entre eux) ; **résonances de lunes** (141 : éclipses croisées, chauffage de marée, déjà `tidal_heating`) ; **lunes habitables** de géantes gazeuses (déjà possibles par `world_layers` : s'assurer que l'habitabilité les compte, les mettre en avant au scanner) ; **étoiles en fuite** et **planètes errantes** en grappe. | 143, 141, 152, 153, 149 | M |
| **AST-6. Petits corps vivants** | **Comètes à retour** annoncées au scanner (date du prochain passage, `next_eclipse` s'en inspire), **astéroïdes à eau** (glace + minerais, lié phase 8), **objets interstellaires** (type ʻOumuamua) : un passage unique très rapide, trajectoire hyperbolique, repérable sur un calendrier ; **champs de débris** de collisions anciennes ; trajectoires de **menace d'impact** sur une planète (lie avec N2 de la 0.14). | 154, 155 | M |
| **AST-7. Galaxies : amas, collisions, quasars** | **Amas globulaires** (grappes d'étoiles très vieilles autour des galaxies) ; **collisions et fusions** de galaxies lointaines (ponts de marée, formes déformées dans `galaxy_shape.rs`) ; **galaxies à noyau actif / quasars** visibles de très loin, jets ; **trous noirs supermassifs** avec la lentille de la 0.13 V1 ; types de galaxies (elliptique, irrégulière, naine) selon l'environnement. Tout est pour les 9 979 galaxies **lointaines** (jamais de changement pour les 21 premières). | 146, 150, 145 | L |
| **AST-8. Dangers de l'espace** | **Vents stellaires** qui poussent le vaisseau près des étoiles massives, **rayonnement** (zone dangereuse autour des pulsars, magnétars, supernovæ) avec compteur au HUD, **rayons gamma** (événement très rare qui stérilise un monde, 151), **orages solaires** plus riches (lien C4 de la 0.11), bulles de vent, tempêtes magnétiques à plusieurs jours. Toujours avec **alerte et possibilité de fuite** (pas de mort instantanée, règle Q8 de la 0.13). | 148, 151 | M |
| **AST-9. Ciel vu du sol** | **Ciel nocturne propre au point de vue** : constellations différentes selon le système, **planètes visibles à l'œil nu** avec leur couleur et leur phase, lune qui change de taille selon l'orbite (déjà `MoonPhases`), **lumière zodiacale**, Voie lactée plus fine (lien 0.13 C2 skybox), nébuleuses et amas visibles au télescope (INS), objets variables qui clignotent. | 74 | M |
| **AST-10. Scanner, dex, carte** | Fiches de chaque nouveau type (nom, rareté, danger, intérêt) ; **filtres de carte** par type d'astre ; `/stats` mis à jour (comptes et pourcentages) ; `/aller <type>` pour tous les nouveaux types ; entrées de dex « découverte rare » ; **objets du jour** (calendrier : prochaine nova, comète, éclipse). | — | M |

Ordre conseillé : **AST-0 → 1 → 2** (les objets les plus visibles), puis **3, 4**, puis **5, 6**, puis **7, 8**,
**9** après la skybox de la 0.13, **10** au fil de l'eau.

## 4. Détails importants

### 4.1 Types d'étoiles : où mettre les nouveaux
`StarKind` est étendu par un **second tirage** après le tirage actuel : `Compact` (étoile à neutrons, pulsar,
magnétar) ~0,3 % des systèmes, jamais dans les 50 plus proches du départ (comme Q4 de la 0.14). Le reste garde
exactement ses valeurs (règle 2). Pulsar : période de 1 ms à 5 s, faisceau = cône tournant (effet de phare dans le
ciel du joueur) ; magnétar : champ 10¹⁴ G, éruptions rares.

### 4.2 Variabilité sans coût
La luminosité variable est une **fonction pure** `luminosity(t)` calculée par frame pour les étoiles proches et par
tranche de 15 min pour les lointaines (comme `storm` par tranches). Le scanner affiche période et amplitude.

### 4.3 Supernova : le seul vrai « événement mondial »
Dépend de l'horloge, **donc** rejouable. Le monde d'un système sans événement ne change pas. Si la supernova a lieu,
les planètes passent à un état « irradiées » (surface calcinée, atmosphère arrachée) par **deltas** ; le système est
marqué dans le dex.

### 4.4 Ce qui reste de `src/astre/`
Chaque module hérité est classé : **réutilisé** (rendu), **converti** (paramètres → profils), **supprimé** (doublon
avec la génération actuelle). Objectif : plus de `#![allow(dead_code)]` global à la fin d'AST-0.

## 5. Mesures et tests

- Chaque type : `/aller etoile <type>`, capture de jour et de nuit, scanner lisible ; **un test de rareté** :
  générer 100 000 systèmes, vérifier que les pourcentages sont dans ±10 % de la cible.
- Pas de régression de performance : banc `bench_*` des étoiles lointaines (`StarSectors`) avant / après.
- Déterminisme : même graine, même horloge → même luminosité sur 2 clients.
- Les effets de volume (nébuleuses) mesurés en images/s (règle 8).

## 6. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Rareté des objets compacts ? | 0,3 % (étoiles à neutrons / pulsars / magnétars réunis), noté dans `/stats`. |
| Q2 | Supernova : visible en direct ou seulement ses restes ? | Les deux ; l'explosion est **très** rare (1 système sur 100 000) mais réelle. |
| Q3 | Les planètes d'une étoile à neutrons sont-elles habitables ? | Non (radiation), mais jouables et riches en minerais (fictif étiqueté). |
| Q4 | Évolution stellaire : réelle (milliards d'années) ou accélérée ? | Affichée au scanner mais **pas animée** ; seule la variabilité courte l'est. |
| Q5 | Réutiliser `src/astre/` ou réécrire ? | Réutiliser le rendu, remplacer la génération par les profils (AST-0 tranche). |
| Q6 | Collisions de galaxies : seulement lointaines ? | Oui, les 21 premières ne changent pas. |

## 7. Prompts

**AST-0** : « Lis `CLAUDE.md` et `roadmaps/a-faire/ROADMAP-astres.md` §3 AST-0. Inventorie `src/astre/`, renomme
`Remnant_stellaire`, ajoute les commandes `/aller etoile <type>` pour chaque objet, décide réutiliser / convertir /
supprimer par module, retire `allow(dead_code)` sur ce qui reste. PR avec le tableau des décisions. »

**AST-1** : « Ajoute les objets compacts (étoile à neutrons, pulsar, magnétar) comme types d'étoile rares : profils,
tirage hors des 50 premiers systèmes, faisceaux, rayonnement, scanner, `/stats`, dex. Capture de chaque type. »

**AST-2 à AST-10** : « Lis `roadmaps/a-faire/ROADMAP-astres.md` §3 AST-n, implémente, teste (rareté, déterminisme, capture),
`PROTOCOL` si nécessaire, PR non fusionnée. »
