# Feuille de route — Amélioration des mondes (0.16)

Objectif : des mondes **plus variés, plus cohérents et plus lisibles**, sans perdre le réalisme. Aujourd'hui
chaque planète est riche à elle seule, mais elles se ressemblent trop entre elles : la génération est
« verticalement riche, horizontalement pauvre ». La 0.16 ajoute ce qui manque : des **systèmes** qui ont une
identité, une **histoire** qui explique chaque planète, une **vie** qui façonne son monde, des **géantes**
qu'on reconnaît, et des **cailloux** qui ne se ressemblent plus. Tout est **mesuré** par un audit de population
qui échoue si un chiffre sort de sa cible.

| Source | Ce qui est repris |
|---|---|
| `analyse-planetes/RAPPORT-IA.md` | Mesures sur 20 000 systèmes, priorités P0-P3, vérification des 5 analyses d'IA |
| `analyse-planetes/ia-*.md` | Idées des 5 IA (Kimi, DeepSeek, ChatGPT, Qwen, Gemini), triées et vérifiées contre le code |
| `analyse-planetes/PLANETES.md` | Description de la génération actuelle (point de départ) |

Point de départ : la 0.15 terminée (minage et destruction). La 0.16 **adapte** ce que la 0.14 (archétypes,
événements) et la 0.15 (gisements, minage) ont construit sur l'ancienne génération (§14). Chaque phase = une
branche `claude/roadmap-0-16-<phase>`, une PR non fusionnée : tu testes, puis tu dis « push main ». Chaque
phase a son **prompt prêt à coller** (§12).

---

## 1. Décisions

### 1.1 Rappel

| Sujet | Décision |
|---|---|
| Qualité | **Aucune concession** : référence Ultra, mesures avant / après (règles 17-18 de la 0.13). |
| Réalisme | ~70 % scientifique ; tout ce qui est généré porte une étiquette **réaliste / spéculatif / fictif** (règle 5 de la 0.10). |
| Déterminisme | Une graine donne le même monde sur toutes les machines (règles 1-3 de la 0.10). |
| Raretés des traits | 98 % ordinaire, 1,5 % peu commun, 0,4 % rare, 0,1 % légendaire (`planetgen/traits.rs`). |
| Zone calme | Aucun archétype tiré dans les 50 systèmes du départ (0.14, Q4). |

### 1.2 Réponses du 05/10/2026

