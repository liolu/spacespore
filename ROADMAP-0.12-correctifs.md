# Feuille de route — Correctifs et finitions (0.12)

Objectif : corriger ce qui ne va pas après les tests de la v0.11.3 (éditeur, vaisseau, personnage,
interface, chargement des systèmes), puis améliorer le rendu au sol, le ciel et les trous noirs, et
enfin ajouter les aliens et la terraformation.

Chaque phase = une branche `claude/correctifs-<phase>`, une PR non fusionnée : tu testes, puis tu dis
« push main ». Chaque phase a son **prompt prêt à coller** (section « Prompts »).

Pour chaque point : **ce que tu as vu**, puis **la cause trouvée dans le code** (✅ = confirmée en lisant
le code, 🔍 = piste à vérifier en jeu), puis **la correction prévue**.

---

## 1. Liste des retours (03/10/2026)

| # | Retour | Phase |
|---|---|---|
| 1 | Dans l'éditeur, ajouter un bloc en ajoute 10 | C1 |
| 2 | Le menu de gauche de l'éditeur a du mal à descendre | C1 |
| 3 | L'éclairage de l'éditeur : sous un certain angle on ne voit rien | C1 |
| 4 | Pas de modèle de vaisseau, et seul le 64³ s'affiche | C1 |
| 5 | Céphalopode : saluer ne marche pas, il dort allongé, il nage dans le mauvais sens | C2 |
| 6 | « Flotter » : faire flotter la tête pour les autres races | C2 |
| 7 | La cape : il manque le bas, et elle rentre dans le modèle | C2 |
| 8 | Le vol ne va pas | C2 |
| 9 | Les propulseurs ne fonctionnent pas correctement | C3 |
| 10 | En voyage, le vaisseau a le nez vers le haut, puis vers l'avant à l'approche | C3 |
| 11 | Vent réaliste : le vaisseau n'est poussé que dans un sens | C3 |
| 12 | La lampe éclaire depuis derrière le personnage, pas depuis la main | C4 |
| 13 | Caméra du personnage un peu plus loin (« -100 ») | C4 |
| 14 | F5 : ajouter la vue de face | C4 |
| 15 | Menu d'infos de l'astre : les valeurs bougent tout le temps | C5 |
| 16 | En vaisseau, l'heure et les degrés s'affolent (au sol c'est normal) | C5 |
| 17 | Ajouter la durée d'un tour sur soi-même et d'un tour d'orbite | C5 |
| 18 | Cercles autour de tous les astres cliquables, et seulement quand ils le sont | C6 |
| 19 | Parfois un système ne veut pas se charger ou se décharger | C7 |
| 20 | Ne charger un système que si on a cliqué sur son étoile | C7 |
| 21 | Les comètes tremblent | C8 |
| 22 | Au sol, on voit toujours les anneaux du trou noir, et pas au milieu | C8 |
| 23 | Le rendu au sol ne génère qu'à ~20 blocs | R1 |
| 24 | Ajouter du brouillard | R2 |
| 25 | Skybox avec les étoiles | V1 |
| 26 | Trous noirs : déformation de la vue, invisibles sauf par leurs effets, étoile aspirée en spirale | V2 |
| 27 | Une animation incroyable pour le voyage entre galaxies | V3 |
| 28 | La vue de la galaxie doit suivre l'inclinaison de la galaxie | V4 |
| 29 | Aliens sur les planètes habitables, qui peuvent terraformer | A1, A2 |

---

## 2. Bloc C — Correctifs

### C1. Éditeur : saisie, défilement, lumière, vaisseaux

| # | Cause | Correction |
|---|---|---|
| 1 | ✅ `editeur/view.rs:487` : tant que le bouton est **enfoncé**, chaque nouvelle case survolée reçoit un bloc. Le bloc qu'on vient de poser devient la case survolée suivante, donc une colonne pousse vers la caméra à chaque image. | Outil **Ajouter** : un clic = un bloc. En glissant, on ne pose que sur le **plan du premier bloc** (la face visée au début du trait) et jamais sur un bloc posé pendant ce trait. Peindre et Retirer gardent le trait continu. Test : un clic = un bloc ; un trait de 5 cases = 5 blocs sur le même plan. |
| 2 | ✅ `editeur/panels.rs:30` : le défilement est **bloqué à 2 000 pixels** (`clamp(0.0, 2_000.0)`) et ne bouge que quand la souris est exactement sur un élément du panneau (`Interaction`). | Limite = hauteur réelle du contenu moins la hauteur visible. Défilement dès que la souris est dans le panneau. Barre de défilement visible et déplaçable. |
| 3 | ✅ `editeur/view.rs:159` : **une seule lumière directionnelle**, sans lumière ambiante. Toute face tournée à l'opposé est noire. | Lumière ambiante, lumière principale qui **suit la caméra** (légèrement au-dessus et à gauche) et contre-jour faible. On voit toujours toutes les faces ; option « lumière du jeu » pour un aperçu réaliste. |
| 4 | 🔍 Les vaisseaux par défaut existent dans le code (`editeur/defaults.rs`, un par catégorie) mais ne sont pas proposés dans la bibliothèque. Pour les grilles > 64 : caméra trop proche, plan de coupe lointain trop court, ou maillage par chunk (E3) jamais lancé. | Bibliothèque : section « Modèles fournis » (1 par catégorie, 1 par famille de race), « Dupliquer pour modifier ». Pour chaque catégorie : caméra cadrée sur la grille, grille visible, maillage vérifié. Test : ouvrir chaque catégorie 64 → 1024, poser un bloc à chaque coin. |

### C2. Éditeur : animations par famille, cape, vol

**Cause commune ✅ :** les animations de `assets/editeur/anims/*.json` visent les **os d'un humain**
(`bras_d`, `bassin`, `cuisse_g`…). Une famille qui n'a pas ces os ne bouge pas (céphalopode : `tete`,
`oeil`, tentacules `tav_*`… pas de `bras_d`), ou bouge mal (dormir couche le `bassin` à -88°).

