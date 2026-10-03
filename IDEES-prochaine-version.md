# Idées pour la prochaine version (0.12)

Ce qui vient après les correctifs de la v0.11.4 (`ROADMAP-0.11.4-correctifs.md`) : rendu au sol, ciel
et espace, aliens. Ce sont des **idées** : elles deviennent une feuille de route quand les questions du
§6 sont tranchées.

Chaque phase = une branche `claude/correctifs-<phase>`, une PR non fusionnée : tu testes, puis tu dis
« push main ». Chaque phase a son **prompt prêt à coller** (section « Prompts »).

Pour chaque point : **ce que tu as vu**, puis **la cause trouvée dans le code** (✅ = confirmée en lisant
le code, 🔍 = piste à vérifier en jeu), puis **la correction prévue**.

---

## 1. Les idées (03/10/2026)

| # | Retour | Phase |
|---|---|---|
| 23 | Le rendu au sol ne génère qu'à ~20 blocs | R1 |
| 24 | Ajouter du brouillard | R2 |
| 25 | Skybox avec les étoiles | V1 |
| 26 | Trous noirs : déformation de la vue, invisibles sauf par leurs effets, étoile aspirée en spirale | V2 |
| 27 | Une animation incroyable pour le voyage entre galaxies | V3 |
| 28 | La vue de la galaxie doit suivre l'inclinaison de la galaxie | V4 |
| 29 | Aliens sur les planètes habitables, qui peuvent terraformer | A1, A2 |

---

## 2. Bloc R — Rendu au sol

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

## 3. Bloc V — Ciel et espace

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

**Correction :** séquence de 8 à 12 s (voir Q3), passable avec Échap : plongée vers le trou noir de
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

## 4. Bloc A — Aliens et terraformation (29)

Base existante : la vie est décrite par `planetgen/life.rs` (faune en paramètres), l'habitabilité par
`planetgen/habitability.rs`, les valeurs vivantes = départ + delta (`live.rs`, `body_deltas`), les
factions par `galaxy_fx.rs` et `economy.rs`. La faune visible est prévue en D3 (`A-FAIRE-PLUS-TARD.md`).

| Phase | Contenu |
|---|---|
| **A1. Aliens** | Une **espèce intelligente** possible sur les mondes habitables (rare, tirée de la graine) : apparence générée avec les familles de l'éditeur (céphalopode, insectoïde, reptilien…), niveau (tribu → cités → spatial), **villages et bâtiments** voxel posés sur le terrain, habitants qui marchent et vivent selon le jour et la nuit. Scanner : espèce, population, attitude. Interaction de base : saluer, commercer (biens de `economy.rs`). |
| **A2. Terraformation** | Projets de longue durée qui changent les **valeurs vivantes** de la planète (deltas, règle 7 de la 0.10) : réchauffer ou refroidir (serre, miroirs), épaissir l'atmosphère, apporter de l'eau (comètes), semer la vie. L'**habitabilité monte** pas à pas, le climat, les biomes et la couleur vue de l'espace suivent. Les aliens spatiaux terraforment aussi leurs planètes voisines. Synchronisé en réseau (deltas). |

Ce bloc dépend des réponses Q1 et Q2.

---

## 5. Ordre conseillé

**R1 → R2** → **V1 → V2 → V3 → V4** → **A1 → A2**, puis release **0.12.0**.

---

## 6. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Aliens : peut-on les **combattre** ? Sont-ils **amicaux** par défaut ? Peut-on **commercer** ? | Pas de combat au début ; attitude tirée de la graine (amicale, méfiante, hostile = refuse le contact) ; commerce oui. |
| Q2 | Terraformation : c'est **le joueur** qui terraforme, **les aliens**, ou **les deux** ? Combien de temps pour rendre une planète habitable ? | Les deux. Plusieurs heures de jeu pour un changement net, plusieurs jours pour une planète entière. |
| Q3 | Durée de l'animation entre galaxies ? | 10 s, passable avec Échap. |

---

## 7. Prompts (à coller dans une nouvelle session, un par phase)

Contexte commun : « Lis `IDEES-prochaine-version.md` et `CLAUDE.md`. `git pull origin main` avant de
coder. Branche `claude/correctifs-<phase>`. Build release, tests. PR non fusionnée (je dirai « push
main »). Ne change ni les tailles ni les décisions sans me demander. »

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
  chargement pendant le tunnel, passable avec Échap (Q3). »
- **V4** — « [contexte commun] Phase V4 : caméra de la vue galaxie alignée sur le plan de la galaxie
  (`tilt`). »
- **A1** — « [contexte commun] Phase A1 : aliens sur les mondes habitables (selon Q1). »
- **A2** — « [contexte commun] Phase A2 : terraformation par deltas des valeurs vivantes (selon Q2). »
