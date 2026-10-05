# Feuille de route — N. Instruments (scanner, outils de mesure, navigation)

Source : bloc **N** de `RAPPORT-ameliorations.md` (idées 185 à 194) + idées 77 à 80, 82, 190 à 193.
Les phases s'appellent **INS-n**.

Objectif : faire du **scanner** (aujourd'hui un panneau qui lit tout) un **ensemble d'instruments** que le joueur
choisit, améliore et utilise avec adresse : viser, mesurer, écouter, comparer, cartographier, poser des balises.
La science du jeu (atmosphère, géologie, ressources, vie) devient un **gameplay d'observation**.

---

## 1. Point de départ (code du 05/10/2026)

- **Scanner** (`scanner.rs`, 587 lignes) : panneau de l'astre **ciblé** (touche I), sections (`Section` : titre +
  lignes libellé / valeur) pour étoiles (`star_sections`), planètes et lunes (`body_sections`), « Rotation et orbite »
  (`rotation_section`), « Ici et maintenant » (`LiveCell`, 1 Hz). Remplit `dex::LastScan`.
- **Dex** (`dex.rs`, K) : chaque astre lu → `Dex` (`dex.json`), découverte (date, horloge, joueur), visites,
  atterrissages, note, historique ; recherche, export / import.
- **Mesures disponibles dans le code** : `LocalWeather` (heure, saison, température, min / max), `suit::Environment`
  (pression, O₂, CO₂, radiation, lave), `Climate`, `weather::sample`, `geoactive`, `meteors`, `sky::MoonPhases`,
  grottes (`/grotte`, scanner : grotte la plus proche), minerais (`resources.rs`), zones (`zones.rs`), stats
  (`stats.rs`), `/profil` (JSON de l'astre).
- **Instruments de vol** : boussole / vent au HUD (`compass`), altitude, cible (cercles `clickables`), cartes
  (vue galaxie), `draw_travel_range`.
- Pas encore : **niveaux de scan** (tout est lu tout de suite), instruments à main, mesures sur le terrain,
  balises, carnet, cartographie au sol, télescope, sondes.

## 2. Règles

1. **La vérité vient des profils** (`StarProfile` / `PlanetProfile`) : un instrument **révèle** une valeur, il ne la
   calcule jamais à part (sinon désaccord avec le scanner et le dex).
2. **Un instrument = un module** (`instruments/<nom>.rs`) qui déclare : ce qu'il mesure, sa portée, son coût
   (énergie / temps), son bruit, et comment il apparaît à l'écran et dans le dex.
3. **Précision plutôt qu'accès** : jusqu'à maintenant tout est affiché ; désormais certaines valeurs sont
   **inconnues** tant qu'on ne les a pas mesurées, puis **connues** (dex) ; jamais de perte d'information déjà
   acquise.
4. **Pas de pénalité de découverte** : pas de blocage de progression derrière un instrument rare ; ils
   accélèrent ou précisent, ils ne verrouillent pas (option « tout révélé » pour les joueurs qui préfèrent).
5. **Même résultat pour tous les joueurs** au même instant (déterminisme, mesures bruitées par une graine fixe).
6. **Interface** : chaque instrument a **une touche, une invite, une icône** (CTL-5) et est utilisable à la manette.
7. **Multijoueur** : balises, notes et cartes partagées par message **dans les limites** (taille, nombre) validées
   par l'hôte (TECH-6).

## 3. Les phases

| Phase | Contenu | Idées | Taille |
|---|---|---|---|
| **INS-1. Cadre des instruments** | `instruments/mod.rs` : trait `Instrument` (id, nom, icône, touche, `use(ctx)`, coût), registre, barre d'instruments (CTL-5), **état de connaissance** (`Known<T>` : inconnu / estimé / mesuré) rangé dans le dex ; le scanner actuel devient **l'instrument « Scanner »** avec un **niveau** (1 rapide, 2 moyen, 3 profond : voir INS-2). Sans changer ce qu'on voit par défaut (option « tout révélé » activée au début). | — | M |
| **INS-2. Niveaux de scan** | **Scan rapide** (type, rayon, masse, température de surface), **moyen** (atmosphère, gravité, eau, rotation, orbite), **profond** (ressources, grottes, vie, géologie, radiations) ; chaque niveau prend du **temps** (barre de scan, quelques secondes à plusieurs minutes de jeu selon la distance) et une **portée** ; les sections du scanner s'affichent selon le niveau ; **résultat enregistré** dans le dex (`Dex::history`). Les valeurs approximées sont notées « ≈ » avec un intervalle. | 78 | L |
| **INS-3. Instruments à main** | **Analyseur** (viser une roche, un liquide, le sol, un végétal : composition, température, acidité, radiation), **thermomètre / baromètre / anémomètre** (lire la valeur *ici*, plus fine que `LocalWeather`), **compteur de radiations** (lien A4), **détecteur de gaz** (grottes, volcans), **sismomètre portable**, **échantillonneur** (ramasse un échantillon qui entre dans l'inventaire / le dex). Chacun s'affiche comme un **objet en main** (PER-7) avec un petit écran. | 185, 186 | L |
| **INS-4. Radar de sol et sondes** | **Radar de sol** (portée 10 à 200 voxels selon le module) : montre en vue d'ensemble les **cavités**, **filons** (`Terrain::kind_in`, `ore_chance`), roche profonde, nappes d'eau ; **sonde** lancée dans un trou ou posée (relevé pendant des minutes de jeu) ; **forage de prospection** (carotte → profil de strates, lié à `strata_color`) ; **balayage orbital** depuis le vaisseau (cartes des minerais d'un astre, résolution selon l'altitude). | 187, 77 | L |
| **INS-5. Télescope et spectromètre** | **Télescope** du vaisseau : zoom progressif (×2 à ×500) sur un astre lointain, **scan à distance** (limité : taille, type, couleur), objets rares signalés (nova, comète) ; **spectromètre** : lire la **composition d'une atmosphère** de loin (raies : O₂, H₂O, CH₄, CO₂), **température de couleur** d'une étoile, **vitesse radiale** (étoile double, exoplanète) ; courbe de lumière d'une étoile variable (AST-2). | 188, 189 | L |
| **INS-6. Navigation et cartographie** | **Cartographie de surface** (le sol exploré se dessine : minicarte, grande carte, couches : relief, biomes, eau, ressources connues), **boussole** complète (nord magnétique de l'astre si champ, direction du soleil, direction du vaisseau, balises), **altimètre / vario / horizon artificiel** (HUD de vol, lié 0.13 P7), **échelle de distance** (UA / années-lumière) à la vue galaxie, **traces** de trajet, **itinéraires** (`/aller` avec étapes), **retour sûr** vers le vaisseau. | 80, 194, 211 | L |
| **INS-7. Balises et carnet partagé** | **Balises** posées sur un lieu (nom, couleur, icône, visibilité : moi / guilde / tous), cercles dans `clickables` / `draw_body_markers`, **carnet** (notes et photos d'un lieu), **export / import** (comme le dex), **partage** par message validé (`Msg::Beacon`, limites par joueur), balises **de danger** automatiques (radiation, éruption, séisme récent), **marqueurs de joueur** (D6 de la 0.14). | 191, 192 | M |
| **INS-8. Station météo et instruments posés** | **Station météo** posée sur un lieu : enregistre températures, pressions, vent, précipitations sur plusieurs jours de jeu, **courbes** consultables, **prévision** (le jeu **calcule vraiment** la météo future : `weather` est f(graine, horloge), donc une prévision exacte est possible, avec une incertitude choisie) ; **sismographe fixe** (prévoit les séismes T5), **détecteur de météores** (annonce une pluie, `shower_strength`), **horloge et calendrier du monde** (éclipses, saisons, pluies d'étoiles) consultables. | 190, 193 | L |
| **INS-9. Mesures dans le dex et collections** | Pour chaque instrument, **entrées de dex** (« premier relevé de radiation », « plus haute altitude mesurée ») ; **collections** (échantillons de roches, de liquides, d'air, de cristaux) avec fiches ; **records personnels** (plus profond, plus chaud, plus froid, plus rapide) ; **comparer** deux astres côte à côte (tableau), **rapport de mission** exportable. | 82, 81 | M |
| **INS-10. Modules et amélioration** | Instruments **améliorables** (portée, précision, énergie) par minerais / pièces (lien inventaire 0.15), **consommation d'énergie** (lien énergie du vaisseau), **pannes** causées par les orages solaires et magnétars (N3 de la 0.14, AST-8), **calibrage**. Optionnel et réglable (désactivé en mode « contemplatif »). | 188 | M |
| **INS-11. Interface et accessibilité des instruments** | Mise en page unifiée (panneau en sections, colonnes fixes déjà utilisés par le scanner), **mode compact** (HUD), **lecteur d'écran** / sous-titres des alertes, **sons d'instruments** (D5 : bip du radar, tic du compteur), **unités** réglables (SI / impérial), **glossaire** à un clic (Jeans, albédo, effet de serre : infobulles pédagogiques, idée 125 du rapport). | 125 | M |

Ordre conseillé : **INS-1 → 2** (le socle et la révélation progressive), puis **3, 6, 7** (les outils du
quotidien), puis **4, 5, 8**, puis **9, 10, 11**.

## 4. Détails importants

### 4.1 `Known<T>` : savoir sans tricher
Chaque valeur du scanner devient `Known<T>` : `Unknown` (affichée « ? »), `Estimated { value, error }` (« ≈ 21 ± 3 °C »),
`Measured(value)`. La **vraie valeur** vient toujours du profil ; `Estimated` = vraie valeur + erreur **déterministe**
(graine du joueur + id de l'astre + niveau), donc stable d'une lecture à l'autre. Le dex garde la meilleure
connaissance acquise.

### 4.2 Prévision météo
La météo étant f(graine, horloge, lieu) (`weather::sample`), la **prévision** appelle la même fonction pour t + Δ.
La fenêtre est limitée par l'instrument (6 h de jeu pour une station de base, 3 jours pour une station avancée) et
on ajoute une **erreur** qui croît avec Δ (peut être 0 pour un mode « parfait »).

### 4.3 Radar de sol : seulement ce qui est vrai
Il interroge `Terrain::kind_in` dans le cylindre autour du joueur (cache par blocs de 32³, `BlockKey`) : les
cavités et filons dessinés **existent vraiment** ; aucune fausse piste. Coût : calcul asynchrone, résultat en
quelques secondes, jamais sur le fil principal.

### 4.4 Balises et réseau
Message `Msg::Beacon { id, abs, name, color, scope }` (position **absolue** f64, règle de l'origine flottante) ; l'hôte
limite à N balises par joueur et par astre, expire les anciennes ; les balises de guilde sont filtrées côté client.

## 5. Mesures et tests

- Tests unitaires : `Known<T>` stable (même erreur pour même entrée), prévision cohérente avec `weather::sample`,
  radar ⊆ voxels réels.
- Captures : panneau de scan niveau 1 / 2 / 3, analyseur sur un sol, carte de surface, station météo avec courbe
  (mondes et conditions vérifiés, `TESTS-JEU.md`).
- Perf : le radar et le balayage orbital ne dépassent pas 1 ms par image sur le fil principal.
- Réseau : balises sous 10 000 messages aléatoires sans panique (TECH-6).

## 6. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Les niveaux de scan bloquent-ils l'information ou la précisent seulement ? | La **précisent** ; option « tout révélé » (par défaut au début). |
| Q2 | Le temps de scan est-il en temps réel ou en temps de jeu ? | Temps réel court (3 à 20 s) pour le confort ; temps de jeu pour les stations météo. |
| Q3 | Les instruments consomment-ils de l'énergie ? | Seulement si le mode « survie » est actif. |
| Q4 | La prévision météo est-elle parfaite ? | Parfaite pour la première station, avec bruit croissant avec le temps (réaliste). |
| Q5 | Les balises sont-elles visibles de l'espace ? | Oui (cercles de `clickables`), limitées en nombre. |
| Q6 | Un joueur peut-il tricher en lisant `/profil` ? | `/profil` reste un outil de test (pas de blocage), mais le dex ne le compte pas comme scan. |

## 7. Prompts

**INS-1** : « Lis `CLAUDE.md` (0.10 phase 9, 0.13.1 dex, scanner) et `ROADMAP-instruments.md` §3 INS-1 et §4.1. Crée
`instruments/mod.rs` (trait, registre, `Known<T>`), fais du scanner existant le premier instrument sans changer son
contenu par défaut (« tout révélé »). Tests de stabilité de `Known<T>`. »

**INS-2** : « §3 INS-2 : trois niveaux de scan avec barre de progression, valeurs ≈ avec intervalle, enregistrement
dans le dex. Capture des 3 niveaux sur un monde varié. »

**INS-3 à INS-11** : « Lis `ROADMAP-instruments.md` §3 INS-n, implémente, teste, capture, PR non fusionnée. »
