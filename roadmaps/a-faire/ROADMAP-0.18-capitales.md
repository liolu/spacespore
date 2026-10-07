# Feuille de route — Les capitales : villes-planètes et mondes-stations (0.18)

Objectif : ajouter le monde le plus **impressionnant** du jeu : la **capitale**. Une planète entièrement
couverte de ville (écuménopole), ou une station si grande qu'elle est devenue un monde. Ses formes vont de la
sphère presque parfaite à l'**agrégat fortement déformé** (modules ajoutés pendant des siècles, tours qui
dépassent l'atmosphère, hémisphère creusé, anneau soudé à l'équateur). Chaque capitale est **unique**,
**reconnaissable de loin**, **belle de près**, et elle a une **raison d'être** dans le jeu (le **monde natal d'une race** extraterrestre, ou la **capitale galactique** où les races commercent et
font de la politique).

Source : planche d'inspiration du 07/10/2026 (`C:\Users\thomr\Desktop\Nouveau dossier\doctravail\06-next maj\0.18`,
15 images). Règle 28 de la 0.14 : **aucun nom ni forme reconnaissable d'une œuvre** dans le jeu ; on reprend des
**principes** (verticalité, dômes, viaducs, trafic en files...), jamais un bâtiment précis.

Point de départ : la 0.17 terminée (réglages, mesures, benchmark : la ville est le scénario K du benchmark).
La 0.18 est découpée en **versions 0.18.1 à 0.18.9**, chacune jouable et publiée. Chaque phase = une branche
`claude/roadmap-0-18-<phase>`, une PR non fusionnée : tu testes, puis tu dis « push main ». Chaque phase a son
**prompt prêt à coller** (§12).

---

## 1. Ce que dit la planche d'inspiration

| Image | Ce qu'on en garde (principe, pas copie) | Style du jeu (§5) |
|---|---|---|
| `4842-metropolis-3108.jpg`, `Ratchet-Clank2.jpg`, `images.jpg`, `imaffffges.jpg` | Tours rondes à **dômes et anneaux**, **viaducs à arches**, **trafic aérien en couloirs**, dirigeables, couleurs cyan / or / vert d'eau, **verdure suspendue**, places rondes lumineuses au sol, grosse lune dans le ciel | S1 Rétro-futur |
| `Impesrial_Palace_Terra2.webp`, `imagfes.jpg`, `imasges.jpg` | **Flèches gothiques**, cathédrales empilées, **statues géantes** qui encadrent une avenue, **pont-avenue** sur des arches de plusieurs centaines de mètres, palais doré au bout, foule de lumières, brume verte, vaisseaux massifs qui passent | S2 Gothique impérial |
| `mmm.jpg`, `imaùùges.jpg`, `mmmmm.jpg` | **Plaine urbaine infinie** jusqu'à l'horizon, **aiguilles** et **dômes** posés dessus, temple-pyramide, ciel doré au couchant, **nuit couverte de lumières** et de files de vaisseaux | S3 Aiguilles et dômes |
| `imahges.jpg` | **Colonnes-tours** avec plateaux en soucoupe et anneaux, place circulaire, **puits** vers la ville du dessous, sol métallique gravé, ciel pastel | S4 Colonnes (monde couvert) |
| `ikmages.jpg` | **Canyons urbains** sans fond, niveaux empilés, **lueur orange des fonderies** tout en bas, passerelles qui traversent le vide | S5 Fonderie / canyons |
| `Worlds_03.webp`, `Capture d'écran 2026-10-07 145503.png` | **Monde-station agrégé** : silhouette **asymétrique**, hérissée, modules collés les uns aux autres, **grappes de dômes** lumineux, nuée de débris et de vaisseaux autour, antennes et anneaux, nébuleuse colorée derrière | Monde-station (§7) |

Ce qui revient partout et fait la beauté : **la verticalité** (plusieurs niveaux de ville), **les lumières**
(fenêtres, files de vaisseaux, lueurs du bas), **la brume** qui donne la profondeur, **un monument** qui attire
l'œil, **le mouvement** (trafic), et **le contraste** entre le haut lumineux et le bas sombre.

---

## 2. Décisions

### 2.1 Réponses du 07/10/2026

| # | Sujet | Décision |
|---|---|---|
| D1 | **Capitale de race** | **Une capitale par race extraterrestre**, sur la **planète d'origine** de cette race (son monde natal). Les races viennent des aliens de la 0.13 L5 ; seules celles qui ont atteint un niveau assez avancé (cités → spatial) ont une capitale. |
| D2 | **Capitale galactique** | Une **station capitale galactique** (le monde-station du §7), construite **par plusieurs races** pour le **commerce et la politique**. |
| D3 | **Quartiers par race** | La capitale galactique a des **quartiers différents selon les races présentes** : chaque race y a son quartier dans **son style**, avec son air, sa gravité et sa lumière. |
| D4 | **Croissance** | La ville **se construit avec une graine selon sa progression et ses ressources** (§6.3) : l'état de la ville (village → cité → métropole → écuménopole) et la place de chaque quartier viennent de **sa progression** et des **ressources** de sa planète (minerais, eau, énergie). |
| D5 | **Destruction** | On peut **détruire** une capitale (ou une partie) **seulement si elle est ennemie du joueur** (relation de `combat.rs` : faction ou race en guerre avec lui). Sinon elle est intouchable. |
| D6 | **Numéros** | Versions **0.18.1 à 0.18.9** ; la 0.20 (débogage, optimisation) devient la **0.17** et passe avant. |

