# Rapport — Toutes les améliorations possibles pour SpaceSpore (05/10/2026, après la v0.13.3)

Sources : `CLAUDE.md`, `ROADMAP-0.13/0.14/0.20`, `A-FAIRE-PLUS-TARD.md`, `IDEES-constructions.md`,
`RAPPORT-generation-terrain.md`, `TESTS-JEU.md`, et un survol du code (43 000 lignes, 318 tests,
~238 `unwrap()`, `PROTOCOL` 34).

Ce document est une **liste d'améliorations et d'idées**, pas une feuille de route : rien n'est décidé ni daté.

Légende : **[Planifié]** = déjà dans une feuille de route · **[Idée]** = banque d'idées, pas planifié ·
**[Nouveau]** = ma suggestion, issue de la lecture (non vérifiée en jeu) · Taille S / M / L / XL.

---

## 1. Déjà planifié (à ne pas oublier)

### 1.1 Version 0.13 (reste à faire)
| Phase | Contenu | Taille |
|---|---|---|
| **P1 Vol orbital** | ZQSD pilote le vaisseau autour de la planète, caméra derrière | L |
| **P2 Transition sans coupure** | Orbite → vol bas → sol en continu, une seule loi de vol | L |
| **P3 Rentrée atmosphérique** | Plasma, lueur selon vitesse × densité de l'air, bouclier en option | M |
| **P4 Traverser les nuages** | Cubes qui s'écartent, sillage | M |
| **P5 Se poser / décoller** | Ombre, train d'atterrissage, poussière soulevée | M |
| **P6 Vitesse et air** | Bang supersonique, effets selon la vitesse | M |
| **P7 Aide à l'approche** | Couloir, point d'impact, alertes, altitude réelle | M |
| **P8 Météo, lumière, cockpit** | Givre, pluie, foudre sur le vaisseau | M |
| **P9 Son et joueurs** | Premier système de son, silence dans le vide | M |
| **C1 Brouillard** | Brouillard au sol, bancs, rayons de soleil | M |
| **C2 Skybox** | Skybox générée depuis la vraie galaxie | L |
| **V1 Trous noirs** | Lentille gravitationnelle, anneau d'Einstein, disque | L |
| **V2 Voyage entre galaxies** | Séquence 10 s, tunnel, flash | M |
| **V3 Galaxie inclinée** | Vue dans le plan de la galaxie | S |
| **L5 Aliens** | Espèce intelligente, villages voxel, commerce, pas de combat | XL |
| **L6 Terraformation** | Changer climat, atmosphère, eau, vie par deltas | XL |

### 1.2 Version 0.14 — mondes vivants
- **Bloc D** : D1 eau vivante (rivières, lacs, cascades, banquise), D2 géologie active (reste : sources chaudes,
  éboulis ; lave/geysers/séismes déjà faits en T5), D3 végétation qui bouge, D4 faune visible, D5 son (`bevy_audio`),
  D6 points d'intérêt (grottes géantes, épaves, ruines, monolithe, marqueurs de joueur).
- **Bloc X (mondes exceptionnels)** : X0 système d'archétypes, X1 œil/verrouillé, X2 mondes d'eau, X3 feu et glace,
  X4 planète étirée / limite de Roche, X5 tempêtes géantes, X6 mondes cristal/métal/fongique/cubique,
  X7 planète cassée / creuse / océan suspendu, X8 mondes morts, vivants, qui évoluent.
- **Bloc N (événements rares)** : N1 éruptions et séismes forts, N2 impacts + mégatsunami + hiver d'impact,
  N3 éruption solaire majeure / inversion des pôles, N4 grandes marées et temps long.

### 1.3 Plus tard
- **0.15 Minage et destruction** (deltas voxel déjà là) ; **musique** (reportée) ; **0.20 Débogage et optimisation**
  (T1 réglages centralisés, T2 mesures, T3 panneau F6, T4 vues de debug, T5 benchmark automatique, T6 balayage,
  T7 optimiser le LOD, T8 optimiser la lumière, T9 préréglages Bas→Ultra).
- **Constructions** (`IDEES-constructions.md`) : mégastructures (Dyson, anneaux), habitats massifs, infrastructure
  orbitale, stations, transport (portes, ascenseur spatial), structures vivantes, bâtiments au sol, plans `.ssvox`
  échangeables, défis de construction, catastrophes qui abîment les bases, gravité de rotation dans les habitats,
  énergie selon le type d'étoile, onglet « Constructions » du dex.

---

## 2. Gameplay (peu ou pas planifié)

1. **[Nouveau] Boucle de jeu / objectif** — aujourd'hui : exploration, économie, guildes, diplomatie, combat, mais
   pas de fil conducteur. Ajouter une progression (rangs d'explorateur via le dex, objectifs de découverte,
   succès) pour donner une raison de voyager. M
