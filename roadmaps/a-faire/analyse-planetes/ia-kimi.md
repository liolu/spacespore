# Analyse de PLANETES.md — SpaceSpore

## Verdict global

C'est un document de conception d'un niveau rare pour un jeu indé : la génération est **causale** (chaque couche découle des précédentes), **calibrée** (tests contre le Système solaire), et surtout **honnête** — la colonne « rigueur » (réaliste / spéculatif / fictif) qui sépare clairement la science du fantasy est une excellente pratique. Les faiblesses ne sont pas dans les formules mais dans les **distributions** : trop de tirages indépendants, trop peu de corrélations entre paramètres, et une architecture de système trop « système solaire moyen ». Détail par détail.

---

## 1. Cohérence scientifique

### Points forts (solides)

- **Relation masse-rayon de Chen & Kipping (2017)** avec test de calibration : c'est la bonne référence, et le raccord à 130 M⊕ vers la branche dégénérée (`R ∝ M^-0,044`) reproduit bien le fait que Jupiter et une géante 10× plus massive ont quasiment le même rayon. La borne haute à 4 000 M⊕ (~12,6 M_J) s'arrête proprement juste sous le seuil de fusion du deutérium (13 M_J) où prennent le relais vos naines brunes. Cohérent.
- **Fuite de Jeans** avec le critère classique `v_lib > 5 × v_thermique`, l'exosphère à ~3×T_eq, et l'amplification par les rayons X des étoiles actives : le mécanisme qui dénude les planètes de naines rouges est le bon.
- **Effet de serre calibré** (Terre +33 K, Vénus +500 K, Titan +12 K) avec itération de Clausius-Clapeyron pour la rétroaction de la vapeur d'eau — c'est le bon modèle (atmosphère grise, τ optique).
- **Diagramme de phase réel pour l'hydrologie** (point triple 6 mbar, point critique 647 K / 220,6 bar) : la mer à sec au-dessus de l'ébullition, la banquise, le supercritique — tout découle de la physique plutôt que de règles ad hoc.
- **Circulation atmosphérique** : 3 cellules vs super-rotation vs flux jour→nuit selon rotation et pression, c'est exactement la taxonomie réelle (Terre / Vénus / mondes verrouillés).
- Détails fins : hélium-3 dans le régolithe exposé au vent stellaire, deutérium dans les océans, glace superionique, tectonique des plaques conditionnée à l'eau liquide (hypothèse défendable), lave à > 1 300 K, Océan sous glace d'Europe via chauffage de marée, limites de Roche respectées pour les anneaux (1,25–2,6 R_p).

### Points faibles ou simplifications à connaître

| Sujet | Problème | Gravité |
|---|---|---|
| **Verrouillage de marée** | Coupure franche à `a < 0,4 × cbrt(M*)` : Mercure (0,39 UA) serait verrouillée chez vous, alors qu'elle est en **résonance spin-orbite 3:2** à cause de son excentricité. Le cas 3:2 est à la fois plus réaliste et plus intéressant en jeu (jours de 176 jours terrestres, le Soleil qui fait des loops). | Faible effort, beau gain |
| **Stabilité des lunes** | L'espacement part de 1,8–2,4 R_p mais rien ne borne l'orbite externe par la **sphère de Hill** de la planète (~1/3 de Hill pour la stabilité prograde). Une géante proche de son étoile avec 4 lunes aura des lunes externes physiquement impossibles. | Réel bug physique |
| **Vents des géantes de glace** | Plafond à 100–150 m/s pour toutes les géantes, mais **Neptune détient le record** (~600 m/s) : les géantes de glace devraient avoir les vents les plus rapides (moins de friction interne). | Détail facile |
| **O₂ abiotique** | 25 % des mondes tempérés avec air reçoivent 21 % d'O₂, indépendamment de la vie. O₂ sans vie existe (photolyse + fuite d'H sur mondes secs de naines M) mais devrait être **corrélé** à la perte d'eau, pas décorrélé de la biologie. Un scanner qui affiche « O₂ 21 %, aucune vie » sans explication est un faux biosignature. | Cohérence interne |
| **Chauffage de marée** | Le vrai scaling est en `a^-7,5 × e²` ; vous utilisez `a^-5 × e`. Votre version « atteint » plus loin — choix de gameplay assumé, mais sachez que c'est généreux. | Acceptable |
| **Saturne** | La branche neptunienne donne ~11,7 R⊕ pour 95 M⊕, soit ~24 % d'écart avec Saturne réelle (9,45, densité exceptionnellement basse) — au-delà des 15 % annoncés du test. C&K est statistique ; Saturne est justement son point faible. | Tolérer ou exclure Saturne du test |
| **Jours des rocheuses** | 10–40 h systématique hors verrouillage : pas de rotateurs lents type Vénus (243 j). Votre tirage d'inclinaison > 90° donne la rétrograde, mais pas la lenteur. | Variété |
| **Métallicité stellaire absente** | C'est le **plus grand manque** : la fréquence des géantes dépend fortement de la métallicité de l'étoile (corrélation observationnelle majeure), et le rapport C/O pilote planètes de carbone vs silicates. Aucun de ces leviers n'existe. | Voir priorités |

