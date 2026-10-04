# Feuille de route — Temps et profondeur (0.11)

Objectif : les mondes de la 0.10 sont **cohérents mais figés**. La 0.11 les fait **vivre dans le temps**
(rotation, jour/nuit, saisons, météo, ciel) et leur donne **de la profondeur** (terrain en voxels 3D,
grottes, vraies montagnes, cratères), puis remplit l'espace entre les planètes (ceintures d'astéroïdes,
anneaux, comètes, étoiles multiples).

Chaque phase se termine par un build instable jouable. Chaque phase a son **prompt prêt à coller**
(section « Prompts »). Une PR par phase, non fusionnée : tu testes, puis tu dis « push main ».

---

## Décisions prises (rappel de la 0.10)

| Sujet | Décision |
|---|---|
| Jour / nuit | **0.11**, avec variation de température entre le jour et la nuit (et selon les saisons). |
| Grottes | **0.11**, terrain en **voxels 3D** près du joueur. |
| Géantes gazeuses | On peut y entrer, le vaisseau perd des PV, explose à 0 PV (fait en 0.10). |
| Compatibilité | Acceptée : prototype, 2 joueurs de test. Nouveau `PROTOCOL` à chaque changement de génération. |
| Minage / destruction | **0.14**. Les voxels 3D de la 0.11 doivent déjà enregistrer des **deltas** (règle 7). |
| Tailles | Aucun compromis sur les tailles : rien de cette feuille de route ne réduit une distance ou un rayon. |

## Décisions 0.11 (réponses du 02/10/2026)

| # | Sujet | Décision |
|---|---|---|
| Q1 | **Vitesse du temps** | **×60** : 1 h réelle d'une planète = 1 min de jeu → jour terrestre = **24 min**. Les durées restent proportionnelles (une planète à 10 h de rotation = 10 min). Plafond : un jour ne dépasse pas **3 h** de jeu (sauf rotation synchrone = jour éternel). |
| Q2 | **Durée des saisons** | Chaque planète garde sa propre année (proportionnelle à sa vraie période orbitale), mais l'échelle est réglée pour qu'**en moyenne une saison dure 1 h de jeu** (année moyenne = 4 h). Une planète proche de son étoile a des saisons plus courtes, une lointaine plus longues (bornes : 10 min à 6 h par saison). Indépendant de la vitesse des jours (Q1) et des orbites affichées. Sans inclinaison axiale ni excentricité : pas de saisons. |
| Q3 | **Style des voxels 3D** | **Cubes**, comme le terrain actuel (plus simple à miner en 0.14). |
| Q4 | **Profondeur des grottes** | Jusqu'à **2 000 unités** sous la surface. |
| Q5 | **Dangers à pied** | **Oui** : combinaison (oxygène, température, pression, radiation), dégâts lents, jamais de mort instantanée. |
| Q6 | **Collisions avec les astéroïdes** | **Oui**, dégâts selon la vitesse ; les petits cailloux rebondissent sans dégât. |
| Q7 | **Découpage** | **Tout est dans la 0.11** (blocs A, B, C et D). Pas de 0.12 / 0.13 pour ces sujets. |

---

## Point de départ (code actuel, v0.10.0)

