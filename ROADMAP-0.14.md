# Feuille de route — Mondes vivants et extraordinaires (0.14)

Objectif : faire **vivre** les planètes de la 0.13 (eau qui coule, volcans actifs, faune, végétation,
son, lieux à découvrir), puis faire apparaître dans la génération des **mondes exceptionnels** et, sur
quelques planètes seulement, des **événements** rares. Exemples de mondes exceptionnels : planète
étirée par une lune trop proche, monde verrouillé avec sa bande de crépuscule éternel, océan sous la
glace, planète en refroidissement, anneaux au ras du ciel, tempête qui couvre un tiers du globe, planète
cassée, monde creux… Chacun est **rare**, se **voit**, et a **une règle qui change la façon de jouer**.

| Source | Ce qui est repris |
|---|---|
| `A-FAIRE-PLUS-TARD.md` (bloc D de la 0.11) | D1 eau vivante, D2 géologie active, D3 faune visible, D4 végétation vivante, D5 son, D6 points d'intérêt (retirés de la 0.13) |
| `prompt0.14.md` (conversation ChatGPT) | Banque d'idées de planètes rares et d'événements, **adaptée** au moteur et au réalisme du jeu (§5, §6) |

**L'ancienne 0.14 (minage et destruction) devient la 0.15.**

Point de départ : la 0.13 terminée (échelle k = 16, relief en voxels, vraie eau, brouillard, skybox,
trous noirs, aliens, terraformation). Chaque phase = une branche `claude/roadmap-0-14-<phase>`, une PR
non fusionnée : tu testes, puis tu dis « push main ». Chaque phase a son **prompt prêt à coller** (§10).

---

## 1. Décisions

### 1.1 Rappel

| Sujet | Décision |
|---|---|
| Qualité | **Aucune concession** : référence Ultra, mesures avant / après (règles 17-18 de la 0.13). |
| Échelle | k = 16, marcheur ~2 voxels (0.13). |
| Réalisme | Tout ce qui est généré porte une étiquette **réaliste / spéculatif / fictif** (règle 5 de la 0.10) ; les mondes tempérés restent rares. |
| Raretés | 98 % ordinaire, 1,5 % peu commun, 0,4 % rare, 0,1 % légendaire (`planetgen/traits.rs`). |
| Minage, destruction | **0.15**. Les deltas voxel existent déjà (`voxel.rs`). |

### 1.2 Réponses du 04/10/2026

