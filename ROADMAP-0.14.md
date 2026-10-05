# Feuille de route — Mondes vivants et extraordinaires (0.14)

Objectif : faire **vivre** les planètes de la 0.13 (eau qui coule, volcans actifs, faune, végétation,
son, lieux à découvrir), puis faire apparaître dans la génération des **mondes exceptionnels** et des
**événements planétaires** : planète-cacahuète tirée par une lune trop proche, monde verrouillé avec sa
bande crépusculaire, océan sous la glace, planète en refroidissement, anneaux au ras du sol, tempête
qui couvre un tiers du globe, planète cassée, monde creux… Rares, spectaculaires, et chacun avec **une
règle qui change la façon de jouer**.

| Source | Ce qui est repris |
|---|---|
| `A-FAIRE-PLUS-TARD.md` (bloc D de la 0.11) | D1 eau vivante, D2 géologie active, D3 faune, D4 végétation, D5 son, D6 points d'intérêt (retirés de la 0.13) |
| `prompt0.14.md` (conversation ChatGPT) | Banque d'idées de planètes rares et d'événements, **adaptée** au moteur et au réalisme du jeu (§4, §5) |

**La 0.14 prévue (minage et destruction) devient la 0.15.**

Point de départ : la 0.13 (échelle k = 16, relief en voxels, vraie eau, ciel, trous noirs, aliens).
Chaque phase = une branche `claude/roadmap-0-14-<phase>`, une PR non fusionnée : tu testes, puis tu dis
« push main ». Chaque phase a son **prompt prêt à coller** (§9).

---

## 1. Décisions déjà prises (rappel)