| Domaine | Aujourd'hui | Fichier |
|---|---|---|
| Rotation | `rotation_period_h`, `axial_tilt_deg`, `tidally_locked` **calculés mais inutilisés** : les planètes ne tournent pas sur elles-mêmes (seuls les nuages tournent). | `planetgen/profile.rs`, `planet.rs` (`rotate_clouds`) |
| Température | `Climate::temperature(lat, alt, Some(Moment { hour, season }))` existe déjà (amplitude jour/nuit `diurnal`, inclinaison `tilt`) mais personne ne passe de `Moment`. | `planetgen/climate.rs` |
| Horloge | Pas d'horloge du monde : les orbites avancent avec le temps réel de la session. | `kepler.rs`, `planet.rs` |
| Lumière | `PointLight` sur l'étoile (lumière réelle), ombres optionnelles, ciel bleu si atmosphère, couleurs de coucher calculées en phase 3. | `planet.rs`, `surface.rs`, `graphics.rs` |
| Terrain | **Champ de hauteur** : quadtree de tuiles 32×32 colonnes sur une sphère-cube, voxel ≤ 11 unités, jupes et murs. Pas de surplomb, pas de dessous. | `terrain.rs` |
| Relief | `ReliefField` : montagnes, rifts, volcans, canyons, plateaux, cratères — doux, issus de la géologie (phase 5). `craters` = âge de la surface. | `planetgen/geology.rs`, `terrain.rs` |
| Décor | 17 sortes (arbres, rochers, cristaux, champignons, pics de glace…) posées sur le terrain. | `decor.rs` |
| Ceintures | ≤ 500 **cubes** par ceinture, tournant d'un bloc (`rotate_y(0,005)`). Pas de forme, de composition ni de collision. | `planet.rs` (`AsteroidBeltRoot`, ~l.1430) |
| Code ancien réutilisable | Astéroïdes de formes variées (tas de gravats, binaires de contact, métalliques, `crater_deform`), comètes. Désactivés. | `astre/planete/meteoroid.rs`, `astre/planete/comet.rs` |
| Étoiles multiples | `StarSystemConfig.stars` est déjà un `Vec`, mais un seul soleil est généré. | `settings.rs`, `planetgen/system.rs` |
| Vie | Faune en **paramètres seulement** (`Fauna`). | `planetgen/life.rs` |
| Aurores, anneaux | Aurores animées (`shimmer_auroras`) ; anneaux = trait (`ring`) sans rendu dédié. | `planet.rs`, `planetgen/traits.rs` |
| Réseau | `PROTOCOL = 15`. | `net.rs` |

---

## Règles d'architecture

Les **règles 1 à 8 de `ROADMAP-0.10.md`** restent valables (génome compact, sous-graines par couche,
couches dérivées, unités réelles, réaliste/spéculatif/fictif, empreinte multijoueur, valeurs vivantes,
origine flottante). La 0.11 en ajoute cinq :

9. **Une seule horloge du monde.** `WorldClock` : secondes (f64) depuis la création du monde, enregistrée
   dans `world.json`, donnée par l'hôte en multijoueur. **Tout ce qui bouge est une fonction de
   `(graine, temps)`** : rotation, orbites, saisons, météo, comètes, pluies de météores. Le réseau ne
   transmet que l'horloge, jamais la météo ni la position d'un nuage.
10. **Repère fixe de l'astre.** Tout ce qui est posé sur un astre (joueur, vaisseau posé, décor, grottes,
    deltas) est stocké en coordonnées **fixes de l'astre** (qui tournent avec lui), jamais en monde. La
    rotation n'est qu'une transformation au rendu, comme l'origine flottante.
11. **Une seule fonction de densité.** Le voxel 3D vaut `densité(graine, point fixe de l'astre) + delta`.
    Le champ de hauteur lointain est **dérivé de la même fonction** (pas de saut entre loin et près).
    Les deltas (creusé / ajouté) sont prévus dès maintenant, par bloc, pour la 0.14.
12. **Pas de liste globale.** Cratères, grottes, astéroïdes, rochers : **hachés par cellule** (la cellule
    donne sa graine), générés à la demande, oubliés quand on s'éloigne. Rien ne grossit avec la taille
    de la galaxie.
13. **Budget.** Objectif ≥ 120 FPS sur la machine de test, génération **hors du fil principal**
    (`AsyncComputeTaskPool`), mémoire mesurée à chaque phase (avant / après dans la PR).

---

## Versions et phases

### Bloc A — Le temps (0.11)