| # | Correction |
|---|---|
| — | **Variantes par famille** : un fichier de race peut remplacer une animation (`"anims": { "saluer": "saluer_tentacule" }`) et donner des **alias d'os** (`"bras_d": "tav_1"`). Une animation sans aucun os présent est retirée de la liste de la race (plus d'animation qui « ne fait rien »). |
| 5 | Céphalopode : **saluer** avec un tentacule avant levé qui ondule ; **dormir** posé, tentacules repliés, respiration lente (pas couché sur le côté) ; **nager** : le corps avance **tête devant** (vérifier le sens de déplacement par rapport à l'avant du modèle, +z), tentacules en propulsion (onde vers l'arrière). Revue de toutes les animations du céphalopode, de la sirène, de la lamia, du slime et du spectre. |
| 6 | **Flotter** : la tête flotte aussi (lente oscillation de la tête et du cou en plus des bras), pour toutes les familles. *Interprétation à confirmer, voir Q2.* |
| 7 | **Cape** (`races/01_humanoide.json`, option `cape`) : ajouter le **segment du bas** (`cape_4`) jusqu'aux mollets, et la reculer d'une case derrière le dos (elle est dans le plan du torse, `z = 13`). Pendant l'animation, l'angle de la cape ne descend jamais sous celui du dos. |
| 8 | **Vol** (`28_voler.json`, `29_planer`, `27_decoller`, `30_vol_stationnaire`) : battement autour du bon axe pour chaque famille ailée (ange, démon, fée, harpie, dragonoïde, mécha), corps penché vers l'avant, jambes repliées, queue en gouvernail. Vérifier que les noms d'os des ailes de chaque race correspondent aux pistes. |
| — | **Test automatique** : pour chaque famille × chaque animation proposée, au moins un os bouge, et l'alerte de collision (E4) ne signale aucune pièce qui traverse le corps. Captures dans la PR. |

