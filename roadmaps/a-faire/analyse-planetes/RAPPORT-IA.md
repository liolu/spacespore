# Rapport : 5 analyses d'IA de `PLANETES.md`, verifiees contre le code

Fichiers du dossier :

| Fichier | Contenu |
|---|---|
| `PLANETES.md` | la doc analysee (corrigee : voir §4) |
| `ia-chatgpt.md`, `ia-deepseek.md`, `ia-gemini.md`, `ia-qwen.md`, `ia-kimi.md` | les 5 analyses, telles quelles |
| `RAPPORT-IA.md` | ce rapport |

Methode : chaque affirmation des IA a ete comparee au code (`src/planetgen/*.rs`, `planet.rs`, `mesher.rs`), et les
questions de **frequence** ont ete **mesurees** sur les 20 000 premiers systemes du monde par defaut (test temporaire
`audit_ia_tmp`, `cargo test --release`, 1,6 s ; retire ensuite). Les IA n'avaient que la doc ; elles devinaient les
frequences. Les mesures disent qui avait raison.

---

## 1. Classement des IA

| Rang | IA | Note | En bref |
|---|---|---|---|
| **1** | **Kimi** | **9/10** | La plus precise. Toutes ses affirmations chiffrees sont vraies, elle a trouve 2 erreurs de la doc et 5 vrais ecarts physiques. Ses priorites sont concretes. |
| 2 | DeepSeek | 8/10 | Courte et pointue. A vu le probleme le plus grave confirme par les mesures (monde moyen = caillou nu) et le Venus impossible a sa vraie place. Une erreur. |
| 3 | ChatGPT | 7/10 | La meilleure vision d'ensemble (audit de diversite, histoire de la planete), mais lit mal deux mecanismes et propose des choses deja faites. Tres long. |
| 4 | Qwen | 6/10 | Tres complet mais generique : ~1 800 lignes, beaucoup de suggestions deja dans le jeu (meteo, eclairs, givre, aurores...). Bonnes idees physiques (photoevaporation, effondrement du CO2). |
| 5 | Gemini | 5,5/10 | Correcte mais superficielle. Un seul apport original et juste : le gameplay pauvre des geantes gazeuses. |

**La meilleure IA : Kimi.** C'est la seule qui a refait les calculs au lieu de commenter le texte : elle a vu que
la formule donne 7 R_terre (et non 8) a 40 M_terre, que Saturne est a +24 % (et non dans les 15 % du test), que Mercure
serait synchrone avec la regle du jeu, que les marees reelles vont en a^-7,5 x e² (le jeu : a^-5 x e), que les geantes
de glace ont les vents les plus forts (Neptune ~600 m/s, le jeu : 100-150 pour toutes). Tout est vrai.
**DeepSeek** est la meilleure sur le rapport qualite / longueur : 85 lignes, et elle a trouve le probleme n° 1.

---

## 2. Ce que disent les mesures (20 000 systemes, 89 429 planetes, 97 664 lunes)

| Mesure | Valeur | Ce que la doc laissait croire |
|---|---|---|
| Rocheuses / mini-Neptunes / geantes de glace / gazeuses | 69 % / 15 % / 7 % / 8 % | - |
| **Rocheuses sans air** | **70,6 %** | 25 % (le tirage) ; la fuite de Jeans et les rayons X des naines M font le reste |
| **Rocheuses = caillou nu** (sans air, sans mer, sans vie) | **68,6 %** | - |
| **Lunes = caillou nu** | **91,8 %** | - |
| Lunes avec de l'air | 2,5 % | - |
| **Rocheuses synchrones** (une face au jour) | **76,7 %** | - |
| Rocheuses avec eau liquide en surface | 1,5 % (dont 65 % synchrones) | - |
| Rocheuses « habitables » (score > 0,6) | **1** sur 61 813 | - |
| Rocheuses avec O2 > 5 % | 3,5 % | - |
| **... dont sans aucune vie** | **94 %** (2 028 sur 2 152) | - |
| ... dont avec des plantes | 1,7 % (36) | - |
| Planetes a plantes avec biomes extraterrestres (sans O2) | 82 % (167 sur 203) | - |
| Vie sur les rocheuses (microbes / simple / complexe) | 1,8 % / 0,2 % / 0,1 % | - |
| **Lunes avec de la vie** | **5,7 %** (plus que les planetes !) | - |
| **Lunes a ocean sous la glace** | **19,4 %** | - |
| Mondes « Venus » (> 30 bar, CO2 > 90 %) | 3,2 % des rocheuses | - |
| ... distance / zone habitable | 0,08 a **0,41** (mediane 0,27) | la vraie Venus est a 0,72 |
| Traits tires (peu commun / rare / legendaire) | 1,45 % / 0,35 % / 0,10 % | conforme |
| Planetes errantes | 3,4 % des systemes | conforme (1/30) |
| Rayon des rocheuses | 0,34 a **1,78** R_terre | la doc disait 2,4 (erreur, corrigee) |
| Rayon des geantes gazeuses | **7,0** a 16,8 R_terre | la doc disait 8 a 14 (erreur, corrigee) |