| # | Sujet | Décision |
|---|---|---|
| Q1 | **Cailloux nus** (69 % des rocheuses, 92 % des lunes) | **Garder la proportion, mais les varier** : couleur selon la composition et l'âge, soufre, verre d'impact, fer exposé, dunes, glaces, failles. |
| Q2 | **Oxygène** (94 % des mondes à O2 sans vie) | **L'O2 vient de la vie** (photosynthèse). O2 sans vie : rare, abiotique (monde asséché par une étoile active), expliqué au scanner. |
| Q3 | **Rotation synchrone** (77 % des rocheuses) | **Résonance 3:2 + verrouillage selon l'âge et la masse.** |
| Q4 | **Mondes habitables** (1 sur 61 813 rocheuses aujourd'hui) | **~1 système sur 3 000** a un monde habitable (score > 0,6). Remplace la rareté extrême de la 0.10. |
| Q5 | **Mondes déjà vus** | Aucun souci à tout casser (2 joueurs, en test). **Seule règle : même graine + même version = même monde.** |
| Q6 | **Paramètres de système** | **Tous** : métallicité de l'étoile, rapport C/O, archétypes de système, géographie galactique. |
| Q7 | **Histoire de la planète** | **Détaillée** : une frise d'époques qui **pilote** l'atmosphère, l'eau et la vie actuelles. |
| Q8 | **Géantes** | **Tout** : aspect physique (Sudarsky, taches, vents), paliers d'altitude, photoévaporation. |
| Q9 | **Corrections de physique** | **Toutes** : serre qui s'emballe, dégazage volcanique, effondrement du CO2, lunes et marées réelles. |
| Q10 | **Vie et biomes** | **Tout** : couleur de la flore selon l'étoile, biomes selon la chimie, la vie change la planète, climat régional. |
| Q11 | **Curiosités** | **~30 %** des mondes ont au moins une curiosité visible ; les traits rares restent rares. |
| Q12 | **Ordre des versions** | **0.14 → 0.15 → 0.16** : la 0.16 vient après et adapte archétypes et gisements. |
| Q13 | **Nouveaux types** | **Tous** : planètes naines, Hycean, mondes de fer / carbone, cœurs nus, océans de vapeur. |
| Q14 | **Départ** | **Variété garantie** près du système 0 (plusieurs sortes de mondes, rien d'exceptionnel : zone calme). |
| Q15 | **Audit** | **Test avec cibles** qui échoue si un chiffre sort de sa plage, et `/stats diversite` en jeu. |

---

## 2. Point de départ (mesures du 05/10/2026, 20 000 systèmes)

| Mesure | Aujourd'hui | Cause dans le code |
|---|---|---|
| Rocheuses / mini-Neptunes / géantes de glace / gazeuses | 69 / 15 / 7 / 8 % | `system.rs` : un `roll` indépendant par planète |
| Rocheuses sans air | **70,6 %** | tirage 25 % + Jeans + rayons X des naines M (66 % des étoiles) |
| Cailloux nus : rocheuses / lunes | **68,6 % / 91,8 %** | idem ; lunes < 0,02 M⊕ sans air, sans tectonique |
| Rocheuses synchrones | **76,7 %** | `a < 0,4 × ∛M` seulement (`system.rs:222`) |
| Eau liquide en surface | 1,5 % des rocheuses | |
| Habitables (score > 0,6) | **1** sur 61 813 rocheuses | |
| Mondes à O2 (> 5 %) | 3,5 % des rocheuses, **94 % sans vie** | O2 tiré dans `atmosphere.rs:285`, vie calculée après |
| Plantes avec biomes extraterrestres | 82 % des mondes à plantes | O2 indépendant de la vie |
| Vie : rocheuses / lunes | 2,1 % / **5,7 %** | 19,4 % des lunes ont un océan sous la glace (seuil de marée 0,05) |
| Vénus | 3,2 % des rocheuses, **toutes à < 0,41 × zone habitable** | composition choisie d'après `T_eq > 400 K`, vapeur plafonnée à 4 % |
| Traits tirés | 1,45 / 0,35 / 0,10 % | conforme |
| Rayons : rocheuses / géantes gazeuses | 0,34-1,78 / 7,0-16,8 R⊕ | Chen & Kipping ; Saturne à +24 % |
| Coût de génération | 20 000 systèmes en 1,6 s (~80 µs) | `planets_uncached` |

Ce qui n'existe pas encore : métallicité, C/O, histoire, corrélations entre planètes d'un système,
photoévaporation, emballement de serre, dégazage, effondrement du CO2, borne de Hill, résonances, rotateurs
lents, composition variable des géantes, taches, biomes par solvant, couleur de flore selon l'étoile,
climat régional, curiosités.

---

## 3. La nouvelle chaîne

```
Galaxie ── gradient de métallicité et d'âge (S1)
   │
Système ── métallicité, C/O, archétype de système (S2, S3)
   │
Étoile ── + courbes dans le temps : luminosité L(t), XUV(t), vent(t) (H1)
   │
Orbites ── selon l'archétype : résonances, migration, excentricités, inclinaisons (S3)
   │
Physique ── composition (fer / silicates / glaces / carbone), enveloppe H/He continue → type (P1)
   │        rotation : verrouillage selon l'âge, 3:2, rotateurs lents ; lunes, Hill, marées (P2)
   │
HISTOIRE ── frise d'époques, de la formation à aujourd'hui (H1-H3)
   │   apport d'eau et de volatils, dégazage, pertes (Jeans, XUV, photoévaporation, impacts),
   │   serre (emballement, effondrement du CO2), océans, glaciations, vie, oxygénation
   │
État actuel ── atmosphère, climat, eau, géologie, relief + traces du passé (H4)
   │
Climat régional ── ombres de pluie, courants, moussons, écotones (R1)
   │
Vie et biomes ── biochimie, couleur selon l'étoile, la vie change la planète (V1-V4)
   │
Ressources ── d'après la composition ET l'histoire (V4, adapte la 0.15)
   │
Visages ── géantes (G1-G2), cailloux variés (K1), curiosités ~30 % (K2), traits, habitabilité
```

La différence de fond : aujourd'hui l'**état actuel est tiré** ; demain il **résulte d'une histoire**. Une planète
rare l'est parce que sa combinaison de conditions et d'histoire est improbable, pas parce qu'un tirage l'a décidé.

---

## 4. Règles d'architecture (0.16)

Les règles des versions précédentes restent valables (1 à 19 de la 0.10 à la 0.13, 20 à 28 de la 0.14, celles
de la 0.15). La 0.16 ajoute (numérotées M1 à M12 pour ne pas dépendre de la numérotation de la 0.15) :

- **M1. Même graine + même version = même monde.** Pas de compatibilité avec les mondes d'avant (Q5). Chaque phase
  qui change la génération augmente `PROTOCOL` (`net.rs`) ; supprimer `saves/settings.json` et
  `saves/astres.json` pour tester. Valeurs affichées arrondies (empreinte réseau, règle de `system.rs`).
- **M2. La physique fixe l'important, le hasard les détails.** Plus une propriété compte (type, air, eau, vie), moins
  elle est tirée et plus elle découle des couches d'avant. Le hasard décide des formes, des curiosités, des
  détails de couleur.
- **M3. Chaque résultat a une raison lisible.** Tout ce qui sort de l'ordinaire a une cause enregistrée
  (`Cause` : texte court + couche) que le scanner et le dex affichent (« O2 : photosynthèse depuis 1,2 Gyr »,
  « pas d'air : soufflé par les éruptions de l'étoile »).
- **M4. Nouvelles couches = nouvelles sous-graines figées.** `Layer::System = 15`, `Layer::History = 16`,
  `Layer::Curiosity = 17`, `Layer::Galaxy = 18` (`seeds.rs`), jamais renumérotées. Ajouter un tirage dans une
  couche ne change pas les autres.
- **M5. Une histoire bornée.** La frise a un nombre fixe d'époques (16, espacées en logarithme du temps), sans boucle
  ouverte, en f64 puis arrondie. Coût cible : **génération d'un système ≤ 0,3 ms** (aujourd'hui ~0,08 ms),
  mesurée par `bench_system_gen`. Rien n'est stocké : la frise est recalculée comme les planètes.
- **M6. Le terrain ne paie pas l'histoire.** Les traces du passé (anciens rivages, lits à sec, bassins) sont des
  **paramètres** passés au relief et aux biomes, évalués dans les mêmes fonctions que la vue de l'espace
  (règle 16). `bench_tiles` ne bouge pas de plus de 5 %.
- **M7. Audit de population avec cibles.** `planetgen::audit` mesure 20 000 systèmes ; le test
  `population_matches_targets` échoue si un chiffre sort de sa plage (§13). Chaque phase met à jour les cibles
  qu'elle change, **avec la raison dans la PR**. `/stats diversite` affiche les mêmes chiffres en jeu.
- **M8. Étiquettes.** Hycean, planètes de carbone, couleur de flore, biochimies autres que carbone / eau : spéculatif ;
  gaz et minerais fictifs : fictif. L'étiquette se voit au scanner.
- **M9. Les calibrages du Système solaire restent vrais.** Terre, Vénus (à 0,72 UA, cette fois), Mars, Titan,
  Jupiter, Saturne (à 15 % près), Io, Europe, Pluton : un test par astre (`solar_system_analogs`).
- **M10. Variété du départ sans tricherie.** La variété près du système 0 vient d'un **choix déterministe parmi les
  tirages** (sous-graine de secours), jamais d'une planète écrite à la main ; rien d'exceptionnel (zone calme 0.14).
- **M11. Les archétypes de la 0.14 se branchent sur la physique.** Quand la 0.16 rend un archétype physique (monde
  de diamant, œil verrouillé, monde-océan), l'archétype tiré de la 0.14 devient une **conséquence** (règle 22 de
  la 0.14) ; on ne garde pas deux chemins.