### C3. Vaisseau : propulseurs, orientation, vent

| # | Cause | Correction |
|---|---|---|
| 9 | ✅ `models.rs:467-476` : la poussée est déduite du **déplacement du vaisseau dans le monde** d'une image à l'autre. Elle s'allume quand la planète suivie bouge en orbite ou quand l'origine flottante se recentre, et ne voit pas les vraies commandes. La direction des tuyères (`steer`) n'est jamais remplie. Les manœuvres clignotent sur un sinus. | Poussée = **commandes du joueur** (avancer, monter, descendre, croisière) ; tuyères orientées dans la direction demandée ; petits propulseurs de manœuvre = rotations et déplacements latéraux réels. Vitesse mesurée dans le repère de l'astre (règle 10), jamais en monde. |
| 10 | ✅ Pendant la croisière vers une planète, `main.rs:1246` appelle `surface::level_ship`, qui met le **dessus du vaisseau à l'opposé du centre de l'astre**. Comme on fonce droit vers ce centre, le dessus regarde vers l'arrière et le nez part vers le haut de l'écran ; près de l'astre, le nez redevient tangent à la surface. S'y ajoute `steer_ship` (`main.rs:1126`), qui tourne l'axe **-Z** vers la destination alors que les modèles ont le **nez vers +Z** (`editeur/defaults.rs:125`). | Une seule fonction d'orientation du vaisseau : le **nez (+Z du modèle) vers la destination** pendant tout le voyage, virage doux (pas de saut), roulis nul ; à l'approche, transition douce vers l'orientation de stationnement (dessous parallèle à la surface). |
| 11 | ✅ `weather.rs:400` : la force = vent **zonal** du lieu (alizés, vents d'ouest), toujours dans la même direction, sans rafales (le commentaire parle de rafales mais il n'y en a pas). | Vent réaliste : direction qui tourne lentement (bruit dans le temps et l'espace), **rafales** (pics courts), **turbulence** près du relief et dans les orages, vent plus fort en altitude. Le vaisseau est poussé, **tangue et roule** un peu, puis se **stabilise** quand on corrige. Toujours fonction de `(graine, temps, lieu)` (règle 9 de la 0.11). Indicateur de vent au HUD (flèche, force). |

### C4. Personnage : lampe, caméra, F5

| # | Cause | Correction |
|---|---|---|
| 12 | ✅ `surface.rs:1823` : la lampe est placée **à la caméra**. En 3e personne, la caméra est derrière le personnage, donc la lumière vient de derrière. | La lampe part de la **main** du modèle (os `main_d`, sinon épaule ou poitrine), éclaire là où l'on regarde. Même source en 1re et 3e personne. |
| 13 | `surface.rs:1255` : caméra à 4,5 voxels derrière. | Caméra un peu plus loin par défaut (≈ 6 voxels, voir Q1) et **réglable à la molette** en 3e personne (entre 3 et 12), mémorisée. |
| 14 | ✅ `surface.rs:1220` : F5 bascule seulement entre 1re et 3e personne. | F5 fait le tour : **1re personne → 3e personne (dos) → face** (caméra devant, tournée vers le visage). En vue de face, les commandes restent celles du personnage. |

### C5. Infos de l'astre et HUD

| # | Cause | Correction |
|---|---|---|
| 15 | ✅ `scanner.rs` : le panneau est **un seul bloc de texte**. Les lignes vivantes (heure, température, météo) y sont ajoutées à la fin : leur longueur change, tout le texte bouge. | Panneau en **sections fixes** (Identité, Physique, Rotation et orbite, Atmosphère, Eau, Vie, Ressources, **Ici et maintenant**) ; libellés et valeurs en **colonnes** ; chiffres à largeur fixe ; valeurs vivantes dans leur propre section, mises à jour **une fois par seconde**, arrondies. Rien ne change de place. |
| 16 | ✅ `world_clock.rs:412` : en vaisseau, le point de mesure est le point **sous le vaisseau** ; dans l'espace, le vaisseau suit la caméra (taille d'icône), le point saute d'un côté à l'autre de l'astre. | En vaisseau dans l'espace : le point = le **point de stationnement** (fixe dans le repère de l'astre). En vol bas : le point sous le vaisseau, lissé. Au sol : inchangé. |
| 17 | La durée du jour (`Spin::day_s`) et de l'année existent mais ne sont pas affichées en clair. | Section « Rotation et orbite » : **jour** (tour sur soi-même) et **année** (tour de l'étoile ; pour une lune, tour de sa planète), en vraie valeur (heures, jours) **et** en temps de jeu (min, h). Rotation synchrone indiquée. Aussi dans `/profil`. |

