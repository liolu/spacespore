# Regles de test en jeu (captures automatiques)

A lire AVANT tout test visuel. Une phase qui touche au rendu n'est pas finie tant qu'une capture
n'a pas montre ce qu'elle doit montrer. Pas de release sur du code non vu a l'ecran.

## Avant chaque test : reflechir aux commandes (obligatoire)

Ecrire, avant de lancer, un mini plan de 4 lignes :
1. **Quoi voir** : la chose precise a prouver (ex. « surface de l'eau avec vagues, de jour »).
2. **Quel monde** : type + conditions (eau liquide ? air ? rotation ? meteo ?) → quelle commande
   (`/aller planete <type>`, `/essai mer`...). Si aucun type ne convient, **ajouter une commande ou un type
   de test au chat** plutot que de tirer au hasard.
3. **Quelles commandes, dans quel ordre** : `/aller` → attendre l'arrivee → `/jour` → `/vol` → `/mer`...
   Les etapes dependantes passent par `CMD2` (plus tard) ou par une macro comme `/essai`.
4. **Comment verifier** : quels messages `NOTIFY` attendre, quoi lire dans le scanner de la capture.

Si une etape manque d'une commande (heure, meteo, position, vue), la creer d'abord : une commande de
test reutilisable vaut mieux qu'un essai de plus.

## Regles d'or

1. **Verifier le monde AVANT de capturer.** Une capture sur une planete seche, gelee ou de nuit ne
   prouve rien. Lire le scanner de la capture (Eau : « liquide, mers X % », Heure, soleil a N deg)
   avant de conclure. Si ce n'est pas le bon monde, ne pas relancer a l'identique : changer la cause.
2. **Ne jamais relancer 2 fois le meme test sans avoir change quelque chose** (type de planete, heure,
   position, variable). Lire le log (`NOTIFY`) pour savoir pourquoi, pas deviner.
3. **Jour obligatoire pour l'eau, le ciel, les ombres.** Une planete synchrone (« rotation synchrone »)
   garde la meme face au soleil : si c'est la nuit, c'est la nuit pour toujours. Prendre une planete
   qui tourne (Jour = quelques h) ou forcer l'heure avec la variable de test (voir plus bas).
4. **Un shader n'est valide que s'il a ete compile par le GPU** : il faut un objet qui l'utilise a
   l'ecran. Une erreur WGSL n'apparait qu'a ce moment (panic dans le log). Chercher `panic|wgsl|naga|error`.
5. **Dire la verite** : si la capture ne montre pas la chose, ecrire « non valide », pas « ca marche ».

## Lancer le jeu de test (Windows, Git Bash)

- Le jeu ecrit ses sauvegardes a cote de l'exe : **copier l'exe dans un dossier de test** (scratchpad),
  jamais lancer dans `target/release` (saves du joueur). Premier lancement = cree les saves (ouvre
  l'editeur) : lancer une fois a vide (`CAPTURE_SECS=4`), puis les vrais tests.
- Git Bash transforme `/aller` en chemin Windows : **`export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*"`**
  sinon la commande devient du chat et le log dit « Le chat demande le multijoueur ».
- Tuer les anciennes instances avant (`Get-Process spacespore | Stop-Process`), sinon deux jeux ecrivent
  la meme capture / le meme log.
- Build : `CARGO_TARGET_DIR` du depot principal pour reutiliser les dependances ; `cargo build --release`.
- Le jeu ouvre des fenetres chez l'utilisateur : tests courts, capture puis fermeture automatique.

## Variables d'environnement

| Variable | Effet |
|---|---|
| `SPACESPORE_TEST_CMD="/cmd1;/cmd2"` | commandes du chat a 6 s (`..._SECS` pour changer) |
| `SPACESPORE_TEST_CMD2`, `..._CMD2_SECS` | seconde commande (30 s par defaut), apres l'atterrissage |
| `SPACESPORE_TEST_FLY=1`, `..._FLY_SECS=<s>` | descend en vol bas a `<s>` s (8 par defaut) : le mettre APRES l'arrivee du `/aller` |
| `SPACESPORE_TEST_LAND=<s>[,<s>]` | appuie sur V a ces instants |
| `SPACESPORE_TEST_FLY=up\|reentry[:s]\|slow\|climb` | vol automatique (a partir du vol bas, **sans** `/vol` : mettre `SPACESPORE_TEST_FLY_SECS=9999` si on ne veut que le mode) : `up` monte droit, `reentry:8` monte 8 s puis descend plein gaz (rentree, bang, condensation), `slow` descend doucement, `climb` monte en avancant |
| `SPACESPORE_TEST_FLYKEY=<s>` | la touche W est tenue a partir de `<s>` (entree en vol depuis la vue espace) |
| `SPACESPORE_TEST_COCKPIT=1` | vue cockpit en vol (F5) |
| `SPACESPORE_TEST_PRECIP=0` | pas de pluie (captures plus claires) |
| `SPACESPORE_TEST_RIM=1` | journalise le liseré de l'atmosphere (`RIM`) |
| `FLIGHT` / `CLOUD` dans le log | avec `SPACESPORE_TEST_FLY` : etat du vol toutes les 0,5 s (alt, vit, vert, mach, dens, heat) : **s'en servir pour choisir l'instant de la capture** |
| `SPACESPORE_CAPTURE=<fichier.png>`, `..._CAPTURE_SECS=<s>` | capture a `<s>` s puis ferme le jeu |
| `SPACESPORE_PERF=<fichier>` (+ `_FROM`, `_TO`) | images/s |
| `SPACESPORE_TEST_CINE=dig\|ride\|galaxie[:t]` | lance la sequence plein ecran a 3 s, a l'instant `t` (la capture a `<s>` s montre `t + s - 3`) : ne depend pas du monde (le shader recouvre le jeu) |
| `SPACESPORE_QUIT_SECS=<s>` | ferme le jeu a `<s>` s sans capture (verifier seulement le journal : erreurs de shader, `NOTIFY`) |
| `SPACESPORE_TEST_TUNNEL=<u>` | `/tunnel creuser` creuse droit devant sur `<u>` unites, sans cible |
| `SPACESPORE_TEST_PHOTO=<s>` | entre en mode photo a `<s>` s (tout est cache sauf les astres) |
| `NOTIFY` dans le log | tous les messages systeme (actif si `SPACESPORE_TEST_CMD` est defini) |

## Commandes du chat pour les tests

| Commande | Effet |
|---|---|
| `/essai mer` | **tout-en-un pour l'eau** : cherche une planete avec mer qui tourne (`/aller planete mer`), puis `/jour`, `/vol`, `/mer`. Lancer avec `SPACESPORE_TEST_CMD="/essai mer"` et capturer vers 60-80 s |
| `/aller planete mer` | planete avec mer d'eau liquide, air, qui ne tourne pas en synchrone |
| `/jour`, `/nuit`, `/heure <h>` | regle l'heure locale de l'astre cible (hote) ; refuse en rotation synchrone et le dit |
| `/vol` | descend en vol bas sur l'astre cible (remplace `SPACESPORE_TEST_FLY`) |
| `/mer` | rivage de la mer la plus proche (en vol bas ou a pied) |
| `/nuage [dedans\|dessus\|dessous]` | en vol bas : va dans la couche de nuages (ou au-dessus : mer de nuages, ombre du vaisseau) |
| `/brouillard [oui\|non\|auto\|0-100]` | brouillard volumetrique : option, ou densite forcee en % (tests) |
| `/meteo [clair\|auto]` | ciel clair force (ni nuages, ni pluie, ni brouillard) : indispensable pour voir les etoiles ou le ciel |
| `/volume [0-100]` | volume du son ; `/bouclier [oui\|non]` : bouclier thermique (surchauffe puis degats a la rentree) |
| `/aller etoile\|planete\|lune <type>`, `/aller suivant` | cherche un type d'astre et s'y rend (types : `/aller`) |
| `/heure` | heure locale, soleil, saison de l'astre cible |
| `/temps <facteur>` | accelere l'horloge du monde (hote) |
| `/grotte`, `/relief <forme>`, `/geologie <evenement>`, `/surplomb` | va a une grotte / forme du relief / evenement geologique |
| `/ceinture`, `/comete`, `/eclipse [lune]` | va a une ceinture, comete, eclipse |
| `/impact` | meteorite qui s'ecrase pres du joueur |
| `/echelle <k>` | taille des voxels (tests d'echelle) |
| `/tp <n\|type>` | va au trou noir d'une galaxie |
| `/stats`, `/profil`, `/graine` | statistiques, export JSON de l'astre cible, code du monde |
| `/aide [commande]` | liste et aide ; **Tab** complete, fleches = historique |

Commandes d'essai a ajouter quand le besoin apparait (pas encore faites) : `/meteo clair|pluie|orage`
(forcer la meteo), `/cam <vue>` (placer la camera sous l'eau, de loin...).

## Recettes (0.13 P : approche planetaire)

- **Rentree** (plasma, alerte, couloir, bang, condensation) : `SPACESPORE_TEST_CMD="/essai mer"`,
  `SPACESPORE_TEST_FLY=reentry:8`, `SPACESPORE_TEST_FLY_SECS=9999`, `SPACESPORE_TEST_PRECIP=0` ; lire les lignes
  `FLIGHT` : le plasma est au maximum quand `heat` > 0,6 (vers t = vol + 9 a 10 s), le bang quand `mach` passe par 1.
  Une planete dense (> 0,5 bar) est necessaire : `/aller planete mer` la choisit.
- **Entree en vol depuis la vue espace** : `/aller planete mer` + `SPACESPORE_TEST_FLYKEY=25`.
- **Nuages** : `/essai mer` puis `SPACESPORE_TEST_CMD2="/nuage"` (dedans) ou `"/nuage dessus"`.
- **Pente** : `/essai mer`, `CMD2="/relief piton"`, `SPACESPORE_TEST_LAND=<s>` : « Pente de 53 deg : trop raide ».
- **Cockpit** : ajouter `SPACESPORE_TEST_COCKPIT=1`.
- **Brouillard** : `/essai mer`, `CMD2="/brouillard 60"` (jour) ; de nuit : `CMD2="/nuit;/brouillard 60"` (phares dans le brouillard).
- **Ciel etoile au sol** : `/essai mer`, `CMD2="/nuit;/meteo clair"`, `SPACESPORE_TEST_LOOKUP=1` + `SPACESPORE_TEST_COCKPIT=1`
  (la vue de derriere regarde toujours le vaisseau : seul le cockpit regarde en l'air) ; depuis l'espace : `/aller planete mer`, capture a 24 s.
  Le log donne `Ciel : calcul ... pret en N s`.
- **Liseré** : `/aller planete mer`, capture a 20 s (vue espace).
- Le son ne se voit pas : `cargo test --release sound approche` verifie la synthese ; ecouter en vrai.

## Choisir le bon monde

- `/aller planete <type>` : `complexe`, `vie`, `habitable` (eau liquide probable) ; `ocean` (> 85 % de mer)
  n'existe pas toujours dans les 4 000 premiers systemes. `/aller suivant` = le suivant du meme type.
- Verifier dans le scanner de la capture : Eau liquide, Pression > 0, Jour (pas « synchrone » pour le jour).
- `/mer` : va au rivage de la mer la plus proche (en vol bas ou a pied). Demande d'etre deja en vol bas
  (`FLY`) : le mettre en `CMD2` apres `FLY_SECS`.
- Temps : `/temps <facteur>` accelere l'horloge ; `/heure` donne l'heure et la hauteur du soleil.
  Toujours regarder « soleil a N deg » : positif = jour.

## Procedure d'une phase de rendu

1. Choisir le monde (regle 1) et l'heure (regle 3), noter le nom du systeme pour le rejouer.
2. Capture de l'etat « avant » si possible, puis « apres ».
3. Lire la capture ET le log ; si le monde est mauvais, corriger le choix, pas relancer.
4. Plusieurs prises de vue pour une phase : de loin (espace / haut), pres du sol, sous l'eau, de nuit.
5. Mesures (regle 18) : `SPACESPORE_PERF` avant / apres.
6. Seulement apres : tests `cargo test --release`, version, PR / push.

## Pieges deja rencontres

- Capture sur « Sol 1 » (airless) : le `/aller` n'avait rien fait (chemin Windows, ou recherche non finie).
- `FLY` declenche avant la fin du `/aller` : on descend sur le mauvais astre.
- Nuit sur planete synchrone : eau et ciel invisibles.
- Deux instances du jeu : la capture / le log sont ecrases par l'autre.
- Phenomene qui depend de l'horloge (geyser entre deux jets, eruption finie, eclipse passee) : le forcer
  (commande ou variable de test) ou choisir un instant ou il est actif, sinon la capture ne montre rien.
- Objet au centre de l'ecran en 3e personne : le personnage le cache. Se placer de cote (decaler le regard)
  ou passer en 1re personne.
- Particules et maillages dynamiques invisibles : verifier `NoFrustumCulling` (boite englobante calculee une
  seule fois sur des sommets a zero) avant d'accuser le monde.
