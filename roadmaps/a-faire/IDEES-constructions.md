# Banque d'idées — Constructions (stations, habitats, mégastructures, bâtiments)

Toutes les constructions imaginables pour SpaceSpore, de la cabane posée au sol jusqu'à la sphère de Dyson.
Ce fichier est une **banque d'idées**, pas une feuille de route : rien n'est décidé. Quand une idée est choisie,
elle passe dans une `ROADMAP-x.y.md` avec ses phases et ses questions.

Pour chaque idée : ce que c'est, puis **en jeu** (comment l'intégrer dans SpaceSpore : échelle, gameplay, ce
qui existe déjà). Étiquettes : **[réel]** (existe ou est en projet), **[théorique]** (physiquement possible,
étudié par des scientifiques), **[fiction]** (science-fiction pure, étiquetée comme les minerais fictifs,
`Realism::Fictional`).

Rappel des échelles du jeu (`CLAUDE.md`) : planète de type Terre ~ 21 000 voxels de rayon au sol (k = 16),
1 R_terre = échelle du système / 109, système ~19 M d'unités en médiane, galaxie de rayon 13,5 G. Une
mégastructure **à vraie taille** est donc immense : il faudra le même LOD que les planètes (loin = maillage
simple, près = voxels).

---

## Comment on construit (mécaniques communes)

Avant les objets eux-mêmes, les idées de **système de construction** qui servent à tous :

- **Plans dans l'éditeur** (0.12, grilles jusqu'à 1024³, E3) : on dessine une station ou un bâtiment dans
  l'éditeur, comme un vaisseau ; nouvelle catégorie de modèle « Construction » (au sol, en orbite, géante).
- **Blocs de construction** (comme les blocs de vaisseau E6) : modules fonctionnels avec une animation et un
  rôle (réacteur, quai, hangar, serre, antenne, tourelle, panneau solaire, réservoir, anneau qui tourne...).
- **Chantier** : une construction se bâtit en étapes (fondations → structure → modules), elle consomme des
  ressources (minerais de la phase 8, minage 0.15) et du temps de jeu (horloge du monde) ; **drones de
  construction** visibles qui volent autour.
- **Échafaudages** et grues visibles pendant le chantier ; le plan apparaît en fantôme (comme le gabarit
  `Ghost` de l'éditeur).
- **Énergie et logistique** : réseau électrique (solaire, géothermie près des geysers de T5, fusion,
  antimatière), stockage, convois de cargos automatiques entre les constructions.
- **Entretien et usure** : une construction abandonnée vieillit (rouille, végétation qui pousse dessus, débris),
  devient une ruine ou une épave à explorer.
- **Multijoueur** : chantiers partagés, construction en coopération, permissions par guilde, construction
  envoyée par deltas (comme les voxels) ; une grande construction est vue par tous.
- **Factions** : les factions IA (`economy.rs`) ont déjà leurs stations ; elles pourraient aussi construire
  (et leurs constructions dépendre de leur richesse et de leur niveau technologique).
- **Niveaux technologiques** : on débloque les constructions dans l'ordre (cabane → base → station →
  habitat → mégastructure), la mégastructure est un objectif de fin de partie collectif (plusieurs joueurs,
  plusieurs heures ou jours de jeu).
- **Découverte** : beaucoup de ces structures peuvent aussi exister **déjà construites** par des aliens (L5)
  ou des civilisations disparues : ruines à explorer, scannables, inscrites au dex.

---

## 1. Mégastructures (échelle stellaire et du système)

Les plus monumentales : elles entourent ou exploitent une étoile entière.

- **Sphère de Dyson** [théorique] — coquille creuse autour d'une étoile qui capte 100 % de son énergie.
  **En jeu** : l'étoile disparaît du ciel des planètes du système (nuit permanente, sauf à l'intérieur) ; vue
  de loin, une étoile sombre qui brille en infrarouge (lueur rouge sourde) ; repérable au scanner comme une
  « étoile anormale ». Version alien abandonnée = donjon géant à explorer de l'intérieur.