### 2.2 Autres choix (propositions, §11)

| Sujet | Proposition |
|---|---|
| **Combien de capitales galactiques** | Une par galaxie générée au départ (21), là où les territoires des races se rencontrent. |
| **Réalisme** | **Fictif** (règle 5) : étiqueté au scanner. La physique de l'astre reste vraie (étoile, orbite, gravité) ; la ville l'adapte (dômes sur un monde sans air, radiateurs géants sur un monde chaud). |
| **Zone calme** | Pas de capitale dans les 50 systèmes du départ (règle 23 de la 0.14). |
| **Échelle** | Tours de **200 à 3 000 voxels** ; **méga-tours** de 20 000 à 200 000 voxels qui **sortent de l'atmosphère** et se voient de l'espace ; canyons jusqu'à 10 000 voxels de profondeur. |
| **Construction du joueur** | Pas dans la 0.18 : la ville se construit seule (D4). Le joueur peut s'y poser, marcher, commercer, et la détruire si elle est ennemie (D5). |

---

## 3. Règles d'architecture (0.18)

Les règles des versions précédentes restent valables. La 0.18 ajoute (numérotées V1 à V12) :

- **V1. La ville est une fonction.** Tout est `f(graine de la capitale, cellule)` (règle 12) : rien n'est stocké,
  les autres joueurs voient la même ville. Seuls les changements (destructions, constructions, états) sont des
  **deltas**.
- **V2. Quatre échelles, un seul modèle.** Planète (vue de l'espace) → région (vol à 10 000 voxels) → quartier
  (vol bas) → rue (à pied). Chaque échelle **affine** la précédente sans la contredire (règle 16) : la tache
  lumineuse vue de l'espace est le quartier qu'on survole.
- **V3. Forme = un champ de distance.** Un astre peut avoir une forme **non sphérique** : `BodyShape`
  (sphère + relief, sphère déformée, ou composition de formes : capsules, boîtes, tores, cylindres, unions
  douces). Une seule fonction pour le terrain, le maillage lointain, les collisions, la gravité et l'ombre.
  Elle reprend la `shape(dir)` de la 0.14 X4.
- **V4. Les bâtiments sont des modules.** Un bâtiment = des **modules** voxel (`.ssvox`, faits dans l'éditeur
  de la 0.12 ou par script) assemblés par une **grammaire** (socle, fût, couronne, flèche, ponts). Les modules sont
  **instanciés** (un maillage, des milliers d'instances), jamais copiés dans le terrain.
- **V5. Des niveaux de détail pour les villes.** De loin : carte de hauteur des toits + **carte de lumière** de
  nuit ; à mi-distance : boîtes simplifiées (imposteurs) ; de près : modules complets ; à pied : détails
  (rambardes, fenêtres, enseignes) et collisions.
- **V6. Les lumières ne sont pas des lampes.** Fenêtres, néons, files de vaisseaux = matériaux **émissifs** et
  particules ; au plus **8 vraies lumières** près du joueur. Budget mesuré avec les outils de la 0.17.
- **V7. Diversité mesurée.** Chaque capitale a une **empreinte** (forme, style, palette, densité, verticalité,
  monuments). Test : deux capitales d'une même galaxie ne se ressemblent jamais (distance d'empreinte minimale),
  et chaque style apparaît.
- **V8. Une capitale raconte quelque chose.** Âge, richesse, état (florissante, en guerre, en déclin,
  abandonnée), culture de la race : tout se voit (couches anciennes en bas, chantiers, ruines, drapeaux,
  couleurs) et se lit au scanner.
- **V9. La beauté se vérifie.** Chaque phase visuelle livre des **captures de référence** (jour, couchant, nuit,
  de l'espace, en vol bas, à pied) par style ; une planche avant / après dans la PR.
- **V10. Coût borné.** Scénario K du benchmark (0.17) : survol, plongée dans un canyon, nuit. Cible : pas plus de
  20 % de perte d'images/s par rapport à une planète ordinaire en Ultra.
- **V11. Le reste du jeu suit.** Scanner, dex, `/aller capitale`, `/stats`, profil, cercles cliquables, carte de
  la galaxie : la capitale y est partout comme un vrai astre.
- **V12. Pas d'œuvres.** Aucun nom, symbole ou bâtiment reconnaissable d'une œuvre protégée (règle 28 de la 0.14).
- **V13. La ville pousse (D4).** Une capitale = **graine + progression + ressources**. La même graine au même stade
  donne toujours la même ville ; un stade de plus **ajoute** des couches sans effacer celles d'avant (le vieux
  centre reste au milieu, les ajouts récents autour et au-dessus). Les ressources de la planète (phase 8 de la
  0.10, gisements de la 0.15) décident **où** et **quoi** : mines et fonderies sur les gisements, ports sur les
  mers, centrales près des geysers et des volcans, fermes où le sol est fertile.
- **V14. Détruire seulement l'ennemi (D5).** Les dégâts sur une capitale ne passent que si sa race (ou, pour la
  capitale galactique, le quartier visé) est **ennemie du joueur** ; sinon les tirs sont sans effet (bouclier) et
  la défense prévient. Ce qui est détruit devient des **deltas** (ruines, cratères, incendies) sauvés et partagés.