### C6. Cercles des astres cliquables

✅ `main.rs:839` : les cercles ne sont dessinés que pour les **planètes et lunes**, au zoom Planète ou
Système, avec leur propre règle de distance. Le clic (`main.rs:640`, `consider(...)`) a une autre règle.

**Correction (18) :** une seule fonction « cet astre est-il cliquable maintenant ? », utilisée **par le
clic et par les cercles**. Cercle pour **tous** les astres cliquables : étoiles, planètes, lunes,
astéroïdes où l'on peut se poser, comètes, trous de ver, trous noirs, autres galaxies. Il apparaît dès
que le clic est possible et disparaît dès qu'il ne l'est plus (fondu court). Couleur par type, cible
en jaune.

### C7. Chargement des systèmes

✅ `planet.rs:2552` `stream_system_bodies` : le système chargé est choisi d'après la **position du
vaisseau** (le plus proche dans un rayon fixe), avec une recherche large une image sur vingt pour les
grands systèmes. Entre les deux, aucun système ne gagne : rien ne se charge, ou le système chargé
change.

**Correction (19 et 20) :** un système ne se **charge que quand on clique son étoile** (ou une de ses
planètes visibles depuis la carte) et reste chargé tant qu'il est la cible ou que le vaisseau est dans
sa zone. Arrivée par un trou de ver, `/aller`, `/tp` ou téléportation : l'étoile d'arrivée devient la
cible, donc le système se charge. Il se décharge quand on cible un autre système et qu'on en sort.
Message à l'écran pendant le chargement. Les étoiles des systèmes non chargés restent visibles
(secteurs). Test : 50 allers-retours entre deux systèmes voisins sans échec.

### C8. Comètes qui tremblent et anneaux du trou noir au sol