2. **[Nouveau] Missions et quêtes** — `economy.rs` / `npc_ui.rs` ont des missions simples ; les étendre (livraison,
   scan d'un monde exotique X, rapatriement d'un échantillon, escorte, enquête sur une ruine). M–L
3. **[Nouveau] Inventaire, artisanat, bases** — le minage 0.15 produit des minerais (Xenium, Aetherite, Chronite)
   mais rien ne les transforme : raffinage, fabrication d'outils / modules, coffres, bases préfabriquées. L
4. **[Nouveau] Progression du vaisseau** — modules (moteurs, bouclier thermique, scanner, soute), carburant /
   énergie, améliorations par minerai, réparation, assurance après destruction. L
5. **[Nouveau] Combat** — enrichir (armes, ennemis, abris), pas de PvE sauvage hors faune hostile optionnelle ;
   règles PvP claires (zones sûres, consentement). M–L
6. **[Nouveau] Survie à pied (A4) approfondie** — nourriture, eau, outils, abris, maladies exotiques selon les
   spores / la vie (thème du jeu : « spore »). M
7. **[Nouveau] Photo / mode capture** — appareil photo avec caméra libre, filtres, sauvegarde dans le dex ; très
   cohérent avec l'exploration. S–M
8. **[Nouveau] Journal de bord** — historique automatique des lieux visités, trajectoires, notes (déjà un début
   avec le dex et ses notes). S
9. **[Idée] Progression collective** — chantiers de serveur (sphère de Dyson), barre partagée. L
10. **[Nouveau] Évènements d'équipe** — expéditions de guilde, chasses aux anomalies (monolithe X, écho temporel). M
11. **[Nouveau] Mode solo vs multi clair** — assistant de première partie qui explique les modes, les graines
    partagées, les parties privées. S

## 3. Monde et univers

12. **[Nouveau] Nébuleuses, amas, supernovæ, pulsars jouables** — `src/astre/` a déjà pulsar, étoile à neutrons,
    hypergéante : donner à chacun un comportement local (rayonnement, effets de ciel, dangers). M–L
13. **[Planifié D6 + Nouveau] Anomalies et lieux à secret** — grottes à ressources rares, structures anciennes,
    énigmes de ruines, cartes au trésor générées. M
14. **[Nouveau] Factions vivantes** — l'économie des factions évolue (guerres, blocus, embargos selon la
    diplomatie) et se voit sur la carte galactique. L
15. **[Nouveau] Carte galactique améliorée** — filtres (types d'étoiles, factions, dex), recherche, itinéraires,
    marqueurs persistants, minimap en vol. M
16. **[Idée] Plans d'eau et climat dynamique** — climat qui réagit à la terraformation et aux événements N. M
17. **[Nouveau] Vie sur les astres sans voxel** — ceintures, anneaux, comètes : lieux d'amarrage, stations
    minières, vendeurs. M
18. **[Nouveau] Trous de ver gameplay** — réseau de trous de ver à découvrir, instabilité, coût d'énergie. S–M

## 4. Éditeur et modèles

19. **[Nouveau] Partage communautaire** — galerie en ligne de `.ssvox` (races, vaisseaux), avec miniature et
    vérification de taille (10 Mo max). M
20. **[Nouveau] Texture / détails** — motifs de surface par face, décalques, lumières animées sur la coque. M
21. **[Nouveau] Import/export étendus** — glTF/OBJ en sortie pour impression ou rendu ; import de `.vox`
    multi-modèles déjà géré. S
22. **[Nouveau] Outils de symétrie plus riches** — symétrie sur 2–3 axes, radiale, miroir par zone. S
23. **[Nouveau] Bibliothèque de pièces / préfabriqués** — cockpits, tuyères, tourelles à glisser dans un modèle
    (partiellement via les blocs de mouvement). M
24. **[Nouveau] Tests de vol dans l'éditeur** — essayer le vaisseau dans une mini-scène physique avant de le
    publier (poussée, virages, hangars). M
25. **[Idée] Plans de construction à échanger** pour les stations (voir constructions). M

## 5. Rendu, ciel, atmosphère

26. **[Planifié 0.13/0.14]** Brouillard C1, skybox C2, lentille gravitationnelle V1, nuages traversables P4,
    rentrée P3, végétation qui bouge D3, faune D4.
27. **[Nouveau] Nuages volumétriques / dessous de cloud réaliste** — la couche en cubes marche, mais un rendu
    volumétrique (ray-march limité) ferait un grand effet depuis l'orbite. L
28. **[Nouveau] Ombres et lumière** — voir 0.20 T8 : retirer l'ombre cubemap de l'étoile près d'une planète,
    cascades ajustées, `relief_shadows` plus rapide (coûte ~40 % d'images/s aujourd'hui). M
29. **[Nouveau] Post-traitement** — bloom, étoile qui éblouit, flare d'objectif, correction de couleurs selon le
    type d'étoile, adaptation de l'œil (nuit/jour). M
30. **[Nouveau] Réflexions et eau** — réflexions planaires ou SSR de la mer, mouillage des surfaces sous la
    pluie, humidité du sol. M
31. **[Nouveau] Particules et poussière** — traînées de moteurs, poussière d'atterrissage, étincelles de chocs,
    débris de météorites. M
32. **[Nouveau] Anti-aliasing et qualité** — options TAA/FXAA/SMAA, échelle de rendu dynamique (DLSS/FSR style via
    résolution adaptative). M
33. **[Nouveau] Mode HDR / daltonisme** — filtres d'accessibilité visuelle. S

## 6. Son et musique

34. **[Planifié D5 / P9]** Sons du monde : vent, pluie, tonnerre, vagues, lave, faune, moteurs, silence dans le vide.
35. **[Plus tard]** **Musique** : adaptative (exploration / combat / orbite), générée selon le type de monde ; une
    banque de thèmes par archétype X.
36. **[Nouveau] Mixage et options audio** — curseurs maître / musique / effets / ambiance, sourdine hors focus,
    sons 3D avec atténuation par l'atmosphère (vitesse du son selon l'astre). M