Rappel : la memoire du projet note que le realisme a ete accepte (mondes temperes rares, roadmap 0.10). Les chiffres
ci-dessus montrent **jusqu'ou** ce choix va : 1 monde habitable pour 20 000 systemes, et 83 % des astres solides
(planetes + lunes) sont des cailloux nus.

---

## 3. Verification des affirmations, IA par IA

Legende : ✅ vrai (verifie) · ⚠️ en partie vrai / exagere · ❌ faux · 🔁 deja dans le jeu

### Kimi

| Affirmation | Verdict | Preuve |
|---|---|---|
| 4 000 M_terre ≈ 12,6 M_Jupiter, juste sous la fusion du deuterium | ✅ | 4000/318 = 12,6 |
| Mercure (0,39 UA) serait synchrone ; devrait etre en resonance 3:2 | ✅ | `system.rs:222` : `a < 0,4 x cbrt(M)` = 0,4 UA |
| Pas de borne de Hill sur les lunes | ✅ | `system.rs:258` : espacement en rayons de planete seulement |
| Vents : 100-150 m/s pour toutes les geantes, Neptune = record | ✅ | `atmosphere.rs:403` |
| O2 a 21 % tire sans lien avec la vie | ✅ | `atmosphere.rs:285` ; mesure : 94 % des mondes a O2 sans vie |
| Marees reelles en a^-7,5 x e², jeu en a^-5 x e | ✅ | `system.rs:500` |
| Saturne a +24 %, pas dans les 15 % du test | ✅ | 11,7 calcule vs 9,45 ; le test verifie seulement `> 8` |
| Jours 10-40 h, aucun rotateur lent type Venus | ✅ | `system.rs:228` |
| Pas de metallicite stellaire | ✅ | aucune trace dans le code |
| La doc dit 8-14 R_terre, la formule donne ~7 a 40 M_terre | ✅ | mesure : 7,0 |
| Composition des geantes fixe (H2 86 %, He 13 %) | ✅ | `atmosphere.rs:263` |
| Biomes extraterrestres = memes seuils que les terrestres | ✅ | `biome.rs:391-417` |
| Orbites presque circulaires, inclinaison ±0,05 | ✅ | `system.rs:376-377` |
| Graines des lunes `+500 + i*8 + j` : invariant a documenter | ✅ | au plus 4 lunes, 9 planetes : pas de collision aujourd'hui |
| Anneaux dans la limite de Roche | ⚠️ | bord externe jusqu'a 2,6 rayons, Roche ~2,44 |

Manque : n'a pas vu que l'air disparait sur 71 % des rocheuses (a sous-estime la monotonie des cailloux).

### DeepSeek

| Affirmation | Verdict | Preuve |
|---|---|---|
| A 0,72 UA d'une etoile comme le Soleil, T_eq ≈ 300 K : jamais de Venus la | ✅ | mesure : aucun Venus au-dela de 0,41 x zone habitable |
| « Le label Venus n'est jamais produit pour de vrai » | ⚠️ | 1 948 Venus existent, mais tous tres pres de l'etoile ; pas d'emballement d'un monde tempere (vapeur plafonnee a 4 %) |
| **« Le monde median du jeu est un caillou gris »** | ✅✅ | mesure : 69 % des rocheuses et 92 % des lunes |
| Lunes petites -> pas de tectonique -> steriles | ✅ | plaques si masse > 0,3 ; 92 % de lunes nues |
| Naines rouges partout -> beaucoup de mondes synchrones | ✅ | mesure : 77 % |
| Aucune correlation a l'echelle du systeme | ✅ | un `roll` independant par planete |
| Traits trop rares pour guider l'exploration | ✅ | 2 % des planetes |
| 6 looks de geantes | ✅ | `planet.rs::gas_look` |
| Atmospheres : 5 gabarits | ✅ | `atmosphere.rs:261-295` |
| « Biome choisi selon une moyenne, risque de biome uniforme » | ❌ | le biome est calcule colonne par colonne (`BiomeField::biome(dir)`) |
| Pas de planete naine | ⚠️ | les rocheuses commencent a 0,34 R_terre, mais les lunes descendent a 0,08 |