---

## 4. Les formes (la silhouette vue de l'espace)

Tirées d'après la race, l'âge, la progression (§6.3) et la planète natale. Chaque forme se voit **de loin** : c'est elle qui rend
une capitale reconnaissable dans le ciel.

| # | Forme | Description | Déformation |
|---|---|---|---|
| F1 | **Sphère couverte** | La ville épouse la planète ; les océans ont disparu ou sont devenus des réservoirs géométriques | faible |
| F2 | **Couronne de méga-tours** | Quelques tours de 20 000 à 200 000 voxels dépassent l'atmosphère, comme des épines | moyenne |
| F3 | **Anneau soudé** | Un anneau orbital relié au sol par des **ascenseurs** ; la ville déborde sur l'anneau | moyenne |
| F4 | **Bourrelet équatorial** | La ville a grandi plus haut à l'équateur : planète aplatie, ceinture de plateformes en surplomb | moyenne |
| F5 | **Hémisphère creusé** | Une moitié de la planète **exploitée** jusqu'au manteau : immense cratère en gradins, fonderies orange au fond (S5) | forte |
| F6 | **Pôles en tours** | Deux tours polaires gigantesques (radiateurs, antennes), la planète en « toupie » | forte |
| F7 | **Excroissances** | Blocs de ville empilés au-delà de la sphère : la planète porte des « continents » artificiels en relief de 1 000 km | forte |
| F8 | **Ville sur une géante** | Plateformes et tours suspendues dans les nuages d'une géante gazeuse, reliées entre elles (pas de sol) | à part |
| F9 | **Lune-forteresse** | Une lune entièrement construite qui garde la capitale (chantier naval, défense) | à part |

La **capitale galactique** (§7) a ses propres formes, bien plus déformées.

---

## 5. Les styles (la ville vue de près)

Chaque race a une **culture** tirée une fois pour toutes (graine de la race) : un style principal, une
**palette**, un **éclairage**, une **verticalité**, une **densité**, une **part de verdure**. Une capitale mélange
son style principal (≈ 70 %) avec des quartiers d'autres styles (commerçants, anciens, étrangers).

| Style | Formes | Matières et palette | Lumière la nuit | Signature |
|---|---|---|---|---|
| **S1 Rétro-futur** | Tours rondes, dômes, anneaux, ailerons, passerelles courbes, jardins suspendus | Crème, cuivre, cyan, vert d'eau ; verre | Cyan et or, enseignes rondes | Places circulaires lumineuses, trafic coloré |
| **S2 Gothique impérial** | Flèches, arcs-boutants, rosaces, viaducs à arches, **statues colossales**, palais doré | Pierre sombre, bronze, or, vitraux | Or chaud, cierges, brume verte | **Avenue des statues** menant au palais |
| **S3 Aiguilles et dômes** | Aiguilles fines très hautes, dômes bas et larges, pyramides tronquées, plaine de blocs à l'infini | Gris clair, blanc, ocre | Blanche, millions de points, files de vaisseaux | Horizon infini, temple-pyramide |
| **S4 Colonnes** | Colonnes-tours à plateaux en soucoupe, anneaux, **sol couvert** (dalle métallique gravée), puits vers le dessous | Métal pastel (lilas, gris bleu), céramique | Douce, diffuse, sous la dalle : néons | Le ciel n'est visible qu'au-dessus ; dessous, une autre ville |
| **S5 Fonderie** | Blocs empilés, gradins, cheminées, passerelles sur le vide, canyons sans fond | Rouille, acier, suie | **Orange** qui monte du fond, étincelles | Les canyons et leur lueur |
| **S6 Organique** | Coquilles, nervures, tours en spirale, membranes | Nacre, chitine, couleurs de la faune locale | **Bioluminescente** | Races organiques (0.13 L5) |
| **S7 Cristallin** | Facettes, pyramides de verre, arcologies transparentes | Verre teinté, métaux clairs | Lumière qui traverse les façades | Mondes riches |
| **S8 Bidonville vertical** | Ajouts de toutes sortes, câbles, bâches, antennes | Récupération, toutes couleurs | Néons désordonnés | Bas-fonds de toutes les capitales |