37. **[Nouveau] Voix / chat vocal de proximité** pour le multijoueur. L

## 7. Multijoueur et réseau (`net.rs`, 2 700 lignes)

38. **[Nouveau] Robustesse** — reconnexion automatique, gestion de la latence (interpolation / prédiction des
    vaisseaux), détection de désynchronisation (diff d'horloge, hash de graine). M
39. **[Nouveau] Sécurité** — validation côté hôte des deltas voxel (`Msg::Voxels`) et des tailles de fichiers
    (`Msg::ModelPart`), limites de débit, liste de bannissement, mot de passe de partie. M
40. **[Nouveau] Découverte de parties** — liste de serveurs / code d'invitation (UPnP via `igd-next` déjà là),
    lancement depuis le launcher. M
41. **[Nouveau] Hôte dédié** — mode serveur sans rendu (`--server`), utile aussi pour le benchmark 0.20. L
42. **[Nouveau] Chat enrichi** — canaux (guilde, local, global), mentions, historique, émotes. S–M
43. **[Nouveau] Compatibilité de version** — `PROTOCOL` est un `u32` strict : message clair + téléchargement
    automatique de la bonne version via le launcher. S

## 8. Interface et ergonomie (`ui.rs` 3 500 lignes, `net_ui.rs`)

44. **[Nouveau] Réaffectation des touches** — je n'ai trouvé aucun menu de rebind ; clavier AZERTY par défaut,
    mais QWERTY / manette à prévoir. M
45. **[Nouveau] Manette** — aucune prise en charge détectée (pas de `gamepad` dans `src`) : navigation spatiale
    et vol bas se prêtent bien à une manette. M–L
46. **[Nouveau] Localisation** — textes en français en dur ; extraire dans des fichiers de langue (FR/EN au
    minimum) pour toucher plus de joueurs. M
47. **[Nouveau] Accessibilité** — taille de texte, contraste, sous-titres des sons, réduction des flashs
    (éclairs, éruptions), mode daltonien. M
48. **[Nouveau] Tutoriel / première partie** — guidage : navigation galaxie → système → atterrir → scanner →
    dex. Aujourd'hui l'éditeur s'ouvre au premier lancement (création de personnage) puis le joueur est seul. M
49. **[Nouveau] HUD configurable** — masquer / déplacer des éléments, mode « photo » sans HUD, indicateurs de
    vol (P7). S–M
50. **[Nouveau] Aide en jeu** — liste des raccourcis, `COMMAND_HELP` du chat déjà là : l'afficher dans une fenêtre. S
51. **[Nouveau] Menu Options** — préréglages graphiques (0.20 T9), détail du sol déjà (T4), échelle de l'UI,
    FOV, sensibilité. S–M
52. **[Nouveau] Dex** — filtres par type de monde / rareté, comparaisons, badges de « première découverte »,
    export visuel (image). S–M

## 9. Performances et optimisation (voir `ROADMAP-0.20`)

53. **[Planifié]** panneau F6, mesures, vues de debug, benchmark automatique, optimisation LOD et lumière.
54. **[Nouveau] Mémoire** — le mode Ultra en vol bas atteint ~0,84 Go de maillages (déjà divisé par 4) ; viser un
    plafond configurable, déchargement plus agressif, mode « Patate » pour petites machines. M
55. **[Nouveau] Chargement et démarrage** — temps de génération des galaxies (12 500 systèmes), écran de
    progression, génération incrémentale. M
56. **[Nouveau] Streaming de tuiles** — cache disque rejeté (relire coûte plus que générer) ; à la place :
    réduire encore le coût de génération (SIMD sur le bruit, génération GPU des hauteurs). L
57. **[Nouveau] Instancing** du décor et des cailloux d'astéroïdes, LOD des modèles du joueur, culling par
    occlusion de l'horizon (déjà fait pour les tuiles : `relief_span`). M
58. **[Nouveau] Profil de build** — `lto`, `codegen-units`, `opt-level` pour le release, test de `mimalloc` ;
    mesurer le temps de compilation et la taille de l'exe. S
59. **[Nouveau] Multithreading** — vérifier les systèmes qui monopolisent le fil principal (`rebuild_clouds`,
    `stats`, `StarSectors`) ; passer plus de choses en tâches asynchrones. M

## 10. Qualité du code et maintenance

60. **[Nouveau] Découper les gros fichiers** — `ui.rs` (3 486 l.), `surface.rs` (3 405), `main.rs` (2 923),
    `planet.rs` (2 779), `net.rs` (2 722), `terrain.rs` (2 550), `asteroids.rs` (2 443) : un module par
    responsabilité, plugins Bevy plus petits. L (étalé)
61. **[Nouveau] `unwrap()` → erreurs gérées** — ~238 occurrences : chaque panic en jeu perd la partie ;
    `expect` avec message ou `?` pour les chemins réseau, fichiers, sauvegardes. M
62. **[Nouveau] Intégration continue** — workflows `build.yml/stable.yml/unstable.yml` : ajouter `cargo test`,
    `clippy`, `cargo fmt --check` et un test de démarrage sans fenêtre avant publication. M
63. **[Nouveau] Tests d'intégration** — 318 tests unitaires surtout sur la génération ; ajouter des tests de
    protocole réseau (sérialisation, compatibilité), de sauvegarde (migration `vX.Y.Z`), de déterminisme
    inter-plateformes (Windows/Linux/Mac : flottants, `f32` vs `f64`). M–L
64. **[Nouveau] Compatibilité des sauvegardes** — migrer `saves/vX.Y.Z/` automatiquement entre versions
    (aujourd'hui : dossier par version) ; sauvegardes de sécurité avant migration. M
65. **[Nouveau] Mise à jour de Bevy** — 0.15 → version récente (migration lourde, à planifier hors des phases de
    fonctionnalité) : gains de performance, API de lumière/ombre. L–XL
66. **[Nouveau] Documentation** — `CLAUDE.md` fait 466 lignes très denses ; en tirer un `docs/ARCHITECTURE.md`
    avec schéma des modules, la liste des règles numérotées, la procédure de test. S–M
67. **[Nouveau] Journaux et rapports de crash** — fichier de log avec rotation, rapport de panic envoyé / copiable
    (`arboard` déjà là), numéro de build dans les captures. S
68. **[Nouveau] Hygiène du dépôt** — `doc info/.../api.txt` contient des clés API en clair (jamais commit) :
    vérifier `.gitignore`, passer en variables d'environnement, **renouveler les clés** si elles ont déjà fuité. S
    (à vérifier en priorité)

## 11. Distribution et outils (`tools/`)

69. **[Nouveau] Launcher** — notes de version affichées, choix stable / instable (déjà), réparation / vérification
    des fichiers, gestion de plusieurs sauvegardes, historique des builds `unstable`. M
70. **[Nouveau] Signature et antivirus** — signer l'exe Windows (SmartScreen), notariser sur Mac. M
71. **[Nouveau] Plateformes** — Linux/Mac construits par la CI mais testés ? Ajouter un test de démarrage sur
    chaque OS, vérifier les chemins de sauvegarde (`dirs`). M
72. **[Nouveau] Télémétrie facultative** — rapports anonymes de FPS / plantages, avec consentement. M

## 12. Réalisme et contenu scientifique

73. **[Nouveau] Réalisme des étoiles** — évolution stellaire dans le temps (naine → géante), supernovæ rares
    (lié N3 / X8), étoiles à flares. L
74. **[Nouveau] Ciel nocturne réaliste** — constellations propres à chaque point de vue, planètes visibles à l'œil
    nu, lune qui change de taille selon l'orbite. M
75. **[Nouveau] Atmosphères et couleurs** — diffusion (Rayleigh/Mie) pour des couchers de soleil exacts selon les
    gaz, déjà préparé par `atmosphere.rs`. M
76. **[Nouveau] Biologie** — écosystèmes (chaînes alimentaires), cycle jour/nuit des espèces (D4), spores
    comme thème central : contagion, symbiose, spores bioluminescentes. L

---

## 12 bis. Suite des propositions (77 à 140)

### A. Exploration et découverte
77. **[Nouveau] Sondes et drones** — lancer une sonde automatique vers un astre lointain : elle revient avec un
    scan partiel dans le dex après un délai (temps de jeu). M
78. **[Nouveau] Niveaux de scan** — scan rapide (type, rayon), moyen (atmosphère), profond (ressources, vie,
    grottes) ; chaque niveau demande un module ou du temps. M
79. **[Nouveau] Radar des anomalies** — le scanner signale des « signatures » (ruines, monolithe, épave) dans
    un rayon, avec une direction et une incertitude. S–M
80. **[Nouveau] Cartographie de surface** — carte au sol qui se dévoile en marchant, marqueurs, boussole
    (la boussole `compass` existe pour le vent). M
81. **[Nouveau] Records et « premières »** — plus grande montagne, plus profond canyon, monde le plus rare vus
    par le joueur ; classement par serveur. S
82. **[Nouveau] Collections** — espèces, minéraux, cristaux, météorites à collecter, albums du dex avec
    pourcentage de complétion par galaxie. S–M
83. **[Nouveau] Défis journaliers / hebdomadaires** — « photographier une aurore », « se poser sur un monde
    œil ». S–M
84. **[Nouveau] Mode explorateur sans combat ni économie** — option de partie pacifique. S

### B. Vaisseau et vol
85. **[Nouveau] Modes de vol** — croisière, combat, atterrissage assisté (stabilisation), vol « sans assistance »
    newtonien. M
86. **[Nouveau] Carburant, chaleur, énergie** — gestion de l'énergie des propulseurs, surchauffe en rentrée
    (lié P3), recharge près d'étoiles ou en station. M
87. **[Nouveau] Intérieur du vaisseau** — pouvoir marcher dans son vaisseau (cockpit, soute) avec le modèle de
    l'éditeur : zone intérieure, siège, coffre. L
88. **[Nouveau] Équipage et passagers** — d'autres joueurs montent à bord, postes (pilote, tireur). L
89. **[Nouveau] Véhicules terrestres** — rover, moto antigravité, planeur ; blocs de mouvement existants. M–L
90. **[Nouveau] Navettes / vaisseau-mère** — hangars (E6/E7) déjà dans l'éditeur ; ajouter lancement de chasseurs
    depuis un porte-vaisseaux piloté. L
91. **[Nouveau] Remorquage et épaves** — récupérer des épaves (D6) pour pièces et plans. M
92. **[Nouveau] Autopilote de croisière optionnel** — Q7 interdit l'autopilote d'atterrissage ; garder une
    croisière entre astres avec alertes de collision (`ship.rs` existe). S

### C. Construction et bases (voir `IDEES-constructions.md`)
93. **[Idée] Premier palier réaliste** — avant les mégastructures : poser une **base sur une planète** (modules
    éditeur placés sur le sol avec collisions, `collision_boxes`), alimentée, protégée. L
94. **[Idée] Stations en orbite** — amarrage (`dock.rs`) étendu aux stations de joueur avec hangars. L
95. **[Nouveau] Droits de propriété** — `claims.rs` existe : visualiser les territoires, protection, taxes de
    faction. M
96. **[Nouveau] Mode créatif / bac à sable** — construire sans coût pour tester, voler librement. S
97. **[Nouveau] Chantiers partagés** — plan découpé en tâches que plusieurs joueurs remplissent. M
98. **[Idée] Gravité artificielle dans les habitats** — règle de marche nouvelle (rotation, gravité décroissante
    vers l'axe). L

### D. Économie, factions, diplomatie (`economy.rs`, `guild.rs`, `diplomacy.rs`)
99. **[Nouveau] Marché vivant** — offre / demande qui dépend des gisements, des événements (N) et du
    commerce des joueurs ; graphiques de prix, routes commerciales rentables. M
100. **[Nouveau] Routes et convois** — PNJ qui transportent, pirates qui attaquent, escorte rémunérée. L
101. **[Nouveau] Contrats de guilde** — coffre commun, rangs, permissions, journal d'activité (`guild_ui.rs`). M
102. **[Nouveau] Diplomatie lisible** — carte de relations, traités (commerce, non-agression), incidents
    diplomatiques et leurs conséquences. M
103. **[Nouveau] Monnaies et banque** — monnaie par faction, taux de change, prêts. M
104. **[Nouveau] Ressources fictives** — Xenium / Aetherite / Chronite : propriétés de jeu uniques
    (propulsion, scanner, temps local Q6) pour que le fictif ait un rôle. M

### E. Aliens, faune et vie (0.13 L5, 0.14 D4)
105. **[Nouveau] Dialogue et langue** — langue alien procédurale que l'on apprend en scannant (glyphes, mots),
    commerce par échange de symboles. L
106. **[Nouveau] Comportements d'espèces** — attitude tirée de la graine (Q5), mémoire des actes du joueur,
    réputation par espèce. M
107. **[Nouveau] Apprivoisement / observation** — ne pas combattre : observer, nourrir, photographier, élever. M
108. **[Nouveau] Migration saisonnière** — troupeaux qui suivent la saison (A3) et le climat. M
109. **[Nouveau] Espèces dans le dex** — fiche avec modèle 3D tournant (modèles de l'éditeur), régime, habitat. S–M
110. **[Nouveau] Villages vus de l'espace** — lumières de nuit, routes (idée « planète qui change »). M

### F. Environnement, physique et événements
111. **[Nouveau] Feu et incendies** — foudre/lave qui allument la végétation, fumée, repousse. M
112. **[Nouveau] Eau dynamique** — niveaux de rivière qui montent avec la pluie, crues, inondations (D1 + C5). L
113. **[Nouveau] Érosion visible** — dépôts et falaises qui reculent sur de longues durées (N4). L
114. **[Nouveau] Neige accumulée et traces** — empreintes dans la neige / sable, neige qui s'épaissit. M
115. **[Nouveau] Effondrements de grottes** — séismes (T5) qui font tomber des plafonds (deltas voxel). M
116. **[Nouveau] Physique des débris** — blocs minés (0.15) qui tombent et roulent ; limiter pour les FPS. L
117. **[Nouveau] Radiations et atmosphère toxique** — dangers à lire au scanner, filtres, combinaison (A4). M
118. **[Nouveau] Température ressentie** — vêtements / isolation, tente, feu de camp. M

### G. Interface et confort supplémentaires
119. **[Nouveau] Barre de commandes rapides** — recherche de commande façon palette (Ctrl+K), pas seulement le
    chat. S
120. **[Nouveau] Raccourci « téléportation sûre »** — retour au vaisseau / dernier point de sauvegarde. S
121. **[Nouveau] Sauvegardes multiples et automatiques** — slots, sauvegarde auto avec rotation, export d'un
    monde partageable (graine + deltas). M
122. **[Nouveau] Graines et partage** — `/graine` existe : bouton « copier le lien du monde », monde du jour. S
123. **[Nouveau] Replays et captures** — enregistrement d'une séquence (positions + horloge) rejouable en caméra
    libre. L
124. **[Nouveau] Mode spectateur** — rejoindre un serveur sans vaisseau pour regarder. S–M
125. **[Nouveau] Infobulles et glossaire** — expliquer les termes du scanner (Jeans, albédo, serre…) en un
    clic : valeur pédagogique forte. S–M

### H. Outils de développement et de test
126. **[Planifié 0.20]** benchmark, mesures, panneau F6. **[Nouveau]** y ajouter un **test de non-régression
    visuelle** : captures de référence par monde (`TESTS-JEU.md`) comparées automatiquement. M–L
127. **[Nouveau] Commandes de test manquantes** — heure, météo, position, vue déjà ? Ajouter `/meteo`,
    `/saison`, `/tp lat lon`, `/camera` pour ne plus tâtonner (règle de `TESTS-JEU.md`). S
128. **[Nouveau] Fuzzing de génération** — générer des milliers de systèmes avec graines aléatoires et vérifier
    les invariants (aucun NaN, planètes qui ne se touchent pas, valeurs bornées). M
129. **[Nouveau] Tests de déterminisme réseau** — deux clients, même graine, mêmes tuiles / mêmes événements
    (règle des graines hachées). M
130. **[Nouveau] Éditeur de monde interne** — outil de debug pour forcer un archétype, un climat, un type
    d'étoile ; accélérerait X0 à X8 (`/aller` existe déjà). M
131. **[Nouveau] Page de statut de la CI** — badge, dernier build instable, notes générées depuis les PR. S

### I. Communauté et vie du projet
132. **[Nouveau] Site / page du jeu** — `docs/` existe (version.json) : page d'accueil avec captures, téléchargement,
    notes de version. S–M
133. **[Nouveau] Journal des modifications dans le jeu** — afficher le changelog au premier lancement d'une
    nouvelle version. S
134. **[Nouveau] Retours joueurs** — bouton « signaler un bug » qui joint log + position + graine. S–M
135. **[Nouveau] Mods** — dossier `saves/editeur/` accepte déjà blocs et races : étendre à races, biomes, minerais
    en JSON (même principe que `build.rs` qui intègre `assets/editeur`). L
136. **[Nouveau] Licence et crédits** — fichier de crédits (Bevy, noise, etc.), licence du jeu et des modèles
    importés de Pixel World (vérifier les droits des couleurs de `BlockData.cs`). S
137. **[Nouveau] Trailers et captures auto** — réutiliser le mode `SPACESPORE_CAPTURE` pour produire des images de
    promo à chaque version. S

### J. Idées plus ambitieuses (long terme)
138. **[Nouveau] Réalité virtuelle** — échelle des planètes 0.13 se prête bien à la VR. XL
139. **[Nouveau] Voyage temporel local** (Q6 0.14) — vallées au temps accéléré comme vrai mécanisme : plantes qui
    poussent, ruines qui vieillissent, énigmes temporelles. L
140. **[Nouveau] Univers persistant à grande échelle** — serveur communautaire où les deltas (cratères, bases,
    terraformation) s'accumulent sur des semaines ; sauvegarde incrémentale, instantanés. XL

---

## 12 ter. Suite de la liste (141 à 230)

Simple banque d'idées : aucun ordre, aucune version, aucune décision.

### K. Astres et espace
141. **Planètes avec plusieurs lunes en résonance** visibles à l'œil nu, avec éclipses croisées.
142. **Étoiles variables** (Céphéides, novæ récurrentes) dont la luminosité change visiblement.
143. **Disques protoplanétaires** : systèmes jeunes encore en formation, planètes qui grossissent.
144. **Supernovæ et restes** : étoile qui explose, nébuleuse qui grandit pendant des années de jeu.
145. **Kilonovæ, magnétars, quasars** dans les galaxies lointaines.
146. **Amas globulaires** autour des galaxies, avec leurs étoiles très vieilles.
147. **Nuages de gaz et poussière** à traverser (vitesse réduite, visibilité réduite, scan brouillé).
148. **Vents stellaires et bulles** qui poussent le vaisseau près des étoiles massives.
149. **Étoiles en fuite / hors galaxie** isolées, avec leurs planètes errantes.
150. **Collisions de galaxies** (galaxies lointaines déformées, ponts de marée).
151. **Rayons gamma** : événement très rare qui stérilise un monde.
152. **Planètes jumelles** orbitant l'une autour de l'autre (système binaire planétaire).
153. **Lunes habitables** autour de géantes gazeuses (en plus des planètes).
154. **Astéroïdes à eau, comètes à retour** dont le passage est annoncé au scanner.
155. **Objets interstellaires** de passage (type Oumuamua) : visite unique, très rapide.

### L. Mondes et surfaces
156. **Mers de sable / de poussière** où l'on s'enfonce (planète désertique).
157. **Lacs d'hydrocarbures** (type Titan) avec reflets et brume orange.
158. **Mondes de lave en surface** avec croûte qui craque, îles flottantes.
159. **Forêts de cristaux** qui diffractent la lumière de l'étoile selon l'heure.
160. **Aurores colorées selon les gaz** (vert, rouge, violet) vues du sol et de l'espace.
161. **Cheminées hydrothermales** sous-marines, vie autour.
162. **Dunes qui se déplacent** avec le vent sur de longues durées.
163. **Glaciers qui avancent**, crevasses, grottes de glace bleue.
164. **Sources chaudes et bassins** colorés (comme les mares prismatiques).
165. **Champs de météorites** au sol sur un monde sans air.
166. **Ciels avec plusieurs lunes de couleurs différentes** (reflet de leur sol).
167. **Arcs-en-ciel, halos, parhélies, éclipses de lune vues du sol** (optique atmosphérique).
168. **Mirages** sur les mondes chauds (effet de réfraction du sol).
169. **Marées intérieures** : lacs et grottes qui montent et descendent.
170. **Plaines de sel brillantes** qui reflètent le ciel après la pluie.

### M. Personnage et joueur
171. **Personnalisation poussée** : cicatrices, tatouages, vêtements en voxels, couleur de la combinaison.
172. **Emotes et gestes** : saluer, s'asseoir, pointer, danser (animations de l'éditeur).
173. **Équipement porté** : sac, outils, scanner à main visibles sur le personnage.
174. **Nom et titre** au-dessus de la tête, rangs de guilde.
175. **Sons de pas** selon la matière du sol (sable, glace, métal, eau).
176. **Fatigue, course, saut** à gravité variable (sauter très haut sur une petite lune).
177. **Escalade et rappel** sur falaises (formes 3D de T2).
178. **Natation et plongée** avec réserve d'air (sous l'eau déjà en O2).
179. **Jetpack** à carburant pour les faibles gravités.
180. **Grappin** pour traverser ravins et arches.
181. **Planeur / parachute** pour descendre d'une falaise.
182. **Lampe frontale et fusées éclairantes** pour les grottes profondes.
183. **Journal du personnage** : lieux vus, exploits, temps passé par monde.
184. **Mort et réapparition** : conséquences claires (perte d'objets, point de retour choisi).

### N. Outils, scanner, instruments
185. **Analyseur** à viser : composition d'une roche, température d'un lieu.
186. **Sismographe** : écouter l'activité d'un monde, prévoir les séismes.
187. **Radar de sol** pour voir les grottes et gisements sous les pieds (profondeur selon le module).
188. **Télescope** dans le vaisseau : zoom sur les astres lointains, scan à distance.
189. **Spectromètre** : lire la composition d'une atmosphère de loin.
190. **Station météo posée** qui enregistre des courbes sur plusieurs jours de jeu.
191. **Balises** laissées sur un lieu (nom, couleur, visible par la guilde).
192. **Cartes et notes partagées** entre joueurs (export d'un « carnet »).
193. **Horloge et calendrier du monde** consultables, avec les événements annoncés (éclipse, pluie de météores).
194. **Altimètre, vario et horizon artificiel** au HUD de vol.

### O. Rendu, effets et ambiance
195. **Flou de mouvement et profondeur de champ** optionnels.
196. **Lueur des objets** (lampes, lave, cristaux) avec émission dynamique.
197. **Rayons crépusculaires** dans la poussière, rayons de lumière dans les grottes.
198. **Condensation et buée** sur la visière quand il fait froid ou humide.
199. **Gouttes de pluie sur l'écran** (visière / verrière) et vapeur qui s'évapore.
200. **Lumière zodiacale** et Voie lactée plus fine dans le ciel étoilé.
201. **Ombres colorées** de plusieurs soleils (système double, ciel à deux couchers).
202. **Sillages de lumière** des vaisseaux des autres joueurs la nuit.
203. **Reflets du cockpit** sur la verrière, reflets de la planète dessous.
204. **Étincelles de rentrée** qui arrachent des morceaux du vaisseau si le bouclier est coupé.
205. **Lumière ambiante des grottes** qui change selon la roche (cristaux, champignons).
206. **Couleur du ciel qui change avec l'altitude** jusqu'au noir de l'espace, sans coupure.

### P. Contrôles et confort
207. **Souris inversée, sensibilité, lissage** séparés vol / marche / vue galaxie.
208. **Touche « interagir »** unique selon le lieu.
209. **Barre d'actions rapides** (1 à 9) configurable pour outils et modules.
210. **Mode une main / clavier réduit** pour l'accessibilité motrice.
211. **Échelle affichée sur la carte** (distance en UA / années-lumière) au zoom.
212. **Historique des messages** `NOTIFY` consultable.
213. **Pause en solo** (menu qui fige l'horloge du monde) ; indicateur d'horloge accélérée.
214. **Version et build** affichés dans le menu et sur les captures.

### Q. Structure du jeu et contenu
215. **Scénarios / campagnes courtes** : départ guidé, un mystère à résoudre (monolithe, signal).
216. **Histoire de l'univers générée** : civilisations disparues, fondations des factions (lore procédural).
217. **Lore dans le dex** : texte généré par monde (histoire géologique, découvreur).
218. **Artefacts rares** à ramasser, avec propriétés étranges (Chronite = temps local).
219. **Énigmes environnementales** : portes, plaques, cristaux à aligner dans les ruines.
220. **Créatures géantes** optionnelles (dragons non jouables de l'éditeur).
221. **Pièges naturels** : sables mouvants, geysers prévisibles, glace fine, poches de gaz.
222. **Régions écrites à la main** (monde de départ scénarisé, tutoriel).
223. **Niveaux de difficulté** : survie dure / normale / contemplative.
224. **Événements communautaires** programmés (pluie de météores géante tous les X jours de serveur).

### R. Technique et ingénierie
225. **Compression des sauvegardes** (`world.json`, deltas voxel) et sauvegarde incrémentale.
226. **Génération sur GPU** (hauteurs, bruit) pour tenir les horizons très lointains.
227. **Atlas de matériaux** pour réduire les changements d'état de rendu.
228. **Mode serveur léger** sans interface pour héberger plusieurs joueurs.
229. **Scripts / macros** de chat écrits par les joueurs.
230. **Profils de qualité détectés** au premier lancement (GPU, RAM).

---

## 13. Limites de ce rapport

- J'ai lu les documents du dépôt et fait quelques recherches ciblées dans le code ; je n'ai **pas lancé le jeu**.
- Les points marqués **[Nouveau]** sont des suggestions : certains existent peut-être partiellement (manette,
  rebind, localisation : recherche textuelle seulement). À vérifier avant de les planifier.
- Aucune modification de code ; seul ce fichier est ajouté (non commité).
