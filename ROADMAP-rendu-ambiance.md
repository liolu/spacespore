# Feuille de route — O. Rendu et ambiance

Source : bloc **O** de `RAPPORT-ameliorations.md` (idées 195 à 206) + idées 27 à 33.
Les phases s'appellent **REN-n**.

Objectif : une **image cohérente et marquante** — lumière, matière, atmosphère, post-traitement — qui reste
**jouable** (budgets de ms par image mesurés) et **lisible** (réglable, accessible), sans toucher à la génération.

Cette feuille de route **complète** : `ROADMAP-0.13.md` (C1 brouillard, C2 skybox, V1 trous noirs, O1 / O2 eau,
P3 rentrée, P4 nuages traversables, P5 poussière), `ROADMAP-0.14.md` (D3 végétation, D4 faune) et `ROADMAP-0.20-debug-opti.md`
(LOD, lumière). Elle **ne refait pas** ce qui y figure : elle y ajoute l'**ambiance**, les **effets de caméra et de
visière** et le **post-traitement**.

---

## 1. Point de départ (code du 05/10/2026)

- **Moteur** : Bevy 0.15 (PBR, MSAA, ombres en cascades, `SurfaceSun` par soleil, lumière ponctuelle des étoiles
  hors surface), `graphics.rs` (323 l.) : réglages (vsync, limite d'images, MSAA, ombres, LOD, nuages, éruptions,
  `render_scale`, `terrain_detail`, `relief_shadows`).
- **Shaders maison** (`assets/shaders/`) : `far_star.wgsl`, `src/water.wgsl`, `src/caustics.wgsl` — très peu :
  presque tout passe par les matériaux standard de Bevy.
- **Ciel** : `sky.rs` (jour / orages / éclipses / `SunDim`), couleurs du ciel de la phase 3 (`atmosphere.rs`), nuit
  avec clair de lune, galaxie visible la nuit (`NIGHT_GALAXY`), brume `gas.rs` (`haze_opacity`).