Diversité **dans** une capitale : quartiers (§6), âge des couches, état, climat (pilotis sur une ancienne mer,
dômes sur un monde sans air, radiateurs sur un monde chaud, glace sur un monde froid), espèce (la **taille des
portes et des marches** suit la taille de l'espèce).

---

## 6. La ville en profondeur : quartiers et niveaux

### 6.1 Quartiers (carte de la planète)

La surface est découpée en **districts** (diagramme de Voronoï sur la sphère, bords suivant le relief ancien) :

| District | Ce qui le distingue |
|---|---|
| **Palais / gouvernement** | Unique ; monument principal, grandes avenues, zone interdite au joueur |
| **Spatioport** | Plateformes d'atterrissage, tours de contrôle, hangars ; **le joueur se pose ici** |
| **Affaires** | Les tours les plus hautes, verre, publicités |
| **Résidentiel haut** | Jardins suspendus, terrasses, calme |
| **Résidentiel dense** | Blocs serrés, lumières de fenêtres, linge, antennes |
| **Industriel** | Usines, cheminées, fumées, rails (S5) |
| **Marché** | Halles, foule, couleurs, enseignes (lien économie) |
| **Ancien** | Couche la plus vieille, architecture d'un autre style, monuments usés |
| **Parc / réservoir** | Rare et précieux : forêt ou lac artificiel, vu de l'espace comme une tache verte ou bleue |
| **Chantier** | Grues, échafaudages, squelettes de tours (la ville grandit) |
| **Ruines** | Quartier effondré ou abandonné (âge, guerre) |

Réseau : **artères** lumineuses entre districts (visibles la nuit de l'espace), **couloirs aériens** au-dessus,
**monorails**, **viaducs**.

### 6.2 Niveaux (la verticalité)

| Niveau | Hauteur | Ambiance |
|---|---|---|
| **Cimes** | sommets des tours | Ciel clair, vent, plateformes privées, vue sur les nuages |
| **Ville haute** | toits et ponts | Lumière du jour, trafic aérien, jardins |
| **Ville moyenne** | façades, viaducs | Ombre, passerelles, enseignes, monorail |
| **Bas-fonds** | le fond des canyons | Presque pas de soleil : lumière artificielle, brume, fumées, bidonvilles (S8) |
| **Sous-ville** | sous la dalle ou dans la croûte | Fonderies, réservoirs, machines, lueurs orange, grottes réutilisées (`caves.rs`) |

La lumière du soleil descend selon la **largeur du canyon** (calcul d'ouverture du ciel par colonne) ; plus bas,
la brume s'épaissit et les lumières artificielles prennent le relais : c'est ce contraste qui fait la profondeur
des images de la planche.

### 6.3 Croissance : la ville se construit selon sa progression et ses ressources (D4, V13)

Une ville n'est pas posée d'un coup : elle **pousse** depuis un point de fondation, étape par étape, et chaque
étape est tirée de la graine de la ville. Le résultat d'aujourd'hui est la **somme de son histoire**.

**Fondation.** Le premier noyau est posé au meilleur endroit de la planète natale : eau douce, côte abritée,
plaine fertile, gisement riche (le score vient des données de la planète : relief, biomes, mers, gisements).

**Stades de progression** (le stade vient du niveau de la race, 0.13 L5, et de l'âge de sa civilisation) :

| Stade | Ville | Ce qui apparaît |
|---|---|---|
| 1 | **Village** | Maisons basses autour du point d'eau, champs, sentiers ; le reste de la planète est naturel |
| 2 | **Cité** | Remparts ou digues, premier monument, routes vers les gisements, port |
| 3 | **Métropole** | Tours, viaducs, banlieues ; d'autres villes sur la planète, reliées par des artères |
| 4 | **Mégalopole** | Les villes se rejoignent en bandes continentales ; mines à ciel ouvert, mers endiguées |
| 5 | **Écuménopole** | Toute la planète couverte ; la nature ne reste que dans les parcs ; la forme (§4) commence à se déformer |
| 6 | **Hors de la planète** | Méga-tours, anneau soudé, lune-forteresse, excroissances (formes F2 à F9) |

**Ressources** : chaque quartier naît là où sa ressource se trouve (règle V13) :

| Ressource de la planète | Ce qui pousse dessus |
|---|---|
| Gisements de métaux (fer, nickel, cuivre...) | Mines, puis fonderies et canyons industriels (S5), puis l'**hémisphère creusé** (F5) si la ville épuise sa planète |
| Eau, mers | Ports, villes sur pilotis, puis réservoirs géométriques |
| Volcans, geysers, chaleur interne | Centrales, quartiers chauds, cheminées |
| Sols fertiles, biomes vivants | Fermes, puis fermes verticales, jardins suspendus, parcs |
| Gaz rares, hélium-3 (lunes, géantes) | Stations orbitales, chantiers navals, lune-forteresse (F9) |
| Rien de tout cela | Ville importatrice : grand spatioport, convois de cargos vers les mondes voisins |

**Couches** : chaque stade laisse sa trace : le **vieux centre** (style ancien, usé) au milieu, des **remparts**
devenus avenues, d'anciens **ports** au milieu des terres quand la mer a été comblée, des **ruines** quand un
quartier a été abandonné.

**Dans le temps de jeu** : la progression avance lentement avec l'horloge du monde et l'économie de la race
(commerce, guerres, sièges de `claims.rs`) : un **chantier** visible annonce le prochain ajout ; tous les joueurs
voient le même stade (f(graine, horloge, deltas)).

---

## 7. La capitale galactique (le monde-station)

Une station **bâtie par plusieurs races** pour le **commerce et la politique** (D2) : des stations de plusieurs
espèces se sont accrochées les unes aux autres pendant des siècles. Une par galaxie générée au départ
(proposition), là où les territoires des races se rencontrent.

- **Quartiers par race (D3)** : **chaque race présente a son quartier**, dans **son style** (§5), avec **son air,
  sa gravité, sa lumière, la taille de ses portes**. Les races présentes sont celles de la galaxie qui ont atteint
  le stade spatial ; leur quartier est d'autant plus grand que la race est puissante (étoiles, commerce). Une
  race qui arrive ajoute un quartier (chantier) ; une race en guerre voit son quartier fermé, gardé ou en ruines.
- **Lieux communs** : **assemblée** (salle politique de toutes les races, monument central), **grand marché
  interracial**, **ambassades**, quais neutres, quartier des marchands sans race.
- **Croissance** : la station suit la règle V13 : un noyau ancien (la première station), puis les quartiers
  des races dans l'ordre de leur arrivée.

- **Forme** : agrégat **fortement asymétrique** de formes (cylindres, tores, plaques, sphères, grappes de dômes),
  avec une **dérive** générée (croissance par couches : le noyau ancien au centre, les ajouts récents dehors),
  des **bras**, des **épines** (antennes, quais), un **disque** de débris et de vaisseaux autour ; taille d'une
  petite lune.
- **Modules d'espèces** : chaque grappe est dans le **style** de l'espèce qui l'a construite (S1 à S8), avec **son
  atmosphère, sa gravité, sa lumière** (un module aquatique rempli d'eau, un module à méthane, un module en
  apesanteur). Le scanner les liste.
- **Gravité** : artificielle, vers le **plancher local** du module (fictif, étiqueté) ; passage doux entre
  modules.
- **Vie** : quais d'amarrage partout, trafic dense, **docks** du joueur (H, `dock.rs`), marché de toutes les
  races, quartier diplomatique.
- **De l'espace** : silhouette hérissée et lumineuse, devant une **nébuleuse** si possible (le jeu en a déjà), la
  plus reconnaissable de la galaxie.

---

## 8. Ce qui la fait vivre

| Élément | Détail |
|---|---|
| **Trafic aérien** | Files de vaisseaux sur des **couloirs** à plusieurs altitudes, f(graine, horloge) : tous les joueurs voient les mêmes ; instanciés ; feux de position la nuit ; certains se posent et décollent du spatioport |
| **Lumières** | Fenêtres qui s'allument au crépuscule (par quartier, par heure locale), néons, hologrammes publicitaires, phares de balisage clignotants en haut des tours |
| **Vue de l'espace la nuit** | **Carte de lumière** : artères, districts, spatioports ; c'est la plus belle image de la capitale |
| **Transports** | Monorails, ascenseurs dans les tours, ascenseurs spatiaux (F3), dirigeables, navettes vers la lune-forteresse |
| **Foule** | De loin : points lumineux qui bougent ; de près : PNJ (modèles de l'éditeur, familles de races de la 0.12) |
| **Ciel urbain** | Brume de pollution teintée par les lumières (le ciel orange la nuit), fumées des cheminées, îlot de chaleur, pluie qui ruisselle sur les tours, **éclairs** sur les méga-tours |
| **Orbite** | Satellites miroirs, stations de défense, chantiers navals, file d'attente de vaisseaux en orbite |
| **Son** | Rumeur de la ville, trafic, annonces du spatioport (avec le son de la 0.13 P9) |
| **Temps** | Jour / nuit, saisons sur les jardins, **fêtes** (illuminations, défilés aériens), **couvre-feu** (état de guerre) |

---

## 9. Jouer dans une capitale

- **Arriver** : le contrôle du trafic donne un **couloir d'approche** et une **plateforme** au spatioport
  (aides d'approche de la 0.13 P7) ; voler hors des couloirs = avertissement (et, si la race est ennemie, défense).
- **Se poser et marcher** : plateformes, rues hautes, halls ; **ascenseurs** pour changer de niveau ; certaines
  zones interdites (palais).
- **Commerce** : le **grand marché** de la race (tous ses biens, meilleurs prix, `economy.rs`) ; réparations
  et améliorations du vaisseau.
- **PNJ** : dialogue (`npc_ui.rs`), missions de la race, colis à livrer, diplomatie (relations de `combat.rs`).
- **Découverte** : monuments, points de vue, sous-ville, ruines ; tout entre dans le **dex**.
- **Multijoueur** : point de rencontre naturel ; les guildes peuvent y avoir une **ambassade** (plus tard).
- **États** : une capitale peut être **assiégée** (lien avec les sièges de `claims.rs`), **en déclin**, ou
  **morte** (monde mort de la 0.14 X8 : même ville, éteinte, envahie par la nature).
- **Détruire (D5, V14)** : seulement une capitale (ou un quartier de la capitale galactique) **ennemie** du joueur.
  Défenses (batteries, boucliers, chasseurs) qui ripostent ; dégâts en **deltas** : tours qui s'effondrent,
  incendies, cratères, quartier en ruines ; la race perd de la puissance (marché, territoire) ; la ville se
  **reconstruit** lentement (chantiers) si la race survit. Une capitale amie ou neutre est intouchable.

---

## 10. Les versions et leurs phases

### 0.18.1 — « Fondations » : la capitale existe

- **CA1. Qui et où** : `planetgen/capital.rs` (nouvelle sous-graine figée `Layer::Capital`) : une capitale par
  **race extraterrestre avancée, sur sa planète natale** (D1, races de la 0.13 L5), stade de progression (§6.3),
  forme (§4) et culture (§5) tirées ; une capitale galactique par galaxie générée au départ (D2) ; type d'astre
  `Capital` dans le profil, le scanner, le dex, `/stats` ; `/aller capitale [style|forme]`. PROTOCOL +1.
- **CA2. Bible visuelle** : pour chaque style, une **planche** (palette, silhouettes, 10 modules d'essai faits
  dans l'éditeur, captures) dans `assets/villes/<style>/` ; c'est la référence des phases suivantes.
- **CA3. Vue de l'espace minimale** : la planète couverte (couleur des toits par district), **carte de lumière**
  la nuit, nuages de pollution. Déjà belle de loin avant tout le reste.

### 0.18.2 — « Formes » : silhouettes non sphériques

- **CA4. `BodyShape`** (règle V3) : sphère déformée et composition de formes ; terrain, collisions
  (`Terrain::floor` / `ceiling`), gravité (vers la surface proche), maillage lointain (nouveau mailleur de surface
  par champ de distance), ombres. Reprend `shape(dir)` de la 0.14 X4.
- **CA5. Les formes F1 à F9** (§4) : méga-tours, anneau soudé et ascenseurs, bourrelet, hémisphère creusé,
  pôles, excroissances, ville sur géante, lune-forteresse. Captures de l'espace pour chacune.

### 0.18.3 — « Quartiers » : la ville vue en vol

- **CA6. Croissance et districts** (§6.3, §6.1, règle V13) : fondation au meilleur endroit, stades 1 à 6, quartiers
  placés sur les ressources (gisements, mers, volcans, sols fertiles), couches anciennes ; districts : Voronoï sur la sphère, types, artères, couloirs aériens ; la carte de lumière de CA3
  vient maintenant des vrais districts (règle V2).
- **CA7. Grammaire des bâtiments** (règle V4) : socle, fût, couronne, flèche, ponts, par style ; îlots, rues,
  places ; modules instanciés ; LOD de ville (règle V5 : toits → imposteurs → modules).
- **CA8. Niveaux** (§6.2) : cimes, ville haute, moyenne, bas-fonds, sous-ville ; **ouverture du ciel** par
  colonne (lumière qui descend), brume des profondeurs, lumières artificielles en bas.

### 0.18.4 — « Monuments »

- **CA9. Monuments uniques** par capitale : palais, dôme du conseil, temple-pyramide, avenue des statues
  colossales, arche, spatioport principal, méga-tour emblème, puits vers la sous-ville. Chaque monument =
  une grammaire à part avec ses variantes ; visibles et cliquables au scanner (« point d'intérêt »).
- **CA10. Viaducs et ponts géants** entre tours et districts, arches de plusieurs centaines de voxels, aqueducs,
  ascenseurs spatiaux.

### 0.18.5 — « Vie »

- **CA11. Trafic** (§8) : couloirs, files instanciées, feux, décollages / atterrissages, f(graine, horloge).
- **CA12. Lumières vivantes** : fenêtres par heure locale, néons, hologrammes, balises ; lumière émissive (règle V6).
- **CA13. Ciel urbain** : brume teintée, fumées, chaleur, pluie sur les façades, éclairs sur les méga-tours ; orbite
  peuplée (satellites, chantiers, file d'attente).

### 0.18.6 — « À pied »

- **CA14. Rues praticables** : plateformes, rues hautes, escaliers, rambardes, collisions des modules, ascenseurs
  entre niveaux, quelques **intérieurs** (hall du spatioport, halle du marché, salle d'amarrage).
- **CA15. Foule et PNJ** : silhouettes de loin, PNJ animés de près (races de l'éditeur), dialogue (`npc_ui.rs`).
- **CA16. Détails** : enseignes, fenêtres, végétation des jardins, linge, câbles, déchets dans les bas-fonds ; sons.

### 0.18.7 — « Capitale galactique »

- **CA17. La capitale galactique** (§7, D2) : agrégat : forme générée par croissance, modules d'espèces, bras, épines, disque de débris.
- **CA18. Quartiers des races** (D3) : un quartier par race présente, dans son style, avec son air, sa gravité, sa
  lumière ; taille selon la puissance de la race ; assemblée, grand marché interracial, ambassades, docks.

### 0.18.8 — « Capitale vivante » : le jeu

- **CA19. Arriver** : contrôle du trafic, couloir d'approche, plateforme attribuée, avertissements, défense.
- **CA20. Commerce et missions** : grand marché, réparations, missions de la race, diplomatie.
- **CA21. États, événements et destruction** : fêtes, couvre-feu, siège, déclin, capitale morte (lien 0.14 X8 et N) ;
  **destruction seulement si ennemie** (D5, règle V14) : défenses, effondrements, incendies, ruines, reconstruction ;
  deltas partagés en réseau ; progression de la ville dans le temps de jeu (chantiers, §6.3).

### 0.18.9 — « Finitions »

- **CA22. Diversité** : test d'empreinte (règle V7) sur toutes les capitales générées ; chaque style et chaque
  forme présents ; planche de toutes les capitales de la galaxie principale.
- **CA23. Performance** : scénario K du benchmark (0.17), budgets (règle V10), réglages de ville dans `Tuning`.
- **CA24. Captures de référence** (règle V9) et mise à jour de `CLAUDE.md`, `TOUCHES.md`, `TESTS-JEU.md`.

```
0.17 terminée
   │
   CA1 → CA2 → CA3         ── 0.18.1 Fondations ──
   CA4 → CA5               ── 0.18.2 Formes ──
   CA6 → CA7 → CA8         ── 0.18.3 Quartiers ──
   CA9 → CA10              ── 0.18.4 Monuments ──
   CA11 → CA12 → CA13      ── 0.18.5 Vie ──
   CA14 → CA15 → CA16      ── 0.18.6 À pied ──
   CA17 → CA18             ── 0.18.7 Capitale galactique ──
   CA19 → CA20 → CA21      ── 0.18.8 Capitale vivante ──
   CA22 → CA23 → CA24      ── 0.18.9 Finitions ──
```

Dépendances : **CA1 a besoin des races de la 0.13 L5** (espèces, niveau, planète natale) ; CA6 des gisements (0.10 phase 8, 0.15) ; CA21 de la destruction de la 0.15 ; CA4 utilise la `shape(dir)` de la 0.14 X4 ; CA15 les races de l'éditeur (0.12) ; CA6-CA8 le relief
des 0.13 T ; CA11 le son de la 0.13 P9 ; CA21 les événements de la 0.14 N ; CA23 les outils de la 0.17.

---

## 11. Questions à trancher

| # | Question | Proposition |
|---|---|---|
| Q1 | Combien de capitales ? | **Décidé** (D1, D2) : une par race extraterrestre avancée sur sa planète natale + une capitale galactique par galaxie générée au départ (21) |
| Q2 | Une capitale près du départ ? | Non (zone calme de la 0.14) ; la plus proche à quelques sauts, signalée sur la carte |
| Q3 | Planète natale | **Décidé** (D1) : la capitale est sur la planète natale de la race (celle où elle est née, 0.13 L5) |
| Q4 | Détruire ou construire ? | **Décidé** (D5) : détruire seulement si ennemie ; pas de construction du joueur en 0.18 |
| Q5 | Gravité de la capitale galactique | Artificielle vers le plancher du module (fictif) |
| Q6 | Taille de la capitale galactique | Celle d'une petite lune (rayon ~ un tiers de la Terre) |
| Q7 | Intérieurs | Seulement quelques lieux (spatioport, marché, amarrage) en 0.18 ; plus tard le reste |
| Q8 | Modules des bâtiments | Faits par **script** (comme les blocs de l'éditeur) puis retouchés dans l'éditeur ; le joueur pourra ajouter les siens dans `saves/villes/` |

---

## 12. Prompts (à coller dans une nouvelle session, un par phase)

Contexte commun : « Lis `roadmaps/a-faire/ROADMAP-0.18-capitales.md` (règles V1 à V12, formes §4, styles §5,
niveaux §6, capitale galactique §7, croissance §6.3, décisions §2 et §11), `CLAUDE.md` et `TESTS-JEU.md`, et regarde la planche
`C:\Users\thomr\Desktop\Nouveau dossier\doctravail\06-next maj\0.18`. `git pull origin main` avant de coder.
Branche `claude/roadmap-0-18-<phase>`. Build release, tests, mesures (outils de la 0.17), captures de référence
(jour, couchant, nuit, espace, vol bas, à pied). PR non fusionnée (je dirai « push main »). Aucune concession sur
la qualité ; aucun nom ni forme d'œuvre protégée. »

- **CA1** — « [contexte commun] Phase CA1 : `planetgen/capital.rs`, une capitale par race avancée sur sa planète natale (D1), une capitale galactique par galaxie (D2), stade, forme et
  culture tirées, type `Capital` partout (profil, scanner, dex, `/stats`), `/aller capitale` ; PROTOCOL +1. »
- **CA2** — « [contexte commun] Phase CA2 : bible visuelle par style (palette, silhouettes, 10 modules d'essai,
  captures) dans `assets/villes/<style>/`. »
- **CA3** — « [contexte commun] Phase CA3 : capitale vue de l'espace : toits colorés par district, carte de
  lumière la nuit, nuages de pollution. »
- **CA4** — « [contexte commun] Phase CA4 : `BodyShape` (règle V3) : terrain, collisions, gravité, maillage
  lointain par champ de distance, ombres ; réutilise `shape(dir)` de la 0.14 X4. »
- **CA5** — « [contexte commun] Phase CA5 : les formes F1 à F9 (§4), une capture de l'espace par forme. »
- **CA6** — « [contexte commun] Phase CA6 : croissance selon la progression et les ressources (§6.3, règle V13), districts (§6.1), artères, couloirs aériens, carte de lumière issue
  des districts. »
- **CA7** — « [contexte commun] Phase CA7 : grammaire des bâtiments par style, îlots, modules instanciés, LOD de
  ville (règle V5). »
- **CA8** — « [contexte commun] Phase CA8 : niveaux (§6.2), ouverture du ciel par colonne, brume des
  profondeurs, lumières artificielles des bas-fonds. »
- **CA9** — « [contexte commun] Phase CA9 : monuments uniques par capitale (palais, dôme, pyramide, avenue des
  statues, arche, spatioport, méga-tour, puits), points d'intérêt au scanner. »
- **CA10** — « [contexte commun] Phase CA10 : viaducs, ponts géants, aqueducs, ascenseurs spatiaux. »
- **CA11** — « [contexte commun] Phase CA11 : trafic aérien f(graine, horloge), couloirs, files instanciées,
  feux, décollages et atterrissages. »
- **CA12** — « [contexte commun] Phase CA12 : fenêtres selon l'heure locale, néons, hologrammes, balises, lumières
  émissives (règle V6). »
- **CA13** — « [contexte commun] Phase CA13 : ciel urbain (brume teintée, fumées, pluie, éclairs), orbite peuplée. »
- **CA14** — « [contexte commun] Phase CA14 : rues praticables, collisions des modules, ascenseurs, intérieurs du
  spatioport, du marché et de l'amarrage. »
- **CA15** — « [contexte commun] Phase CA15 : foule de loin, PNJ animés de près (races de l'éditeur), dialogue. »
- **CA16** — « [contexte commun] Phase CA16 : détails de rue, jardins, bas-fonds, sons de la ville. »
- **CA17** — « [contexte commun] Phase CA17 : la capitale galactique (§7, D2), forme par croissance, bras, épines, débris. »
- **CA18** — « [contexte commun] Phase CA18 : un quartier par race présente (D3) dans son style (air, gravité, lumière), assemblée, ambassades, passages,
  docks, marché commun. »
- **CA19** — « [contexte commun] Phase CA19 : contrôle du trafic, couloir d'approche, plateforme attribuée,
  avertissements, défense. »
- **CA20** — « [contexte commun] Phase CA20 : grand marché, réparations, missions, diplomatie. »
- **CA21** — « [contexte commun] Phase CA21 : fêtes, couvre-feu, siège, déclin, capitale morte ; destruction seulement si ennemie (D5, V14) ; progression dans le temps de jeu ; deltas réseau. »
- **CA22** — « [contexte commun] Phase CA22 : test d'empreinte (règle V7), planche de toutes les capitales. »
- **CA23** — « [contexte commun] Phase CA23 : scénario K du benchmark, budgets (règle V10), réglages dans `Tuning`. »
- **CA24** — « [contexte commun] Phase CA24 : captures de référence, mise à jour de `CLAUDE.md`, `TOUCHES.md`,
  `TESTS-JEU.md`. »