### ChatGPT

| Affirmation | Verdict | Preuve |
|---|---|---|
| O2 21 % trop frequent, sans lien avec la vie | ✅ | mesure : 94 % sans vie |
| Contradiction « il faut un liquide » vs microbes 3 % / 1 % sans liquide | ✅ | `life.rs:96-99` |
| « O2 -> vegetation -> animaux » | ❌ | les plantes viennent de `life.rs` (vie simple), pas de l'O2 ; l'O2 ne change que la palette terrestre / extraterrestre |
| Petites lunes devraient perdre air et activite | 🔁 | deja le cas : 97,5 % des lunes sans air (masse < 0,02, Jeans) |
| Risque d'archetypes repetes | ✅ | confirme par les mesures |
| Faire de `/stats` un outil d'audit de diversite | ✅ | tres bonne idee (le test de ce rapport en est un debut) |
| « Histoire planetaire » (ocean ancien, bombardement...) | ✅ idee | aujourd'hui seul `salt` (mer evaporee) existe |
| Notes chiffrees (8/10, 9/10...) | ⚠️ | non argumentees |

### Qwen

| Affirmation | Verdict | Preuve |
|---|---|---|
| O2 sans cause | ✅ | idem |
| Chevauchement rocheuse / mini-Neptune 1,5-2,4 R_terre | ⚠️ | base sur l'erreur de la doc : en vrai 1,5-1,78 |
| Fuite de Jeans seule, pas de photoevaporation | ✅ | `atmosphere.rs:162` |
| Pas d'effondrement du CO2 sur les mondes froids | ✅ | seulement du givre visuel (`co2_frost`) |
| Pas d'emballement de serre | ✅ | vapeur plafonnee a 4 % |
| Microbes dans l'air sans liquide | ✅ | `life.rs:96` |
| Champ magnetique surtout cosmetique | ⚠️ | il reduit la radiation et fait les aurores, mais ne protege pas l'atmosphere |
| Climat en bandes trop regulieres | ⚠️ | `belt_humidity` + bruit ; pas d'ombre pluviometrique |
| Ajouter meteo vivante, eclairs, givre, aurores animees, lunes dans le ciel, saisons, geysers | 🔁 | deja la : `weather.rs`, `sky.rs`, `geoactive.rs`, givre du matin, eclipses |

### Gemini

| Affirmation | Verdict | Preuve |
|---|---|---|
| Atmosphere calculee avant la geologie : pas de degazage volcanique | ✅ | `system.rs:507` puis `530` (seule IA a le dire) |
| O2 avant la vie | ✅ | idem |
| Geantes : meme experience (brouillard, degats) | ✅ | `gas.rs` ; idee de paliers d'altitude interessante |
| Seulement 3 biomes extraterrestres | ✅ | + verre et soufre |
| Diagrammes de phase reels pour methane, ammoniac, lave | ⚠️ | seulement des points de gel / ebullition |
| Analyse sans chiffres ni code | - | la plus courte et la moins precise |

---

## 4. Erreurs de `PLANETES.md` trouvees (corrigees dans le dossier)