- **Essaim de Dyson** [théorique] — millions de satellites ou miroirs indépendants en orbite qui collectent
  l'énergie. **En jeu** : version réaliste et progressive : chaque panneau construit s'ajoute à l'essaim
  (cellules hachées comme les astéroïdes `asteroids.rs`) ; l'étoile scintille et s'assombrit à mesure que
  l'essaim grandit ; énergie reçue = part de l'étoile couverte.
- **Bulle de Dyson** [théorique] — voiles solaires immobiles (« statites ») tenues en place par la pression
  de la lumière. **En jeu** : coquille de voiles très fines, translucides, qui ondulent.
- **Filet / coquille de Dyson partielle** [théorique] — un seul anneau ou une ceinture de collecteurs (étape
  avant l'essaim). **En jeu** : premier palier de la mégastructure, visible comme un anneau autour de l'étoile.
- **Cerveau de Matriochka** [théorique] — couches de Dyson emboîtées, chacune utilise la chaleur perdue de la
  précédente pour calculer. **En jeu** : couches de couleurs différentes (de la plus chaude à la plus froide) ;
  bonus de recherche ou de scanner pour toute la galaxie ; IA de faction qui y vit.
- **Cerveau de Jupiter** [théorique] — ordinateur de la taille d'une planète. **En jeu** : planète artificielle
  couverte de circuits lumineux la nuit.
- **Anneau-Monde (Ringworld)** [théorique / fiction] — anneau autour d'une étoile, face interne habitable
  avec atmosphère, gravité par rotation, murs de bord de 1 000 km. **En jeu** : un « astre » à part avec terrain
  voxel sur la face interne (relief, mers, biomes de `planetgen`), horizon qui **monte** au loin, l'arche de
  l'anneau visible dans le ciel, écrans d'ombre pour la nuit.
- **Disque d'Alderson** [théorique] — disque plein autour de l'étoile, habitable sur les deux faces.
  **En jeu** : surface immense, crépuscule éternel près du bord.
- **Topopolis** [théorique] — un cylindre habitable si long qu'il fait plusieurs fois le tour de l'étoile, en
  nœuds. **En jeu** : vol à l'intérieur d'un tube sans fin, paysages qui changent.
- **Monde-coquille (Shellworld) / monde supramondain** [théorique] — coquilles emboîtées autour d'une petite
  planète ou d'une géante, soutenues par la pression ou par des piliers. **En jeu** : plusieurs « sols »
  superposés, la vraie planète visible tout en bas.
- **Moteur stellaire de Shkadov** [théorique] — miroir géant qui renvoie la lumière d'un côté et pousse
  lentement l'étoile (et son système) dans la galaxie. **En jeu** : le système entier se déplace dans la
  galaxie au fil des jours de jeu (position `abs_center` qui change).
- **Propulseur de Caplan** [théorique] — moteur stellaire qui puise la matière de l'étoile par champ
  magnétique et éjecte un jet. **En jeu** : jet de plasma visible sur des millions d'unités.
- **Station de « star lifting »** [théorique] — prélever la matière d'une étoile (hydrogène, métaux) ;
  allonge sa vie. **En jeu** : ressource rare, étoile qui pâlit au fil du temps.
- **Rayon de Nicoll-Dyson** [théorique] — essaim de Dyson qui concentre toute l'énergie en un laser
  interstellaire. **En jeu** : arme de fin de partie ou propulsion de voiles à travers la galaxie.
- **Lentille gravitationnelle solaire** [réel / projet] — télescope placé à 550 UA d'une étoile qui utilise sa
  gravité comme lentille. **En jeu** : observatoire qui révèle les planètes de systèmes lointains sans y aller
  (scanner à distance, dex).
- **Sphère de Penrose / centrale de trou noir** [théorique] — extraire l'énergie de rotation d'un trou noir.
  **En jeu** : autour des trous noirs galactiques (`black_hole.rs`) ; disque d'accrétion exploité.
- **Kugelblitz** [théorique] — trou noir artificiel créé par concentration de lumière, source d'énergie ou
  moteur. **En jeu** : projet de fin de partie, très dangereux.
- **Étoile artificielle / allumage d'une géante** [fiction] — allumer une géante gazeuse pour en faire une
  petite étoile. **En jeu** : change le système (nouvelle source de lumière, zone habitable déplacée).
- **Planète artificielle** [fiction] — monde entièrement construit, machine de la taille d'une planète.
  **En jeu** : type de planète rare, surface en plaques métalliques, failles lumineuses.

## 2. Habitats spatiaux massifs (échelle planétaire)

Des espaces de vie gigantesques sans entourer une étoile.

- **Cylindre d'O'Neill (Island Three)** [théorique] — deux cylindres de ~30 km tournant en sens inverse ;
  intérieur avec terres, rivières, villes, fenêtres et miroirs pour le jour. **En jeu** : on y entre en vaisseau
  par l'axe (gravité nulle au centre), on se pose sur le sol intérieur ; terrain voxel enroulé, ciel = l'autre
  côté du cylindre.
- **Tore de Stanford** [théorique] — station en forme de beignet de ~1,8 km qui tourne pour la gravité.
  **En jeu** : première grande station habitable que les joueurs peuvent construire.
- **Roue / anneau de Bishop** [théorique] — anneau de 1 000 km de rayon sans toit (l'atmosphère tenue par
  des murs et la rotation). **En jeu** : ciel ouvert sur l'espace, étoiles visibles de jour.
- **Sphère de Bernal** [théorique] — sphère habitable de ~500 m, population sur l'équateur intérieur.
  **En jeu** : petit habitat de colonie.
- **Cylindre de McKendree** [théorique] — cylindre en nanotubes de carbone de 4 600 km de rayon.
  **En jeu** : un continent entier à l'intérieur.
- **Orbital de la Culture** [fiction, Iain M. Banks] — anneau de ~3 millions de km de diamètre en orbite
  autour d'une étoile, jour et nuit par sa propre rotation. **En jeu** : entre le tore et l'anneau-monde.
- **Globus Cassus** [théorique] — la Terre transformée en coquille creuse géante. **En jeu** : planète creuse
  (déjà dans la banque d'idées de `roadmaps/a-faire/prompt0.14.md`) : surface extérieure + monde intérieur.
- **Astéroïde creusé en rotation** [théorique] — habitat taillé dans un astéroïde (Kalpana One, astéroïde de
  Rama). **En jeu** : sur les gros astéroïdes de `asteroids.rs` où l'on se pose ; entrée par un puits, ville
  intérieure.
- **Arche / vaisseau générationnel** [théorique] — habitat qui voyage des siècles entre les étoiles.
  **En jeu** : vaisseau-monde des factions qui se déplace lentement entre les systèmes, on peut s'y amarrer
  (`dock.rs`).
- **Halo** [fiction] — anneau-monde plus petit autour d'une planète. **En jeu** : visible dans le ciel des
  planètes comme une arche.
- **Monde-disque** [fiction] — monde plat (Pratchett). **En jeu** : planète étrange rare, chute d'eau au bord.
- **Cycleur d'Aldrin** [réel / projet] — station sur une orbite qui passe régulièrement près de deux planètes.
  **En jeu** : transport en commun entre deux planètes, horaires selon l'horloge du monde.

## 3. Infrastructures orbitales et planétaires

Elles relient la surface et l'espace.

- **Ascenseur spatial** [théorique] — câble de la surface jusqu'au-delà de l'orbite géostationnaire, avec un
  contrepoids. **En jeu** : câble visible du sol jusqu'au ciel (lien direct avec le bloc P : monter sans
  fusée), cabine qui monte en quelques minutes, station en haut ; sur une planète à rotation lente il est
  impossible (calcul de l'orbite géostationnaire depuis le jour réel).
- **Crochet orbital (skyhook)** [théorique] — attache en rotation qui plonge dans la haute atmosphère pour
  attraper les vaisseaux et les lancer. **En jeu** : mini-jeu d'accrochage, lancement vers une autre planète.
- **Anneau orbital** [théorique] — anneau continu autour d'une planète à basse altitude, soutenu par un
  anneau tournant, avec des ascenseurs vers le sol. **En jeu** : visible dans le ciel d'une planète comme une
  ligne d'un horizon à l'autre.
- **Boucle de lancement (Lofstrom)** [théorique] — boucle de 2 000 km qui lance des charges à vitesse
  orbitale. **En jeu** : lanceur de cargos.
- **Canon / catapulte électromagnétique (mass driver)** [théorique] — rail qui lance des charges depuis une
  lune sans air. **En jeu** : sur les lunes, expédie le minerai vers l'orbite (traînée lumineuse).
- **StarTram** [théorique] — tube sous vide qui monte en pente jusqu'à 20 km d'altitude.
- **Fontaine spatiale** [théorique] — tour tenue par un flux de projectiles qui remonte. **En jeu** : tour
  impossible qui défie la gravité.
- **Cité flottante / station nuageuse** [fiction / théorique] — villes suspendues dans l'atmosphère d'une
  géante gazeuse (Bespin) ou de Vénus (à 50 km, où la pression et la température sont terrestres).
  **En jeu** : seul endroit où l'on « se pose » sur une géante (`gas.rs`) ; vue sur la mer de nuages (P4).
- **Ville flottante sur l'océan** [réel / projet] — plateformes et cités flottantes (Kamino). **En jeu** : sur les
  planètes océans (O1) ; quais pour se poser.
- **Habitat sous-marin** [réel] — base au fond de la mer, dômes, sas. **En jeu** : avec la vraie eau (O1/O2),
  plongée du vaisseau, caustiques.
- **Ville sous dôme** [théorique] — dôme pressurisé sur une planète sans air ou toxique. **En jeu** : dedans,
  air respirable (combinaison `suit.rs` inutile).
- **Ville souterraine** [réel] — dans les grottes (`caves.rs`) ou les tubes de lave (lunes, Mars).
- **Arcologie** [théorique] — une seule tour-ville immense qui contient toute une ville.
- **Écuménopole (Coruscant)** [fiction] — planète entièrement couverte de ville. **En jeu** : type de planète
  rare des civilisations avancées (L5), lumières la nuit vues de l'espace.
- **Processeur d'atmosphère** [fiction] — usines de terraformation (Aliens). **En jeu** : bâtiment de la
  terraformation (L6) qui change les valeurs vivantes (deltas).
- **Pare-soleil / miroir orbital** [théorique] — miroir géant au point de Lagrange pour refroidir ou réchauffer
  une planète. **En jeu** : terraformation (L6), ombre visible sur la planète.
- **Bouclier magnétique artificiel** [théorique] — au point L1 de Mars pour protéger une atmosphère.
  **En jeu** : terraformation, aurores visibles.
- **Centrale solaire orbitale** [réel / projet] — satellites qui envoient l'énergie au sol par micro-ondes vers
  une antenne (rectenna). **En jeu** : faisceau visible de nuit.
- **Chantier naval orbital / cale sèche** [fiction] — on y construit les grands vaisseaux (croiseurs, capitaux
  de l'éditeur). **En jeu** : un vaisseau de 1024³ se construit ici, étape par étape.
- **Dépôt de carburant orbital** [réel / projet] — réservoirs en orbite, ravitaillement.
- **Station au point de Lagrange** [réel] — L1 à L5 (comme les Troyens de `belts.rs`).
- **Bouclier planétaire / canon de défense** [fiction] — défense d'une planète entière.
- **Tour météo / contrôle du climat** [fiction] — change la météo locale (`weather.rs`).
- **Puits de forage profond** [réel] — jusqu'au manteau ou dans la glace d'une lune (océan caché d'Europe).

## 4. Stations spatiales fonctionnelles

Plus petites, dédiées à une tâche.

- **Port spatial / station de transit** [fiction] — hub commercial (Babylon 5, la Citadelle). **En jeu** : déjà les
  stations des factions (`economy.rs`) ; à construire en voxel avec quais, hangars (`dock.rs`), marché.
- **Forteresse / station de défense** [fiction] — base très armée (Étoile de la Mort). **En jeu** : combat
  (`combat.rs`), tourelles (bloc E6), zone protégée.
- **Station de recherche / laboratoire orbital** [réel] — étude de phénomènes (Solaris). **En jeu** : près des
  trous noirs, des étoiles à neutrons, des comètes ; bonus de scanner.
- **Raffinerie / station minière** [théorique] — près des astéroïdes ou des géantes (hélium-3). **En jeu** :
  minage 0.15, ceintures de `belts.rs`.
- **Écopeur de géante gazeuse** [théorique] — station qui plonge dans une géante pour récolter les gaz.
- **Récolteur de comète** [théorique] — capture une comète (`comets.rs`) pour son eau et ses glaces.
- **Observatoire / radiotélescope** [réel] — en orbite ou sur la face cachée d'une lune. **En jeu** : révèle
  des systèmes lointains au dex.
- **Relais de communication** [réel] — étend la portée du chat ou du réseau entre systèmes.
- **Phare / balise** [fiction] — marque une route ou un danger, visible de loin.
- **Ferme orbitale / station agricole** [théorique] — serres en anneau, nourriture.
- **Hôpital, prison, casino, station de loisir** [fiction] — rôles pour les factions et les quêtes.
- **Banque de graines / coffre génétique** [réel] — sauvegarde de la vie d'une planète (lien avec la vie,
  `life.rs`).
- **Usine d'antimatière** [théorique] — près d'une étoile, très coûteuse, carburant de fin de partie.
- **Chantier de démolition / casse** [fiction] — épaves recyclées.
- **Poste d'écoute caché** [fiction] — station furtive dans une ceinture d'astéroïdes, repaire de pirates.
- **Douane / point de contrôle** [fiction] — entrée d'un territoire de faction.
- **Ambassade** [fiction] — contact avec les aliens (L5, Q5).
- **Archive de données / bibliothèque** [fiction] — garde l'histoire de la galaxie (dex partagé).

## 5. Transport et voyage (espace et temps)

- **Portes des étoiles / relais de masse** [fiction] — structures fixes qui relient deux points de la galaxie.
  **En jeu** : les trous de ver existent déjà (`Wormhole`) : version construite, réseau de portes entre les
  systèmes des joueurs.
- **Balise d'hyperespace** [fiction] — guide les vaisseaux en voyage plus rapide que la lumière.
- **Porte galactique** [fiction] — voyage entre galaxies (V2 du bloc V) par une structure plutôt qu'un saut.
- **Stabilisateur de trou de ver** [théorique / fiction] — garde un trou de ver ouvert (matière exotique).
- **Anneau de distorsion (Alcubierre)** [théorique] — moteur qui contracte l'espace devant le vaisseau.
- **Autoroute laser / lanceur de voiles** [théorique] — lasers géants qui poussent des voiles (Breakthrough
  Starshot). **En jeu** : trajets entre systèmes avec une traînée lumineuse.
- **Collecteur de Bussard** [théorique] — entonnoir magnétique qui ramasse l'hydrogène interstellaire.
- **Tube de transit entre deux lunes** [fiction] — pont ou câble entre deux corps proches (planètes doubles,
  lunes en rotation synchrone).
- **Gare spatiale / train interplanétaire** [fiction] — cycleurs réguliers entre planètes.
- **Cylindre de Tipler** [théorique] — cylindre géant en rotation qui permettrait de voyager dans le temps.
  **En jeu** : artefact mystérieux, l'horloge du monde s'y comporte bizarrement autour.

## 6. Structures vivantes ou ésotériques

- **Vaisseau-monde biologique** [fiction] — architecture organique, vivante, bio-ingénierée. **En jeu** :
  matériau « vivant » qui pousse et se répare seul.
- **Artefacts d'anciennes civilisations** [fiction] — fonction inconnue, matériaux indestructibles (Rama,
  Précurseurs, monolithe). **En jeu** : points d'intérêt rares (D6), énigmes, voxels indestructibles (pas de
  minage), scanner qui ne comprend pas (« ??? »).
- **Arbre de Dyson** [théorique, Freeman Dyson] — arbres génétiquement modifiés qui poussent sur les comètes
  et créent leur propre atmosphère dans une serre. **En jeu** : forêts géantes sur les comètes.
- **Récif spatial** [fiction] — colonie d'organismes qui grandit en orbite comme du corail.
- **Ruche alien** [fiction] — structures organiques qui couvrent une planète (Zerg).
- **Nuage de nanites** [fiction] — ville faite d'un essaim de nanomachines qui change de forme.
- **Cristaux pensants** [fiction] — structures de cristal qui chantent ou communiquent (planète cristalline).
- **Monolithe** [fiction] — objet parfait au milieu de nulle part, qui réagit à la présence du joueur.
- **Ancre dimensionnelle / univers de poche** [fiction] — porte vers un petit monde dans un autre espace.
- **Tombeau de civilisation** [fiction] — nécropole géante en orbite ou sur une planète morte.
- **Créature-station** [fiction] — station construite dans le corps d'une créature spatiale géante.

## 7. Bâtiments au sol (échelle du joueur)

Ce qu'un joueur peut poser lui-même sur une planète, avec le terrain voxel et l'éditeur :

- **Abri et base** : module d'habitation, sas, dôme gonflable, base enterrée dans une grotte.
- **Plateforme d'atterrissage, hangar, spatioport** (lien avec P5 : se poser sur une zone plate préparée).
- **Énergie** : panneaux solaires (suivent le soleil, rien la nuit), éoliennes (selon le vent de `weather.rs`),
  géothermie près des geysers et de la lave (T5), réacteur.
- **Ressources** : foreuse, mine, raffinerie, réservoirs, convoyeurs, pompe à eau, récolteur de glace.
- **Nourriture** : serres, fermes hydroponiques, élevages (faune de 0.14).
- **Science** : laboratoire, station météo, sismographe (séismes de T5), télescope au sol, radar.
- **Transport** : routes, ponts sur les gorges (T2), tunnels, téléphérique, rail, ascenseur vers une station.
- **Défense** : tourelles, murs, boucliers, champs de mines (pour le combat de `combat.rs`).
- **Communication** : antenne, balise de position (marqueurs des joueurs, D6), relais.
- **Décoration et monuments** : statues, drapeaux de guilde, jardins, phares, œuvres voxel.
- **Terraformation** (L6) : générateurs d'atmosphère, fontes de glace, semeurs de vie.

## 8. Idées de gameplay autour des constructions

- **Une planète qui change** : vue de l'espace, les villes s'allument la nuit, les routes se voient, les
  champs changent la couleur du sol (même règle que le sol vu de l'espace).
- **Progression collective** : une sphère de Dyson demande l'effort de tout un serveur ; barre d'avancement
  partagée.
- **Constructions des factions et des aliens** : les IA bâtissent selon leur richesse ; on peut les aider,
  les commercer ou les détruire.
- **Ruines et archéologie** : constructions abandonnées générées depuis la graine (épaves, stations mortes,
  anneaux brisés), à fouiller pour des plans.
- **Plans à échanger** : un plan de station est un fichier `.ssvox` partageable (comme les modèles, `Msg::Share`).
- **Catastrophes** : séismes (T5) qui abîment une base, météorites (`meteors.rs`), tempêtes, éruptions.
- **Gravité et rotation** : les grands habitats tournent ; marcher à l'intérieur avec une gravité qui baisse
  vers l'axe (règle physique nouvelle pour le marcheur).
- **Énergie de l'étoile** : la puissance d'une station solaire dépend du type d'étoile (`star.rs`) et de la
  distance ; une naine rouge donne peu, une géante bleue beaucoup.
- **Défis de construction** : construire près d'un trou noir, sur une planète à séismes, dans une géante
  gazeuse, sur une comète qui passe près de l'étoile.
- **Vue d'ensemble** : carte des constructions de la galaxie dans le dex (onglet « Constructions »).
