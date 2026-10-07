# Feuille de route — Correctifs (publiés en v0.12.0)

Objectif : corriger ce qui ne va pas après les tests de la v0.11.3 (éditeur, vaisseau, personnage,
interface, chargement des systèmes, comètes). Publiés avec l'éditeur en **v0.12.0** (04/10/2026).

Les idées plus grosses (rendu au sol, ciel, trous noirs, voyage entre galaxies, aliens) sont dans
`roadmaps/a-faire/ROADMAP-0.13.md`.

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

---

## 2. Les correctifs

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
| 22 | 🔍 Les anneaux du trou noir (`astre/Remnant_stellaire/black_hole.rs`, 180 cubes de l'anneau photonique) sont dessinés sans tenir compte de la planète devant eux ni de l'atmosphère, et leur position ne suit pas l'origine flottante. | Au sol : cachés par la planète et par la brume de jour ; position recalculée depuis l'absolu (règle de l'origine flottante). Repris complètement par la phase V1 de `roadmaps/a-faire/ROADMAP-0.13.md`. |

---

## 3. Ordre conseillé

**C1 → C2** (l'éditeur, que tu testes déjà) → **C3 → C4 → C5 → C6 → C7 → C8**, puis release **v0.12.0**.

---

## 4. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | « Dézoom léger -100 » : de combien ? | Caméra de 4,5 à **6 voxels** derrière par défaut, réglable à la molette de 3 à 12. |
| Q2 | « Pour les autres, flotter : fait flotter la tête » : la tête doit-elle **bouger** (oscillation lente), ou le corps doit-il **monter** (le personnage flotte au-dessus du sol) ? | La tête et le cou oscillent lentement, le corps monte et descend un peu. |
| Q3 | Charger seulement au clic (20) : et si on arrive par un trou de ver ou qu'on vole jusqu'à un système sans cliquer ? | L'étoile d'arrivée devient automatiquement la cible. Voler dans un système sans cible = il se charge quand on entre dans sa zone. |

---

## 5. Prompts (à coller dans une nouvelle session, un par phase)

Contexte commun : « Lis `roadmaps/fait/ROADMAP-0.12-correctifs.md` et `CLAUDE.md`. `git pull origin main` avant de
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
- **C7** — « [contexte commun] Phase C7 : chargement des systèmes au clic sur l'étoile (selon Q3),
  `planet.rs:2552`, arrivées par trou de ver / `/aller` / `/tp`, test de 50 allers-retours. »
- **C8** — « [contexte commun] Phase C8 : mesure puis correction du tremblement des comètes (ordre des
  systèmes, f64), anneaux du trou noir cachés au sol et bien centrés. »