| Erreur | Correction | Trouvee par |
|---|---|---|
| Rocheuses « 0,34 a ~2,4 R_terre » | **0,34 a 1,78** (8 M_terre ^ 0,279) | mesure (Qwen s'en est servi a tort) |
| Geantes gazeuses « 8 a 14 R_terre » | **7 a 14** (17 pour un Jupiter chaud) | **Kimi** |
| « Saturne a 15 % pres » | Saturne verifiee seulement > 8 ; +24 % en vrai | **Kimi** |
| « 25 % sans atmosphere » | 25 % tires, **71 % au final** | mesure |

---

## 5. Ce qu'il faut ameliorer (synthese, par priorite)

Toute modification de la generation = augmenter `PROTOCOL` (`net.rs`) et supprimer `saves/settings.json` et `saves/astres.json`.

### P0 - Decisions a prendre (gros effet sur ce que voit le joueur)

1. **Le monde moyen est un caillou nu** (69 % des rocheuses, 92 % des lunes). *(DeepSeek, mesure)*
   Choix : soit on garde le realisme et on **diversifie les cailloux** (couleur du regolithe selon la composition et
   l'age, soufre, verre d'impact, fer expose, dunes, glace : idee Qwen / Gemini), soit on **reduit la perte d'air**
   (le tirage de 25 % s'ajoute a la fuite de Jeans, qui est deja forte autour des naines M).
2. **O2 sans vie dans 94 % des cas.** *(les 5 IA)* Decider l'O2 **apres** la vie : O2 eleve = vie simple+ avec
   photosynthese ; O2 abiotique (photolyse d'un monde dessèche autour d'une etoile active) rare et etiquete au scanner.
   Probleme d'ordre : l'O2 est tire dans `atmosphere.rs` (couche 5), la vie vient apres (couche 10).
3. **77 % des rocheuses sont synchrones.** *(DeepSeek, Kimi)* Ajouter la resonance 3:2 pour les orbites excentriques
   (Mercure) et faire dependre le verrouillage de l'age et de la masse, pas d'une seule distance.

### P1 - Physique (petits changements, coherence)

4. **Emballement de serre** : basculer un monde tempere en Venus d'apres le flux recu (et l'eau), pas seulement
   `T_eq > 400 K`. *(DeepSeek, Qwen)*
5. **Degazage volcanique** apres la geologie : volcanisme fort -> plus de CO2 / SO2 / pression. *(Gemini)*
6. **Vents des geantes de glace** a 400-600 m/s. *(Kimi)*
7. **Borne de Hill** pour les lunes des planetes proches de leur etoile. *(Kimi)*
8. **Effondrement du CO2** sur les mondes froids (calottes, pression qui baisse). *(Qwen)*
9. **Photoevaporation** des mini-Neptunes proches -> coeurs nus, « vallee des rayons ». *(Qwen, Kimi)*
10. Verifier si **19 % de lunes a ocean sous la glace** est voulu : seuil de maree 0,05 (= Europe) assez genereux, et
    c'est ce qui donne plus de vie aux lunes qu'aux planetes. *(mesure, aucune IA ne l'a vu)*
11. Microbes « sans liquide » : les conditionner (brumes organiques) ou les etiqueter speculatifs. *(ChatGPT, Qwen)*

### P2 - Diversite (plus de travail, plus de variete)

12. **Metallicite de l'etoile** dans le genome : frequence des geantes, planetes de fer / carbone. *(Kimi)*
13. **Archetypes de systeme** (chaine resonante type TRAPPIST, geante migratrice, systeme excentrique, jeune
    bombarde) au lieu de planetes independantes. *(Kimi, DeepSeek)*
14. **Geantes** : classes de Sudarsky (palette d'apres la temperature), grandes taches, composition variable ;
    gameplay par paliers d'altitude. *(Kimi, Qwen, DeepSeek, Gemini)*
15. **Flore adaptee a l'etoile** (vegetation sombre sous une naine M : 66 % des etoiles). *(Kimi)*
16. **Traits mineurs frequents** (curiosites locales) sous les traits rares actuels. *(Qwen, DeepSeek)*
17. **Histoire de la planete** (ocean ancien, glaciation, impact geant) qui explique relief, sel, minerais. *(ChatGPT, Qwen)*

### P3 - Outil

18. **Audit de population permanent** : garder le test de ce rapport comme `#[ignore]` (ou `/stats diversite`) pour
    suivre ces pourcentages a chaque changement. *(ChatGPT, DeepSeek, Kimi)*

---

## 6. En une phrase

Les 5 IA s'accordent sur l'essentiel (architecture excellente, O2 sans cause, risque de repetition) ; **Kimi** est la
plus juste et la plus precise, **DeepSeek** a vu le vrai probleme n° 1, et les mesures montrent qu'il est plus grave
que toutes ne le pensaient : **le joueur verra surtout des cailloux nus, synchrones, autour d'une naine rouge**.
