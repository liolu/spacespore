# Feuille de route technique (05/10/2026, après la v0.13.3)

Objectif : rendre le projet **solide, mesurable et facile à faire évoluer**, sans ajouter de contenu de jeu.
Elle ne remplace pas `roadmaps/a-faire/ROADMAP-0.20-debug-opti.md` (réglages, mesures, benchmark, LOD, lumière) : elle la
**complète** et s'y réfère (« voir 0.20 »). Les phases s'appellent **TECH-n** pour ne pas se confondre avec
les T1..T9 de la 0.20 et les T1..T5 du terrain.

Constats tirés du code (vérifiés le 05/10/2026) :

| Constat | Détail |
|---|---|
| Le projet | 107 fichiers `.rs`, ~43 000 lignes, 318 tests, workspace de 5 caisses |
| La CI ne lance **aucun test** | `build.yml` ne fait que `cargo build --release` ; ni `cargo test`, ni `clippy`, ni `fmt` |
| Pas de `[profile.release]` | ni `lto`, ni `codegen-units`, ni `panic`, ni `strip` : réglages par défaut de Cargo |
| Réseau | UDP (`UdpSocket`), messages `Msg` en **JSON** (`serde_json`), ~35 variantes ; `ModelPart` transporte des morceaux de 16 Kio dans un `String` |
| Panics | ~238 `unwrap()`, 16 `expect`, 30 `panic!` / `unsafe` ; le plus dans `editeur/edit.rs` (25), `net.rs` (22), `editeur/format.rs` (18) |
| Gros fichiers | `ui.rs` 3 486 l., `surface.rs` 3 405, `main.rs` 2 923, `planet.rs` 2 779, `net.rs` 2 722, `terrain.rs` 2 550, `asteroids.rs` 2 443 |
| Systèmes Bevy | ~125 appels `add_systems`, 4 `add_plugins` dans `main` : presque tout est enregistré à plat |
| Bevy | 0.15 (en retard de plusieurs versions) |
| Secrets | `doc info/` est dans `.gitignore` (bien) ; mais les clés d'`api.txt` restent en clair sur le disque |

---

## 1. Règles techniques (valables pour toutes les phases)

1. **Une phase = une PR**, build release qui passe, tests qui passent, pas de changement de comportement
   visible sauf si la phase le dit.
2. **Refactoring sans changement de comportement** : le monde généré reste identique (mêmes graines, mêmes
   tuiles). On le prouve par un test de **hachage du monde** (voir TECH-2), pas par un coup d'œil.
3. **Mesurer avant et après** pour toute phase de performance (règle 8 du projet, banc `bench_*`).
4. **Jamais de `unwrap()` nouveau** sur un chemin réseau, fichier, sauvegarde ou saisie du joueur.
5. **Changer le format réseau ou les sauvegardes** = augmenter `PROTOCOL` / la version de sauvegarde **et**
   écrire la migration dans la même PR.
6. **Pas de dépendance nouvelle** sans une ligne de justification dans la PR (taille, licence, maintenance).

---

## 2. Les phases

| Phase | Contenu | Taille |
|---|---|---|
| **TECH-1. Filet de sécurité (CI)** | Voir §3 | S |
| **TECH-2. Tests de non-régression du monde** | Voir §4 | M |
| **TECH-3. Profil de build et temps de compilation** | Voir §5 | S |
| **TECH-4. Erreurs et panics** | Voir §6 | M |
| **TECH-5. Sauvegardes versionnées et migrations** | Voir §7 | M |
| **TECH-6. Réseau : format, fiabilité, sécurité** | Voir §8 | L |
| **TECH-7. Découpage du code** | Voir §9 | L (étalé) |
| **TECH-8. Journaux, crashs, télémétrie locale** | Voir §10 | S |
| **TECH-9. Outils de test en jeu** | Voir §11 | M |
| **TECH-10. Performances moteur (hors 0.20)** | Voir §12 | L |
| **TECH-11. Multiplateforme et distribution** | Voir §13 | M |
| **TECH-12. Mise à jour de Bevy** | Voir §14 | XL |

Ordre conseillé : **1 → 2 → 3 → 4 → 5** (le socle), puis **6, 7, 8, 9** dans l'ordre qui t'arrange, puis
**10** (après la 0.20 T2 pour avoir des mesures), **11**, et **12** en dernier, hors d'une phase de fonctionnalité.
Les phases 1, 3 et 8 sont petites et peuvent se glisser n'importe quand.