- **M12. Les gisements suivent l'histoire.** Les gisements de la 0.15 sont recalculés depuis la composition et
  l'histoire ; le minage garde ses deltas (`BodyDelta::ores`).

---

## 5. Bloc A — Audit et fondations

### A0 — Audit de population (avant tout le reste)

- `planetgen/audit.rs` : parcourt N systèmes (20 000 par défaut) et compte tout ce que mesure le §2, plus :
  combinaisons (tempéré + océan + O2 + forêt...), archétypes de système, types (y compris les nouveaux),
  histoires (nombre de mondes ayant eu un océan, une glaciation globale, un emballement...), curiosités,
  variété au départ (§6 S4).
- Test `population_matches_targets` (`#[ignore]`, `cargo test --release population -- --ignored --nocapture`)
  avec les cibles du §13. Au départ, les cibles **décrivent l'existant** (le test passe), puis chaque phase les
  déplace.
- `/stats diversite [n|tout]` : le même rapport en jeu, panneau F3, fichier `saves/vX.Y.Z/stats/`.
- Corrélations : « vie complexe → O2 », « eau → vie », « O2 → forêt » en %, pour repérer les biais.

### A1 — Fondations

- Nouvelles couches (`Layer::System`, `History`, `Curiosity`, `Galaxy`, règle M4).
- `Cause` (règle M3) dans les profils, affichée au scanner (section « POURQUOI ») et dans `/profil`.
- `bench_system_gen` (règle M5) ; mesure de référence avant la 0.16.
- Test `solar_system_analogs` (règle M9) : écrit d'abord avec les tolérances actuelles (Saturne exclue,
  Vénus à 0,41), resserré au fil des phases.
- Doc : invariant des graines de lunes (`base + 500 + planète × 8 + lune` : au plus 8 lunes, 9 planètes), écrit
  et testé.

---

## 6. Bloc S — Galaxie et systèmes

### S1 — Géographie galactique