| Sujet | Décision |
|---|---|
| Qualité | **Aucune concession** : référence Ultra, mesures avant / après (règles 17-18 de la 0.13). |
| Échelle | k = 16 (voxel 16 fois plus petit qu'en 0.12), marcheur ~2 voxels. |
| Réalisme | Chaque chose générée porte une étiquette **réaliste / spéculatif / fictif** (règle 5 de la 0.10). Les mondes tempérés restent rares (décision 0.10). |
| Raretés | 98 % ordinaire, 1,5 % peu commun, 0,4 % rare, 0,1 % légendaire (`planetgen/traits.rs`). |
| Grottes | 2 000 unités de profondeur (décision 0.13 Q2). |
| Minage, destruction | **0.15** (avant : 0.14). Les deltas voxel existent déjà (`voxel.rs`). |

---

## 2. Point de départ (code actuel)

| Domaine | Aujourd'hui | Fichier |
|---|---|---|
| Traits rares | **Des étiquettes seulement** : 6 peu communs (geysers géants, super-tempête, lacs de lave, cratère géant, arches, champ inversé), 5 rares (pluie de diamants, océan bioluminescent, forêt pétrifiée, glace superionique, noyau de fer exposé), 5 légendaires (ruines, monolithe, cristaux chantants, anomalie gravitationnelle, écho temporel). Affichés au scanner et dans `/stats`, **rien ne change dans le monde** (sauf les arches de `rocks.rs`). | `planetgen/traits.rs`, `scanner.rs`, `stats.rs` |
| Traits naturels | Anneaux, aurores, rotation synchrone, rétrograde, monde-océan, océan de magma : vrais dans le monde. | `traits.rs::traits_of` |
| Rotation synchrone | Face +X vers l'étoile (A1), jour éternel. Climat : pas de différence jour / nuit spécifique. | `world_clock.rs`, `planetgen/climate.rs` |
| Lunes et marées | Marées de 0 à 3 voxels (`terrain::Tide`), lunes ≤ planète/3, pas de limite de Roche. | `terrain.rs`, `sky.rs`, `planetgen/system.rs` |
| Planètes errantes | 1 système sur 30, gelées, physique sans étoile. | `PlanetConfig::rogue` |
| Météo | Pluies exotiques (méthane, acide, verre, fer), orages, poussière, brouillard. | `weather.rs` |
| Impacts | Rares, cratère en deltas voxel, réseau. | `meteors.rs` |
| Orages magnétiques | Par tranches de 15 min, aurores avivées. | `sky.rs` |
| Découverte | Dex (0.13.1) : historique au scanner, panneau K, notes. | `scanner.rs`, panneau K |
| Faune | Paramètres seulement (`Fauna`). | `planetgen/life.rs` |

---

## 3. Règles d'architecture (0.14)

Les règles 1 à 19 (0.10, 0.11, 0.13) restent valables. La 0.14 en ajoute :

20. **Un trait = une règle visible.** Plus aucun trait « étiquette » : chaque trait et chaque archétype
    change la génération (relief, eau, ciel, climat, vie) **et** se voit (de l'espace ou au sol), ou il
    est retiré.
21. **Archétypes = couche de génération.** `planetgen/archetypes.rs`, nouvelle sous-graine figée
    (`Layer::Archetype`, règle 2). Un archétype est tiré **après** les couches physiques et seulement
    si l'astre le permet (comme `TraitContext`). Il **modifie** des couches existantes par des
    paramètres (jamais en écrasant tout) : relief, hydrologie, climat, rotation, lunes, anneaux.
22. **Cohérence physique d'abord.** Un archétype réaliste doit sortir de la physique (lune trop proche →
    limite de Roche, marées, chaleur ; rotation synchrone → climat jour / nuit). Les fictifs sont
    **légendaires**, étiquetés « fictif », et gardent une logique interne.
23. **Les événements sont des fonctions du temps.** Un événement = `f(graine, horloge)` (règle 9) :
    tous les joueurs le voient au même moment, l'hôte n'envoie que l'horloge. Ce qu'il change **dure**
    par des **deltas** (voxels, valeurs vivantes), sauvés et envoyés comme les cratères d'impact.
24. **Prévenir avant de frapper.** Tout événement dangereux est annoncé (scanner, signes visibles :
    secousses, fumée, mer qui se retire) au moins quelques minutes de jeu avant.
25. **Trouvables.** Chaque archétype et chaque événement est cherchable (`/aller planete <archétype>`),
    compté dans `/stats`, enregistré dans le dex, avec un **test** qui vérifie sa fréquence dans la
    galaxie principale (ni absent, ni trop commun).
26. **Pas de noms de films.** Les idées viennent de films, séries et BD, mais aucun nom, aucune forme
    reconnaissable d'une œuvre protégée : ce sont des archétypes génériques.

---

## 4. Bloc D — Le monde qui vit (bloc D de la 0.11, à l'échelle 0.13)

| Phase | Contenu | Taille |
|---|---|---|
| **D1. Eau vivante** | **Rivières** calculées depuis le relief (écoulement vers la mer, bassins hachés, largeur selon le débit), **lacs** dans les cuvettes, **cascades** sur les falaises, **deltas** et estuaires, glace qui fond et gèle selon la saison (A3), **banquise**, vagues sur les lacs. Passent par `kind_at` (règle 16) ; les grandes rivières se voient de l'espace. `PROTOCOL` +1. | XL |
| **D2. Géologie active** | Coulées de lave **lumineuses** (visibles la nuit), **geysers** (jets réguliers, f(horloge)), **cryovolcans** (type Encelade), fumerolles, sources chaudes, petits **séismes** (secousse, éboulis) selon l'activité de `geology.rs`. Le trait « geysers géants » devient réel (règle 20). | M |
| **D3. Végétation vivante** | Arbres et herbe qui **bougent avec le vent** (rafales de `weather::wind`), plantes qui s'ouvrent le jour, **bioluminescence** la nuit, feuillage selon la **saison**, forêts denses en instances et LOD. Le trait « forêt pétrifiée » devient réel. | L |
| **D4. Faune** | Créatures procédurales depuis `Fauna`, construites avec les **familles de l'éditeur** (squelettes et animations de `assets/editeur/races/`) : troupeaux, volants, aquatiques, fouisseurs, diurnes / nocturnes, fuite devant le joueur, pas de combat ; tailles réelles (`max_size_m`) à l'échelle k = 16. Le trait « océan bioluminescent » devient réel (plancton, méduses). | XL |
| **D5. Son** | Vent, pluie, tonnerre, vagues, rivières et cascades, écho des grottes, lave, geysers, faune, moteurs du vaisseau, **silence dans le vide**, son étouffé sous l'eau. Caisse audio proposée dans la PR (Q5). | L |
| **D6. Points d'intérêt** | Lieux rares au scanner : grottes géantes, arches, cratères géants, cascades, sources chaudes, geysers, épaves, ruines (fictif, étiqueté), **monolithe** (le trait devient réel), marqueurs des joueurs ; dans le dex. | M |

---

## 5. Bloc X — Mondes exceptionnels

### 5.1 Le catalogue

Rareté : **P** = peu commun (1,5 %), **R** = rare (0,4 %), **L** = légendaire (0,1 %), **N** = naturel
(découle de la physique, pas de tirage). Réalisme : ré = réaliste, sp = spéculatif, fi = fictif.

| Phase | Archétype | Rar. | Réal. | Ce qui change dans la génération | Ce qui change en jeu |
|---|---|---|---|---|---|
| **X1** | **Œil (verrouillé)** : face jour brûlée, face nuit gelée, **bande crépusculaire** habitable | N | ré | Climat selon l'angle au point sous l'étoile (pas la latitude), calotte de glace côté nuit, désert ou mer bouillante côté jour, vents permanents du jour vers la nuit | Coucher de soleil **éternel** dans la bande ; la vie (D3, D4) ne pousse que là ; traverser = survie (A4) |
| X1 | **Crépuscule éternel** (bande étroite très contrastée) | P | ré | Variante plus extrême : atmosphère fine, bande de quelques centaines de voxels | Route de la bande, lieux d'intérêt alignés |
| X1 | **Saisons de siècles** (année très longue + forte inclinaison) | R | ré | Saison figée sur des heures de jeu, hémisphère en nuit polaire de plusieurs heures | Forêt fossilisée, glaciers qui recouvrent des ruines (D6) |
| X1 | **Nuit de 20 ans** (rotation extrêmement lente, non synchrone) | R | ré | Jour de 3 h de jeu (plafond A1) mais terminateur qui avance au pas | Le terminateur bouge à vitesse de marche : on peut le suivre |
| **X2** | **Monde-océan profond** (100 % océan) | P | ré | Pas de terre ; fond : canyons, volcans sous-marins, récifs géants | Plongée (O2), îles volcaniques rares, ruines englouties (L) |
| X2 | **Océan peu profond** (10 à 50 voxels d'eau partout) | P | ré | Niveau de la mer juste au-dessus du relief | On voit le fond partout : récifs, montagnes sous-marines |
| X2 | **Océan sous la glace** (type Europe) | N/P | ré | Croûte de glace épaisse + océan liquide dessous (couche 3D), **crevasses** et geysers (D2) | Descendre par une crevasse ; vie bioluminescente sous la glace |
| X2 | **Monde-pluie** (il pleut partout, tout le temps) | P | ré | Météo forcée, nuages permanents, rivières (D1) partout | Visibilité faible, son (D5) |
| X2 | **Océan central / continent annulaire** et **maelström** | R | sp | Masque de continent en anneau ; tourbillon permanent au centre | Le maelström aspire le vaisseau en vol bas |
| **X3** | **Monde en refroidissement** : une moitié en fusion, l'autre avec océans et premières plantes | R | sp | Mélange de deux couches (magma / croûte) selon une frontière qui avance lentement avec l'horloge | La **frontière** : coulées, vapeur, nouvelles terres (deltas) |
| X3 | **Continents flottant sur le magma** | R | sp | Océan de magma + plaques de croûte | Lave lumineuse (D2), danger (A4) |
| X3 | **Volcan qui sort de l'atmosphère** (type Olympus Mons, en plus grand) | P | ré | Un massif-bouclier géant (relief T1) jusqu'à la limite de l'atmosphère | Sommet sans air : ciel noir en plein jour |
| X3 | **Glace noire** à fissures lumineuses | R | sp | Glace très sombre et brillante, fissures émissives (D2 cryovolcanisme) | Monde noir brillant, lumière qui vient du sol |
| X3 | **Glace transparente** sur des centaines de voxels | R | sp | Glace en matière translucide (eau de O1) ; on voit le sol ou l'océan dessous | Marcher sur une vitre géante |
| **X4** | **Lune trop proche / planète-cacahuète** | R | ré | Lune près de la **limite de Roche** : planète étirée vers la lune (forme non sphérique), **bourrelets d'océan permanents**, marées de dizaines de voxels, chaleur de marée, montagnes orientées | La lune **remplit le ciel** ; la mer avance et recule à vue d'œil |
| X4 | **Lunes en résonance** (2 à 4 lunes, marées qui s'additionnent) | P | ré | `Tide` avec plusieurs astres (existe déjà), amplitudes plus fortes, cycles longs | Grandes marées prévisibles (calendrier au scanner), côtes changeantes |
| X4 | **Anneaux bas** (anneaux juste au-dessus de l'atmosphère) | R | ré | Anneau de rayon intérieur très proche ; ombre de l'anneau sur une bande de la planète | Arche immense dans le ciel, bande d'ombre permanente, chutes de débris (X5) |
| X4 | **Planète après collision** : moitié intacte, moitié océan de magma, anneau de débris | L | ré | Bassin d'impact géant + magma + anneau | Débris qui tombent (événement N2) |
| **X5** | **Tempête géante** (30 % du globe, œil, murs de nuages) | P | ré | Système dépressionnaire fixe dans `weather.rs`, visible de l'espace | Vents destructeurs, éclairs permanents, l'œil est calme |
| X5 | **Supercellules** (des milliers d'orages) | R | ré | Météo orageuse partout, tornades | Éclairs dangereux pour le vaisseau |
| X5 | **Monde électrique** | R | sp | Arcs entre nuages, montagnes et mer | Le vaisseau posé sur un sommet attire la foudre |
| X5 | **Aurores géantes** (champ magnétique très fort) | P | ré | Aurores jusqu'à l'équateur, de jour comme de nuit | Ciel animé permanent, boussole folle |
| X5 | **Planète dans une nébuleuse** | R | ré | Système placé dans un nuage (`galaxy_fx.rs`), skybox colorée (C2 de la 0.13) | Nuits presque aussi claires que les jours |
| X5 | **Planète autour d'un trou noir** | L | sp | Système centré sur un trou noir (V1 de la 0.13) avec une étoile compagne | Disque et lentille dans le ciel ; **temps ralenti** près du trou noir (fictif léger, étiqueté) |
| **X6** | **Monde cristallin** / **cristaux chantants** (le trait devient réel) | R / L | sp / fi | Croûte de cristal, montagnes de cristal translucide, cristaux géants hachés par cellule | Reflets, sons (D5), ressources |
| X6 | **Monde de diamant** (riche en carbone) | R | sp | Composition carbone (`resources.rs`), cavernes de diamant | Gisements profonds (minage en 0.15) |
| X6 | **Monde métallique** / **noyau exposé** (le trait devient réel) | R | sp | Surface métallique, plaines de fer, lacs de métal fondu côté chaud | Tempêtes magnétiques, boussole inutile |
| X6 | **Monde vertical** (falaises partout) | R | sp | Relief T1 / T2 poussé à l'extrême : plateaux étagés séparés de parois | Vivre par niveaux ; vol bas obligatoire |
| X6 | **Forêt géante** (arbres de centaines de voxels) | R | sp | Décor géant (D3) avec étages : sol sombre, troncs, canopée | Marcher sur la canopée |
| X6 | **Monde fongique** (champignons géants, spores, ciel violet) | R | sp | Biome spores étendu, champignons géants, brume de spores (C1 de la 0.13) | Spores = brouillard coloré, faune (D4) mutante |
| X6 | **Monde cubique** (la géologie est en gros cubes) | L | fi | Relief quantifié en gros cubes (falaises à angle droit) | Clin d'œil voxel ; panorama unique |
| **X7** | **Planète cassée** (morceaux tenus par la gravité) | L | fi | Plusieurs fragments séparés (astéroïdes géants de `asteroids.rs`, terrain sur chaque morceau) | Voler d'un fragment à l'autre, ponts naturels |
| X7 | **Planète creuse** (monde intérieur + soleil intérieur) | L | fi | Croûte + **surface intérieure** (deuxième sphère inversée), soleil central, gouffres d'accès | Deux mondes en un ; entrée par un gouffre polaire |
| X7 | **Océan suspendu** (mers qui flottent, cascades dans le vide) | L | fi | Couche d'eau en altitude soutenue par l'anomalie gravitationnelle (le trait devient réel) | Cascades qui tombent dans le ciel |
| X7 | **Géante gazeuse habitable** (îles flottantes dans les nuages) | R | sp | Îles volantes et créatures planantes géantes à l'altitude respirable | Se poser sur une île flottante |
| **X8** | **Monde mort** (ruines d'une civilisation, le trait devient réel) | R | fi | Villes en ruines, routes, statues (générées avec les outils des aliens de la 0.13) | Exploration, épaves (D6) |
| X8 | **Nature reconquise** (ruines sous la forêt) | R | fi | Ruines + forêt dense (D3) | Gratte-ciel qui dépassent des arbres |
| X8 | **Monde dévasté** (radioactif, villes vitrifiées) | R | fi | Biomes de verre et de cendres, radiation (A4) | Combinaison obligatoire |
| X8 | **Terraformation inachevée** | P | sp | Mers géométriques, montagnes trop régulières, zones mortes (valeurs vivantes de la 0.13 L6) | Reprendre le travail (terraformation) |
| X8 | **Monde-observatoire** / **monolithe** | L | fi | Structures alignées sur une étoile ou une galaxie | Énigme : trouver ce qu'elles regardent |
| X8 | **Planète-machine** / **monde-ville** | L | fi | Surface entièrement artificielle (blocs, tours, canyons urbains) | Lumières la nuit visibles de l'espace |
| X8 | **Planète vivante** (la géologie réagit) | L | fi | Montagnes « os », rivières « veines » qui battent, grottes « organes » | Le terrain réagit au joueur (sans destruction avant 0.15) |
| X8 | **Écho temporel** (vallée au temps accéléré, le trait devient réel) | L | fi | Zone où l'horloge locale va plus vite : végétation qui pousse à vue d'œil | Revenir après une heure : la vallée a changé |
| X8 | **Planète qui évolue** (vie primitive → forêts → mégafaune avec le temps de jeu) | R | sp | Stade de vie = f(horloge) | Revenir des jours plus tard : un autre monde |

### 5.2 Les phases du bloc X

| Phase | Contenu | Taille |
|---|---|---|
| **X0. Le système d'archétypes** | `planetgen/archetypes.rs` (règle 21), `Layer::Archetype`, tirage par rareté et conditions, effets sous forme de **paramètres** passés aux couches (relief, hydrologie, climat, rotation, lunes, anneaux, météo, vie). Les traits actuels sont reliés à un effet ou retirés (règle 20). Scanner : section « Anomalie » ; dex ; `/aller planete <archétype>` ; `/stats` ; **signal d'anomalie** visible de loin (cercle de couleur, règle 25). Test de fréquence dans la galaxie principale. `PROTOCOL` +1. | L |
| **X1. Lumière et rotation** | Œil verrouillé, crépuscule éternel, saisons de siècles, nuit de 20 ans. | L |
| **X2. Mondes d'eau** | Océan profond, peu profond, sous la glace, monde-pluie, océan central et maelström. | L |
| **X3. Feu et glace** | Monde en refroidissement, continents sur magma, volcan géant, glace noire, glace transparente. | L |
| **X4. Lunes, marées, anneaux** | Lune trop proche (forme étirée : maillage lointain et terrain non sphériques), lunes en résonance, anneaux bas, planète après collision. | XL |
| **X5. Ciels extrêmes** | Tempête géante, supercellules, monde électrique, aurores géantes, nébuleuse, trou noir. | L |
| **X6. Croûtes étranges** | Cristal, diamant, métal, vertical, forêt géante, fongique, cubique. | L |
| **X7. Mondes impossibles** | Planète cassée, creuse, océan suspendu, géante gazeuse habitable. | XL |
| **X8. Vestiges et mondes vivants** | Monde mort, nature reconquise, dévasté, terraformation inachevée, observatoire, machine, planète vivante, écho temporel, planète qui évolue. | XL |

---

## 6. Bloc N — Événements planétaires

Tous suivent les règles 23 et 24 : `f(graine, horloge)`, annoncés, conséquences en **deltas**.

| Phase | Événements | Ce qu'on voit | Ce qui dure | Taille |
|---|---|---|---|---|
| **N1. Volcans et séismes** | **Éruption** (d'après l'activité de `geology.rs`) : panache, bombes, coulée qui descend la pente réelle ; **séisme** fort avec éboulements | Secousses avant, fumée, ciel cendré, lave la nuit | Coulée figée qui remplit une vallée, **nouvelle île**, rivière déviée (deltas voxel) | L |
| **N2. Ce qui tombe du ciel** | Impact d'astéroïde complet : météore → impact → **onde de choc** → incendies → **hiver d'impact** (ciel sombre, froid) ; **mégatsunami** (impact en mer) ; **anneau qui s'effondre** (pluie de météores pendant des heures) ; **lune qui se brise** (devient un anneau, morceaux qui tombent) | Le bolide des heures avant (scanner), la mer qui se retire, la vague | Cratère (deltas, comme B5), côtes rasées, nouvel anneau (valeurs vivantes) | XL |
| **N3. L'étoile et le champ magnétique** | **Éruption solaire majeure** (au-delà des orages de C4) : aurores partout, radiation (A4), **pannes** du vaisseau (HUD, scanner brouillés), navigation dangereuse ; **inversion des pôles** : aurores à l'équateur, boussole inversée, migrations de la faune (D4) | Alerte au scanner, ciel en feu | Rien (temporaire), sauf la boussole inversée jusqu'à la fin de l'inversion | M |
| **N4. Les marées et le temps long** | **Grandes marées de résonance** (X4) ; **saisons de siècles** qui avancent (X1) ; **dérive continentale accélérée** (fictif, légendaire : continents qui bougent visiblement en jours de jeu) ; **planète qui évolue** (X8) | Calendrier des marées au scanner | Côtes, glaciers, stade de la vie (valeurs vivantes) | L |

---

## 7. Ordre et versions

```
0.13 terminée
   │
   D1 → D2 → D3 → D4 → D5 → D6     le monde qui vit
   │                               ── release 0.14.0 « Vivant » ──
   X0 → X1 → X2 → X3 → X4 → X5     archétypes (X2 a besoin de l'eau de D1)
   │  → X6 → X7 → X8
   │                               ── release 0.14.1 « Extraordinaire » ──
   N1 → N2 → N3 → N4               événements (N1 a besoin de D2, N2 de X4)
                                   ── release 0.14.2 « Événements » ──
   │
   0.15 : minage et destruction (avant : 0.14)
```

Dépendances : X2 utilise les rivières (D1) et l'eau de la 0.13 ; X4 et N2 réutilisent anneaux,
astéroïdes et marées ; X8 réutilise les bâtiments des aliens (0.13 L5) ; X5 utilise la skybox et les
trous noirs de la 0.13 (C2, V1).

---

## 8. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Les **mondes fictifs** (creux, cassé, océan suspendu, cubique, vivant, machine) : on les garde ? | Oui, **légendaires** et étiquetés « fictif » ; jamais dans les 20 systèmes autour du départ. |
| Q2 | Les **événements** peuvent-ils modifier le terrain pour de bon (île nouvelle, côte rasée) avant le minage (0.15) ? | Oui, par deltas ; ils ne touchent jamais la zone autour d'un joueur posé sans l'avoir annoncé. |
| Q3 | **Fréquence** des grands événements ? | Un grand événement par planète tous les quelques jours de jeu, annoncé 5 à 30 min avant ; un petit (séisme, geyser géant) toutes les heures. |
| Q4 | Faut-il **forcer** un monde exceptionnel près du départ pour que le joueur en voie un vite ? | Oui : un archétype peu commun garanti à moins de 5 systèmes du départ (jamais un fictif). |
| Q5 | Caisse audio pour le son (D5) ? | `bevy_audio` (intégré à Bevy) d'abord ; `bevy_kira_audio` si on a besoin d'effets (écho, étouffement). |
| Q6 | « Temps ralenti » près d'un trou noir (X5) et « vallée au temps accéléré » (X8) : l'horloge locale peut-elle différer de l'horloge du monde ? | Seulement pour l'**affichage** et la croissance locale (plantes, ruines) ; l'horloge du monde reste unique (règle 9). |

---

## 9. Prompts (à coller dans une nouvelle session, un par phase)

Contexte commun : « Lis `ROADMAP-0.14.md`, `prompt0.14.md`, `ROADMAP-0.13.md` et `CLAUDE.md` (règles 1 à
26). `git pull origin main` avant de coder. Branche `claude/roadmap-0-14-<phase>`. Build release,
tests, mesures avant / après et captures. PR non fusionnée (je dirai « push main »). Aucune concession
sur la qualité ; ne change ni les tailles ni les décisions sans me demander. »

- **D1** — « [contexte commun] Phase D1 : rivières depuis le relief, lacs, cascades, deltas, glace
  saisonnière, banquise ; visibles de l'espace ; PROTOCOL +1. »
- **D2** — « [contexte commun] Phase D2 : lave lumineuse, geysers, cryovolcans, fumerolles, sources
  chaudes, séismes ; trait « geysers géants » réel. »
- **D3** — « [contexte commun] Phase D3 : végétation au vent, jour / nuit, bioluminescence, saisons,
  forêts en instances ; trait « forêt pétrifiée » réel. »
- **D4** — « [contexte commun] Phase D4 : faune procédurale avec les familles de l'éditeur, troupeaux,
  volants, aquatiques, fuite ; trait « océan bioluminescent » réel. »
- **D5** — « [contexte commun] Phase D5 : son (Q5). »
- **D6** — « [contexte commun] Phase D6 : points d'intérêt au scanner et dans le dex ; monolithe réel. »
- **X0** — « [contexte commun] Phase X0 : `planetgen/archetypes.rs` (règles 20, 21, 25), traits reliés à
  un effet ou retirés, scanner, dex, `/aller`, `/stats`, signal d'anomalie, test de fréquence ;
  PROTOCOL +1. »
- **X1** — « [contexte commun] Phase X1 : œil verrouillé avec bande crépusculaire, crépuscule éternel,
  saisons de siècles, nuit de 20 ans. »
- **X2** — « [contexte commun] Phase X2 : mondes-océans (profond, peu profond, sous la glace), monde-pluie,
  océan central et maelström. »
- **X3** — « [contexte commun] Phase X3 : monde en refroidissement, continents sur magma, volcan géant,
  glace noire, glace transparente. »
- **X4** — « [contexte commun] Phase X4 : lune près de la limite de Roche (planète étirée, bourrelets,
  marées géantes), lunes en résonance, anneaux bas, planète après collision. »
- **X5** — « [contexte commun] Phase X5 : tempête géante, supercellules, monde électrique, aurores
  géantes, planète dans une nébuleuse, planète autour d'un trou noir (Q6). »
- **X6** — « [contexte commun] Phase X6 : mondes de cristal, diamant, métal, vertical, forêt géante,
  fongique, cubique. »
- **X7** — « [contexte commun] Phase X7 : planète cassée, creuse, océan suspendu, géante gazeuse
  habitable (Q1). »
- **X8** — « [contexte commun] Phase X8 : monde mort, nature reconquise, dévasté, terraformation
  inachevée, observatoire, machine, planète vivante, écho temporel, planète qui évolue. »
- **N1** — « [contexte commun] Phase N1 : éruptions et séismes annoncés, conséquences en deltas (Q2, Q3). »
- **N2** — « [contexte commun] Phase N2 : impact complet, hiver d'impact, mégatsunami, anneau qui
  s'effondre, lune qui se brise. »
- **N3** — « [contexte commun] Phase N3 : éruption solaire majeure (pannes, radiation), inversion des
  pôles. »
- **N4** — « [contexte commun] Phase N4 : grandes marées de résonance, saisons de siècles, dérive
  continentale, planète qui évolue. »