- **Effets déjà présents** : eau (vagues, écume, sous l'eau, caustiques), lave lumineuse (`GeoLava`, `GeoLight`),
  particules face caméra (`GeoParticles`, météo), aurores (rideaux la nuit), queues de comètes, éruptions solaires,
  lampes (marcheur, phares) puissance calée sur l'étoile.
- **Limites connues** : lumière et LOD = les deux plus gros problèmes (0.20), nuages = cubes, pas de post-traitement
  poussé (pas de bloom / flare / profondeur de champ / flou de mouvement vus dans les options), pas de réflexions
  de la mer, pas d'effets de visière / cockpit.

## 2. Règles

1. **Un effet = un réglage** dans Options (Désactivé / Bas / Haut / Ultra) et une **valeur de repli** : tout doit
   se couper pour une petite machine.
2. **Budget** : chaque effet reçoit un budget (ms GPU) mesuré par `RenderDiagnosticsPlugin` (0.20 T2) ; rien n'est
   fusionné sans le chiffre avant / après.
3. **Réalisme physique d'abord** : couleurs de l'atmosphère, diffusion, lumière des étoiles viennent des profils
   (`atmosphere.rs`, `lumens`), pas de valeurs peintes à la main.
4. **Accessibilité** : flou, bloom, secousses, flashs ont une **option de réduction** (CTL-7).
5. **Shader compilé = shader validé** (règle 4 de `TESTS-JEU.md`) : toute nouvelle `.wgsl` passe par une capture.
6. **Le monde ne change pas** : on n'écrit que dans le rendu, jamais dans la génération.
7. **Compatibilité Bevy** : privilégier les API stables ; éviter les internes qui changent à chaque version
   (voir TECH-12).

## 3. Les phases

| Phase | Contenu | Idées | Taille |
|---|---|---|---|
| **REN-1. Pipeline d'image et post-traitement** | Passer en **HDR** avec **tonemapping** réglable (AgX / TonyMcMapface), **bloom** (lampes, lave, étoiles, soleil) avec seuil et force, **exposition automatique** (œil qui s'adapte du jour à la nuit, de la lumière aux grottes : rétablissement progressif), **correction colorimétrique** par type d'étoile et d'atmosphère (soleil rouge = image plus chaude), **vignettage** léger, **grain** optionnel. Réglages dans Options + préréglages. | 29, 33, 195 | M |
| **REN-2. Soleils, lentilles et éblouissement** | **Disque solaire** physique (taille réelle à l'écran, couleur du type d'étoile), **éblouissement** quand on regarde le soleil (exposition qui chute, bloom), **flare d'objectif** discret, **rayons crépusculaires** (volumétrique léger en espace écran), **ombres colorées** de plusieurs soleils (double coucher), **éclipses** avec couronne (lien `sky.rs::occultation`) — réutilise `SunDim`. | 197, 201 | M |
| **REN-3. Atmosphère physique** | **Diffusion** Rayleigh / Mie pour des **couchers de soleil exacts** selon les gaz (`atmosphere.rs` donne déjà des couleurs de ciel : passer à un modèle de diffusion par rayon, précalculé dans une texture 3D / LUT), **perspective aérienne** (montagnes qui bleuissent à distance, couleur selon le gaz), ciel qui **noircit avec l'altitude** (0.13 P2 : sans coupure), halo atmosphérique vu de l'espace (limbe lumineux), **brume de poussière / de méthane** teintée. Sert de base à MON-6 (optique du ciel) et 0.13 C1. | 206, 200 | L |
| **REN-4. Matière : métal, verre, cristal, eau** | **Réflexions** (SSR de la mer et des surfaces lisses, cubemap locale pour le métal), **réfraction** (verre, glace, cristaux : `VoxelType::Glass`, cristaux de MON-4), **matériaux émissifs** corrects (lueurs sans lumière parasite, `Material::lumineuse` de l'éditeur), **rugosité par matière** (mouillé, givré, sableux), **sol mouillé** après la pluie (lien MON-5). Réutilise `water.wgsl` pour les réflexions planaires de la mer. | 30, 196, 203 | L |
| **REN-5. Lumière dans les volumes** | **Brouillard volumétrique** léger (0.13 C1 étend), **rayons de lumière** dans la poussière et dans les grottes (`dim_star_light` + lampe), **phares** qui éclairent le brouillard, **lueur de lave et de cristaux** qui éclaire alentour (lumières ponctuelles bornées : budget de 8 par vue), **lumière ambiante des grottes** selon la roche (cristaux, champignons, lave : couleur du `CaveStyle`). | 197, 205 | L |
| **REN-6. Caméra, visière et cockpit** | **Visière du casque** (buée par froid et humidité, givre, gouttes de pluie, éclaboussures, poussière, reflets du visage, reflet du soleil), **verrière du cockpit** (gouttes, givre en altitude : lien 0.13 P8, **reflets** de la planète dessous, rayures), **essuie-glace** / dégivrage animé, **secousses** de caméra (chocs, séismes, rentrée : `Surface::tilt`) avec option de réduction, **flou de mouvement** et **profondeur de champ** optionnels (mode photo, cinématique), effets désactivés en mode « performance ». | 198, 199, 203, 195 | L |
| **REN-7. Particules et traînées** | **Traînées de moteurs** (flamme, plasma, gaz selon le propulseur de l'éditeur), **poussière d'atterrissage** (selon la matière du sol), **étincelles de rentrée** (arrachent des morceaux visibles si le bouclier est coupé : voir 0.13 P3, Q8), **débris de météorites**, **traînées d'étoiles filantes** (`meteors.rs`), **sillages de lumière** des autres vaisseaux la nuit, **particules GPU** (mêmes règles de budget que `GeoParticles`), `NoFrustumCulling` conservé là où les sommets sont réécrits à chaque image. | 31, 202, 204 | M |
| **REN-8. Nuages volumétriques** | Remplacer (ou compléter, en détail proche) la couche **en cubes** de `weather.rs` par un rendu **volumétrique** : **ray-march** limité (budget 1–2 ms), formes stables (même champ `cloud_field`), **ombres de nuages** au sol, **bords éclairés** à contre-jour, **traversée** (0.13 P4) avec brume interne, **orages** avec **éclairs internes** qui illuminent le nuage ; repli sur les cubes en qualité basse. | 27 | XL |
| **REN-9. Espace profond et galaxies** | Étoiles à **couleurs et scintillement** physiques, **Voie lactée** plus fine (poussière sombre, bandes de formation d'étoiles), **nébuleuses** volumétriques (AST-4), **galaxies lointaines** en sprites avec spirales, **lentille gravitationnelle** (0.13 V1), **tunnel de voyage** entre galaxies (0.13 V2), **lumière zodiacale**, bloom des étoiles brillantes. Partage avec la skybox 0.13 C2. | 200 | L |
| **REN-10. Anti-crénelage et échelle** | **TAA** (avec netteté et rejet des fantômes), **FXAA / SMAA** de repli, **échelle de rendu dynamique** (la résolution interne s'adapte pour tenir 60 images/s : `render_scale` existe), **upscale** type FSR (si licence compatible), amélioration du **`msaa_samples`** avec les nouveaux effets, **détection de la machine** au premier lancement (GPU / RAM) → préréglage proposé. | 32 | M |
| **REN-11. Accessibilité visuelle** | **Daltonisme** (filtres deutéranopie / protanopie / tritanopie, palette de l'interface), **contraste élevé** de l'interface, **réduction des flashs** (éclairs, éruptions, bloom soudain), **réduction de la secousse**, **réduction du mouvement** (parallaxe, flou), **taille du texte**, **mode HDR** réel (écran HDR : luminosité de pointe réglable), **sous-titres** visuels pour les alertes sonores (D5). | 33 | M |
| **REN-12. Mode photo** | Touche dédiée (CTL-9 ou palette) : **caméra libre** détachée (gèle le temps en solo), FOV, exposition, DoF, bloom, grain, **cadres**, **filtres** par monde, **HUD masqué**, **sauvegarde** dans `export/photos/` avec métadonnées (astre, heure, graine) et **lien au dex** (« photo prise »). | — | M |
| **REN-13. Qualité et mesures** | **Préréglages** Patate / Bas / Moyen / Haut / Ultra qui regroupent tous les effets ci-dessus (lié 0.20 T9), **tableau de bord** des coûts par effet (0.20 T3, F6), tests de capture de référence (TECH-9) pour détecter une régression visuelle, **profil de couleur** neutre pour les captures de test. | — | M |

Ordre conseillé : **REN-1** (le socle HDR / tonemapping) → **REN-3** (atmosphère) → **REN-2 et REN-5** → **REN-6**
(effets de visière : forte valeur ressentie) → **REN-4, 7** → **REN-9** → **REN-8** (le plus lourd, après les mesures
de la 0.20) → **REN-10, 11, 12, 13** au fil de l'eau.

## 4. Détails importants

### 4.1 Le socle (REN-1) change **tout** l'aspect
Le passage en HDR + tonemapping modifie les valeurs d'exposition de **toutes** les lumières déjà calées (`lumens`,
`light_range_for`, lampes, éclairs, lave). À faire **en une fois**, avec une **table de recalage** (soleil, lampe,
lave, éclair, étoile lointaine) et une capture de référence **par monde de test** avant / après. Les shaders
existants (`water.wgsl`, `far_star.wgsl`) doivent écrire dans l'espace linéaire HDR.

### 4.2 Atmosphère par rayon (REN-3)
LUT de transmittance et de diffusion précalculées **par atmosphère de planète** (une texture par astre chargé,
recalculée seulement quand `PlanetConfig::atmosphere` change : jamais en cours de jeu). Les couleurs du ciel de la
phase 3 servent de **vérification** (test : la couleur moyenne du zénith reste dans ±10 % de l'ancienne).

### 4.3 Nuages volumétriques (REN-8)
Le champ de densité reste celui de `weather::cloud_field` (déterministe, partagé entre joueurs). Le rendu lit ce
champ dans une **grille 3D basse résolution** reconstruite en arrière-plan (comme `rebuild_clouds` aujourd'hui),
puis fait le ray-march en espace écran à **demi résolution** avec accumulation temporelle. Les cubes restent le
repli pour la qualité basse et pour la vue lointaine depuis l'espace.

### 4.4 Effets de visière
Un **calque d'écran** (UI) au-dessus du rendu, piloté par des **variables d'état** : humidité, température, pluie,
poussière, gel, lumière. Pas de simulation : une valeur → un motif de gouttes (bruit + texture procédurale) avec
évaporation dans le temps. Coût quasi nul.

### 4.5 Ce qui n'est **pas** inclus ici
Lentille gravitationnelle, skybox, brouillard sol : déjà en 0.13. Végétation et faune : 0.14. LOD et lumière du
moteur : 0.20. Seul le **rendu** de ce qui est listé ci-dessus est traité ; pas de nouvelle génération.

## 5. Mesures et tests

- **Avant / après** pour chaque phase : images/s médianes et 1 % bas (`SPACESPORE_PERF`), ms GPU de la passe, mémoire
  GPU, sur 3 scènes : surface de jour, orbite, grotte (scénarios de TECH-9).
- **Captures de référence** : un monde de test par phase (désert, mer, glace, géante, grotte, nuit), conditions
  vérifiées avant capture.
- **Shaders** : chercher `panic|wgsl|naga|error` dans le log (règle 4 de `TESTS-JEU.md`).
- **Multi-OS** : démarrage sur Windows / Linux / macOS pour les nouveaux shaders (TECH-11), Metal et Vulkan
  diffèrent sur HDR et compute.
- **Régression** : test d'empreinte du monde **inchangé** (le rendu ne doit pas toucher à la génération).

## 6. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | HDR et tonemapping par défaut ? | **Oui** (REN-1), avec préréglage « Classique » qui ressemble à l'image actuelle. |
| Q2 | Nuages volumétriques obligatoires ? | **Non** : option Haut / Ultra, repli en cubes. |
| Q3 | Quel upscaler ? | Échelle de rendu maison + TAA d'abord ; FSR seulement si sa licence et son intégration à Bevy 0.15 sont propres. |
| Q4 | Mode photo : stocke-t-il les photos dans le dex ? | Oui (miniature + métadonnées), avec limite de taille. |
| Q5 | Effets de visière : par défaut ? | Oui, intensité réglable, désactivables (accessibilité). |
| Q6 | Flou de mouvement par défaut ? | Non (désactivé), opt-in. |
| Q7 | Mise à jour de Bevy avant ou après REN-1 ? | Avant si possible (les API de post-traitement évoluent beaucoup) : voir TECH-12. |

## 7. Prompts

**REN-1** : « Lis `CLAUDE.md`, `TESTS-JEU.md` et `ROADMAP-rendu-ambiance.md` §3 REN-1 et §4.1. Passe en HDR avec
tonemapping, bloom et exposition automatique réglables, recale les lumières existantes avec une table, capture
avant / après sur les mondes de test, mesure images/s. Option « Classique » proche de l'image actuelle. »

**REN-2** : « §3 REN-2 : disque solaire, éblouissement, flare discret, rayons crépusculaires, ombres colorées de
plusieurs soleils. Capture sur un système double de jour et au coucher. »

**REN-3 à REN-13** : « Lis `ROADMAP-rendu-ambiance.md` §3 REN-n, implémente avec réglage + repli, mesure avant /
après, capture sur monde adapté, PR non fusionnée. »