---

## 3. TECH-1 — Filet de sécurité (CI)

**Problème.** Les workflows compilent et publient, mais rien ne vérifie que le code marche. Un test cassé ou
un avertissement passe en instable sans que personne le voie.

**À faire.**
- Nouveau workflow `ci.yml`, déclenché sur chaque PR et sur `main` (pas sur la publication) :
  1. `cargo fmt --check` ;
  2. `cargo clippy --release -- -D warnings` sur le jeu (d'abord en mode « avertissements seulement », puis
     bloquant quand la liste est vide) ;
  3. `cargo test --release` (les tests `#[ignore]` des bancs restent exclus) ;
  4. `cargo build --release -p spacespore-updater -p spacespore-launcher -p spacespore-installer`.
- **Cache Cargo** (`Swatinem/rust-cache`) : Bevy se recompile en 10 à 20 min sans cache.
- Matrice : Windows tout le temps ; Linux et macOS seulement sur `main` pour économiser les minutes.
- `unstable.yml` / `stable.yml` : n'autoriser la publication que si `ci.yml` est vert sur le même commit.
- `cargo audit` (ou `cargo deny`) hebdomadaire : failles connues dans les dépendances, licences.

**Critères.** Une PR avec un test cassé est rouge ; une PR propre est verte en < 15 min avec cache.

## 4. TECH-2 — Tests de non-régression du monde

**Problème.** « Le monde reste identique » est la règle d'or (graines figées, règle 12 / 20 / 23), mais rien
ne le prouve automatiquement quand on touche à `planetgen/`, `terrain.rs` ou `mesher.rs`.

**À faire.**
- **Test d'empreinte** : pour 50 graines fixes, générer systèmes, planètes, lunes, ceintures, comètes, et
  calculer un hachage (FNV / SipHash fixe) de leurs profils sérialisés ; comparer à un fichier
  `tests/golden/monde.txt`. Un changement volontaire = on régénère le fichier dans la PR (visible en revue).
- **Test d'empreinte du terrain** : pour 20 planètes de types variés, hacher la hauteur de colonne et le
  `VoxelType` de 10 000 points fixes (`Terrain::kind_at`), niveaux fin et grossier.
- **Terrain et maillage d'accord** (règle 16) : test qui compare `terrain.rs` et `mesher.rs` (couleur / hauteur)
  sur les mêmes points : la vue lointaine ne doit jamais contredire le sol.
- **Déterminisme inter-plateformes** : lancer ces tests sur Windows, Linux et macOS en CI ; toute différence
  trahit un `f32` / `f64` ou un ordre d'itération (`HashMap`) non déterministe.
- **Fuzzing léger** : 5 000 graines aléatoires, vérifier les invariants (aucun `NaN` / infini, planètes qui ne
  se touchent pas — test existant `planets_follow_their_star_and_never_touch` —, valeurs dans leurs bornes).
- Remplacer `HashMap` par `BTreeMap` ou un tri explicite partout où l'ordre d'itération influence le monde.

**Critères.** Modifier une constante de génération fait échouer le test d'empreinte avec un message qui dit
quelle couche a changé.

## 5. TECH-3 — Profil de build et temps de compilation

**Problème.** Aucun `[profile.release]` : le binaire n'est ni optimisé au maximum ni réduit.

**À faire (mesurer chaque réglage, garder seulement ceux qui paient).**
- `[profile.release]` : `lto = "thin"` puis `"fat"`, `codegen-units = 1`, `panic = "abort"` (attention : voir
  TECH-8, il faut alors un crash handler), `strip = "symbols"`, `opt-level = 3`.
- Comparer : images/s (`SPACESPORE_PERF`), taille de l'exe, temps de build complet et incrémental.
- Profil `[profile.release-fast]` (hérite de release, `lto = off`, `codegen-units = 16`) pour les itérations
  de test en jeu, afin de ne pas payer 10 min de build à chaque essai.
- Édition de liens rapide : `lld` / `mold` en dev (`.cargo/config.toml`), `bevy/dynamic_linking` en dev seulement.
- Essayer `mimalloc` (allocateur) sur le banc des tuiles : souvent -5 à -15 % de temps d'allocation.
- Supprimer les features Bevy inutiles (`default-features = false` + liste) : moins de code, moins de temps.