- Chaque système reçoit une **métallicité de base** et un **âge moyen** d'après sa place : gradient radial
  (~ −0,06 dex par kpc à l'échelle de la galaxie du jeu), centre vieux et riche en métaux, bord jeune et pauvre ;
  halo et amas globulaires vieux et très pauvres. Les galaxies extérieures et lointaines ont leur propre
  métallicité moyenne (petites galaxies plus pauvres).
- L'âge de l'étoile (aujourd'hui tiré dans `star.rs`) est tiré **autour** de l'âge local.
- Visible : la carte de la galaxie peut afficher la métallicité (filtre), le scanner d'étoile l'affiche.
- Effet sur le jeu : l'exploration a une direction (systèmes riches vers le centre, mondes jeunes au bord).

### S2 — Métallicité et C/O du système

- `[Fe/H]` : base galactique + écart propre (σ ~0,2 dex). Effets :
  - **fréquence des géantes** ∝ 10^(2 [Fe/H]) (Fischer & Valenti 2005) : un système pauvre a peu de géantes ;
  - masse du disque → nombre et masse des planètes ;
  - composition des rocheuses : part de fer, de silicates (P1).
- **C/O** : surtout ~0,55 (Soleil) ; rare au-delà de 0,8 → **planètes de carbone** (carbures, graphite, diamant
  en profondeur, ciel de méthane ; spéculatif).
- Les ceintures (C / S / M) suivent la métallicité.

### S3 — Archétypes de système

Tirés au niveau du système (`Layer::System`), d'après l'étoile et la métallicité ; ils fixent orbites,
excentricités, inclinaisons et nature des planètes **ensemble** :

| Archétype | Ce qui change | Surtout autour de |
|---|---|---|
| **Ordinaire** | comme aujourd'hui (corrigé) | tous |
| **Chaîne résonante compacte** (type TRAPPIST-1) | 4 à 8 rocheuses serrées, rapports de périodes 3:2, 4:3, 5:3 ; plusieurs dans la zone habitable | naines M |
| **Géante migratrice** | Jupiter chaud ou tiède, survivants excentriques, ceinture de débris massive, axes renversés | F, G, K riches en métaux |
| **Système agité** | excentricités jusqu'à 0,7, inclinaisons fortes, orbites polaires, peu de planètes | tous |
| **Système de géantes** | 2 à 4 géantes, peu de rocheuses, nombreuses lunes | riches en métaux |
| **Système pauvre** | 1 à 3 petites rocheuses, pas de géante | pauvres en métaux, halo |
| **Jeune bombardé** | étoile < 0,5 Gyr, cratères frais partout, comètes nombreuses, océans de magma | régions jeunes |
| **Système mort** | étoile morte (naine blanche), planètes survivantes, débris | naines blanches |

- Les espacements, excentricités et inclinaisons ne sont plus tirés planète par planète mais **par archétype**
  (stabilité : espacement en rayons de Hill mutuels ≥ ~8-10, règle simple).
- L'affichage logarithmique des orbites reste ; la chaîne résonante garde ses rapports de périodes.

### S4 — Variété garantie au départ (Q14)

- Les **20 systèmes les plus proches du système 0** doivent montrer au moins : une rocheuse avec de l'air, une
  géante avec anneaux, une lune glacée, un monde avec une mer (eau, méthane ou lave), un caillou coloré
  (K1), une planète naine.
- Méthode (règle M10) : si une sorte manque, le système le plus proche qui peut l'avoir prend un tirage de
  secours (sous-graine `Layer::System` + n), **toujours le même** : la variété ne dépend que de la graine du monde.
- Rien d'exceptionnel (zone calme de la 0.14) ; aucun monde habitable imposé.

---

## 7. Bloc P — Physique des planètes

### P1 — Composition et nouveaux types

- Composition tirée d'après la métallicité, le C/O et la distance : **fer** (noyau), **silicates**, **glaces**,
  **carbone**, **enveloppe H/He** (fraction de masse, continue).
- Le type découle de la composition (fini le tirage du type seul) :

| Type | Condition | Remarque |
|---|---|---|
| Rocheuse | enveloppe < 0,1 % | |
| **Monde de fer** | fer > 60 % | type Mercure, dense, champ magnétique fort |
| **Monde de carbone** | C/O > 0,8 | spéculatif, graphite / diamant, ciel de méthane |
| **Monde-océan** | glaces > 25 % | eau profonde, glace haute pression au fond |
| **Planète naine** | 0,1 à 0,34 R⊕ (tirée surtout au-delà des glaces) | Pluton, Cérès, Éris : azote gelé, tholins, cryovolcans |
| **Hycean** | océan + enveloppe H2 de 0,1 à 1 % | spéculatif ; chaud et humide loin de la zone habitable |
| Mini-Neptune | enveloppe 1 à 10 % | |
| Géante de glace / gazeuse | enveloppe > 10 % | |

- Rayon d'après la composition (fer, silicates, glaces, enveloppe) plutôt qu'une seule loi : **frontière
  rocheuse / mini-Neptune continue**. Saturne revient dans les 15 % (règle M9).
- Mise à jour de `size_class`, `/aller planete <type>`, `/stats`, scanner, dex, `PLANETES.md`.

### P2 — Rotation, lunes et marées réelles

- **Verrouillage** : temps de verrouillage `t_lock ∝ ω a⁶ Q / (m_étoile² k₂ R³)` ; verrouillée si `t_lock < âge`.
  Une jeune planète proche peut encore tourner.
- **Résonance 3:2** (Mercure) si l'orbite est excentrique (e > ~0,1) ; jour solaire très long, l'étoile qui
  revient en arrière dans le ciel (rendu dans `sky.rs` / `world_clock.rs`).
- **Rotateurs lents et rétrogrades** (type Vénus) : freinés par les marées atmosphériques d'une atmosphère épaisse.
- **Lunes** : limite de Hill (orbites stables < ~0,4 r_H prograde), masse totale des lunes bornée (< ~2 % de la
  planète sauf impact géant : système double type Terre-Lune ou Pluton-Charon), lunes **capturées** (rétrogrades,
  inclinées, irrégulières, autour des géantes).
- **Marées** en `a^-7,5 × e²` (formule réelle) ; Io, Europe, Encelade restent calibrés (règle M9) ;
  la part de lunes à océan sous la glace est recalculée et vérifiée par l'audit (19 % aujourd'hui).

### P3 — Photoévaporation et vents des géantes

- Perte d'enveloppe **limitée par l'énergie** : Ṁ = ε π F_XUV R³ / (G M), intégrée sur l'histoire XUV de l'étoile
  (saturée ~100 Myr pour une G, ~1 Gyr pour une M). Les mini-Neptunes proches perdent leur gaz → **cœurs nus**
  (« chthoniennes »). L'audit doit montrer la **vallée des rayons** (~1,5-2 R⊕).
- Jupiters chauds : gonflement d'après le flux reçu (plus de ×1,2 fixe).
- Vents : géantes de glace 400-600 m/s (Neptune), gazeuses 100-200 m/s.

---

## 8. Bloc H — Histoire de la planète (Q7 : détaillée)

### H1 — Le moteur d'époques

- `planetgen/history.rs` : **16 époques** de la formation à l'âge actuel (espacées en log du temps), règle M5.
- Courbes de l'étoile : luminosité L(t) (déjà dans `star.rs` : +30 % en 4,6 Gyr pour une G), XUV(t)
  (saturation puis déclin), vent(t), éruptions des naines M jeunes.
- État par époque : réserve de volatils (eau, CO2, N2, CH4), pression, température, état de l'eau, activité
  interne (`internal_activity(m, t)`), flux d'impacts (bombardement tardif), vie.
- Sortie : **état actuel** (remplace les tirages de `atmosphere.rs` et `hydrology.rs`) + **frise** de 3 à 8
  événements marquants + **traces** pour le relief (H4).

### H2 — L'atmosphère par l'histoire

- **Apport** : volatils d'après la distance (au-delà des glaces : riche), les impacts, la composition (P1).
- **Dégazage volcanique** (Q9) : l'activité interne alimente CO2, SO2, N2, H2O à chaque époque (Gemini).
- **Pertes** : Jeans (existant), XUV hydrodynamique, photoévaporation (P3), érosion par impacts, vent stellaire
  sans champ magnétique (le champ magnétique **protège** enfin l'atmosphère).
- **Serre qui s'emballe** (Q9) : au-delà du flux limite (~1,06 fois la Terre pour une G, selon la température de
  l'étoile, Kopparapu 2013), l'océan s'évapore, l'hydrogène fuit, le monde devient un **Vénus** ou un **océan de
  vapeur** (jeune). Vénus apparaît enfin à 0,72 UA (règle M9).
- **Effondrement du CO2** (Q9) : si les pôles passent sous le point de condensation du CO2, l'atmosphère se
  dépose en calottes, la pression baisse (Mars) ; cycles saisonniers du CO2 (pression qui varie avec la saison,
  calottes qui avancent et reculent : rendu par `Climate::season`).
- **Effondrement de l'azote** sur les planètes naines loin de leur étoile (Pluton).
- Le tirage « 25 % sans air » disparaît : l'absence d'air **découle** des pertes (Q1 : la proportion de cailloux
  reste voisine, la cible de l'audit le vérifie).

### H3 — Eau, climat passé, vie, oxygène

- Océans qui naissent, gèlent (**Terre boule de neige**), s'évaporent ; mers évaporées → sel (existant `salt`),
  anciens rivages.
- **Vie** (avec V1) : apparition quand un liquide dure assez ; vie simple, puis complexe, selon le temps passé
  dans des conditions stables (pas l'âge seul).
- **Oxygénation** (Q2) : la photosynthèse produit l'O2, qui ne s'accumule qu'après un délai (puits : fer,
  volcans ; Terre : ~1 à 2 Gyr). L'O2 **abiotique** est rare : photolyse de l'eau sur un monde qui l'a perdue
  autour d'une étoile active, étiqueté au scanner.
- **Habitables** (Q4) : la cible de l'audit est **~1 système sur 3 000** avec un monde au score > 0,6. Les
  réglages (fréquence des océans, délai de l'oxygène, seuils de stabilité) visent cette cible, en restant
  physiques (règle M2).

### H4 — Traces visibles et récit

- Paramètres passés au relief et aux biomes (règle M6) : **ancien niveau de la mer** (rivages et terrasses
  fossiles), **lits de rivières à sec**, deltas fossiles, **bassins d'impact géants** du bombardement,
  **provinces volcaniques** (coulées massives), **glaciations passées** (vallées en U, moraines, blocs
  erratiques), **cicatrices de dégazage**.
- Scanner : section « HISTOIRE » (frise courte : « il y a 3,8 Gyr : océan global ; 2,1 Gyr : refroidissement... »).
  Dex : frise complète.
- Ressources (règle M12) : sel et lithium des mers évaporées, fer rubané (oxygénation), charbon et pétrole
  (vie ancienne), uranium des granites (tectonique ancienne), placers des anciennes rivières.

---

## 9. Bloc V — Vie et biomes

### V1 — La vie par l'histoire

- La vie (`life.rs`) lit la frise : liquide stable pendant assez longtemps, énergie (lumière, chaleur interne,
  marées), radiation tolérable. Fini les microbes « sans liquide » sauf cas étiquetés (brumes organiques
  épaisses, spéculatif).
- **Biochimie** : carbone / eau (réaliste), azotosomes dans le méthane, carbone dans l'ammoniac (spéculatifs),
  silicium et aetherion (fictif), soufre (spéculatif).
- Adaptations : thermophiles, résistants aux radiations, grandes formes volantes en faible gravité, vie
  aquatique dominante sur les mondes-océans, métabolismes sans O2.

### V2 — Biomes selon la chimie

- Familles de biomes par solvant / chimie, au lieu des 3 biomes extraterrestres calqués sur les terrestres :
  - **carbone / eau, avec O2** : biomes terrestres actuels ;
  - **carbone / eau, sans O2** : tapis microbiens, stromatolithes, marais pourpres (bactéries anoxygéniques) ;
  - **méthane** : rives de tholins, « algues » de méthane, plaines d'hydrocarbures ;
  - **ammoniac** : forêts froides bleutées, glaces vivantes ;
  - **soufre** : plaines chimiosynthétiques autour des volcans ;
  - **fictifs** : cristal, spores, fongique (gardés, réservés aux mondes à gaz fictifs).
- Nouvelles matières voxel au besoin (une par biome, règle de la phase 6) ; décor (`decor.rs`) par famille.

### V3 — Couleur de la flore selon l'étoile

- Pigments d'après le spectre de l'étoile (Kiang 2007, spéculatif) : **sombre à noire** sous une naine M,
  rougeâtre sous une K, verte sous une G, bleutée ou jaune sous une F, selon la lumière au sol (atmosphère
  comprise). Teinte aussi du décor et de la couleur vue de l'espace (règle 16).
- Variante propre à chaque monde (légère) pour que deux mondes à forêts ne soient pas identiques.

### V4 — La vie change la planète

- Une biosphère modifie : **l'air** (O2, CH4, baisse du CO2), **les sols** (humus, argiles), **l'albédo** et la
  couleur, **les nuages** (noyaux de condensation), **l'érosion** (racines), **les ressources** (calcaire,
  charbon, pétrole, phosphates, fossiles).
- Rétroaction dans l'histoire (H3) : une biosphère qui baisse le CO2 refroidit le monde (glaciation possible).

---

## 10. Bloc R — Climat régional

### R1 — Le climat n'est plus seulement en bandes

- **Ombres de pluie** : derrière une chaîne de montagnes (vent dominant de `weather.rs`), le côté sous le vent est
  sec (déserts d'abri).
- **Courants océaniques** : côtes ouest froides et sèches, côtes est chaudes et humides (courants de bord ouest),
  d'après les continents et la rotation.
- **Moussons** : contraste terre / mer saisonnier (`Climate::season`).
- **Continentalité** : intérieurs plus secs et plus contrastés que les côtes.
- **Écotones** : transitions douces entre biomes (mélange par bruit aux frontières), **microclimats** (vallées
  froides, versants au soleil, oasis autour des sources chaudes).
- Mondes synchrones : vrai climat jour / nuit (point chaud sous l'étoile, anneau tempéré, face nuit glacée), en
  accord avec l'œil verrouillé de la 0.14 (règle M11).
- Une seule fonction pour le terrain et la vue de l'espace (règle 16) ; humidité régionale calculée **par
  cellule grossière** et mise en cache par astre (règle M6, `bench_tiles`).

---

## 11. Bloc G — Géantes

### G1 — Aspect physique

- **Classes de Sudarsky** d'après la température : I (< 150 K, nuages d'ammoniac, ocre et crème), II (~250 K,
  nuages d'eau, blanc), III (> 350 K, sans nuages, bleu profond), IV (> 900 K, métaux alcalins, sombre), V
  (> 1 400 K, nuages de silicates, rougeoyant).
- **Composition variable** : métallicité de l'enveloppe (1 à 10 × solaire, d'après S2) → méthane, brumes,
  couleur.
- **Taches** : grande tache persistante (0 à 1), ovales blancs, tempêtes, cicatrices d'impact rares ; nombre et
  largeur des bandes d'après la rotation.
- Aurores et éclairs visibles côté nuit ; lunes volcaniques qui laissent un tore (Io).
- `GasLook` agrandi : plus de 30 aspects qui **découlent** de la physique, plus 6 palettes figées.

### G2 — Paliers d'altitude

- Quatre paliers en descendant (`gas.rs`) : **haute atmosphère** (brume, aurores, vue sur les anneaux),
  **couche de nuages** (nuages en volume de la bonne chimie, éclairs, turbulences), **zone critique** (chaleur,
  pression, obscurité, dégâts qui montent), **cœur** (lueur, `GAS_CORE`).
- Chaque palier a ses dangers, sa lumière, sa météo (vents par bandes, vents verticaux) et, pour la 0.15, ses
  ressources (deutérium, hélium-3 en haut, minerais rares près du cœur).
- Les mini-Neptunes et Hycean ont leurs propres paliers (Hycean : océan sous le gaz).

---

## 12. Bloc K — Cailloux variés et curiosités

### K1 — Chaque caillou est différent (Q1)

- Couleur du sol d'après **la composition** (fer → ocre et rouille, carbone → noir, glaces → blanc, soufre →
  jaune, olivine → vert), **l'âge** (régolithe mûr sombre, cratères frais clairs), **la radiation** (glaces
  brunies), **le vent de l'étoile**.
- Surfaces : **verre d'impact**, champs de dunes (vent faible mais sur des milliards d'années), failles et
  escarpements de contraction (Mercure), terrains chaotiques, **fer exposé**, plaines de soufre, glace en
  mosaïque, **tholins** rouges (planètes naines), dépôts de givre aux pôles, poussière qui lévite au terminateur
  (spéculatif).
- Lunes : familles (vieille et cratérisée, fracturée, cryovolcanique, sulfureuse type Io, capturée sombre,
  berger d'anneau, binaire).
- Mesure : l'audit compte les « sortes de cailloux » ; aucune ne doit dépasser ~20 % des cailloux.

### K2 — Curiosités (~30 %, Q11)

- `planetgen/curiosities.rs` (`Layer::Curiosity`) : chaque monde peut avoir 0 à 2 curiosités **permises par sa
  physique** (comme `TraitContext`), **placées** sur la surface (coordonnées) et visibles : sources chaudes, lacs
  salés colorés, dunes chantantes, champs de météorites, geysers isolés, cristaux affleurants, arche géante,
  grotte remarquable, cratère frais avec rayons, lac de lave, cheminées hydrothermales, givre coloré, chutes
  d'eau géantes, fossiles affleurants (vie ancienne).
- Cible de l'audit : **25 à 35 %** des mondes solides en ont au moins une. Les traits rares / légendaires
  restent à 2 %.
- Scanner : curiosité la plus proche (comme la grotte la plus proche) ; dex ; `/aller curiosite <nom>`.

### K3 — Calibrage final

- Tous les chiffres de l'audit dans leurs cibles (§13), `PLANETES.md` réécrit pour la nouvelle chaîne,
  `analyse-planetes/RAPPORT-IA.md` complété par un « après ».

---

## 13. Cibles de l'audit (point de départ, réglées pendant la 0.16)

| Mesure | Aujourd'hui | Cible 0.16 |
|---|---|---|
| Systèmes avec un monde habitable (score > 0,6) | ~1 / 20 000 | **~1 / 3 000** (1 / 2 000 à 1 / 5 000) |
| Mondes à O2 sans vie | 94 % des mondes à O2 | **< 10 %**, tous avec une cause abiotique |
| Mondes à plantes avec biomes de la bonne chimie | 18 % « terrestres » | 100 % cohérents (O2 ↔ photosynthèse) |
| Rocheuses synchrones | 76,7 % | 45-65 % (le reste : 3:2, jeunes, lentes) |
| Cailloux nus : rocheuses / lunes | 68,6 / 91,8 % | 60-75 / 85-95 % (Q1 : garder) |
| Sorte de caillou la plus fréquente | — | < 20 % des cailloux |
| Vénus au-delà de 0,5 × zone habitable | 0 | > 0 (Vénus à 0,72 UA) |
| Mondes ayant eu un océan dans leur histoire | — | mesuré, > 5 % des rocheuses |
| Lunes à océan sous la glace | 19,4 % | mesuré après P2, justifié |
| Curiosités | 0 | 25-35 % des mondes solides |
| Traits tirés | 1,9 % | inchangé (~2 %) |
| Planètes naines | 0 | mesuré, surtout au-delà des glaces |
| Cœurs nus / vallée des rayons | — | creux visible entre 1,5 et 2 R⊕ |
| Systèmes à géantes : pauvres / riches en métaux | — | rapport ~10 (10^(2 × 0,5)) |
| Archétypes de système | — | chacun entre 1 % et 60 % |
| Variété au départ (20 systèmes) | — | 100 % des sortes du §6 S4 |
| Coût d'un système | ~0,08 ms | ≤ 0,3 ms |

---

## 14. Liens avec la 0.14 et la 0.15

| Version | Ce qui existe | Ce que la 0.16 en fait |
|---|---|---|
| 0.14 X1 œil verrouillé | archétype tiré | **conséquence** du climat synchrone (R1, P2) |
| 0.14 X2 mondes-océans | archétype | **conséquence** de la composition (P1) et de l'histoire (H3) |
| 0.14 X3 refroidissement, continents sur magma | archétype | époques jeunes de l'histoire (H1), système jeune (S3) |
| 0.14 X4 lune trop proche, Roche | archétype | P2 (Hill, Roche, marées réelles) |
| 0.14 X6 mondes de diamant, de métal | archétype | **types** physiques (P1 : carbone, fer) |
| 0.14 X8 monde mort, terraformation inachevée | archétype | peut lire l'histoire (H4) |
| 0.14 D3-D4 végétation, faune | sur l'ancienne vie | biochimies et couleurs (V1-V3) |
| 0.14 N1-N4 événements | planètes actives | activité tirée de l'histoire (H1) |
| 0.15 gisements, minage | `resources.rs` | recalculés depuis composition + histoire (règle M12, H4) |

Règle M11 : un seul chemin par phénomène. Les tests de fréquence de la 0.14 sont repris par l'audit A0.

---

## 15. Ordre et versions

```
0.15 terminée
   │
   A0 → A1                          audit et fondations
   │
   S1 → S2 → S3 → S4                galaxie et systèmes
   │
   P1 → P2 → P3                     physique des planètes
   │                                ── release 0.16.0 « Systèmes » ──
   H1 → H2 → H3 → H4                histoire (H2 a besoin de P3, H3 de V1)
   │
   V1 → V2 → V3 → V4                vie et biomes (V1 en même temps que H3)
   │                                ── release 0.16.1 « Histoire » ──
   R1                               climat régional
   │
   G1 → G2                          géantes
   │
   K1 → K2 → K3                     cailloux, curiosités, calibrage
                                    ── release 0.16.2 « Visages » ──
```

Dépendances : S4 a besoin de K1 pour « caillou coloré » (en attendant : n'importe quelle rocheuse sans air) ;
G1 utilise la métallicité (S2) ; H2 utilise la photoévaporation (P3) ; V3 utilise le spectre de l'étoile (déjà
dans `star.rs`) ; R1 utilise les vents de `weather.rs` ; K3 recalibre tout.

---

## 16. Risques

| Risque | Parade |
|---|---|
| **Coût** de l'histoire sur ~145 000 systèmes au lancement | Rien n'est stocké, la frise est calculée à la demande (comme les planètes) ; 16 époques fixes ; `bench_system_gen` ≤ 0,3 ms (règle M5) |
| **Déterminisme** en f64 (`powf`, `ln`, `exp` sur 16 époques) | Arrondi des sorties, tests de reproductibilité (`the_world_is_reproducible`) sur Windows / Linux / Mac en CI |
| **Cascade trop complexe** à régler (chaque réglage bouge tout) | Audit avec cibles (A0) à chaque phase ; un réglage = une PR avec l'avant / après des chiffres |
| **Habitables 1/3 000** difficile à atteindre physiquement | Leviers listés en H3 ; si impossible sans tricher, on te demande (§17) |
| **Tests calibrés** de la 0.10-0.13 qui cassent | Remplacés par `solar_system_analogs` (règle M9), jamais supprimés sans équivalent |
| **0.14 / 0.15 à adapter** (archétypes, gisements) | §14 ; une phase ne fusionne pas tant que les tests de la 0.14 / 0.15 passent |
| **Climat régional** coûteux dans les tuiles | Cellules grossières en cache par astre (règle M6) |
| **Taille de la 0.16** | Trois releases, chaque phase jouable seule |

---

## 17. Questions restantes

Aucune bloquante : Q1 à Q15 sont tranchées (§1.2). À trancher pendant les phases, avec mesures à l'appui :

- Cible exacte des synchrones (45-65 % proposé, §13).
- Part des lunes à océan sous la glace après les marées réelles (P2).
- Si la cible des habitables (1/3 000) demande un réglage non physique : lequel accepter (H3).

---

## 18. Prompts (à coller dans une nouvelle session, un par phase)

Contexte commun : « Lis `ROADMAP-0.16.md`, `analyse-planetes/RAPPORT-IA.md`, `analyse-planetes/PLANETES.md` et
`CLAUDE.md` (règles M1 à M12, décisions Q1 à Q15 du §1.2). `git pull origin main` avant de coder. Branche
`claude/roadmap-0-16-<phase>`. Build release, tests, audit de population avant / après (§13), `bench_system_gen`
et `bench_tiles`, captures. PROTOCOL +1 si la génération change. PR non fusionnée (je dirai « push main »).
Aucune concession sur la qualité ; ne change ni les cibles ni les décisions sans me demander. »

- **A0** — « [contexte commun] Phase A0 : `planetgen/audit.rs`, test `population_matches_targets` (cibles =
  l'existant au départ), `/stats diversite`, corrélations. »
- **A1** — « [contexte commun] Phase A1 : couches `System`, `History`, `Curiosity`, `Galaxy` (règle M4), `Cause`
  au scanner (section POURQUOI) et dans `/profil`, `bench_system_gen`, test `solar_system_analogs`, invariant des
  graines de lunes. »
- **S1** — « [contexte commun] Phase S1 : gradient galactique de métallicité et d'âge, âge des étoiles tiré
  autour de l'âge local, galaxies extérieures, affichage au scanner et sur la carte. »
- **S2** — « [contexte commun] Phase S2 : [Fe/H] et C/O par système, fréquence des géantes ∝ 10^(2[Fe/H]), masse
  du disque, ceintures ; audit. »
- **S3** — « [contexte commun] Phase S3 : archétypes de système (§6 S3), orbites, excentricités et inclinaisons
  par archétype, stabilité en rayons de Hill mutuels, chaîne résonante ; `/aller systeme <archétype>`. »
- **S4** — « [contexte commun] Phase S4 : variété garantie dans les 20 systèmes du départ (règle M10), tirages
  de secours déterministes, test. »
- **P1** — « [contexte commun] Phase P1 : composition (fer, silicates, glaces, carbone, enveloppe), type qui en
  découle, mondes de fer, de carbone, océans, planètes naines, Hycean ; rayon par composition, Saturne à 15 %. »
- **P2** — « [contexte commun] Phase P2 : verrouillage selon t_lock et l'âge, résonance 3:2 (ciel), rotateurs lents,
  Hill, masse des lunes, lunes capturées, marées en a^-7,5 e² ; océans sous la glace mesurés. »
- **P3** — « [contexte commun] Phase P3 : photoévaporation (cœurs nus, vallée des rayons), gonflement des
  Jupiters chauds, vents des géantes de glace. »
- **H1** — « [contexte commun] Phase H1 : `planetgen/history.rs`, 16 époques, courbes L(t), XUV(t), vent(t),
  activité interne, impacts ; sortie état actuel + frise + traces ; règle M5. »
- **H2** — « [contexte commun] Phase H2 : atmosphère par l'histoire : apport, dégazage, pertes (Jeans, XUV,
  impacts, vent sans champ magnétique), serre qui s'emballe (Vénus à 0,72 UA, océans de vapeur), effondrement du
  CO2 et de l'azote, cycles saisonniers du CO2 ; fin du tirage 25 %. »
- **H3** — « [contexte commun] Phase H3 (avec V1) : océans, boule de neige, mers évaporées, vie selon la durée
  des conditions stables, oxygénation par la photosynthèse avec délai, O2 abiotique rare ; habitables ~1/3 000. »
- **H4** — « [contexte commun] Phase H4 : traces (anciens rivages, lits à sec, bassins d'impact, provinces
  volcaniques, glaciations) dans le relief et les biomes (règles 16 et M6), section HISTOIRE au scanner et frise
  au dex, ressources issues de l'histoire. »
- **V1** — « [contexte commun] Phase V1 : vie lue dans la frise, biochimies étiquetées, adaptations ; plus de
  microbes sans liquide non étiquetés. »
- **V2** — « [contexte commun] Phase V2 : familles de biomes par chimie (§9 V2), matières voxel et décor. »
- **V3** — « [contexte commun] Phase V3 : couleur de la flore selon le spectre de l'étoile au sol (spéculatif),
  variante par monde, vue de l'espace = au sol. »
- **V4** — « [contexte commun] Phase V4 : la biosphère change l'air, les sols, l'albédo, les nuages, l'érosion
  et les ressources ; rétroaction dans l'histoire. »
- **R1** — « [contexte commun] Phase R1 : ombres de pluie, courants, moussons, continentalité, écotones,
  microclimats, climat jour / nuit des synchrones ; cache par astre, `bench_tiles`. »
- **G1** — « [contexte commun] Phase G1 : classes de Sudarsky, composition des enveloppes, taches et ovales,
  bandes selon la rotation, aurores et éclairs, tore volcanique ; 30+ aspects. »
- **G2** — « [contexte commun] Phase G2 : quatre paliers d'altitude dans les géantes (`gas.rs`), dangers,
  lumière, météo, ressources ; mini-Neptunes et Hycean. »
- **K1** — « [contexte commun] Phase K1 : couleurs et surfaces des cailloux selon composition, âge, radiation ;
  familles de lunes ; aucune sorte > 20 %. »
- **K2** — « [contexte commun] Phase K2 : `planetgen/curiosities.rs`, ~30 % des mondes solides, placées et
  visibles, scanner, dex, `/aller curiosite`. »
- **K3** — « [contexte commun] Phase K3 : toutes les cibles de l'audit atteintes, `PLANETES.md` réécrit,
  `RAPPORT-IA.md` complété par un après. »