Petite incohérence documentaire : le tableau annonce 8–14 R⊕ pour les géantes gazeuses, mais la formule donne ~7 R⊕ à 40 M⊕. Trivial, mais à aligner.

**Score : 8,5/10.** La physique est sérieuse, les tricheries sont étiquetées, les écarts sont des simplifications assumées, pas des erreurs.

---

## 2. Qualité de la génération procédurale

### Ce qui est excellent

- **Sous-graines figées par couche (SplitMix64, entiers)** : ajouter une couche sans invalider l'univers existant, c'est la bonne architecture, et le versionnage via `PROTOCOL` + les tests de reproductibilité montrent une discipline d'ingénierie rare.
- **Cascade causale** étoile→orbite→physique→atmosphère→climat→… : la variété *émergente* (un monde glacé au-delà de la ligne des glaces a des anneaux clairs, de l'inventaire d'eau, éventuellement un océan sous glace) vaut plus que n'importe quel bruit décoratif.
- **Traits dérivés vs tirés** : distinguer « anneaux / synchrone / monde-océan » (conséquences des données) des anomalies tirées évite les contradictions.
- **Même code pour voxel au sol et maillage orbital** : la continuité espace→surface est le meilleur investissement crédibilité possible.
- **Rien n'est stocké**, tout est recalculé depuis le génome (2 nombres) : scalabilité parfaite.

### Limites structurelles

- **Trop de tirages uniformes indépendants.** À l'intérieur d'une couche, les paramètres sont des `u` décorrélés : hauteur des volcans indépendante de leur nombre, pression indépendante de l'âge et de l'histoire d'accrétion. Résultat : des « soupes de dés » statistiquement variées mais sans *histoire*. Une planète ne raconte rien : elle est un point dans un hypercube, pas le résultat d'un scénario (migration, impact géant, désiccation progressive).
- **Pas de couche galactique** : chaque système est i.i.d. Pas de gradient de métallicité, pas de régions, pas de systèmes jeunes vs vieux spatialement regroupés. L'exploration n'a pas de géographie.
- **Composition des géantes fixe** : H₂ 86 % / He 13 % pour toutes — seuls les traces varient. Les vraies géantes ont des métallicités d'enveloppe de 1× à >10× solaire, qui pilotent nuages et couleurs (classes de Sudarsky).
- **Attention au schéma additif des graines** : `base + 500 + i_p×8 + i_lune` est sans collision *aujourd'hui* (planètes ≤ 8, lunes ≤ 4), mais l'invariant « espace lunes < prochain entier réservé » doit être documenté, sinon une future couche le cassera silencieusement.

**Score : 8/10** pour l'ingénierie, **6,5/10** pour la richesse générative.

---

## 3. Risques de répétition

C'est le vrai point sensible. Quantifions :

| Couche | Variété réelle | Risque |
|---|---|---|
| Atmosphères rocheuses | **5 gabarits** (Vénus, CO₂/SO₂ mince, tempérée, Titan, Mars) + géantes à composition fixe | **Élevé** : après ~15 mondes, le joueur a tout vu. Les gaz fictifs (3 %) sont trop rares pour compenser |
| Look des géantes | 6 palettes, dont Jupiter 60 % / Saturne 40 % | **Élevé** : les géantes se ressemblent très vite (bandes 8–16, mêmes ocres) |
| Biomes | 21, mais **extraterrestre = palette-swap** du terrestre (seul critère : O₂ ≥ 5 %, mêmes seuils de température en miroir) | **Élevé** : jungle fongique = jungle recolorée perçue |
| Architecture des systèmes | Orbites quasi circulaires (`0,3u³`), inclinaison ±0,05 rad, espacement ×1,4–2,3, 3–6 planètes favorisées | **Élevé** : chaque système ressemble au Système solaire. Pas de systèmes dynamiquement chauds (excentriques > 0,5, orbites polaires), pas de chaînes résonantes type TRAPPIST — alors que les naines M font 66 % de vos étoiles ! |
| Naines M + mondes habitables | La majorité des mondes « habitables » seront verrouillés sous étoile rouge | Répétition *réaliste* mais réelle : assumez-en la variété (mondes-œil, ceintures de scories, végétation noire) |
| Traits | 98 % ordinaire : légendaire = 1/1000 mondes | Choix défendable (rareté = valeur), mais combiné à 5 atmosphères, le joueur peut explorer 50 mondes sans rien voir de mémorable |
| Relief au sol | Une seule amplitude par planète (`rayon × (0,025 + 0,025u)`), mêmes gammes de tailles de landforms | **Moyen** : la variété perçue vient surtout de la palette de biomes, pas de la grammaire du terrain |