| # | Sujet | Décision |
|---|---|---|
| Q1 | **Mondes fictifs** (creux, cassé, océan suspendu, cubique, vivant, machine) | **Oui**, gardés : légendaires et étiquetés « fictif ». |
| Q2 | **Les événements changent le terrain pour de bon** (avant le minage de la 0.15) | **Oui**, par deltas. |
| Q3 | **Fréquence des événements** | Seulement sur **un petit pourcentage de planètes** (« planètes actives »), et **très rares** : environ **un par heure de jeu** sur une planète active. |
| Q4 | **Monde exceptionnel près du départ** | **Non** : aucun monde exceptionnel tiré dans les **50 systèmes les plus proches du départ** (système 0). |
| Q5 | **Son** | **Oui**, `bevy_audio` (intégré à Bevy) ; une autre caisse seulement si un effet manque, à justifier dans la PR. |
| Q6 | **Temps local** (près d'un trou noir, vallée au temps accéléré) | **D'accord** : seuls l'affichage et la croissance locale (plantes, ruines) changent ; l'horloge du monde reste unique. |
| Q7 | **Part des planètes actives** | **Très rare** : il faut de la **chance** pour voir un événement. Point de départ : **1 planète solide sur 1 000** (0,1 %), une partie seulement pour les plus destructeurs ; réglé par le test de fréquence (§6.1). |
| Q8 | **Musique** | Pas en 0.14 : D5 = sons du monde seulement. **La musique est pour plus tard** (notée dans `A-FAIRE-PLUS-TARD.md`). |

---

## 2. Point de départ (code actuel)

| Domaine | Aujourd'hui | Fichier |
|---|---|---|
| Traits tirés | **Des étiquettes seulement** : 6 peu communs (geysers géants, super-tempête, lacs de lave, cratère géant, arches, champ inversé), 5 rares (pluie de diamants, océan bioluminescent, forêt pétrifiée, glace superionique, noyau de fer exposé), 5 légendaires (ruines, monolithe, cristaux chantants, anomalie gravitationnelle, écho temporel). Affichés au scanner et dans `/stats` ; **rien ne change dans le monde** (sauf les arches de `rocks.rs`). | `planetgen/traits.rs`, `scanner.rs`, `stats.rs` |
| Traits naturels | Anneaux, aurores, rotation synchrone, rétrograde, monde-océan, océan de magma : vrais dans le monde. | `traits.rs::traits_of` |
| Rotation synchrone | Face +X vers l'étoile (A1), jour éternel ; le climat ne distingue pas la face jour de la face nuit. | `world_clock.rs`, `planetgen/climate.rs` |
| Lunes, marées | Marées de 0 à 3 voxels (`terrain::Tide`), lunes ≤ planète / 3, pas de limite de Roche. | `terrain.rs`, `sky.rs`, `planetgen/system.rs` |
| Planètes errantes | 1 système sur 30, gelées, sans étoile. | `PlanetConfig::rogue` |
| Météo | Pluies exotiques, orages, poussière, brouillard, rafales (0.12 C3). | `weather.rs` |
| Impacts | Rares, cratère en deltas voxel, envoyés en réseau. | `meteors.rs` |
| Orages magnétiques | Par tranches de 15 min, aurores avivées. | `sky.rs` |
| Découverte | Dex (0.13.1) : historique au scanner, panneau K, notes. | `scanner.rs` |
| Faune | Paramètres seulement (`Fauna` : espèces, taille, locomotion). | `planetgen/life.rs` |
| Départ | Système 0 (le vaisseau commence en orbite dans le système 0). | `main.rs` |

---

## 3. Règles d'architecture (0.14)

Les règles 1 à 19 (0.10, 0.11, 0.13) restent valables. La 0.14 en ajoute :

20. **Un trait = une règle visible.** Plus aucun trait « étiquette » : chaque trait et chaque archétype
    change la génération (relief, eau, ciel, climat, vie) **et** se voit (de l'espace ou au sol), ou il
    est retiré.
21. **Archétypes = une couche de génération.** `planetgen/archetypes.rs`, nouvelle sous-graine figée
    (`Layer::Archetype`, règle 2). L'archétype est tiré **après** les couches physiques et seulement si
    l'astre le permet (comme `TraitContext`). Il **modifie** les couches existantes par des paramètres
    (jamais en remplaçant tout) : relief, hydrologie, climat, rotation, lunes, anneaux, météo, vie.
    Un astre ordinaire ne paie rien (aucun calcul en plus, `bench_tiles` inchangé).
22. **La physique d'abord.** Un archétype réaliste sort de la physique (lune trop proche → limite de
    Roche, marées, chaleur ; rotation synchrone → climat jour / nuit). Les fictifs sont **légendaires**,
    étiquetés, et gardent une logique interne (Q1).
23. **Zone calme du départ.** Aucun archétype **tiré** dans les **50 systèmes les plus proches du système
    0** (Q4). Les archétypes **naturels** (qui découlent de la physique : verrouillé, monde-océan) restent
    possibles partout. La zone ne dépend que de la graine du monde : identique pour tous les joueurs.
24. **Événements : rares, sur peu de planètes.** Une planète est **active** si sa géologie, ses lunes ou
    son étoile le permettent **et** si le tirage le dit (un petit pourcentage, §6.1). Seules les planètes
    actives ont des événements, **environ un par heure de jeu** (Q3), jamais sur les autres. Un
    événement = `f(graine, horloge)` (règle 9) : tous les joueurs le voient au même moment, l'hôte
    n'envoie que l'horloge.
25. **Ce qu'un événement change dure.** Coulée figée, nouvelle île, côte rasée, cratère, nouvel anneau :
    par **deltas** (voxels, valeurs vivantes), sauvés et envoyés comme les cratères d'impact (Q2).
26. **Prévenir avant de frapper.** Tout événement dangereux est annoncé (scanner, signes visibles :
    secousses, fumée, mer qui se retire) au moins quelques minutes de jeu avant. Il ne frappe jamais un
    joueur posé sans annonce.
27. **Trouvables et mesurés.** Chaque archétype et chaque événement est cherchable (`/aller planete
    <archétype>`), compté dans `/stats`, enregistré dans le dex, déclenchable en test (`/evenement`),
    et un **test de fréquence** vérifie sa part dans la galaxie principale (ni absent, ni trop commun).
28. **Pas de noms d'œuvres.** Les idées viennent de films, séries et BD ; aucun nom ni forme reconnaissable
    d'une œuvre protégée : ce sont des archétypes génériques.

---

## 4. Bloc D — Le monde qui vit (bloc D de la 0.11, à l'échelle de la 0.13)

| Phase | Contenu | Fini quand | Taille |
|---|---|---|---|
| **D1. Eau vivante** | **Rivières** calculées depuis le relief (écoulement vers la mer, bassins hachés, largeur selon le débit), **lacs** dans les cuvettes, **cascades** sur les falaises, deltas et estuaires, glace qui fond et gèle selon la saison (A3), **banquise**, vagues sur les lacs selon le vent. Passent par `kind_at` (règle 16) ; les grandes rivières se voient de l'espace. `PROTOCOL` +1. | Une rivière descend toujours (test), rejoint la mer ou un lac ; pas de rivière coupée entre deux tuiles ; `bench_tiles` mesuré | XL |
| **D2. Géologie active** | Coulées de lave **lumineuses** (visibles la nuit), **geysers** (jets réguliers, f(horloge)), **cryovolcans** (type Encelade), fumerolles, sources chaudes, petits **séismes** (secousse, éboulis) selon l'activité de `geology.rs`. Le trait « geysers géants » devient réel. | Geysers synchronisés entre deux joueurs ; lave visible de l'orbite la nuit | M |
| **D3. Végétation vivante** | Arbres et herbe qui **bougent avec le vent** (rafales de `weather::wind`), plantes qui s'ouvrent le jour, **bioluminescence** la nuit, feuillage selon la **saison**, forêts denses en instances et LOD. Le trait « forêt pétrifiée » devient réel. | Forêt dense en Ultra sans baisse sous la cible de FPS (mesure) | L |
| **D4. Faune visible** | Créatures procédurales depuis `Fauna` (corps, pattes, ailes, nageoires) construites avec les **familles de l'éditeur** (squelettes et animations de `assets/editeur/races/`) : troupeaux, volants, aquatiques, fouisseurs, **diurnes / nocturnes**, fuite devant le joueur, pas de combat ; tailles réelles (`max_size_m`) à l'échelle k = 16. Le trait « océan bioluminescent » devient réel (plancton, méduses). Créatures = f(graine, cellule, horloge) : les mêmes pour tous les joueurs. | Deux joueurs voient le même troupeau au même endroit ; aucune créature dans le sol ou dans l'eau si elle ne nage pas | XL |
| **D5. Son** | `bevy_audio` (Q5), **sons du monde seulement, pas de musique** (Q8, plus tard) : vent, pluie, tonnerre, vagues, rivières et cascades, écho des grottes, lave, geysers, faune, moteurs et propulseurs du vaisseau, **silence dans le vide**, son étouffé sous l'eau, volume par catégorie dans Options. | Aucun son dans l'espace hors du vaisseau ; transitions sans coupure | L |
| **D6. Points d'intérêt** | Lieux rares au scanner et dans le dex : grottes géantes, arches, cratères géants, cascades, sources chaudes, geysers, épaves, ruines (fictif, étiqueté), **monolithe** (le trait devient réel), marqueurs posés par les joueurs (réseau). | `/aller` vers chaque sorte ; marqueurs vus par les autres joueurs | M |

---

## 5. Bloc X — Mondes exceptionnels

### 5.1 Le catalogue

Rareté : **N** = naturel (découle de la physique, pas de tirage), **P** = peu commun (1,5 %), **R** =
rare (0,4 %), **L** = légendaire (0,1 %). Réalisme : ré = réaliste, sp = spéculatif, fi = fictif.
Tous les tirés respectent la zone calme du départ (règle 23).

| Phase | Archétype | Rar. | Réal. | Ce qui change dans la génération | Ce qui change en jeu |
|---|---|---|---|---|---|
| **X1** | **Œil (verrouillé)** : face jour brûlée, face nuit gelée, **bande crépusculaire** habitable | N | ré | Climat selon l'angle au point sous l'étoile (et plus la latitude), calotte côté nuit, désert ou mer bouillante côté jour, vents permanents du jour vers la nuit | Coucher de soleil **éternel** dans la bande ; la vie (D3, D4) ne pousse que là ; traverser = survie (A4) |
| X1 | **Crépuscule étroit** (bande de quelques centaines de voxels, contraste extrême) | P | ré | Atmosphère fine, bande étroite | Route de la bande ; lieux d'intérêt alignés |
| X1 | **Saisons de siècles** (année très longue, forte inclinaison) | R | ré | Saison quasi figée, nuit polaire de plusieurs heures de jeu | Glaciers qui recouvrent des ruines, forêt fossile (D6) |
| X1 | **Jour sans fin** (rotation très lente, non synchrone) | R | ré | Jour au plafond de 3 h de jeu (A1), terminateur qui avance au pas | On peut **suivre le coucher du soleil** à pied |
| **X2** | **Monde-océan profond** (100 % eau) | P | ré | Aucune terre ; fond : canyons, volcans sous-marins, récifs géants | Plongée (0.13 O2), îles volcaniques rares |
| X2 | **Océan peu profond** (10 à 50 voxels d'eau partout) | P | ré | Mer juste au-dessus du relief | On voit le fond partout : récifs, montagnes sous-marines |
| X2 | **Océan sous la glace** (type Europe) | N / P | ré | Croûte de glace épaisse + océan liquide dessous (couche 3D), **crevasses**, geysers (D2) | Descendre par une crevasse ; vie bioluminescente sous la glace |
| X2 | **Monde-pluie** | P | ré | Pluie permanente, nuages partout, rivières (D1) partout | Visibilité faible, son (D5) |
| X2 | **Océan central** dans un continent annulaire, **maelström** au centre | R | sp | Masque de continent en anneau, tourbillon permanent | Le maelström aspire le vaisseau en vol bas |
| **X3** | **Monde en refroidissement** : une moitié en fusion, l'autre avec mers et premières plantes | R | sp | Deux couches (magma / croûte) séparées par une frontière qui avance avec l'horloge | La **frontière** : coulées, vapeur, terres nouvelles (deltas) |
| X3 | **Continents sur un océan de magma** | R | sp | Océan de magma + plaques de croûte | Lave lumineuse (D2), danger (A4) |
| X3 | **Volcan qui dépasse l'atmosphère** | P | ré | Bouclier géant (relief T1) jusqu'à la limite de l'atmosphère | Au sommet : ciel noir en plein jour |
| X3 | **Glace noire** à fissures lumineuses | R | sp | Glace très sombre et brillante, fissures émissives | Monde noir, lumière qui vient du sol |
| X3 | **Glace transparente** sur des centaines de voxels | R | sp | Glace translucide (rendu de l'eau de la 0.13) | Marcher sur une vitre au-dessus du vide ou de l'océan |
| **X4** | **Lune trop proche / planète étirée** | R | ré | Lune près de la **limite de Roche** : planète allongée vers la lune (forme non sphérique : terrain **et** maillage lointain), **bourrelets d'océan permanents**, marées de dizaines de voxels, chaleur de marée | La lune **remplit le ciel** ; la mer avance et recule à vue d'œil |
| X4 | **Lunes en résonance** (2 à 4 lunes) | P | ré | Marées de plusieurs astres (existe), amplitudes plus fortes, cycles longs | Calendrier des grandes marées au scanner, côtes changeantes |
| X4 | **Anneaux bas** | R | ré | Anneau juste au-dessus de l'atmosphère, ombre en bande sur la planète | Arche immense dans le ciel, bande d'ombre permanente |
| X4 | **Planète après collision** : moitié intacte, moitié océan de magma, anneau de débris | L | ré | Bassin d'impact géant + magma + anneau | Débris qui tombent (si planète active, N2) |
| **X5** | **Tempête géante** (30 % du globe, œil, murs de nuages) | P | ré | Système dépressionnaire fixe dans `weather.rs`, visible de l'espace (le trait « super-tempête » devient réel) | Vents destructeurs, éclairs, l'œil est calme |
| X5 | **Supercellules** | R | ré | Orages partout, tornades | Éclairs dangereux pour le vaisseau |
| X5 | **Monde électrique** | R | sp | Arcs entre nuages, montagnes et mer | Le vaisseau posé sur un sommet attire la foudre |
| X5 | **Aurores géantes** (champ magnétique très fort) | P | ré | Aurores jusqu'à l'équateur, de jour comme de nuit | Ciel animé permanent, boussole folle |
| X5 | **Planète dans une nébuleuse** | R | ré | Système dans un nuage (`galaxy_fx.rs`), skybox colorée (0.13 C2) | Nuits presque aussi claires que les jours |
| X5 | **Planète autour d'un trou noir** | L | sp | Système centré sur un trou noir (0.13 V1) avec une étoile compagne | Disque et lentille dans le ciel ; temps « ralenti » à l'affichage (Q6) |
| **X6** | **Monde cristallin** / **cristaux chantants** (le trait devient réel) | R / L | sp / fi | Croûte et montagnes de cristal translucide, cristaux géants hachés par cellule | Reflets, sons (D5) |
| X6 | **Monde de diamant** (riche en carbone) | R | sp | Composition carbone (`resources.rs`), cavernes de diamant | Gisements profonds (minage en 0.15) |
| X6 | **Monde métallique** / **noyau exposé** (le trait devient réel) | R | sp | Surface métallique, lacs de métal fondu côté chaud | Tempêtes magnétiques, boussole inutile |
| X6 | **Monde vertical** (falaises partout) | R | sp | Relief T1 / T2 poussé à l'extrême : plateaux étagés séparés de parois | Vivre par niveaux ; vol bas obligatoire |
| X6 | **Forêt géante** (arbres de centaines de voxels) | R | sp | Décor géant (D3) à étages : sol sombre, troncs, canopée | Marcher sur la canopée |
| X6 | **Monde fongique** (champignons géants, spores, ciel violet) | R | sp | Biome spores étendu, brume de spores (0.13 C1) | Faune (D4) étrange |
| X6 | **Monde cubique** | L | fi | Relief quantifié en gros cubes, falaises à angle droit | Panorama unique |
| **X7** | **Planète cassée** (morceaux tenus ensemble) | L | fi | Fragments séparés (formes de `asteroids.rs`, terrain sur chaque morceau) | Voler d'un fragment à l'autre, ponts naturels |
| X7 | **Planète creuse** (monde intérieur, soleil intérieur) | L | fi | Croûte + **surface intérieure** (sphère inversée), soleil central, gouffres polaires | Deux mondes en un |
| X7 | **Océan suspendu** (le trait « anomalie gravitationnelle » devient réel) | L | fi | Couche d'eau en altitude, cascades qui tombent dans le vide | Voler sous une mer |
| X7 | **Géante gazeuse habitable** (îles flottantes) | R | sp | Îles volantes et créatures planantes à l'altitude respirable | Se poser sur une île dans les nuages |
| **X8** | **Monde mort** (le trait « ruines » devient réel) | R | fi | Villes en ruines, routes, statues (outils des aliens de la 0.13 L5) | Exploration, épaves (D6) |
| X8 | **Nature reconquise** | R | fi | Ruines + forêt dense (D3) | Tours qui dépassent des arbres |
| X8 | **Monde dévasté** (radioactif, villes vitrifiées) | R | fi | Biomes de verre et de cendres, radiation (A4) | Combinaison obligatoire |
| X8 | **Terraformation inachevée** | P | sp | Mers géométriques, montagnes trop régulières, zones mortes (valeurs vivantes, 0.13 L6) | Reprendre la terraformation |
| X8 | **Monde-observatoire** / **monolithe** | L | fi | Structures alignées sur une étoile ou une galaxie | Énigme : trouver ce qu'elles regardent |
| X8 | **Planète-machine** / **monde-ville** | L | fi | Surface artificielle (blocs, tours, canyons urbains) | Lumières la nuit visibles de l'espace |
| X8 | **Planète vivante** | L | fi | Montagnes « os », rivières « veines » qui battent, grottes « organes » | Le terrain réagit au joueur |
| X8 | **Écho temporel** (le trait devient réel) | L | fi | Vallée où la croissance va plus vite (Q6) | Revenir après une heure : la vallée a changé |
| X8 | **Planète qui évolue** | R | sp | Stade de la vie = f(horloge) | Revenir des jours plus tard : un autre monde |

Traits actuels sans archétype : **pluie de diamants** (X5, géantes), **glace superionique** (X3, glace
noire profonde), **lacs de lave actifs** (D2), **cratère d'impact géant** (X4, collision), **champ
inversé** (X5, aurores ; N3), **arches** (déjà réelles). Aucun ne reste une simple étiquette (règle 20).

### 5.2 Les phases du bloc X

| Phase | Contenu | Fini quand | Taille |
|---|---|---|---|
| **X0. Système d'archétypes** | `planetgen/archetypes.rs` (règles 20, 21, 23, 27), `Layer::Archetype`, tirage par rareté et conditions, effets en paramètres passés aux couches. Traits actuels reliés à leur archétype. Scanner : section « Anomalie » ; dex ; `/aller planete <archétype>` ; `/stats` ; **signal d'anomalie** visible de loin (cercle de couleur au zoom système). **Zone calme** des 50 systèmes. `PROTOCOL` +1. | Test : aucun tiré dans les 50 premiers systèmes ; parts dans la galaxie principale proches des raretés ; `bench_tiles` d'un astre ordinaire inchangé | L |
| **X1. Lumière et rotation** | Œil verrouillé, crépuscule étroit, saisons de siècles, jour sans fin. | Le climat de la bande est habitable sur un œil tempéré ; captures de la bande | L |
| **X2. Mondes d'eau** | Océan profond, peu profond, sous la glace, monde-pluie, océan central et maelström. | On descend par une crevasse jusqu'à l'océan sous la glace | L |
| **X3. Feu et glace** | Monde en refroidissement, continents sur magma, volcan géant, glace noire, glace transparente. | Frontière du refroidissement qui avance entre deux visites | L |
| **X4. Lunes, marées, anneaux** | Planète étirée (terrain, maillage lointain, collisions et gravité sur une forme non sphérique), lunes en résonance, anneaux bas, planète après collision. | Marcher et se poser partout sur une planète étirée ; marée visible en quelques minutes | XL |
| **X5. Ciels extrêmes** | Tempête géante, supercellules, monde électrique, aurores géantes, nébuleuse, trou noir. | Tempête visible de l'espace ; l'œil est calme au sol | L |
| **X6. Croûtes étranges** | Cristal, diamant, métal, vertical, forêt géante, fongique, cubique. | Chaque monde reconnaissable de l'orbite (captures) | L |
| **X7. Mondes impossibles** | Planète cassée, creuse, océan suspendu, géante gazeuse habitable. | Entrer dans la planète creuse et en ressortir ; voler entre deux fragments | XL |
| **X8. Vestiges et mondes vivants** | Monde mort, nature reconquise, dévasté, terraformation inachevée, observatoire, machine, planète vivante, écho temporel, planète qui évolue. | Une planète qui évolue a changé de stade entre deux visites | XL |

---

## 6. Bloc N — Événements (rares, sur peu de planètes)

### 6.1 Qui, combien, quand

- **Planètes actives** : seules elles ont des événements (règle 24). Condition physique (volcanisme pour
  N1, lunes ou anneaux pour N2 et N4, étoile active pour N3) **et** tirage **très rare** (Q7) : environ
  **1 planète ou lune solide sur 1 000**. Le joueur doit avoir de la **chance** pour tomber sur une
  planète active au bon moment. Le test de fréquence vérifie qu'il y en a quelques-unes dans la galaxie
  principale (hors zone calme), jamais une par système.
- **Rythme** : sur une planète active, **environ un événement par heure de jeu** en moyenne (Q3), tiré
  par tranches de temps comme les orages de `sky.rs`. Les plus destructeurs (impact géant,
  mégatsunami, lune qui se brise) sont réservés à une partie des planètes actives et bien plus rares.
- **Aucun événement** dans les 50 systèmes du départ (même zone calme que les archétypes, règle 23).
- Scanner : « Planète active » + **prochain événement prévu** (dans X min) quand on en approche.

### 6.2 Les phases

| Phase | Événements | Signes avant | Ce qui dure (deltas, Q2) | Taille |
|---|---|---|---|---|
| **N1. Volcans et séismes** | **Éruption** (selon `geology.rs`) : panache, bombes, coulée qui suit la vraie pente ; **séisme fort** avec éboulements | Secousses, fumée, ciel cendré | Coulée figée qui remplit une vallée, **nouvelle île**, rivière déviée | L |
| **N2. Ce qui tombe du ciel** | Impact complet : bolide → impact → **onde de choc** → incendies → **hiver d'impact** (ciel sombre, froid) ; **mégatsunami** (impact en mer) ; **effondrement d'anneau** (pluie de météores pendant des heures) ; **lune qui se brise** (devient anneau, morceaux qui tombent) | Le bolide repéré au scanner, la mer qui se retire | Cratère (comme B5), côtes rasées, nouvel anneau (valeurs vivantes) | XL |
| **N3. Étoile et champ magnétique** | **Éruption solaire majeure** (au-delà des orages de C4) : aurores partout, radiation (A4), **pannes** du vaisseau (HUD et scanner brouillés), navigation dangereuse ; **inversion des pôles** : aurores à l'équateur, boussole inversée, migrations (D4) | Alerte au scanner, ciel en feu | Boussole inversée jusqu'à la fin de l'inversion | M |
| **N4. Marées et temps long** | **Grandes marées de résonance** (X4), **saisons de siècles** qui avancent (X1), **dérive continentale accélérée** (fictif, légendaire), **planète qui évolue** (X8) | Calendrier au scanner | Côtes, glaciers, stade de la vie | L |

Test commun : `/evenement <type>` (hôte, comme `/impact` et `/eclipse`) déclenche l'événement près du
joueur ; deux joueurs voient le même événement au même moment et les mêmes deltas après.

---

## 7. Ordre et versions

```
0.13 terminée
   │
   D1 → D2 → D3 → D4 → D5 → D6     le monde qui vit
   │                               ── release 0.14.0 « Vivant » ──
   X0 → X1 → X2 → X3 → X4          archétypes (X2 a besoin de D1, X4 des anneaux et des marées)
   │  → X5 → X6 → X7 → X8
   │                               ── release 0.14.1 « Extraordinaire » ──
   N1 → N2 → N3 → N4               événements (N1 a besoin de D2, N2 et N4 de X4)
                                   ── release 0.14.2 « Événements » ──
   │
   0.15 : minage et destruction
```

Dépendances : X2 utilise les rivières (D1) et l'eau de la 0.13 ; X4 et N2 réutilisent anneaux,
astéroïdes et marées ; X8 réutilise les bâtiments des aliens (0.13 L5) et la terraformation (0.13 L6) ;
X5 utilise la skybox et les trous noirs de la 0.13 (C2, V1) ; D4 utilise les familles de l'éditeur.

---

## 8. Risques

| Risque | Parade |
|---|---|
| **Coût** des archétypes sur toutes les planètes | Règle 21 : un astre ordinaire ne calcule rien de plus ; mesure par archétype |
| **Formes non sphériques** (planète étirée, cassée, creuse) : terrain, collisions, gravité, LOD supposent une sphère | Isoler la forme dans une fonction `shape(dir)` utilisée partout ; X4 d'abord, X7 ensuite |
| **Réseau** : événements et faune doivent être identiques chez tous | Tout = f(graine, cellule, horloge) ; deltas pour ce qui dure ; tests à deux joueurs |
| **Sauvegardes** : beaucoup de deltas après des heures d'événements | Deltas par bloc (`voxel.rs`), compressés ; mesure de la taille de `world.json` |
| **Trop ou pas assez de mondes rares** | Test de fréquence (règle 27), raretés réglables |
| **Taille de la 0.14** | Trois releases ; chaque phase jouable seule |

---

## 9. Questions restantes

Aucune pour l'instant : Q1 à Q8 sont tranchées (§1.2).

---

## 10. Prompts (à coller dans une nouvelle session, un par phase)

Contexte commun : « Lis `ROADMAP-0.14.md`, `prompt0.14.md`, `ROADMAP-0.13.md` et `CLAUDE.md` (règles 1 à
28, décisions Q1 à Q8 du §1.2). `git pull origin main` avant de coder. Branche `claude/roadmap-0-14-<phase>`.
Build release, tests, mesures avant / après et captures. PR non fusionnée (je dirai « push main »).
Aucune concession sur la qualité ; ne change ni les tailles ni les décisions sans me demander. »

- **D1** — « [contexte commun] Phase D1 : rivières depuis le relief, lacs, cascades, deltas, glace
  saisonnière, banquise ; visibles de l'espace ; PROTOCOL +1. »
- **D2** — « [contexte commun] Phase D2 : lave lumineuse, geysers, cryovolcans, fumerolles, sources
  chaudes, séismes ; trait « geysers géants » réel. »
- **D3** — « [contexte commun] Phase D3 : végétation au vent, jour / nuit, bioluminescence, saisons,
  forêts en instances ; trait « forêt pétrifiée » réel. »
- **D4** — « [contexte commun] Phase D4 : faune procédurale avec les familles de l'éditeur, f(graine,
  cellule, horloge), troupeaux, volants, aquatiques, fuite ; trait « océan bioluminescent » réel. »
- **D5** — « [contexte commun] Phase D5 : son avec `bevy_audio` (Q5), sons du monde sans musique (Q8),
  silence dans le vide, volumes. »
- **D6** — « [contexte commun] Phase D6 : points d'intérêt au scanner et dans le dex ; monolithe réel ;
  marqueurs des joueurs en réseau. »
- **X0** — « [contexte commun] Phase X0 : `planetgen/archetypes.rs` (règles 20, 21, 23, 27), traits reliés
  à leur archétype, zone calme des 50 systèmes du départ, scanner, dex, `/aller`, `/stats`, signal
  d'anomalie, test de fréquence ; PROTOCOL +1. »
- **X1** — « [contexte commun] Phase X1 : œil verrouillé et sa bande crépusculaire, crépuscule étroit,
  saisons de siècles, jour sans fin. »
- **X2** — « [contexte commun] Phase X2 : mondes-océans (profond, peu profond, sous la glace), monde-pluie,
  océan central et maelström. »
- **X3** — « [contexte commun] Phase X3 : monde en refroidissement, continents sur magma, volcan géant,
  glace noire, glace transparente. »
- **X4** — « [contexte commun] Phase X4 : forme non sphérique `shape(dir)`, planète étirée par une lune
  près de la limite de Roche, lunes en résonance, anneaux bas, planète après collision. »
- **X5** — « [contexte commun] Phase X5 : tempête géante, supercellules, monde électrique, aurores
  géantes, planète dans une nébuleuse, planète autour d'un trou noir (Q6). »
- **X6** — « [contexte commun] Phase X6 : mondes de cristal, diamant, métal, vertical, forêt géante,
  fongique, cubique. »
- **X7** — « [contexte commun] Phase X7 : planète cassée, creuse, océan suspendu, géante gazeuse
  habitable (Q1). »
- **X8** — « [contexte commun] Phase X8 : monde mort, nature reconquise, dévasté, terraformation
  inachevée, observatoire, machine, planète vivante, écho temporel, planète qui évolue. »
- **N1** — « [contexte commun] Phase N1 : planètes actives très rares (§6.1, Q7), éruptions et séismes annoncés,
  conséquences en deltas, `/evenement`. »
- **N2** — « [contexte commun] Phase N2 : impact complet, hiver d'impact, mégatsunami, anneau qui
  s'effondre, lune qui se brise. »
- **N3** — « [contexte commun] Phase N3 : éruption solaire majeure (pannes, radiation), inversion des
  pôles. »
- **N4** — « [contexte commun] Phase N4 : grandes marées de résonance, saisons de siècles, dérive
  continentale, planète qui évolue. »
