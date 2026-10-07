# Feuille de route — Éditeur de modèles voxel (0.12) : personnages, vaisseaux, objets

Objectif : un **éditeur voxel intégré à SpaceSpore** pour créer son **personnage** (selon sa race), son
**vaisseau** (de la navette au vaisseau amiral) et d'autres objets. Les pièces mobiles (bras, ailes,
portes, verrière, propulseurs…) s'animent toutes seules. Tout doit rester **simple pour le joueur** :
il pose des **blocs de mouvement** déjà animés, puis il les peint et les remplace par ses propres blocs.

Développé **en parallèle** du jeu, sur des branches séparées (`claude/editeur-*`). Une PR par phase, non
fusionnée : tu testes, puis tu dis « push main ». Chaque phase a son **prompt prêt à coller** (section
« Prompts »).

Base de départ : l'éditeur du projet Unity **Pixel World**
(`D:\... logiciel\unity\Pixel world\Assets\Scripts\VoxelEditor\`), à **porter** en Rust/Bevy, pas à
recopier tel quel.

---

## 1. La demande, reformulée

### 1.1 Parcours du joueur

1. **Ouvrir l'éditeur** : il s'ouvre de lui-même à la **création du personnage** (nouvelle partie), sur
   le type Personnage. Ensuite il reste accessible depuis le jeu (menu) pour les vaisseaux et les objets.
2. **Choisir le type de modèle** : **Personnage**, **Vaisseau**, **Autre** (objet, arme, meuble, décor…).
3. Selon le type :
   - **Personnage** → choisir la **race** (voir §3). Le joueur reçoit le **squelette (rig) de base** de la
     race, sous forme de blocs blancs déjà animés. Grille **fixe : 16 de large × 32 de haut × 32 de long**
     (X × Y × Z). La longueur de 32 laisse la place aux queues, aux corps de centaure, de dragon et de lamia.
   - **Vaisseau** → choisir la **catégorie**, qui fixe la taille de la grille (§1.2).
   - **Autre** → grille libre jusqu'à 64³.
4. **Construire** le modèle : poser, retirer, peindre, avec une **très grande palette de couleurs**
   rangée par ordre colorimétrique (§2).
5. **Animer** : les pièces mobiles viennent des **blocs de mouvement** (§4). La bibliothèque
   d'animations (§3.3 et §5) fait le reste.
6. **Enregistrer** et **utiliser** le modèle en jeu (personnage à pied, vaisseau piloté).

### 1.2 Catégories de vaisseaux

| Catégorie | Grille | Exemples |
|---|---|---|
| Chasseur | 64 × 64 × 64 | intercepteur, navette, chasseur type X-wing |
| Corvette | 128 × 128 × 128 | cargo léger, vaisseau d'exploration |
| Frégate | 256 × 256 × 256 | cargo lourd, minier |
| Croiseur | 512 × 512 × 512 | vaisseau de guerre, transporteur |
| Capital | 1024 × 1024 × 1024 | vaisseau amiral, station mobile |

Une grille 1024³ compte **un milliard de cases** : impossible en tableau plein. Il faut un stockage
**creux par blocs** (chunks 32³), des outils de volume (boîte, sphère, remplissage, symétrie, copier-coller)
et un maillage par chunk (§6, phase E3).

### 1.3 Ce qu'on garde de l'éditeur Unity

| Unity (Pixel World) | Fichier | Dans SpaceSpore |
|---|---|---|
| Outils Ajouter / Retirer / Peindre (touches 1-2-3) | `VoxelEditorManager.cs` | Repris, plus pipette, remplissage, formes, sélection |
| Survol de la case visée, cube fantôme | `UpdateHover`, `DrawHoverCube` | Repris (rayon → case, face visée) |
| Annuler / Rétablir (Ctrl+Z / Ctrl+Y) | `Undo`, `Redo` | Repris, par **lots** (un trait = une action) et par chunk pour les gros volumes |
| Grille affichée (G), cadrage (F) | `DrawGrid`, `FocusSelected` | Repris |
| Plusieurs modèles ouverts, flèches gauche/droite | `models`, `SelectModel` | Repris (onglets) |
| Étiquettes (type, zone, rareté…) | `TagCategories` | Repris, adaptées (race, catégorie, faction…) |
| Caméra orbitale | `VoxelEditorCamera.cs` | Reprise (clic droit = orbite, molette = zoom, clic molette = déplacement) |
| Maillage par faces visibles | `VoxelModelMeshBuilder.cs` | Remplacé par le maillage **glouton** (greedy) déjà utilisé par `mesher.rs` |
| JSON `[x, y, z, "Bloc"]` | `VoxelModelSerializer.cs` | **Importé** seulement. Nouveau format binaire compressé (§6) |
| Couleur = `BlockType` (enum fixe) | `BlockType.cs` | Remplacé par une **palette de couleurs** par modèle (+ matière) |
| Bibliothèque par étiquettes | `VoxelModelLibrary.cs` | Reprise : bibliothèque de modèles du joueur + modèles fournis |

---

## 2. La palette de couleurs

**Le problème :** une palette très large devient vite illisible. Trier par la teinte HSV classique donne
un résultat moche : les jaunes paraissent plus clairs que les bleus, et les gris tombent au hasard.

**La solution : l'espace OKLCH** (clarté perçue L, saturation C, teinte H). Deux couleurs à la même
clarté L **paraissent** vraiment aussi claires. C'est la méthode des palettes modernes (CSS `oklch`).

### 2.1 Disposition

- **Grille principale** : **36 colonnes de teinte** (tous les 10°, rouge → orange → jaune → vert → cyan
  → bleu → violet → magenta) × **12 lignes de clarté** (du presque noir au presque blanc).
- **Curseur de saturation** (vif / moyen / pastel / terne) : il change toute la grille. 36 × 12 × 4 =
  **1 728 couleurs** rangées sans trou.
- **Colonne des gris** à part (16 nuances, du noir au blanc) : les couleurs sans saturation n'ont pas de
  vraie teinte, on ne les mélange pas aux autres.
- **Nuancier libre** (roue + triangle, ou saisie hexadécimale) pour toute autre couleur.
- **Récentes** (16 dernières) et **pipette** (Alt+clic sur un bloc).
- **Palettes thématiques** : peaux, cheveux, métaux, coques, peintures militaires, néons.
- **Palette du modèle** : les couleurs **utilisées** dans le modèle, triées (gris d'abord, puis par
  teinte, puis par clarté). Un clic droit sur l'une d'elles **remplace la couleur partout** (repeindre
  tout le vaisseau d'un coup).

### 2.2 Matières

Chaque entrée de la palette porte une **matière** en plus de sa couleur : **mate**, **métal**, **verre**
(transparent, pour les verrières) et **lumineuse** (émissive : feux, propulseurs, yeux). Le modèle
enregistre jusqu'à **255 couleurs + matières** (un octet par voxel, 0 = vide), comme MagicaVoxel.

---

## 3. Personnages : races, rigs et animations

### 3.1 Principe

- Chaque race fournit un **rig** : une liste de **membres mobiles** (os) avec leur pivot, leur parent et
  leurs limites de rotation, plus un **gabarit** en blocs blancs posé sur la grille.
- **Membre mobile** = un groupe de voxels qui tourne autour d'un pivot (bras, avant-bras, aile, queue…).
  Pas de déformation : style voxel rigide, chaque voxel suit un seul os.
- **Partie fixe** = voxels attachés à un os, **sans articulation propre** (cornes, casque, barbe, épaulettes).
  Ils suivent l'os (les cornes bougent avec la tête), mais ne bougent jamais seuls.
- **Chaîne** = une suite d'os qui ondulent ensemble (queue, corps de serpent, tentacule, long cou).
  Elle est animée de façon **procédurale** (onde sinusoïdale amortie), pas image par image.
- Les animations visent des **noms d'os standard** (`tete`, `bras_g`, `jambe_d`, `queue_1`…). Une même
  animation « marcher » sert donc à toutes les races qui ont ces os.

### 3.2 Liste des races et de leurs différences

Légende : **mobiles** = os animés ; **fixes** = parties attachées sans articulation ;
**locomotion** = façon de se déplacer (choisit les animations de base).

| # | Famille | Races | Membres mobiles (en plus du tronc) | Parties fixes typiques | Locomotion |
|---|---|---|---|---|---|
| 1 | **Humanoïde** | Humain, elfe, nain, orc, gobelin, gnome, mort-vivant, androïde | tête, cou ; 2 bras (épaule, avant-bras, main) ; 2 jambes (cuisse, tibia, pied) | oreilles pointues, barbe, cheveux, casque | bipède |
| 2 | **Humanoïde animal** (kemonomimi / anthropomorphe) | Chat, loup, renard, lapin, ours, raton | comme 1 + **2 oreilles** (tombent, se dressent) + **queue** (chaîne de 3) ; jambes **digitigrades** en option (3 segments) | museau, crinière, griffes | bipède |
| 3 | **Reptilien** | Homme-lézard, drakéide sans ailes, homme-crocodile | comme 1 + **queue épaisse** (chaîne de 4) + **mâchoire** | écailles, crête, cornes | bipède |
| 4 | **Ailé céleste** | Ange, valkyrie | comme 1 + **2 ailes à plumes** (2 segments chacune : bras d'aile, pointe) | auréole (flotte, ne tourne pas), plumes | bipède + vol |
| 5 | **Démon** | Démon, succube, diablotin | comme 1 + **2 ailes membraneuses** (2 segments) + **queue fine** (chaîne de 4, pointe) | **cornes**, sabots ou griffes | bipède + vol |
| 6 | **Fée / insectoïde ailé** | Fée, pixie, homme-libellule | comme 1 + **4 ailes** (battement rapide, os unique chacune) + 2 **antennes** | — | vol stationnaire |
| 7 | **Harpie / homme-oiseau** | Harpie, aarakocra, tengu | tête, **bras = ailes** (3 segments), jambes à **serres** (digitigrades), **queue en éventail** | bec, plumage | bipède + vol |
| 8 | **Dragonoïde** (homme-dragon) | Dragonoïde, drakéide ailé | comme 1 + **long cou** court (chaîne de 2), **mâchoire**, **2 ailes** membraneuses (3 segments), **queue** (chaîne de 5) ; jambes digitigrades en option | **cornes**, piquants du dos, crête, écailles | bipède + vol |
| 9 | **Drakéide sans ailes** | Drakéide terrestre | comme 3, cou plus long, mâchoire | cornes, crête | bipède |
| 10 | **Centaure** (taure) | Centaure, minotaure-taure, cerf-taure | **torse humain** (tête, 2 bras) sur un **corps de cheval** : **4 jambes**, queue | sabots, cornes (minotaure) | quadrupède |
| 11 | **Drider** (araignée-taure) | Drider, arachné | torse humain (tête, 2 bras) + **8 pattes d'araignée** (2 segments) + abdomen | mandibules, yeux multiples | octopode |
| 12 | **Lamia / naga** | Lamia, naga | torse humain (tête, 2 bras) + **corps de serpent** (chaîne de 8, ondulation) | capuchon de cobra, écailles | reptation |
| 13 | **Sirène / triton** | Sirène, triton | torse humain (tête, 2 bras) + **queue de poisson** (chaîne de 4) + **nageoire caudale** | nageoires dorsales, coquillages | nage (rampe à terre) |
| 14 | **Slime** | Slime, gelée, blob | **aucun membre** : corps unique qui **s'écrase et s'étire** (squash and stretch) ; yeux | couronne, objets inclus dans le corps | bonds |
| 15 | **Spectre** | Fantôme, esprit, liche flottante | tête, 2 bras, **traîne** (chaîne de 3) à la place des jambes | capuche, chaînes | flotte |
| 16 | **Satyre / faune** | Satyre, faune | comme 1, **jambes de chèvre** digitigrades, **queue courte** | **cornes** de bélier | bipède |
| 17 | **Minotaure** | Minotaure, homme-taureau | comme 1 + **queue** (chaîne de 2) | **cornes**, anneau de nez | bipède |
| 18 | **Quadrupède animal** | Cheval, loup, chat, ours | cou, tête, **4 pattes** (2 segments), **queue**, **2 oreilles** | crinière, cornes, bois | quadrupède |
| 19 | **Insectoïde** | Homme-mante, homme-fourmi, alien insecte | tête + **mandibules**, **2 antennes**, **4 bras** ou 2 bras + 2 faux, **2 à 4 jambes** ; ailes optionnelles | carapace | bipède ou hexapode |
| 20 | **Céphalopode / tentaculaire** | Alien poulpe, mind-flayer | tête, **4 à 8 tentacules** (chaînes de 4) | — | rampe / flotte |
| 21 | **Golem / géant** | Golem, colosse, titan de pierre | comme 1, mouvements lourds (animations plus lentes, sans flexion du dos) | cristaux, runes | bipède lourd |
| 22 | **Dryade / sylvain** | Dryade, homme-arbre | comme 1 + **branches** (2 à 4, balancement lent) | feuillage, mousse | bipède |
| 23 | **Mécha / robot** | Robot, mécha, drone humanoïde | comme 1 + options : **réacteurs dorsaux** (lumineux), **antenne** rotative, **roues** à la place des jambes | panneaux, écrans | bipède / roues / vol |

**Parties fixes partout possibles** : cornes, casques, armures, bijoux, cicatrices, cheveux courts. Les
**cheveux longs** et les **capes** peuvent devenir une chaîne (option « flotte au vent »).

**Membres optionnels** (cases à cocher au choix de la race) : queue, oreilles mobiles, ailes, bras
supplémentaires. Exemple : humain + queue = « humain à queue », sans créer de race.

### 3.3 Bibliothèque d'animations

| Groupe | Animations |
|---|---|
| Base (toutes les races) | repos (idle), marcher, courir, sauter, tomber, atterrir, s'accroupir, nager, flotter (apesanteur), piloter (assis), mourir |
| Interaction | saluer, montrer du doigt, miner, ramasser, utiliser, frapper, tirer, viser, porter |
| Émotes | danser, s'asseoir, rire, applaudir, hausser les épaules, dormir |
| Vol (races ailées) | décoller, voler, planer, vol stationnaire, piqué, battement rapide (fée) |
| Locomotion propre | trot et galop (quadrupèdes, centaures), reptation (lamia), ondulation (sirène), bond (slime), marche d'araignée (drider), glisse (spectre) |
| Procédurales (toujours actives) | **respiration**, **queue** et **oreilles** qui ondulent, **ailes repliées** qui frémissent, **slime** qui tremble, cheveux et capes |

Les animations sont des **images clés** (rotation de chaque os) mélangées entre elles (passer de marcher
à courir sans à-coup). Une animation peut **ignorer** les os absents : « saluer » marche pour un centaure
comme pour un humain.

---

## 4. Les blocs de mouvement (le cœur de la simplicité)

**Idée :** le joueur ne règle jamais un pivot ni un os à la main. Il prend un **bloc de mouvement** dans
une liste, comme un bloc normal.

1. **Choisir** un bloc de mouvement (ex. « Aile type X-wing », « Porte coulissante », « Bras gauche »).
2. **Placer** : un **gabarit en blocs blancs** suit la souris et **joue son animation de repos** en boucle.
   Le joueur voit tout de suite comment la pièce bougera et s'il y a la place. Molette = tourner,
   Maj+molette = taille (si le bloc l'accepte), X = miroir.
3. **Poser** : le gabarit **s'arrête** (pose neutre) et devient une **zone de mouvement** (contour coloré).
4. **Remplacer** : le joueur **peint ou remplace** les blocs blancs par les siens, **ajoute** des blocs
   dans la zone (ils rejoignent la pièce mobile) ou **en retire**. Un bloc blanc restant est un bloc
   comme un autre (il reste blanc).
5. **Tester** : bouton **▶ Aperçu** (touche P) qui joue toutes les animations du modèle ; on peut
   choisir l'animation (repos, marche, ouverture de porte…).

Règles :

- Un voxel appartient à **une seule** zone (ou au corps fixe). Peindre dans une zone l'y ajoute.
- Les zones peuvent être **emboîtées** (main dans avant-bras dans bras ; volet dans aile).
- Une zone **désactive les collisions** avec ses voisins pendant l'aperçu : l'éditeur prévient (en
  rouge) si une pièce traverse le corps en bougeant.
- Pour un personnage, le **rig de la race** est simplement un ensemble de blocs de mouvement déjà posés.
- Mode **avancé** (plus tard, E8) : déplacer le pivot, changer les angles, créer sa propre animation.

---

## 5. Vaisseaux : zones d'animation

| Bloc de mouvement | Mouvement | Déclenché par |
|---|---|---|
| **Porte** pivotante / coulissante / **rampe** | rotation ou translation | approche du joueur, atterrissage |
| **Verrière de cockpit** | rotation vers le haut ou l'arrière | monter / descendre du vaisseau |
| **Ailes repliables** | rotation (se replient vers le haut) | posé = repliées, vol = dépliées |
| **Ailes en X** (type X-wing, S-foils) | 4 ailes qui s'écartent en X | mode combat |
| **Ailes à géométrie variable** | balayage avant / arrière | vitesse |
| **Train d'atterrissage** | sort / rentre | approche du sol |
| **Propulseur principal** | flamme lumineuse + intensité selon la poussée | poussée (Z / Maj) |
| **Propulseurs de manœuvre** | petites flammes | rotations et déplacements latéraux |
| **Tuyère orientable** | rotation suivant la direction de poussée | pilotage |
| **Tourelle** | lacet + tangage vers la cible | combat |
| **Radar / antenne** | rotation continue | toujours |
| **Anneau rotatif** (gravité) | rotation lente continue | toujours |
| **Panneaux solaires** | dépliés / pliés | posé / en vol |
| **Baie de hangar** | grande porte + **emplacement de vaisseau** (§5.1) | entrée / sortie d'un vaisseau |
| **Bras minier / grappin** | bras articulé | minage (0.14) |
| **Feux de position** | clignotement | toujours (lumineux) |

**États du vaisseau** : *posé*, *décollage*, *vol*, *combat*, *atterrissage*, *détruit*. Chaque zone dit
dans quel état elle est ouverte ou fermée ; le jeu change d'état, les zones suivent.

Les **propulseurs** restent visuels (Q3) : leur **direction** sert seulement à orienter les flammes.

### 5.1 Porte-vaisseaux : emplacements de hangar (ajout du 03/10/2026)

- Les **portes de hangar** d'un porte-vaisseau sont des **emplacements de vaisseau** : le bloc « Hangar »
  pose une baie (grande porte animée) et une **place d'amarrage** à la taille d'une catégorie de vaisseau.
- Chaque emplacement a son **animation d'entrée et de sortie** : approche, la porte s'ouvre, le vaisseau
  ralentit, passe la porte et se pose sur sa place ; la porte se ferme. La sortie fait l'inverse
  (décollage de la place, passage de la porte, accélération). Le chemin est une suite de points posés
  dans l'éditeur (comme un bloc de mouvement), joué par le lecteur d'animations.
- **Très gros vaisseaux** (capital) : **soutes à cargos**. Ils peuvent faire entrer des **vaisseaux cargo**
  chargés de ressources ; à l'amarrage, la cargaison est déchargée dans le porte-vaisseau (lien avec
  l'économie et le minage, 0.14).
- Le vaisseau du joueur (ou d'un autre joueur) peut s'amarrer dans un emplacement libre à sa taille, puis
  en ressortir.
- Phases : le bloc Hangar, ses places et ses chemins dans l'éditeur en **E6** ; l'amarrage en jeu en
  **E7** ; la cargaison des cargos avec l'économie (0.14).

---

## 6. Règles d'architecture

1. **Un seul format de modèle** : `ModelFile { version, kind (perso | vaisseau | autre), race / catégorie,
   size, palette[255] (couleur + matière), chunks, zones, tags }`. Binaire, chunks 32³ compressés
   (RLE + deflate, la caisse `zip` est déjà là), extension `.ssvox`. Fichiers dans `saves/modeles/`.
2. **Stockage creux** : `HashMap<IVec3 chunk, Chunk>` ; un chunk est vide, **uniforme** (un seul index)
   ou plein (32 Kio). Mémoire proportionnelle à ce qui est construit, pas à la taille de la grille.
3. **Maillage glouton par chunk**, hors du fil principal (`AsyncComputeTaskPool`), seuls les chunks
   modifiés sont remaillés. Une **zone de mouvement = un maillage à part** (enfant transformé), pour
   l'animer sans remailler.
4. **Rig = données, pas code** : races, os, gabarits, blocs de mouvement et animations décrits dans des
   fichiers (`assets/editeur/races/*.ron` ou JSON), chargés au démarrage. Ajouter une race = ajouter un
   fichier.
5. **Animation rigide** : chaque os = `Transform` Bevy ; images clés + chaînes procédurales + mélange.
   Pas de skinning.
6. **Un module** `src/editeur/` (mode du jeu `AppState::Editeur`), séparé du reste. Il réutilise
   `mesher.rs` et l'interface existante (`ui.rs`). Aucune modification de la génération du monde
   (`PROTOCOL` inchangé).
7. **Réseau** : un modèle est identifié par son **empreinte** (hash). Les pairs ne l'envoient qu'une
   fois, compressé, puis le gardent en cache (`saves/cache_modeles/`). Taille maximale envoyée : 10 Mo (Q4).
8. **Budget** : éditeur à ≥ 60 FPS avec un croiseur 512³ chargé ; pose d'un bloc remaillée en < 5 ms ;
   remplissage d'une boîte de 256³ en < 1 s.

---

## 7. Phases

| Phase | Contenu | Taille |
|---|---|---|
| **E0. Fondations et format** | Module `src/editeur/`, état `AppState::Editeur`, ouvert à la **création du personnage** (nouvelle partie) et depuis le menu du jeu (Q1). Format `.ssvox` (règles 1-2), lecture / écriture, tests aller-retour. **Import** du JSON de Pixel World (`BlockType` → couleur). | M |
| **E1. Éditeur de base** | Écran « choisir le type » (perso / vaisseau / autre) puis race ou catégorie. Grille aux bonnes tailles, caméra orbitale, outils **ajouter / retirer / peindre / pipette**, survol, annuler / rétablir, grille (G), cadrage (F), **symétrie miroir** (X, actif par défaut pour les persos), bibliothèque de modèles (ouvrir, dupliquer, renommer, étiquettes, supprimer). | L |
| **E2. Palette** | Palette OKLCH (§2) : grille 36 × 12, saturation, gris, nuancier libre, récentes, palettes thématiques, palette du modèle triée, remplacer une couleur partout, matières (mate, métal, verre, lumineuse) avec rendu en jeu. | M |
| **E3. Grands vaisseaux** | Chunks creux, maillage glouton asynchrone par chunk, LOD de l'aperçu. Outils de volume : **boîte, sphère, cylindre, ligne, remplissage** (pot de peinture), **sélection** (déplacer, copier, coller, tourner, miroir), **calques** masquables, coupe (voir l'intérieur, tranche par tranche). Annuler par chunk. Mesures de mémoire et de temps (règle 8). | XL |
| **E4. Blocs de mouvement** | Le système du §4 : liste de blocs, gabarit blanc animé qui suit la souris, pose, zone colorée, remplacement des blocs, zones emboîtées, avertissement de collision, **aperçu ▶** avec choix de l'animation. Lecteur d'animations (images clés + chaînes procédurales + mélange). | L |
| **E5. Races et animations de personnage** | Les 23 familles du §3.2 en fichiers de données (rig + gabarit + membres optionnels). Bibliothèque d'animations du §3.3 (base, interaction, émotes, vol, locomotions propres, procédurales). Écran de choix de race avec aperçu animé. | XL |
| **E6. Animations de vaisseau** | Les blocs du §5, les **états du vaisseau** et leur déclenchement, flammes de propulseur selon la poussée, tourelles qui visent, feux clignotants. **Hangars** (§5.1) : bloc Hangar, places d'amarrage par catégorie, chemins d'entrée et de sortie animés, soutes à cargos des capitaux. | L |
| **E7. Dans le jeu** | Le **vaisseau du joueur** = son modèle (à la place de `ship.rs`), échelle **4 voxels = 1 bloc** (Q2), collisions (boîtes par chunk). Le **personnage à pied** = son modèle (`surface.rs`), animations pilotées par le jeu (marcher, courir, sauter, nager, piloter, apesanteur). Modèles des autres joueurs par empreinte (règle 7). Modèles fournis par défaut (1 perso par famille, 1 vaisseau par catégorie). **Amarrage** dans les hangars des porte-vaisseaux (entrée et sortie animées, §5.1). | L |
| **E8. Mode avancé** (optionnel) | Pivot et angles modifiables, éditeur d'animation (frise des images clés), création de ses propres blocs de mouvement, import / export **MagicaVoxel `.vox`**, partage de modèles entre joueurs. | L |

Ordre conseillé : E0 → E1 → E2 → E4 → E5 → E3 → E6 → E7 → E8. Le personnage (petite grille) permet de
valider le système d'animation **avant** de s'attaquer aux grilles géantes des vaisseaux.

---

## 8. Décisions et questions

### Décisions (réponses du 02/10/2026)

| # | Sujet | Décision |
|---|---|---|
| Q1 | **Où vit l'éditeur** | **Dans le jeu** (`AppState::Editeur`). Il est proposé à la **création du personnage**, puis accessible depuis le menu pour les vaisseaux et les objets. |
| Q2 | **Taille d'un voxel** | **4 voxels de modèle = 1 bloc du jeu** pour les **vaisseaux**, posés et en vol bas (chasseur 64 = 16 blocs, capital 1024 = 256 blocs) ; dans l'espace, taille d'icône qui suit la caméra. **Personnage** (décision du 03/10/2026, E7) : il garde la taille du marcheur (~2 blocs), le modèle est réduit en conséquence. |
| Q3 | **Statistiques du vaisseau** | **Aucune** : le modèle est purement visuel, il ne change ni la masse, ni la vitesse, ni les PV. |
| Q4 | **Taille maximale d'un vaisseau** | Un modèle de vaisseau pèse au plus **10 Mo** (fichier `.ssvox` compressé), quelle que soit la catégorie. La grille n'est pas limitée, c'est le poids du fichier : compteur « x,x / 10 Mo » affiché dans l'éditeur, enregistrement refusé au-delà. C'est aussi la taille maximale envoyée aux autres joueurs (règle 7). |
| Q6 | **Grille perso 16 × 32 × 32** | Pour toutes les races **jouables**, dragonoïdes compris. Les **vrais dragons** (quadrupèdes géants) ne sont **pas une race jouable** : ce seront des créatures (type « Autre »), plus tard. |

### Encore ouvertes

| # | Question | Proposition |
|---|---|---|
| Q5 | Les autres joueurs voient-ils **ton** modèle ? | Oui, envoyé une fois par empreinte (règle 7). |
| Q7 | Nombre de couleurs par modèle : 255 suffisent ? | Oui (comme MagicaVoxel) ; la grande palette sert à choisir, le modèle garde ses 255. |
| Q8 | Qui entre dans quel hangar (§5.1) ? | Croiseur : chasseurs ; capital : chasseurs et corvettes, plus des soutes à cargos (cargos jusqu'à la frégate). Une place = une catégorie au plus. |

---

## 9. Prompts (à coller dans une nouvelle session, une par phase)

Chaque prompt suppose : « Lis `roadmaps/fait/ROADMAP-0.12-editeur.md` et `CLAUDE.md`. Crée la branche `claude/editeur-eX`
depuis `main`. Build release. Ouvre une PR non fusionnée avec mesures (FPS, mémoire) et captures. »

- **E0** — « Phase E0 de `roadmaps/fait/ROADMAP-0.12-editeur.md` : crée `src/editeur/` et l'état `AppState::Editeur`,
  ouvert à la création du personnage et depuis le menu du jeu. Implémente le format `.ssvox` (règles 1 et 2 : palette 255
  couleurs + matière, chunks 32³ creux, compression RLE + deflate) avec tests aller-retour, et l'import
  du JSON de Pixel World (lis `D:\... logiciel\unity\Pixel world\Assets\Scripts\VoxelEditor\VoxelModelSerializer.cs`
  et `VoxelTerrain\BlockType.cs`, associe une couleur à chaque `BlockType`). »
- **E1** — « Phase E1 de `roadmaps/fait/ROADMAP-0.12-editeur.md` : porte l'éditeur de Pixel World
  (`VoxelEditorManager.cs`, `VoxelEditorCamera.cs`) en Bevy : écran de choix du type puis race /
  catégorie, grille aux tailles du §1, caméra orbitale, outils ajouter / retirer / peindre / pipette,
  survol, annuler / rétablir par lots, grille, cadrage, symétrie miroir, bibliothèque de modèles avec
  étiquettes. Clavier AZERTY. »
- **E2** — « Phase E2 de `roadmaps/fait/ROADMAP-0.12-editeur.md` : palette OKLCH du §2 (grille 36 teintes × 12 clartés,
  4 saturations, gris à part, nuancier libre, récentes, palettes thématiques, palette du modèle triée,
  remplacer une couleur partout) et matières mate / métal / verre / lumineuse, rendues dans l'éditeur. »
- **E3** — « Phase E3 de `roadmaps/fait/ROADMAP-0.12-editeur.md` : grilles jusqu'à 1024³. Chunks creux, maillage glouton
  asynchrone par chunk (réutilise `mesher.rs` si possible), outils de volume (boîte, sphère, cylindre,
  ligne, remplissage, sélection avec copier / coller / tourner / miroir), calques, vue en coupe, annuler
  par chunk. Respecte le budget de la règle 8 et mesure-le dans la PR. »
- **E4** — « Phase E4 de `roadmaps/fait/ROADMAP-0.12-editeur.md` : blocs de mouvement du §4. Gabarit blanc animé qui suit
  la souris, pose qui fige le gabarit en zone, remplacement des blocs, zones emboîtées, alerte de
  collision, aperçu ▶. Lecteur d'animations rigides (images clés, chaînes procédurales, mélange) avec
  des données dans `assets/editeur/` (règle 4). »
- **E5** — « Phase E5 de `roadmaps/fait/ROADMAP-0.12-editeur.md` : les 23 familles de races du §3.2 en fichiers de données
  (os, pivots, gabarit, parties fixes, membres optionnels) et la bibliothèque d'animations du §3.3.
  Écran de choix de race avec aperçu animé. Vérifie chaque race dans l'aperçu (capture par famille). »
- **E6** — « Phase E6 de `roadmaps/fait/ROADMAP-0.12-editeur.md` : blocs de mouvement de vaisseau du §5 (portes, verrière,
  ailes repliables, ailes en X, train, propulseurs, tourelles, radar, anneau, panneaux, feux), états du
  vaisseau et leur déclenchement, hangars des porte-vaisseaux du §5.1 (places, chemins d'entrée et de
  sortie, soutes à cargos). »
- **E7** — « Phase E7 de `roadmaps/fait/ROADMAP-0.12-editeur.md` : utilise les modèles en jeu. Vaisseau du joueur
  (`ship.rs`) et personnage à pied (`surface.rs`) tirés de leur `.ssvox`, échelle 4 voxels = 1 bloc du jeu (Q2),
  collisions, animations pilotées par le jeu, envoi des modèles aux autres joueurs par empreinte
  (règle 7, `net.rs`). Modèles par défaut fournis. »
- **E8** — « Phase E8 de `roadmaps/fait/ROADMAP-0.12-editeur.md` : mode avancé (pivots, éditeur d'images clés, blocs de
  mouvement personnalisés), import / export MagicaVoxel `.vox`. »