| Phase | Contenu | Taille |
|---|---|---|
| A1. Horloge et rotation | `WorldClock` (sauvegarde, réseau, commande `/heure` et `/temps x10` en test), vitesse ×60 (Q1), rotation des planètes et lunes autour de leur axe incliné, rotation synchrone (face jour éternelle), orbites calculées depuis l'horloge. Joueur, vaisseau posé et décor dans le **repère fixe de l'astre** (règle 10). | L |
| A2. Jour / nuit | Lumière selon l'heure locale, aube et crépuscule (couleurs de la phase 3), ciel qui s'assombrit, **ciel de nuit** (étoiles, galaxie, autres planètes et lunes visibles depuis le sol, avec leurs phases), lueur de la lune, ligne jour/nuit visible depuis l'espace, **phares du vaisseau** et **lampe du joueur**, lumières de ville éventuelles (rien pour l'instant). | L |
| A3. Température vivante et saisons | `Moment { heure, saison }` passé partout : température qui suit le soleil (avec retard l'après-midi), saisons (inclinaison + excentricité, Q2), calottes et neige qui avancent et reculent, givre le matin, HUD température / heure / saison, scanner mis à jour (min / max du jour). | M |
| A4. Survie à pied | Combinaison : oxygène, température, pression, radiation (étoile active, pas de champ magnétique), alertes, dégâts lents (Q5). Abri = vaisseau. | M |

### Bloc B — La profondeur (0.11)

| Phase | Contenu | Taille |
|---|---|---|
| B1. Voxels 3D | Blocs 3D (32³) près du joueur, champ de hauteur au loin, même fonction de densité (règle 11), raccord sans fissure, collisions du marcheur et du vaisseau en 3D, génération asynchrone, **format des deltas** prêt pour la 0.14 (rien n'est encore creusé). Cubes (Q3). | XL |
| B2. Grottes | Réseaux selon la géologie : **tubes de lave** (volcanisme), **karst** (eau + roche calcaire), **grottes de glace** (mondes gelés), **géodes de cristaux** (rare), failles. Entrées visibles en surface, salles, puits, rivières et lacs souterrains, obscurité totale (lampe), stalactites / stalagmites, champignons lumineux si vie, minerais plus riches en profondeur (phase 8). Profondeur max Q4. | L |
| B3. Montagnes et falaises | Chaînes le long des limites de plaques (bruit « crêtes »), vrais sommets, **falaises verticales**, **surplombs et arches** (grâce au 3D), canyons profonds, mesas, cols, éboulis au pied des pentes, neiges éternelles selon l'altitude, érosion (pentes adoucies sur les mondes à air et à eau). Volcans : cône, cratère sommital, coulées figées. | L |
| B4. Cratères | Formes réelles selon la taille : **cuvette simple**, **cratère complexe** (pic central, terrasses), **bassin à anneaux**. Tailles en loi de puissance, nombre selon l'âge de la surface, rebords, **éjectas** et rayons clairs (récents), cratères érodés, remplis d'eau / de lave / de glace, **cratères emboîtés**. Hachés par cellule (règle 12). | M |
| B5. Météores et impacts | Étoiles filantes la nuit (atmosphère), **pluies de météores** périodiques (passage dans le sillage d'une comète), bolides, impacts rares visibles qui créent un **nouveau cratère enregistré en delta** (premier vrai test des deltas). Sans air : impacts sans traînée. | M |

### Bloc C — Le ciel et l'espace vivants (0.11)

| Phase | Contenu | Taille |
|---|---|---|
| C1. Ceintures d'astéroïdes | Ceintures générées par `planetgen` (place logique : entre rocheuses et géantes, ceinture glacée externe type Kuiper), masse, largeur, densité. **Vrais astéroïdes** : formes irrégulières (code de `meteoroid.rs` : tas de gravats, binaires de contact, métalliques), types C / S / M (lien ressources phase 8), rotation propre, orbites (plus de bloc qui tourne). Streaming par cellule, champs denses navigables, **collisions** (Q6), **se poser** sur les gros (microgravité, voxels 3D). | XL |
| C2. Anneaux, Troyens, comètes | **Anneaux** planétaires (glace / roche, divisions, ombre sur la planète, traversables en particules), **Troyens** aux points L4/L5 des géantes, **comètes** (code de `comet.rs`) sur orbites très excentriques, **queue** de gaz et de poussière opposée à l'étoile, qui grandit près de l'étoile. Planètes errantes rares entre les étoiles. | L |
| C3. Étoiles doubles et triples | Systèmes multiples réalistes (~1/3 des étoiles) : orbites autour d'une étoile (type S) ou des deux (type P), zone habitable recalculée, deux soleils et **deux ombres**, double coucher, éclairage combiné. | L |
| C4. Phénomènes du ciel | **Éclipses** (solaires, lunaires, ombre portée visible), **phases des lunes**, **marées** (niveau de la mer qui monte et descend avec les lunes et le soleil), **aurores** la nuit selon le champ magnétique et l'activité de l'étoile, éruptions solaires visibles. | M |
| C5. Météo | Nuages qui se forment et se déplacent avec les vents (phase 3), **pluie**, **neige**, grêle, **orages et éclairs**, **tempêtes de poussière**, **brouillard** du matin, vents qui poussent le vaisseau. **Pluies exotiques** : méthane (Titan), acide sulfurique (Vénus), verre, fer. Météo = f(graine, temps, lieu) (règle 9). | L |

### Bloc D — La vie et l'ambiance (0.11)

| Phase | Contenu | Taille |
|---|---|---|
| D1. Eau vivante | Rivières (écoulement calculé grossièrement depuis le relief), lacs, **cascades**, glace qui fond / gèle selon la saison, banquise, vagues selon le vent. | L |
| D2. Géologie active | Coulées de lave lumineuses (visibles la nuit), **geysers**, **cryovolcans** (type Encelade), fumerolles, petits **séismes** (secousse) selon l'activité. | M |
| D3. Faune visible | Créatures procédurales depuis `Fauna` (corps, pattes, ailes, nageoires), troupeaux, volants, aquatiques, **diurnes / nocturnes** (lien jour/nuit), fuite devant le joueur. Pas de combat. | L |
| D4. Végétation vivante | Arbres et herbe qui bougent avec le vent, plantes qui s'ouvrent le jour, bioluminescence la nuit, feuillage selon la saison. | M |
| D5. Son | Vent, pluie, tonnerre, écho des grottes, lave, faune, silence dans le vide. | M |
| D6. Points d'intérêt | Lieux rares à trouver (scanner) : grottes géantes, arches, cratères géants, sources chaudes, épaves, ruines (fictif, étiqueté), marqueurs posés par les joueurs. | M |

### Ordre

Tout dans la **0.11** : **A1 → A2 → A3 → B1 → B2 → B3 → B4 → A4 → B5 → C1 → C2 → C3 → C4 → C5 → D1 →
D2 → D3 → D4 → D5 → D6**, puis la 0.14 (minage et destruction).

- A1 d'abord : tout le reste dépend de l'horloge et du repère fixe de l'astre.
- B1 avant toute la profondeur : grottes, surplombs et cratères en 3D en ont besoin.
- A4 (survie) après les grottes : on y teste l'obscurité, le froid et l'oxygène.
- C1 (ceintures) après B1 : se poser sur un astéroïde utilise les voxels 3D de B1.
- C3 (étoiles multiples) après C1–C2 : les ceintures et comètes doivent déjà gérer un seul soleil.
- La faune (D3) après la météo et le jour/nuit : son comportement en dépend.

### 0.14 — Minage et destruction (rappel)
- Creuser / poser des voxels avec les deltas de B1, ressources de la phase 8, astéroïdes minables (C1).
- Destruction de planètes : masse → gravité, atmosphère, orbite, marées recalculées ; débris → anneaux (C2).

---

## Risques connus

| Risque | Parade |
|---|---|
| Joueur qui glisse ou tremble sur une planète qui tourne | Règle 10 : tout est fixe de l'astre ; test « 10 min debout sans bouger d'un voxel ». |
| Fissures entre champ de hauteur et voxels 3D | Même fonction de densité (règle 11) ; test de raccord sur les 6 faces. |
| Chute des FPS (3D, météo, astéroïdes) | Génération asynchrone, budget par image, mesures dans chaque PR (règle 13). |
| Désynchronisation multijoueur | Seule l'horloge est transmise ; test « deux machines, même graine, même heure → même météo et mêmes astéroïdes ». |
| Saisons invisibles | Q2 : 1 h de jeu par saison en moyenne ; commande `/temps` pour tester. |

---

## Prompts (à me coller au début de chaque phase)

Colle **un prompt à la fois**. Je fais une PR par phase, sans la fusionner : tu testes, puis tu dis
« push main ».

> **Contexte commun** (à mettre en tête de chaque prompt) :
> « Lis `ROADMAP-0.11.md`, `ROADMAP-0.10.md` et `CLAUDE.md`. On travaille sur la 0.11, en respectant les
> règles d'architecture 1 à 13. Fais `git pull origin main` avant de coder. Montre-moi en jeu (captures)
> ce qui change, mesure FPS et mémoire avant / après, ajoute des tests, ouvre une PR non fusionnée. Ne
> change pas les tailles ni les décisions de la feuille de route sans me demander. »

### A1 — Horloge et rotation
« [contexte commun] Phase A1. Crée `WorldClock` (secondes f64 depuis la création du monde), enregistrée
dans `world.json`, donnée par l'hôte en multijoueur (nouveau `PROTOCOL`). Vitesse du temps selon la
décision Q1. Fais tourner planètes et lunes autour de leur axe incliné avec `rotation_period_h` et
`axial_tilt_deg` ; rotation synchrone pour `tidally_locked`. Les orbites se calculent depuis l'horloge.
Le joueur, le vaisseau posé et le décor vivent dans le repère fixe de l'astre (règle 10) : test « 10 min
debout sans bouger », test vol bas sans tremblement. Ajoute `/heure` (affiche heure locale et saison) et
`/temps <facteur>` pour accélérer en test. »

### A2 — Jour / nuit
« [contexte commun] Phase A2. Lumière selon l'heure locale de l'astre : aube, jour, crépuscule (couleurs
de coucher de la phase 3), nuit. Ciel de nuit depuis le sol : étoiles, galaxie, autres planètes et lunes
visibles avec leurs phases. Lueur de la lune. Ligne jour/nuit visible depuis l'espace. Phares du
vaisseau et lampe du joueur (touche libre, AZERTY). Captures : midi, coucher, minuit, lever, et une
planète en rotation synchrone (face jour, face nuit, terminateur). »

### A3 — Température vivante et saisons
« [contexte commun] Phase A3. Passe un `Moment { heure, saison }` à `Climate::temperature` partout où
on lit une température. La température suit le soleil avec un retard (max vers 14–15 h), les saisons
viennent de l'inclinaison et de l'excentricité (vitesse selon Q2). Neige et calottes avancent et reculent,
givre le matin. HUD : heure, saison, température locale. Scanner : min / max du jour et de l'année. Tests :
Terre (≈ 10 °C d'écart jour/nuit), Lune (≈ 250 °C d'écart), Vénus (presque rien). »

### B1 — Voxels 3D
« [contexte commun] Phase B1. Remplace le champ de hauteur près du joueur par des blocs voxel 3D (32³),
en cubes (Q3). Une seule fonction de densité (règle 11) ; le champ de hauteur lointain en est dérivé, sans
fissure au raccord. Collisions du marcheur et du vaisseau en 3D, génération asynchrone, LOD. Prépare le
format des deltas par bloc (sauvegarde + réseau) pour la 0.14, sans outil de creusage. Pour le test,
ajoute un seul surplomb artificiel visible. Tests : raccord sur les 6 faces, même densité sur deux
machines, FPS ≥ 120 au sol. »

### B2 — Grottes
« [contexte commun] Phase B2. Génère des grottes dans les voxels 3D selon la géologie : tubes de lave,
karst, grottes de glace, géodes de cristaux (rare), failles. Entrées visibles en surface, salles, puits,
rivières et lacs souterrains, stalactites / stalagmites, champignons lumineux si vie, minerais plus riches
en profondeur (phase 8). Profondeur max selon Q4. Hachées par cellule (règle 12). Ajoute au scanner
« grotte la plus proche ». Captures : une grotte de chaque type. »

### B3 — Montagnes et falaises
« [contexte commun] Phase B3. Refais les montagnes : chaînes le long des plaques, sommets nets, falaises
verticales, surplombs et arches, canyons profonds, mesas, cols, éboulis, neiges éternelles selon
l'altitude, érosion selon l'air et l'eau. Volcans avec cône, cratère sommital et coulées figées. Garde la
couverture des océans de la phase 4 (test existant). Captures : chaîne jeune, chaîne érodée, falaise,
arche, volcan. »

### B4 — Cratères
« [contexte commun] Phase B4. Cratères réels selon la taille : cuvette simple, complexe (pic central,
terrasses), bassin à anneaux. Tailles en loi de puissance, nombre selon l'âge de la surface (`craters`),
rebords, éjectas et rayons clairs pour les récents, érosion sur les mondes à air, remplissage eau / lave /
glace, cratères emboîtés. Hachés par cellule (règle 12). Captures : Lune, Mars, Terre (presque aucun). »

### A4 — Survie à pied
« [contexte commun] Phase A4. Combinaison du joueur : oxygène, température, pression, radiation (étoile
active sans champ magnétique). Alertes, dégâts lents (Q5), le vaisseau sert d'abri et recharge. HUD
simple. Tests : la nuit lunaire fait perdre de la vie, une planète tempérée non. »

### B5 — Météores et impacts
« [contexte commun] Phase B5. Étoiles filantes la nuit sur les mondes à air, pluies de météores
périodiques (calculées depuis la graine et l'horloge), bolides, impacts rares visibles qui créent un
cratère enregistré comme delta (règle 7) et partagé en réseau. Sans air : impacts sans traînée.
Commande de test `/impact`. »

### C1 — Ceintures d'astéroïdes
« [contexte commun] Phase C1. Ceintures générées par `planetgen` (entre rocheuses et géantes, ceinture
glacée externe), avec masse, largeur, densité. Remplace les cubes par de vrais astéroïdes (formes de
`astre/planete/meteoroid.rs`), types C / S / M liés aux ressources, rotation propre, orbites. Streaming par
cellule (règle 12), collisions selon Q6, on peut se poser sur les gros (microgravité, voxels 3D).
Captures : vue de loin, traversée d'un champ dense, atterrissage. »

### C2 — Anneaux, Troyens, comètes
« [contexte commun] Phase C2. Anneaux planétaires (glace / roche, divisions, ombre sur la planète,
particules quand on les traverse), Troyens en L4/L5 des géantes, comètes (`astre/planete/comet.rs`) sur
orbites excentriques avec queue opposée à l'étoile qui grandit près d'elle, planètes errantes rares. »

### C3 — Étoiles doubles et triples
« [contexte commun] Phase C3. Systèmes multiples (~1/3), orbites type S et P stables, zone habitable
recalculée avec les deux étoiles, deux soleils et deux ombres, double coucher, éclairage combiné. Nouveau
`PROTOCOL`. Captures : coucher double depuis une planète. »

### C4 — Phénomènes du ciel
« [contexte commun] Phase C4. Éclipses solaires et lunaires (ombre visible au sol), phases des lunes,
marées (le niveau de la mer varie avec les lunes et le soleil), aurores la nuit selon le champ magnétique
et l'activité de l'étoile, éruptions visibles. Commande pour aller à la prochaine éclipse. »

### C5 — Météo
« [contexte commun] Phase C5. Météo = f(graine, temps, lieu) (règle 9) : nuages qui se forment et se
déplacent avec les vents, pluie, neige, grêle, orages et éclairs, tempêtes de poussière, brouillard du
matin, vent qui pousse le vaisseau. Pluies exotiques selon l'atmosphère (méthane, acide, verre, fer).
Test : deux machines, même heure → même météo. »

### D1 à D6 — Eau vivante, géologie active, faune, végétation, son, points d'intérêt
« [contexte commun] Phase D<n> (voir le tableau du bloc D de `ROADMAP-0.11.md`). Fais exactement le
contenu de la ligne D<n>. Captures de chaque élément. »