Le calcul brutal : ~4 types × 5 atmosphères × 21 biomes × 6 looks de géante, ça fait un espace de combinaisons honnête, mais le joueur n'en perçoit que les « têtes d'affiche ». **Ce qui sauve la variété, ce sont les croisements émergents** (monde verrouillé + océan sous glace + aurores + geysers). Encore faut-il qu'ils soient *visibles*.

---

## 4. Priorités d'amélioration

### P0 — Fort impact, faible effort (une à quelques journées chacune)

1. **Métallicité stellaire + C/O** comme paramètres système : fréquence des géantes, composition des rocheuses (planètes de carbone, mondes de fer type Mercure déjà amorcé via densité), couleurs. Un seul nouveau scalaire dans le génome, un effet sur tout.
2. **Résonance spin-orbite 3:2** pour les rocheuses proches à excentricité élevée (au lieu du verrouillage binaire) — Mercure devient possible.
3. **Borne de Hill sur les orbites de lunes** — corrige un vrai bug physique.
4. **Cohérence O₂ ↔ vie/eau** : si O₂ > 5 % sans vie, exiger la voie abiotique (monde sec + étoile active) et l'étiqueter dans le scanner.
5. **Vents de géantes de glace portés à ~400–600 m/s**, au-dessus des géantes gazeuses.
6. **Classes de Sudarsky pour le look des géantes** : la température d'équilibre détermine nuages alcalins/silicates/eau → palettes bien plus nombreuses que le split 60/40, *physiquement motivées*.

### P1 — Impact structurel, effort moyen

7. **Archétypes de système** tirés au niveau du génome système, au lieu de planètes i.i.d. : *compact multi résonant* (TRAPPIST, chaîne de Laplace), *géante migratrice* (système saccagé : survivants excentriques, ceinture de débris massive, axes renversés), *dynamiquement chaud* (e jusqu'à 0,7, fortes inclinaisons), *jeune en bombardement* (cratères saturés, comètes nombreuses). Chaque système raconte une histoire → c'est **le** remède anti-répétition.
8. **Photovaporisation des mini-Neptunes proches** : perte d'enveloppe → planètes chthoniennes (cœurs nus, fer exposé), et en bonus vous reproduisez la **vallée des rayons** (Fulton gap à ~1,8 R⊕) — testable avec votre `/stats` contre les populations d'exoplanètes.
9. **Nouveaux gabarits atmosphériques** : mondes-océans à vapeur (runaway), mondes **Hycean** (H₂ + océan — vous avez déjà tous les modules), brume organique type Terre archéenne (ciel orange).
10. **Flore adaptée au spectre stellaire** : végétation noire/sombre sous naines M. Énorme payoff visuel pour 66 % de vos étoiles.
11. **Relief lié à l'âge et au régime d'érosion** : jeunes mondes anguleux, vieux mondes adoucis — un deuxième axe de morphologie au-delà du simple scalaire d'amplitude.

### P2 — Long terme

12. **Couche galactique** : gradient de métallicité et d'âge → l'exploration a une géographie et un but (chercher les systèmes riches du centre, les mondes vierges du bord).
13. **Audit statistique continu** : étendre vos tests existants (excellents) en tests de *population* via `/stats` — occurrence par type, vallée des rayons, distribution des périodes — comparés aux données d'exoplanètes. Vous avez déjà l'outillage, c'est du plus-value quasi gratuit.
14. **« Headline feature » par monde** : choisir automatiquement la propriété la plus rare d'un astre et la mettre en avant (nom, scan, dex). Le joueur perçoit la variété par les exceptions, pas par les distributions.

---

**En une phrase** : la physique est au niveau d'un documentaire, l'ingénierie des graines est exemplaire, mais la génération est « verticalement riche, horizontalement pauvre » — chaque monde est profond, ils se ressemblent trop entre eux. La priorité n'est pas d'ajouter des paramètres, mais des **scénarios** (métallicité, archétypes de système, histoires d'évolution) qui corrèlent les paramètres existants.