**Critères.** Tableau avant / après (FPS, taille, temps de build) joint à la PR ; aucun test cassé.

## 6. TECH-4 — Erreurs et panics

**Problème.** ~238 `unwrap()` : un fichier de modèle corrompu ou un paquet réseau mal formé peut faire
planter la partie.

**À faire.**
- Trier les `unwrap()` en trois classes : **sûr** (invariant prouvé → `expect("raison")`), **entrée externe**
  (réseau, fichiers, `.ssvox`, `.vox`, JSON de sauvegarde, saisie → `?` / message au joueur), **test** (laisser).
- Commencer par : `net.rs` (22), `editeur/format.rs` (18), `editeur/edit.rs` (25), `editeur/vox.rs` (9),
  `guild.rs` (13), chargements de `saves/`.
- Un type d'erreur par domaine (`thiserror` ou énumération maison) : `FormatError`, `NetError`, `SaveError`.
- Message au joueur via `NOTIFY` au lieu d'un plantage : « Modèle illisible : … » ; le jeu continue.
- Lint `clippy::unwrap_used` en avertissement sur les modules traités (activé module par module).
- **Fichiers hostiles** : tester `.ssvox` tronqués, tailles absurdes (bombe de décompression : 10 Mo au plus
  **après** décompression aussi), `.vox` avec dimensions géantes, JSON de monde invalide.

**Critères.** Un fichier corrompu ou un paquet aléatoire ne fait jamais paniquer (test de fuzz de 10 000 cas).

## 7. TECH-5 — Sauvegardes versionnées et migrations

**Problème.** Les sauvegardes vivent dans `saves/vX.Y.Z/` (un dossier par version) : changer de version
« perd » le monde, ou oblige à copier à la main. `world.json` grossit avec les deltas (voxels, `body_deltas`,
dex, horloge).

**À faire.**
- Champ `schema` (entier) dans `settings.json`, `world.json`, `dex.json`, `chat_history` ; séparé de la
  version du jeu : on ne migre que quand le schéma change.
- Module `migrate.rs` : chaîne de fonctions `v1 → v2 → v3`, chacune testée avec un fichier d'exemple figé
  (`tests/saves/…`). Sauvegarde de sécurité (`.bak`) avant toute migration.
- Dossier de sauvegarde **stable** (`saves/monde-<nom>/`) au lieu d'un dossier par version ; les anciens
  dossiers sont importés une fois.
- **Écriture atomique** : écrire dans `fichier.tmp` puis renommer (un plantage pendant l'écriture ne détruit
  plus le monde) ; conserver les 3 dernières copies (rotation).
- **Deltas voxel** : stockage binaire compact par bloc (`BlockKey` 32³), compression (`flate2` / `zip` déjà
  présent), écriture incrémentale (seulement les blocs modifiés), au lieu de réécrire tout `world.json`.
- Plusieurs mondes (slots) et export / import d'un monde (zip : graine + deltas + dex).
- Sauvegarde auto toutes les 30 s déjà faite pour l'horloge : la généraliser avec « sale » (dirty flag).

**Critères.** Ouvrir une sauvegarde de la 0.13.2 dans la version courante marche ; tuer le processus pendant
l'écriture ne corrompt rien (test qui interrompt l'écriture).

## 8. TECH-6 — Réseau : format, fiabilité, sécurité

**Problème.** UDP + JSON : lourd (texte, `String` pour les données binaires), pas de fiabilité maison
visible, pas de limites de débit, `PROTOCOL` strict.

**À faire.**
- **Sérialisation binaire** (`bincode` / `postcard` / `rmp`) à la place de JSON : messages 3 à 10 fois plus
  petits, plus rapides. Garder le JSON en mode debug (`SPACESPORE_NET_JSON=1`).
- **Taille des datagrammes** : rester sous le MTU (~1 200 octets) ; les `ModelPart` de 16 Kio sont fragmentés par
  IP (perte d'un fragment = perte du morceau). Passer à des morceaux de ~1 000 octets, **accusés de réception**,
  retransmission, reprise (`Transfers`), vitesse limitée pour ne pas étouffer le jeu.
- **Canaux** : fiable ordonné (chat, fichiers, deltas voxel), fiable non ordonné, non fiable (positions).
  Soit une petite couche maison, soit une caisse (`renet`, `laminar`, `quinn`) à justifier (règle 6).
- **Interpolation et prédiction** des positions des joueurs (tampon de 100 ms), correction douce ; pas de
  téléportation visible.
- **Autorité de l'hôte** : valider les `VoxelEdit` / `Voxels` (portée, cadence, taille), les `Share` (taille,
  type), les messages de guilde ; limiter le débit par pair ; expulser / bannir (liste persistante).
