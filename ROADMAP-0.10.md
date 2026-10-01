# Feuille de route — Génération des étoiles et des planètes (0.10 → 0.14)

Objectif : remplacer la génération actuelle (quelques paramètres tirés au hasard) par une chaîne
**étoile → orbite → physique → atmosphère → climat → eau → géologie → relief → biomes → vie**,
déterministe (graine), cohérente, jouable et légère en mémoire.

Chaque phase se termine par un build instable jouable. Chaque phase a son **prompt prêt à coller**
(section « Prompts »).

---

## Décisions prises (réponses du 01/10/2026)

| Sujet | Décision |
|---|---|
| Taille des étoiles | La règle « ×100 la planète » vaut pour une étoile **classique moyenne (type G)**. Les autres types gardent les **proportions réelles** (naine rouge ~0,1–0,6 R☉, géante ~10–100 R☉, naine blanche ~taille d'une planète…). |
| Jour / nuit | **0.11**, avec variation de température entre jour et nuit. |
| Géantes gazeuses | **On peut y entrer**, mais le vaisseau perd des PV tant qu'il y reste ; à 0 PV il explose (respawn existant). |
| Grottes | **0.11** (demande un terrain en voxels 3D). |
| Compatibilité | Acceptée : prototype, 2 joueurs de test. Chaque version peut changer les mondes (nouveau `PROTOCOL`). |
| Minage / destruction | **0.14** environ. Les joueurs mineront et pourront détruire des planètes : masse, gravité et orbite deviennent des **valeurs vivantes**. Les données doivent le permettre dès la 0.10 (voir règle 7). |

---

## Point de départ (code actuel)

| Domaine | Aujourd'hui | Fichier |
|---|---|---|
| Étoile | rayon 0,6–1,5 M (unités du jeu), intensité, couleur, éruptions. Pas de type, d'âge ni d'activité. | `settings.rs` (`make_star`, `StarConfig`) |
| Orbite | 2,4 à 7 rayons d'étoile, excentricité 0, pas d'inclinaison axiale ni de rotation | `settings.rs` (`make_planets`), `kepler.rs` |
| Planète | rayon = étoile/100, relief (`terrain_height`), bruit, `sea_level`, `atmosphere` oui/non, nuages | `PlanetConfig` |
| Climat | une température : `-270 + 1100 × R_étoile / distance` | `PlanetConfig::temperature` |
| Surface | 5 matières (eau, sable, herbe, pierre, neige) | `terrain.rs` (`surface_type`), `planet.rs` (`VoxelType`) |
| Lunes | grises, sans air, rayon ≤ planète/3 | `terrain.rs` (`BodyParams::moon`) |
| Rendu | maillage lointain (`mesher.rs`), tuiles voxel de près (`terrain.rs`), ciel bleu si atmosphère, lumière réelle de l'étoile | `surface.rs` |
| PV / explosion | PV du vaisseau, destruction et réapparition | `combat.rs`, `net.rs` (`local.hp`) |
| Code ancien réutilisable | types spectraux O→M et couleurs, géantes, naines, pulsars… (désactivés) | `astre/etoile/*`, `astre/Remnant_stellaire/*` |

---

## Règles d'architecture (toutes les versions)

1. **Génome compact, profil calculé à la demande.** Un système n'enregistre que sa graine et quelques
   nombres (~160 000 systèmes). Le profil complet (`StarProfile`, `PlanetProfile`) est calculé au
   chargement du système et mis en cache.
2. **Une sous-graine par couche.** `graine → étoile, orbite, physique, atmosphère…`. Ajouter une couche
   ne change pas les précédentes. Une graine donne toujours le même monde, sur toutes les machines.
3. **Chaque couche se calcule depuis les précédentes.** On tire la masse, puis on *calcule* la densité,
   la gravité, la rétention de l'atmosphère, la température, puis l'eau, puis les biomes. Des tests
   d'invariants interdisent les incohérences (−150 °C avec 30 % de forêt tropicale…).
4. **Unités réelles pour la science, unités du jeu pour le rendu.** Calculs en R⊕, M⊕, UA, L☉, K. Une
   seule couche de conversion. Repère : 1 R⊕ ≈ 6 000 unités, 1 R☉ ≈ 650 000 unités (×109, comme la réalité).
   Les orbites du jeu sont plus serrées que les vraies : la distance « physique » en UA est convertie
   séparément de la distance affichée.
5. **Réaliste / spéculatif / fictif séparés.** Gaz, minéraux, biomes et traits portent cette étiquette.
   ~70 % scientifique pour la physique, ~70 % créatif pour la vie, le décor et les anomalies.
6. **Multijoueur.** L'empreinte du monde n'utilise que des valeurs calculées sans trigonométrie. Changer
   la génération impose d'augmenter `PROTOCOL`.
7. **Valeurs vivantes (préparation du minage, 0.14).** Masse, rayon, composition et orbite *calculées*
   depuis la graine sont la valeur de départ. Toute modification future (minage, destruction) sera un
   **delta enregistré** dans la sauvegarde du monde (`world.json`) et partagé en réseau. Le code lit
   toujours « valeur de départ + delta », jamais la valeur de départ seule. La gravité et l'orbite se
   recalculent depuis la masse courante.
8. **Origine flottante.** Ne jamais garder une position « monde » en mémoire : stocker l'absolu et
   convertir (voir `CLAUDE.md`).

### Taille des seeds (question personnelle)

La graine elle-même est déjà petite (un nombre sur 64 bits, ex. `42`), et chaque planète a une graine
de 32 bits. Ce qui prend de la place, c'est **ce qu'on en génère** : aujourd'hui chaque système stocke
ses planètes et ses lunes en entier (~800 Mo de mémoire au total). Deux améliorations sans rien perdre :

- **Ne plus stocker les planètes** : les recalculer à la demande depuis `graine du monde + indice du
  système`. Même monde, mêmes fonctions, ~2 à 3 fois moins de mémoire. C'est la règle 1, prévue en phase 0.
- **Graine partageable courte** : afficher la graine du monde sous forme de code court (ex. `K7Q2-M9XA`,
  même principe que les codes d'invitation). C'est seulement l'affichage, la graine ne change pas.

Réduire la graine elle-même (moins de bits) ferait perdre des mondes possibles sans gagner de mémoire :
ce n'est pas recommandé.

---

## Versions et phases

### 0.10 — Génération physique

| Phase | Contenu | Taille |
|---|---|---|
| 0. Fondations | Module `planetgen/`, profils, sous-graines, conversion d'unités, cache, génome léger (planètes recalculées à la demande), export JSON du profil, graine partageable courte | M |
| 1. Étoiles | Types O→M pondérés, naine blanche, géante rouge, sous-géante, naine brune ; masse → luminosité → température → couleur → rayon (proportions réelles hors type G) ; âge, activité, UV/X | M |
| 2. Orbites et physique | UA et zone de l'eau liquide, excentricité, inclinaison, inclinaison axiale, période de rotation (stockée, utilisée en 0.11), rotation synchrone ; classes de taille, masse → rayon, densité, gravité, vitesse de libération ; gravité réelle à pied ; **géantes gazeuses** (entrée possible, dégâts, explosion) | L |
| 3. Atmosphère et climat | Rétention des gaz, composition, pression, albédo, effet de serre, T_eq, `temperature(lat, alt)`, nuages par type, vents ; couleur du ciel et des couchers depuis la composition et le spectre | L |
| 4. Eau et glace | État de l'eau, océans, calottes, glaciers, océans exotiques (méthane, ammoniac, lave) | M |
| 5. Géologie et relief | Âge, activité, tectonique, volcanisme, séismes, champ magnétique ; montagnes, volcans, canyons, plateaux, cratères, érosion | L |
| 6. Sols et biomes | Sols, biomes terrestres et extraterrestres, palette voxel élargie, couleur globale | M |
| 9. Interface et traits | Habitabilité, dangers, traits rares (98/1,5/0,4/0,1 %), panneau « scanner », lunes (marées), anneaux, aurores | M |
| 7. Vie et décor | Probabilité de vie (indépendante de l'habitabilité), végétation et rochers posés sur le terrain ; faune en paramètres seulement | L |
| 8. Ressources | Minerais réels et fictifs (abondance, profondeur, difficulté), lien avec l'économie | M |

Ordre : **0 → 1 → 2 → 3 → 4 → 5 → 6 → 9 → 7 → 8**. L'interface passe avant la vie pour vérifier en
jeu que les chiffres sont cohérents.

### 0.11 — Temps et profondeur
- Rotation des planètes : jour/nuit, température qui varie entre jour et nuit (et selon les saisons).
- Grottes : terrain en voxels 3D (tubes de lave, karst, glace…) à la place du champ de hauteur près du joueur.
- Faune visible, étoiles doubles et triples.

### 0.12 – 0.13 — à définir

### 0.14 — Minage et destruction (prévision)
- Minage réel du terrain (voxels 3D de la 0.11) avec les ressources de la phase 8.
- Destruction de planètes : la masse diminue → gravité, rétention de l'atmosphère, orbite et marées
  recalculées ; débris / anneaux.
- Tout passe par les deltas de la règle 7, synchronisés en réseau.

---

## Prompts (à me coller au début de chaque phase)

Chaque prompt commence par la même phrase de contexte. Colle **un prompt à la fois**. Je fais une PR
par phase, sans la fusionner : tu testes, puis tu me dis « push main ».

> **Contexte commun** (à mettre en tête de chaque prompt) :
> « Lis `ROADMAP-0.10.md` et `CLAUDE.md`. On travaille sur la génération 0.10, en respectant les règles
> d'architecture 1 à 8 de la feuille de route. Fais `git pull origin main` avant de coder. Montre-moi
> en jeu (captures) ce qui change, ajoute des tests, ouvre une PR non fusionnée. Ne change pas les
> tailles ni les décisions de la feuille de route sans me demander. »

### Phase 0 — Fondations
« [contexte commun] Phase 0. Crée le module `planetgen/` avec `StarProfile` et `PlanetProfile`
(sections orbite, physique, composition, atmosphère, climat, hydrologie, géologie, relief, biologie,
ressources, gameplay, traits — vides ou par défaut pour l'instant). Ajoute les sous-graines par couche,
la conversion unités réelles ↔ jeu (1 R⊕ ≈ 6 000, 1 R☉ ≈ 650 000), le cache des profils du système
chargé. Les planètes et lunes ne sont plus stockées dans `settings.systems` : elles sont recalculées
à la demande depuis la graine du monde et l'indice du système, avec exactement le même résultat
qu'aujourd'hui (test de non-régression sur 3 000 systèmes). Prépare les « valeurs vivantes » (règle 7 :
valeur de départ + delta, deltas vides pour l'instant). Ajoute une commande de chat qui exporte en
JSON le profil de l'astre ciblé, et l'affichage de la graine du monde en code court. Mesure la mémoire
avant et après. »

### Phase 1 — Étoiles
« [contexte commun] Phase 1. Génère les étoiles par type spectral O, B, A, F, G, K, M avec des
fréquences réalistes (beaucoup de M, très peu de O), plus des étoiles rares : naine blanche, géante
rouge, sous-géante, naine brune. Masse → luminosité → température → couleur (corps noir) → rayon,
âge, durée de vie, activité magnétique, UV/X, vent stellaire. Taille : une étoile G moyenne garde
l'échelle actuelle (×100 la planète, 600 000 à 1 500 000 unités) ; les autres types ont les proportions
réelles par rapport à elle. Les géantes ne doivent pas déborder sur les systèmes voisins (vérifie avec
l'espacement réel des étoiles et dis-moi si un plafond est nécessaire). Branche le type sur la lumière
(couleur, intensité), les éruptions et les étoiles lointaines. Réutilise ce qui sert dans
`astre/etoile/*`. »

### Phase 2 — Orbites, physique, géantes gazeuses
« [contexte commun] Phase 2. Place les orbites en UA (zone de l'eau liquide calculée depuis la
luminosité de l'étoile), avec excentricité, inclinaison, période, inclinaison axiale, période de
rotation (stockée seulement : le jour/nuit est pour 0.11) et rotation synchrone. Classes de taille de
« minuscule » à « géante » ; masse → rayon (relation masse-rayon), densité, gravité, vitesse de
libération. La gravité et le saut à pied viennent de la vraie gravité. Ajoute les géantes gazeuses et
neptuniennes : rendu propre, on peut y entrer en vol, mais le vaisseau perd des PV tant qu'il est
dedans (vitesse selon pression et profondeur) et explose à 0 PV avec le respawn existant de `combat.rs`.
Affiche l'avertissement à l'entrée et les PV qui baissent. »

### Phase 3 — Atmosphère et climat
« [contexte commun] Phase 3. Calcule la rétention des gaz (vitesse de libération, température,
activité de l'étoile), la composition (N2, O2, CO2, CH4, H2, He, Ar, H2O, NH3, SO2… et gaz fictifs
étiquetés), la pression de surface, l'albédo, l'effet de serre et la température d'équilibre
T_eq = [L(1−A)/(16πσd²)]^¼ + serre. Remplace `PlanetConfig::temperature` par
`temperature(latitude, altitude)` (prévoir les paramètres heure et saison pour 0.11). Nuages par type,
vents globaux. Couleur du ciel, des couchers de soleil et de la brume depuis la composition et le
spectre de l'étoile. »

### Phase 4 — Eau et glace
« [contexte commun] Phase 4. Détermine l'état de l'eau (liquide, glace, vapeur, supercritique) selon
température et pression ; couverture océanique (remplace `sea_level`), calottes et glaciers selon
latitude et altitude ; océans exotiques (méthane, ammoniac, lave) avec leur couleur et leur matière
voxel ; eau souterraine (valeur seulement). »

### Phase 5 — Géologie et relief
« [contexte commun] Phase 5. Âge de la planète et de la surface, activité géologique, tectonique,
volcanisme, séismes, champ magnétique. Le relief du terrain voxel doit découler de la géologie au lieu
de bruits indépendants : chaînes de montagnes, volcans (boucliers, cônes, caldeiras), canyons et
failles, plateaux, cratères (surtout mondes vieux, sans air et lunes), érosion selon pluie, vent,
glace et âge. Garde les performances des tuiles (mesure les FPS avant/après). Pas de grottes (0.11). »

### Phase 6 — Sols et biomes
« [contexte commun] Phase 6. Sols (sable, argile, régolithe, volcanique, glace, sel, métal) depuis la
géologie et le climat. Classification des biomes (température × humidité × altitude × sol × radiation)
en biomes terrestres et extraterrestres (forêt de cristal, plaines de spores, désert de verre, marais
de soufre…). Élargis la palette voxel (une matière par biome) et rends la couleur globale de la planète
cohérente avec ses biomes, vue de l'espace comme au sol. »

### Phase 9 — Interface, traits, lunes
« [contexte commun] Phase 9. Score d'habitabilité (eau, température, pression, radiation, gravité,
toxicité…), niveaux de danger, traits et anomalies rares (98 / 1,5 / 0,4 / 0,1 %). Panneau « scanner »
de l'astre ciblé : type principal et secondaire, gravité, pression, température, atmosphère, eau,
habitabilité, traits. Applique toute la chaîne aux lunes (avec effets de marée), ajoute anneaux et
aurores. »

### Phase 7 — Vie et décor
« [contexte commun] Phase 7. Probabilité de vie microbienne, simple, complexe, indépendante de
l'habitabilité. Pose la végétation et le décor sur les tuiles du terrain selon le biome (arbres voxel,
buissons, rochers, cristaux, champignons géants…), avec instanciation et apparition selon la distance.
Faune : paramètres seulement (créatures visibles en 0.11). Mesure les FPS. »

### Phase 8 — Ressources
« [contexte commun] Phase 8. Composition globale → minerais réels (fer, cuivre, or, titane, uranium…)
et fictifs (Xenium, Aetherite…, étiquetés), avec abondance, profondeur, distribution, rareté,
difficulté d'extraction. Branche-les sur l'économie existante (biens « Ressources ») et sur le scanner.
Pas encore de minage (0.14), mais les données doivent être prêtes pour lui. »

### 0.11 — Jour/nuit et grottes (à coller plus tard)
« Lis `ROADMAP-0.10.md` et `CLAUDE.md`. Version 0.11 : fais tourner les planètes sur elles-mêmes avec la
période de rotation de la phase 2 (attention au repère du marcheur et à l'origine flottante), jour/nuit
et température qui varie avec l'heure et la saison. Puis remplace le champ de hauteur près du joueur
par un terrain en voxels 3D pour avoir des grottes (tubes de lave, karst, glace). Une PR par sujet. »

### 0.14 — Minage et destruction (à coller plus tard)
« Lis `ROADMAP-0.10.md` et `CLAUDE.md`. Version 0.14 : minage réel du terrain voxel 3D avec les
ressources de la phase 8, et destruction de planètes. Toute modification est un delta enregistré dans
`world.json` et partagé en réseau (règle 7). Quand la masse change, recalcule gravité, rétention de
l'atmosphère, orbite et marées ; une planète détruite laisse des débris ou un anneau. »
