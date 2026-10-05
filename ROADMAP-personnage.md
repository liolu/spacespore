# Feuille de route — M. Personnage (avec versions chibi)

Source : bloc **M** de `RAPPORT-ameliorations.md` (idées 171 à 184) + idées 6, 87, 89, 118 et **la demande du
05/10/2026 : ajouter des versions chibi des personnages**.
Les phases s'appellent **PER-n**.

Objectif : un personnage **personnalisable, expressif et agréable à jouer**, qui existe en deux styles —
**normal** (proportions de l'éditeur actuel) et **chibi** (grosse tête, petit corps, grands yeux) — pour toutes
les races, au choix du joueur, visible par les autres joueurs.

---

## 1. Point de départ (code du 05/10/2026)

- **Éditeur de modèles** (`src/editeur/`) : 23 familles de races (`assets/editeur/races/` : humanoïde, animal,
  reptilien, aile céleste, démon, fée, harpie, dragonoïde, drakéide, centaure, drider, lamia, sirène, slime,
  spectre, satyre, minotaure, quadrupède, insectoïde, céphalopode, golem, dryade, mécha), **57 animations**
  (`assets/editeur/anims/`), `RaceDef` (os = parties nom / parent / pivot / boîtes, `sym`, `fixed`, `options`,
  `anims`, `alias`), grille perso **16 × 32 × 32** (Q6 du 0.12, les vrais dragons non jouables).
- **En jeu** (`models.rs`) : `ModelKey` `perso:<race>` fabriqué par `editeur::defaults`, ou empreinte d'un
  `.ssvox` ; `Rig` (une entité par zone et matière, anime par `zone_locals_with`) ; `Fit::Character { height: 2.0 }`
  = hauteur du marcheur, ~2 blocs (décision du 03/10) ; choix `GameSettings::character_model` (« Utiliser comme
  mon personnage »).
- **À pied** (`surface.rs`) : F5 = 1re personne → de dos → de face, molette = distance caméra (3 à 12 voxels),
  lampe tenue en main droite (`Rig::hand`), embarquement / débarquement animés, `walker_anim`.
- **Réseau** (`net_models.rs`) : `Looks` (modèles, état du vaisseau, `WalkState`, `DockState`), fichiers demandés
  par empreinte, `sync_remote_walkers`.
- **Survie** (`suit.rs`) : `Environment`, `Suit` (O2, vie), alertes, rescue.
- Pas encore : emotes, tenues, sons de pas, escalade, jetpack, grappin, parachute, expressions de visage.

## 2. Règles

1. **Un style = une donnée du modèle** (`style: normal | chibi` dans `meta.json`), jamais un cas particulier du
   code de rendu : le `Rig` ne sait pas qu'il est chibi, il anime des os.
2. **Même squelette, mêmes noms d'os** pour les deux styles : une animation écrite pour une race marche pour sa
   version chibi (avec des **amplitudes réadaptées** si besoin, `RaceDef::anims` / `alias` existent).
3. **Le chibi est un choix d'apparence** : même hitbox de gameplay (voir Q1), même interactions, mêmes rangs de
   vitesse (pas de joueur plus difficile à toucher).
4. **Compatibilité des fichiers** : un `.ssvox` sans champ `style` est « normal » ; le format ne casse pas
   (le `PROTOCOL` change seulement pour les nouveaux messages réseau).
5. **Limites de fichier inchangées** (10 Mo, grille perso 16 × 32 × 32 ; chibi : 16 × 24 × 24 par défaut, voir 4.2).
6. **Les variations cosmétiques** (tenues, cicatrices, accessoires) sont des **couches** ou des **options de race**
   (comme `options` aujourd'hui), pas des modèles séparés.
7. **Sons et animations** (pas, emotes) viennent de `animations` + `D5 son` : pas de code en dur.

## 3. Les phases

| Phase | Contenu | Idées | Taille |
|---|---|---|---|
| **PER-1. Style chibi : format et gabarits** | `format::Model::style` (`Normal` / `Chibi`) dans `meta.json` (défaut Normal), `RaceDef::chibi` : **variante par race** (proportions : tête ≈ 45–55 % de la hauteur, corps court, bras et jambes courts et épais, mains et pieds simplifiés) ; **gabarits chibi** des 23 familles (`assets/editeur/races/*_chibi.json` ou un champ `chibi` dans chaque race : à trancher en 4.1) ; **grille chibi** 16 × 24 × 24 ; test `every_race_has_a_chibi`. Aucune apparition en jeu encore. | — | M |
| **PER-2. Chibi dans l'éditeur** | Choix **« Style : Normal / Chibi »** à la création d'un perso (`Nouveau`) ; bouton **« Convertir en chibi »** : redimensionne les zones par os (tête × 1,6, corps × 0,7…) avec aperçu avant / après, **annulable** (un lot, `Doc::…`) ; gabarit `Ghost` chibi ; aperçu animé avec les mêmes animations ; **bibliothèque** : filtre par style ; modèles fournis chibi (« Modèles fournis », `defaults::all_ids`). Les modèles chibi gardent l'**option** d'un visage (voir PER-4). | — | L |
| **PER-3. Chibi en jeu** | `GameSettings::character_style` ; `ModelKey` `perso:<race>:chibi` fabriqué par `editeur::defaults` ; `Fit::Character` avec **hauteur visuelle** réglée (chibi ≈ 1,3 blocs, hitbox inchangée : voir Q1) ; caméra de marche adaptée (distance en fonction de la hauteur visible), lampe en main (`Rig::hand`), embarquement dans les cockpits (le chibi doit « rentrer » dans la verrière, pas flotter), **réseau** : `Looks::style` (le style voyage avec le modèle), `sync_remote_walkers` ; choix dans le menu (aperçu) ; commande `/style normal|chibi`. | — | M |
| **PER-4. Visages et expressions** | **Visage** en voxels sur un « panneau » de la tête : yeux (grands pour le chibi : 3 × 3 voxels), bouche, sourcils, joues ; **expressions** (neutre, content, surpris, fâché, fatigué, endormi) pilotées par l'état (survie A4 : essoufflé, dégâts, froid), les emotes et le chat (`/emote`) ; clignement et regard (la tête suit la caméra du joueur) ; un jeu de visages par race (insectoïde, slime, spectre, mécha : écran, lueur). Technique : **zone « visage »** de la race avec des **états** (`Zone` + états, comme les `SHIP_STATES` des blocs), pas de texture. | 172 | L |
| **PER-5. Personnalisation et tenues** | **Palette de personnage** (peau, cheveux, yeux, combinaison, accent : utilise la palette OKLCH de `palette.rs`), **tenues** en couches voxel (combinaison, manteau, sac, casque, gants, bottes) posées sur les os, **accessoires** (lunettes, oreilles, cornes, queue, ailes déjà en `options`), **cicatrices et tatouages** (peinture sur un calque du modèle), **nom et titre** au-dessus de la tête, **rang de guilde** (`clan_tag` existe), sauvegarde de **plusieurs personnages** (`saves/personnages/`), apparence partagée aux autres (`Looks`). | 171, 174 | L |
| **PER-6. Emotes et gestes** | **Emotes** (saluer, pointer, s'asseoir, danser, rire, inviter, s'excuser) = animations de `assets/editeur/anims/` (réutilise les 57) ; **roue d'emotes** (CTL-5) ; **s'asseoir** sur un siège / un rebord ; **gestes** partagés au réseau (`Looks::anim`, déjà un champ d'animation) ; **version chibi** de chaque emote (exagérée : bras plus grands, rebond). | 172 | M |
| **PER-7. Équipement visible** | **Sac à dos** (taille selon le chargement), **scanner à main** (tenu en main gauche, éclaire un écran quand on scanne), **outils** (pioche du minage 0.15, lampe, analyseur) en main droite, **casque** qui se ferme / s'ouvre selon l'air (A4 : visière), **jetpack** et **parachute** visibles ; ils suivent les **os** (`Rig::hand`, nouvelles ancres : dos, hanche, main gauche). | 173 | M |
| **PER-8. Mouvement : pas, saut, gravité** | **Sons de pas** par matière du sol (sable, glace, métal, eau, neige, roche, herbe : liés à D5), **foulées** et **poussière** (MON-5 : empreintes), **course / marche / accroupi**, **saut à gravité variable** (très haut sur une petite lune, `surface.rs::jump`), **fatigue** (endurance, essoufflement), **glisser** sur la glace, **marche dans l'eau** (ralentissement, éclaboussures), animations **adaptées au chibi** (pas courts et rapides). | 175, 176 | M |
| **PER-9. Déplacements spéciaux** | **Escalade** de parois (T2 de la 0.13 : falaises 3D, prises = cases du voxel, endurance), **rappel**, **natation et plongée** (réserve d'air, O2 de A4), **grappin** (ravins, arches), **planeur / parachute** (descente de falaise ou largage), **jetpack** à carburant (faibles gravités, lié à l'énergie du vaisseau), **sauts de lune en lune** déjà ? vérifier `Hop` suborbital. Toujours avec un **mode « sans danger »** (pas de chute mortelle par défaut). | 177–181 | L |
| **PER-10. Lumière, vision, grottes** | **Lampe frontale** réglable (portée, angle), **fusées éclairantes** lancées (grottes profondes, `dim_star_light`), **lunettes** de vision nocturne / thermique (option), éclairage du personnage **par la lampe de ses voisins**, ombre du personnage sur le sol (qualité). | 182 | S |
| **PER-11. Vie du personnage** | **Journal** de bord (lieux, exploits, temps par monde, lié au dex), **mort et réapparition** claire (perte d'objets limitée, point de retour choisi, `Surface::request_rescue` existe), **santé et soins** (trousse, abri), **statistiques** (distance marchée, mondes visités). | 183, 184 | M |
| **PER-12. Compagnon et monture (optionnel)** | **Petit compagnon** (drone, créature chibi en suiveur, **mascotte**) qui suit et réagit (pointe les ressources, éclaire) ; **montures** simples (rover, créature domestiquée D4) hors périmètre tant que D4 n'existe pas. | 89 | M |

Ordre conseillé : **PER-1 → 2 → 3** (le chibi de bout en bout, en premier), puis **4, 5, 6**, puis **7, 8**, puis
**9, 10, 11**, **12** en option.

## 4. Détails importants

### 4.1 Comment fabriquer un chibi (décision technique à trancher, Q2)
Trois options :
- **A. Gabarit séparé par race** (`*_chibi.json`) : le plus propre à dessiner, double le nombre de fichiers (23 →
  46), animations à partager par `alias`.
- **B. Champ `chibi` dans chaque `RaceDef`** : les mêmes os avec des **boîtes alternatives** (`boxes_chibi`) ;
  un seul fichier par race, animations partagées automatiquement. **Ma recommandation.**
- **C. Transformation automatique** (tête × 1,6, corps × 0,7, membres × 0,6) appliquée à un modèle existant :
  rapide, mais risque d'artefacts sur les races complexes (dragon, drider, céphalopode). Utile comme **base**
  du bouton « Convertir en chibi » (PER-2), pas comme gabarit final.

### 4.2 Proportions de référence
| | Normal | Chibi |
|---|---|---|
| Hauteur | 32 cases | 24 cases (grille 16 × 24 × 24) |
| Tête | ~ 6–8 cases (1/4) | ~ 11–13 cases (≈ 50 %) |
| Corps | ~ 10 | ~ 6 |
| Jambes | ~ 14 | ~ 5 |
| Yeux | 1 case | 3 × 3 cases |
| Mains / pieds | détaillés | blocs simplifiés |

Hauteur visuelle en jeu ≈ 1,3 bloc ; yeux de la caméra en 1re personne **à la hauteur de la tête chibi**, pas du
corps normal (sinon la caméra voit le sol de trop près).

### 4.3 Gameplay et hitbox
`Fit::Character { height }` donne la **taille visuelle** ; la **boîte de collision** du marcheur reste celle de
2 blocs (Q1), pour que le style ne change ni les passages ni les combats. Les portes et les cockpits des modèles
de l'éditeur sont dimensionnés pour la boîte normale ; le chibi y rentre toujours.

### 4.4 Animations
Les 57 animations visent des **os par nom** : elles marchent sans changement. Les amplitudes (longueur de pas,
balancier des bras) sont mises à l'échelle par un champ `RaceDef::stride_scale` (chibi : 0,6 ; tête qui rebondit
davantage). Quelques animations propres au chibi : **marche sautillante**, **saut en boule**, **bâillement**,
**boude** (voir PER-6).

### 4.5 Réseau
`Looks::style` + empreinte du modèle ; un joueur sans le modèle le demande comme aujourd'hui (`Transfers`). Pas de
nouveaux paquets lourds. `PROTOCOL` incrémenté une fois à PER-3.

## 5. Mesures et tests

- `every_race_has_a_chibi` ; `chibi_keeps_every_bone` (mêmes os que le normal) ; `chibi_animations_move_something`
  (extension de `every_family_animation_moves_something`).
- Captures éditeur : `SPACESPORE_EDITOR_DEMO=race:<id>` avec `SPACESPORE_RACE_STYLE=chibi` (à ajouter), comparaison
  normal / chibi par race.
- En jeu : `SPACESPORE_TEST_LAND` + `/style chibi`, capture de dos, de face, en cockpit, avec un faux joueur
  (`SPACESPORE_TEST_PEER=marcheur`) en chibi.
- Banc : coût du `Rig` chibi identique au normal (même nombre de zones) ; 20 joueurs chibi à l'écran sans perte.

## 6. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Le chibi a-t-il une hitbox plus petite ? | **Non** : même boîte de gameplay, seule l'apparence change (équité). |
| Q2 | Gabarit séparé ou boîtes alternatives ? | Boîtes alternatives (option B) + bouton de conversion (option C) pour les modèles du joueur. |
| Q3 | Toutes les races en chibi ? | Oui, les 23 ; les plus complexes (dragonoïde, drider, céphalopode, mécha) dessinées à la main. |
| Q4 | Chibi possible aussi pour les vaisseaux (« mini-vaisseaux ») ? | Hors périmètre ; idée à noter dans le rapport. |
| Q5 | Le style est-il visible par tous ou option « voir les autres en normal » ? | Option côté joueur (réglage d'affichage). |
| Q6 | Visages en voxel ou texture ? | Voxel (états de zone), cohérent avec le reste ; pas de texture. |
| Q7 | Les aliens (0.13 L5) auront-ils une variante chibi ? | Non par défaut ; possible plus tard via les mêmes familles. |

## 7. Prompts

**PER-1** : « Lis `CLAUDE.md` (sections 0.12 E4, E5, E7) et `ROADMAP-personnage.md` §3 PER-1 et §4.1. Ajoute
`Model::style`, `RaceDef::chibi` (boîtes alternatives) pour les 23 familles, la grille chibi, et les tests
`every_race_has_a_chibi` / `chibi_keeps_every_bone`. Rien en jeu encore. »

**PER-2** : « §3 PER-2 : choix du style à la création, bouton « Convertir en chibi » (aperçu, annulable), gabarit
`Ghost`, filtre de bibliothèque, modèles fournis. Captures par race, normal / chibi. »

**PER-3** : « §3 PER-3 : `character_style`, `ModelKey` chibi, `Fit::Character` avec hauteur visuelle, caméra,
cockpit, `Looks::style` réseau (PROTOCOL), `/style`. Capture à pied, en cockpit et avec un faux joueur chibi. »

**PER-4 à PER-12** : « Lis `ROADMAP-personnage.md` §3 PER-n, implémente (versions normal et chibi), teste,
capture, PR non fusionnée. »