| # | Cause | Correction |
|---|---|---|
| 21 | 🔍 Pistes : la caméra se place **avant** la mise à jour de la comète (une image de retard, `asteroids.rs:55`), précision `f32` loin de l'origine (aphélie lointaine), queue recalculée à chaque image. Les captures ne montrent rien : il faut **mesurer**. | Journal des écarts image par image (comète, caméra, vaisseau) avec `/aller comete`. Ordre des systèmes : comète → vaisseau → caméra. Position calculée en `f64` relative à l'origine. Test : écart de position lissé sous 0,1 % de la taille à l'écran. |
| 22 | 🔍 Les anneaux du trou noir (`astre/Remnant_stellaire/black_hole.rs`, 180 cubes de l'anneau photonique) sont dessinés sans tenir compte de la planète devant eux ni de l'atmosphère, et leur position ne suit pas l'origine flottante. | Au sol : cachés par la planète et par la brume de jour ; position recalculée depuis l'absolu (règle de l'origine flottante). Repris complètement en V2. |

---

## 3. Bloc R — Rendu au sol

### R1. Détail du terrain au-delà de ~20 blocs (23)

✅ `terrain.rs:291` : une tuile se découpe seulement si la caméra est à moins de **1,8 fois sa
largeur** (`SPLIT_FACTOR`) : les tuiles les plus fines ne couvrent qu'environ deux tuiles autour du
joueur. S'y ajoutent `MAX_TILES = 520` et `MAX_TILE_TASKS = 10` (`surface.rs:55`).

**Correction :** réglage **« distance de détail »** (Options), qui pilote le découpage ; plus de tuiles
gardées ; génération limitée en **millisecondes par image** et non en nombre de tâches ; tuiles devant
la caméra en premier ; **transition douce** entre niveaux (plus de marche visible). Mesures avant /
après (FPS, temps d'une tuile). Si la 0.20 (`ROADMAP-0.20-debug-opti.md`, T1 et T2) est faite avant,
ces valeurs passent par `Tuning` et le panneau F6.

### R2. Brouillard (24)

✅ `gas.rs:155` : il n'y a qu'un **brouillard de distance** (la brume de l'horizon), avec une visibilité
d'au moins **3 000 unités**. Le brouillard de la météo (`Sample::fog`) ne fait que réduire un peu cette
distance : au sol, on ne le voit pas.

**Correction :** **brouillard au ras du sol**, plus épais dans les vallées et au-dessus de l'eau, qui
monte avec l'altitude ; **bancs de brouillard** de la météo (matin, mers froides, marécages) avec les
volumes de brouillard de Bevy 0.15 (`FogVolume`, `VolumetricFog`) près du joueur ; couleur selon le
soleil (doré à l'aube, bleuté la nuit) ; les phares et la lampe éclairent le brouillard. Brouillard
sombre et léger dans les grottes. Coût mesuré, réglage de qualité dans Options.

---

## 4. Bloc V — Ciel et espace

### V1. Skybox (25)

Aujourd'hui : étoiles lointaines en points (`assets/shaders/far_star.wgsl`, secteurs d'étoiles). Pas
de fond de ciel.

**Correction :** **skybox générée** depuis la vraie galaxie, vue depuis le système où l'on est :
étoiles (couleur et éclat réels), bande de la Voie lactée, nébuleuses, autres galaxies. Rendue une fois
en cubemap (composant `Skybox` de Bevy), en arrière-plan, et refaite quand on change de système. Les
étoiles proches restent de vrais objets (cliquables). Au sol : visible la nuit, effacée par le ciel de
jour. Coût par image ≈ nul.

### V2. Trous noirs réalistes (26, et 22)

Aujourd'hui : sphère noire + disque et **anneau photonique en cubes** (180 voxels) qui tournent.

**Correction :**
- **Horizon invisible** : on ne voit pas le trou noir lui-même, seulement l'ombre et la déformation.
- **Lentille gravitationnelle** : passe de post-traitement (shader plein écran) qui **déforme l'image**
  autour du trou noir (étoiles et skybox étirées, **anneau d'Einstein**, image dédoublée).
- **Disque d'accrétion** en shader : plus chaud et plus clair à l'intérieur, côté qui s'approche plus
  brillant (effet Doppler), partie arrière du disque visible **au-dessus et en dessous** de l'ombre
  (comme dans *Interstellar*).
- **Étoile aspirée** : si une étoile est proche, un **flot de matière en spirale** part d'elle vers le
  disque ; sa lumière tourne autour du trou noir et le **trahit**.
- Remplace les cubes de l'anneau. Réglage de qualité (sans lentille sur les petites machines).

### V3. Voyage entre galaxies (27)

Aujourd'hui : `HYPERJUMP_DIST` (`main.rs:936`), le vaisseau **saute** directement.

**Correction :** séquence de 8 à 12 s (voir Q6), passable avec Échap : plongée vers le trou noir de
départ (lentille de V2 qui grossit), **tunnel** (étoiles étirées en traits, couleurs qui glissent du
bleu au rouge, distorsion), sortie par un **flash**, puis la nouvelle galaxie qui apparaît de loin et
grandit jusqu'à la vue d'arrivée. Le monde se charge pendant le tunnel (il masque le chargement).
En multijoueur, les autres voient le vaisseau disparaître dans un éclair.

### V4. Vue de la galaxie inclinée (28)

✅ La galaxie a une inclinaison (`tilt`, `settings.rs:708`), mais la caméra de la vue galaxie garde le
haut du monde (`look_at(target_pos, Vec3::Y)`, `main.rs:1252`).

**Correction :** en vue galaxie, la caméra prend le **plan de la galaxie** comme sol : le haut de
l'écran suit l'axe de la galaxie, l'orbite de la caméra tourne autour de cet axe. Transition douce
quand on passe d'une galaxie à l'autre.

---

## 5. Bloc A — Aliens et terraformation (29)

Base existante : la vie est décrite par `planetgen/life.rs` (faune en paramètres), l'habitabilité par
`planetgen/habitability.rs`, les valeurs vivantes = départ + delta (`live.rs`, `body_deltas`), les
factions par `galaxy_fx.rs` et `economy.rs`. La faune visible est prévue en D3 (`A-FAIRE-PLUS-TARD.md`).

| Phase | Contenu |
|---|---|
| **A1. Aliens** | Une **espèce intelligente** possible sur les mondes habitables (rare, tirée de la graine) : apparence générée avec les familles de l'éditeur (céphalopode, insectoïde, reptilien…), niveau (tribu → cités → spatial), **villages et bâtiments** voxel posés sur le terrain, habitants qui marchent et vivent selon le jour et la nuit. Scanner : espèce, population, attitude. Interaction de base : saluer, commercer (biens de `economy.rs`). |
| **A2. Terraformation** | Projets de longue durée qui changent les **valeurs vivantes** de la planète (deltas, règle 7 de la 0.10) : réchauffer ou refroidir (serre, miroirs), épaissir l'atmosphère, apporter de l'eau (comètes), semer la vie. L'**habitabilité monte** pas à pas, le climat, les biomes et la couleur vue de l'espace suivent. Les aliens spatiaux terraforment aussi leurs planètes voisines. Synchronisé en réseau (deltas). |

Ce bloc dépend des réponses Q3 et Q4.

---

## 6. Ordre conseillé

**C1 → C2** (l'éditeur, que tu testes déjà) → **C3 → C4 → C5 → C6 → C7 → C8** → **R1 → R2** →
**V1 → V2 → V3 → V4** → **A1 → A2**.

C1 à C8 sont des correctifs : on peut les publier ensemble en **v0.11.4**. Les blocs R, V et A forment
la **0.12.0**.

---

## 7. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | « Dézoom léger -100 » : de combien ? | Caméra de 4,5 à **6 voxels** derrière par défaut, réglable à la molette de 3 à 12. |
| Q2 | « Pour les autres, flotter : fait flotter la tête » : la tête doit-elle **bouger** (oscillation lente), ou le corps doit-il **monter** (le personnage flotte au-dessus du sol) ? | La tête et le cou oscillent lentement, le corps monte et descend un peu. |
| Q3 | Aliens : peut-on les **combattre** ? Sont-ils **amicaux** par défaut ? Peut-on **commercer** ? | Pas de combat au début ; attitude tirée de la graine (amicale, méfiante, hostile = refuse le contact) ; commerce oui. |
| Q4 | Terraformation : c'est **le joueur** qui terraforme, **les aliens**, ou **les deux** ? Combien de temps pour rendre une planète habitable ? | Les deux. Plusieurs heures de jeu pour un changement net, plusieurs jours pour une planète entière. |
| Q5 | Charger seulement au clic (20) : et si on arrive par un trou de ver ou qu'on vole jusqu'à un système sans cliquer ? | L'étoile d'arrivée devient automatiquement la cible. Voler dans un système sans cible = il se charge quand on entre dans sa zone. |
| Q6 | Durée de l'animation entre galaxies ? | 10 s, passable avec Échap. |

---

## 8. Prompts (à coller dans une nouvelle session, un par phase)

Contexte commun : « Lis `ROADMAP-0.12-correctifs.md` et `CLAUDE.md`. `git pull origin main` avant de
coder. Branche `claude/correctifs-<phase>`. Build release, tests. PR non fusionnée (je dirai « push
main »). Ne change ni les tailles ni les décisions sans me demander. »

- **C1** — « [contexte commun] Phase C1 : éditeur. Un clic = un bloc avec l'outil Ajouter (trait sur le
  plan du premier bloc, `editeur/view.rs:487`), défilement des panneaux sans limite de 2 000 px
  (`editeur/panels.rs:30`), éclairage qui montre toutes les faces (`editeur/view.rs:159`), modèles
  fournis dans la bibliothèque, grilles 128 à 1024 qui s'affichent. »
- **C2** — « [contexte commun] Phase C2 : animations par famille (variantes et alias d'os dans les
  fichiers de race), céphalopode (saluer, dormir, nager), flotter (selon Q2), cape (segment du bas,
  derrière le dos), vol de toutes les familles ailées, test automatique famille × animation. »
- **C3** — « [contexte commun] Phase C3 : propulseurs pilotés par les commandes et non par le
  déplacement monde (`models.rs:467`), orientation du vaisseau nez +Z vers la destination pendant la
  croisière (`main.rs:1126` et `1246`, `surface::level_ship`), vent réaliste avec rafales et direction variable (`weather.rs:400`). »
- **C4** — « [contexte commun] Phase C4 : lampe tenue dans la main (`surface.rs:1823`), caméra 3e
  personne plus loin et réglable à la molette (Q1), F5 = 1re personne → dos → face. »
- **C5** — « [contexte commun] Phase C5 : panneau scanner en sections et colonnes fixes, valeurs vivantes
  à 1 Hz ; point de mesure stable en vaisseau (`world_clock.rs:412`) ; durée du jour et de l'année
  (vraie et en jeu) dans le scanner et `/profil`. »
- **C6** — « [contexte commun] Phase C6 : une seule règle « cliquable » partagée par le clic
  (`main.rs:640`) et les cercles (`main.rs:839`), cercles pour tous les types d'astres. »
- **C7** — « [contexte commun] Phase C7 : chargement des systèmes au clic sur l'étoile (selon Q5),
  `planet.rs:2552`, arrivées par trou de ver / `/aller` / `/tp`, test de 50 allers-retours. »
- **C8** — « [contexte commun] Phase C8 : mesure puis correction du tremblement des comètes (ordre des
  systèmes, f64), anneaux du trou noir cachés au sol et bien centrés. »
- **R1** — « [contexte commun] Phase R1 : distance de détail du terrain (`terrain.rs:291`,
  `surface.rs:55`), budget en ms, priorité devant la caméra, transitions douces, mesures avant / après. »
- **R2** — « [contexte commun] Phase R2 : brouillard au ras du sol, bancs de brouillard de la météo
  (`FogVolume`), couleur selon le soleil, grottes ; coût mesuré, réglage de qualité. »
- **V1** — « [contexte commun] Phase V1 : skybox générée depuis la galaxie réelle (cubemap, refaite
  quand on change de système), visible la nuit au sol. »
- **V2** — « [contexte commun] Phase V2 : trous noirs : horizon invisible, lentille gravitationnelle en
  post-traitement, disque d'accrétion en shader, étoile aspirée en spirale ; remplace les cubes de
  l'anneau. »
- **V3** — « [contexte commun] Phase V3 : séquence de voyage entre galaxies (tunnel, flash, arrivée),
  chargement pendant le tunnel, passable avec Échap (Q6). »
- **V4** — « [contexte commun] Phase V4 : caméra de la vue galaxie alignée sur le plan de la galaxie
  (`tilt`). »
- **A1** — « [contexte commun] Phase A1 : aliens sur les mondes habitables (selon Q3). »
- **A2** — « [contexte commun] Phase A2 : terraformation par deltas des valeurs vivantes (selon Q4). »