- **Handshake** : `Hello` avec `proto`, hash de la graine et des constantes de génération ; message clair si
  la version diffère, avec lien vers le launcher.
- **Mot de passe / jeton** de partie, option « liste blanche ».
- **Hôte dédié** (`--server`, sans fenêtre ni rendu) : prérequis pour la 0.20 T5 (benchmark sans réseau) et
  pour les serveurs de communauté. Utiliser `MinimalPlugins` + les systèmes de monde seulement.
- **Tests réseau** : simulateur de perte / latence / réordonnancement (une socket factice), tests d'intégration
  à deux clients dans le même processus, hachage des tuiles identique des deux côtés.

**Critères.** Transfert d'un modèle de 5 Mo réussi à 10 % de perte de paquets ; hôte qui ignore sans planter
10 000 messages aléatoires ; `PROTOCOL` incrémenté avec la migration documentée.

## 9. TECH-7 — Découpage du code

**Problème.** Sept fichiers de 2 400 à 3 500 lignes concentrent la logique ; 125 systèmes sont enregistrés à plat ;
temps de compilation et revue pénibles.

**À faire (un fichier par PR, comportement identique, test d'empreinte vert).**
- **Un plugin Bevy par domaine** : `SurfacePlugin`, `NetPlugin`, `UiPlugin`, `TerrainPlugin`, `WeatherPlugin`,
  `SkyPlugin`, `AsteroidsPlugin`… chacun enregistre ses ressources et ses systèmes ; `main.rs` ne fait plus
  qu'assembler. Les **ensembles de systèmes** (`SystemSet`) nomment l'ordre (entrée → simulation → caméra →
  rendu) au lieu d'ordres implicites.
- `ui.rs` (3 486 l.) → `ui/{menu,hud,options,chat,notifications,theme}.rs`.
- `surface.rs` (3 405) → `surface/{walk,flight,landing,frame,light,rescue}.rs`.
- `main.rs` (2 923) → entrée minimale + `app/{setup,select,clickables,markers,fly_ship}.rs`.
- `planet.rs` (2 779) → `planet/{load,lod,stars,galaxy,spawn}.rs`.
- `net.rs` (2 722) → `net/{msg,socket,host,client,transfer,sync}.rs` (cohérent avec TECH-6).
- `terrain.rs` (2 550) → `terrain/{kind,column,tiles,tint,tide,mesh}.rs`.
- `asteroids.rs` (2 443) → `asteroids/{cells,shape,field,trails,hits}.rs`.
- `src/astre/` (dossiers `planete`, `etoile`, `Remnant_stellaire`) : renommer en minuscules
  (`remnant_stellaire`) et unifier la structure ; vérifier que les chemins `use` suivent.
- **Frontières nettes** : le code de génération (`planetgen/`) ne dépend pas de Bevy (pur calcul, testable sans
  fenêtre) ; vérifier avec un `cargo tree` / un test de compilation sans `bevy` si possible.
- **Conventions** : taille max de fichier indicative (800 lignes), un `mod.rs` qui ne contient que des
  réexportations, commentaires de tête qui disent le rôle du module.

**Critères.** `main.rs` < 300 lignes ; plus aucun fichier > 1 500 lignes ; empreinte du monde inchangée ;
temps de build incrémental mesuré avant / après.

## 10. TECH-8 — Journaux, crashs, télémétrie locale

**À faire.**
- `tracing` vers un fichier `logs/spacespore-<date>.log` avec rotation (dossier `/logs/` déjà ignoré par git),
  niveaux réglables (`SPACESPORE_LOG=debug`).
- **Gestionnaire de panic** (`std::panic::set_hook`) : écrit `logs/crash-<date>.txt` (message, trace,
  version, build, graine du monde, position, dernières commandes du chat), garde une copie de la sauvegarde,
  affiche une fenêtre « Le jeu a planté — copier le rapport ». Indispensable avant `panic = "abort"`.
- Numéro de build, version, GPU, OS, RAM dans l'en-tête du log et du rapport de crash.
- Bouton « Signaler un bug » : zip du log + `settings.json` sans secrets + capture, copié dans le presse-papiers
  (`arboard` est déjà là).
- **Télémétrie** : uniquement locale (fichier de FPS / pics par session, déjà proche de `SPACESPORE_PERF`) ;
  l'envoi à un serveur seulement avec consentement explicite, et seulement si on en a un usage.
- Filtrer les secrets des logs (jetons, chemins personnels) ; le chemin d'`api.txt` n'apparaît jamais.

## 11. TECH-9 — Outils de test en jeu

(Complète `TESTS-JEU.md` et la 0.20 T4 / T5.)

**À faire.**
- **Commandes de test manquantes** : `/meteo <type> <force>`, `/saison <n>`, `/tp <lat> <lon> [alt]`,
  `/camera <vue>`, `/pause`, `/graine <n>`, `/archetype <nom>` (quand X0 existera), `/pic` (déclenche une
  mesure). Une commande par besoin de capture, pas de tirage au hasard.
- **Scénarios scriptés** (fichiers `tests/scenarios/*.txt` : une commande par ligne avec attentes
  `attendre NOTIFY "…"`, `capture x.png`, `verifier scanner.eau = liquide`) exécutés par
  `SPACESPORE_SCENARIO=<fichier>`. Remplace les longues chaînes de `SPACESPORE_TEST_CMD`.
- **Captures de référence** : image de référence par monde de test, comparaison automatique (différence
  moyenne / SSIM) avec tolérance, pour attraper une régression visuelle sans regarder.
- **Vérifications automatiques du monde avant capture** : la commande lit le scanner (eau liquide ? jour ?
  synchrone-nuit ?) et **refuse** de capturer si les conditions ne sont pas bonnes (règle 1 de `TESTS-JEU.md`).
- **Mode sans fenêtre** (`--headless`) pour les tests de logique et le serveur dédié (TECH-6).
- **Banc de régression de performance** : `bench_*` existants lancés en CI nocturne, résultats comparés à la
  dernière valeur (alerte si > +15 %).

## 12. TECH-10 — Performances moteur (hors 0.20)

(À faire **après** la 0.20 T2 : on ne devine pas, on mesure.)

**Pistes.**
- **Budgets par système** : chaque système lourd (`rebuild_clouds`, `stream_galaxy_stars`, `update_tiles`,
  `stats`) reçoit un budget en ms par image (règle 5 de la 0.20) et rend la main.
- **Instancing et lots de rendu** : décor, cailloux d'astéroïdes, étoiles lointaines (`StarSectors`) ; moins de
  matériaux distincts (atlas), moins de changements d'état.
- **Culling** : par horizon (fait pour les tuiles), par occlusion des gros reliefs, par distance angulaire pour
  les astres.
- **Génération du bruit plus rapide** : SIMD (`wide` / `std::simd`), tables précalculées, évaluation par blocs ;
  essai de la génération des hauteurs sur GPU (`compute shader`) si le banc le justifie.
- **Mémoire** : plafond configurable des maillages, déchargement progressif, pools de tampons (`Vec`
  réutilisés) pour éviter les allocations par image, `Arc` partagés déjà utilisés (grottes, rochers).
- **Parallélisme** : `par_iter` Bevy sur les gros ensembles (particules, nuages), pool de tâches dédié à la
  génération avec priorité (devant la caméra d'abord, déjà fait pour les tuiles).
- **Démarrage** : génération incrémentale des galaxies (12 500 systèmes), écran de progression, cache des
  index spatiaux (`FarGalaxyLoaded`).
- **Précision** : revue des `f32` restants (origine flottante en place) ; tests aux grandes distances
  (millions d'unités) pour attraper les tremblements.

**Critères.** Chaque piste est acceptée avec un chiffre : gain d'images/s (médiane et 1 % bas) ou de Mo.

## 13. TECH-11 — Multiplateforme et distribution

**À faire.**
- **Tests sur Linux et macOS** : démarrage du jeu avec capture automatique de 4 s (`SPACESPORE_CAPTURE_SECS`)
  sur chaque OS en CI, pour attraper les plantages de démarrage / shader (règle 4 de `TESTS-JEU.md`).
- Chemins de sauvegarde (`dirs`) vérifiés par OS ; pas de séparateur `\` en dur ; casse des noms de fichiers
  (Linux est sensible : `assets/editeur/…`, `Remnant_stellaire` vs `remnant_stellaire`).
- **Signature** : certificat de signature de code Windows (SmartScreen), notarisation macOS.
- **Launcher** : vérification d'intégrité (hash des fichiers), réparation, retour à une version précédente
  sans écraser le launcher (déjà la règle du projet), notes de version.
- **Mise à jour différentielle** : télécharger seulement les fichiers changés (gros gain, l'exe pèse beaucoup).
- Générer des **notes de version** à partir des titres de PR (déjà `git log -1` dans `stable.yml` : améliorer).
- Reproductibilité : `Cargo.lock` versionné (déjà), `rust-toolchain.toml` pour figer la version de Rust.

## 14. TECH-12 — Mise à jour de Bevy

**Problème.** Bevy 0.15 : chaque version suivante apporte des gains de rendu et d'API (lumières, ombres, post
traitement, WESL / shaders, ECS), mais casse beaucoup de choses. Rester en retard rend la migration de plus
en plus chère.

**Méthode.**
- Branche dédiée, **hors d'une phase de fonctionnalité**, une version à la fois (0.15 → 0.16 → …) pour pouvoir
  s'arrêter si un point bloque.
- Avant : TECH-1 (CI) et TECH-2 (empreintes du monde) **obligatoires** ; capture de référence des mondes de test.
- Lire le guide de migration, lister les API utilisées (`Query`, `Commands`, `Material`, `Gizmos`, shaders
  `.wgsl` : `water.wgsl`, `caustics.wgsl`, `far_star.wgsl`), migrer module par module.
- Vérifier les shaders **compilés** par le GPU (règle 4 de `TESTS-JEU.md`) et chaque rendu spécial : eau,
  caustiques, nuages en cubes, tuiles à fondu, anneaux, trous noirs.
- Mesurer images/s et mémoire avant / après (mêmes scénarios) ; résultat joint à la PR.
- Dépendances : `noise`, `rand 0.8` (0.9 existe), `ureq 3`, `igd-next`, `arboard` : mise à jour groupée,
  `cargo update` + `cargo audit`.
- Attention au **déterminisme** : `rand` / `noise` qui changent d'algorithme changent le monde ! Garder une copie
  figée des générateurs utilisés par la génération (`seeds.rs`), jamais la version « dernière ».

---

## 15. Dette connue et petits chantiers à glisser quand on passe

- `src/astre/Remnant_stellaire/` : nom avec majuscule (voir TECH-7 et Linux).
- `CLAUDE.md` : 466 lignes, très denses ; en tirer `docs/ARCHITECTURE.md` (carte des modules, règles numérotées,
  procédure de test) et garder dans `CLAUDE.md` seulement les consignes.
- `version.json` à la racine et `docs/version.json` : documenter qui écrit quoi (le bot de la release).
- `roadmaps/a-faire/prompt0.14.md` / `roadmaps/a-faire/prompt0.20.md` (991 lignes chacun, apparemment les mêmes) : archiver dans `docs/archive/`.
- Fichiers de roadmaps éparpillés (`ROADMAP-*.md`, `roadmaps/`) : un index `ROADMAPS.md`.
- Constantes de génération dispersées : un fichier `constants.rs` par domaine avec la **raison** de chaque valeur
  (la règle « valeurs = f(graine) » est dans le code, mais pas leur justification).
- `settings.rs` (1 713 l.) : sépare réglages du joueur, réglages du monde, état de partie.
- Avertissements du compilateur : viser zéro (prérequis du `-D warnings` de TECH-1).
- Ajouter `#![forbid(unsafe_code)]` aux modules qui n'en ont pas besoin ; auditer les 30 `panic!` / `unsafe`.
- Clés d'`api.txt` : même ignorées par git, les **renouveler** si elles ont déjà été poussées (`git log --all --
  "doc info"` pour vérifier) et les passer en variables d'environnement.

## 16. Questions à trancher

| # | Question | Proposition |
|---|---|---|
| Q1 | Quelle bibliothèque réseau : couche maison sur UDP ou caisse (`renet`, `quinn`) ? | Maison d'abord (le protocole est simple), caisse seulement si la fiabilité devient trop longue à écrire. |
| Q2 | Format de sérialisation binaire ? | `postcard` ou `bincode 2` (compact, rapide, pas de schéma externe). |
| Q3 | Un dossier de sauvegarde stable remplace-t-il `saves/vX.Y.Z/` ? | Oui, avec import des anciens dossiers et `.bak` avant migration. |
| Q4 | Bloquer la publication si la CI est rouge ? | Oui pour `stable`, avertissement pour `unstable` au début. |
| Q5 | Quand mettre à jour Bevy ? | Entre deux versions de contenu, après TECH-1 et TECH-2, jamais au milieu d'un bloc de rendu (0.13 P, 0.14 D). |
| Q6 | `panic = "abort"` ? | Seulement avec le gestionnaire de crash de TECH-8 déjà en place. |
| Q7 | Télémétrie à distance ? | Non tant qu'on n'en a pas besoin ; locale seulement. |

## 17. Prompts (à coller dans une nouvelle session, un par phase)

**TECH-1** : « Lis `CLAUDE.md` et `roadmaps/a-faire/ROADMAP-technique.md` §3. Ajoute `.github/workflows/ci.yml` (fmt, clippy,
test, build des outils, cache Cargo, audit hebdomadaire) et fais dépendre `unstable.yml` / `stable.yml` de son
succès. Corrige les avertissements qui bloquent. PR, ne fusionne pas. »

**TECH-2** : « Lis `roadmaps/a-faire/ROADMAP-technique.md` §4. Écris les tests d'empreinte du monde (50 graines) et du terrain
(20 planètes), le test terrain / maillage, et le fuzz de 5 000 graines. Remplace les `HashMap` dont l'ordre
influence la génération. Les fichiers de référence vont dans `tests/golden/`. »

**TECH-3** : « Lis §5. Mesure d'abord (FPS `SPACESPORE_PERF`, taille, temps de build), puis essaie un à un `lto`,
`codegen-units`, `strip`, `mimalloc`, `lld`, et un profil `release-fast`. Garde ce qui gagne, tableau dans la PR. »

**TECH-4** : « Lis §6. Classe les `unwrap()` (sûr / entrée externe / test), traite `net.rs`, `editeur/format.rs`,
`editeur/edit.rs`, `editeur/vox.rs`, `guild.rs` et les chargements de `saves/`, ajoute les types d'erreur et un
fuzz de fichiers corrompus. Aucun changement de comportement hors erreurs. »

**TECH-5** : « Lis §7. Ajoute le champ `schema`, `migrate.rs` avec tests sur fichiers figés, l'écriture atomique
avec rotation, le dossier de sauvegarde stable avec import des anciens, et le stockage binaire compressé des
deltas voxel. »

**TECH-6** : « Lis §8. Passe `Msg` en binaire, découpe les fichiers en morceaux ≤ 1 000 octets avec accusés et
reprise, ajoute interpolation, validation des messages par l'hôte, limites de débit, bannissement, handshake avec
hash de génération, mode `--server`, et un simulateur de perte pour les tests. Augmente `PROTOCOL`. »

**TECH-7** : « Lis §9. Découpe UN fichier (indique lequel) en modules et plugins Bevy, sans changer le comportement ;
les tests d'empreinte de TECH-2 doivent rester verts. »

**TECH-8** : « Lis §10. Ajoute `tracing` vers fichier avec rotation, le gestionnaire de panic avec rapport de crash,
le bouton « Signaler un bug » et le filtrage des secrets. »

**TECH-9** : « Lis §11 et `TESTS-JEU.md`. Ajoute les commandes de test manquantes, le lanceur de scénarios
`SPACESPORE_SCENARIO`, les captures de référence avec comparaison, et le refus de capturer hors conditions. »

**TECH-10** : « Lis §12 et les résultats de la 0.20 T2. Prends UNE piste, mesure avant / après avec le banc
correspondant, joins les chiffres à la PR. »

**TECH-11** : « Lis §13. Ajoute les tests de démarrage Linux / macOS en CI, vérifie les chemins et la casse,
prépare la signature de code et la mise à jour différentielle du launcher. »

**TECH-12** : « Lis §14. Migre Bevy d'une version (indique laquelle) sur une branche dédiée, module par module,
vérifie chaque shader et chaque rendu spécial, mesure avant / après, garde les générateurs de graine figés. »
