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
| `SPACESPORE_CAPTURE=<fichier.png>`, `..._CAPTURE_SECS=<s>` | capture a `<s>` s puis ferme le jeu |
| `SPACESPORE_PERF=<fichier>` (+ `_FROM`, `_TO`) | images/s |
| `NOTIFY` dans le log | tous les messages systeme (actif si `SPACESPORE_TEST_CMD` est defini) |

## Commandes du chat pour les tests

| Commande | Effet |
|---|---|
| `/essai mer` | **tout-en-un pour l'eau** : cherche une planete avec mer qui tourne (`/aller planete mer`), puis `/jour`, `/vol`, `/mer`. Lancer avec `SPACESPORE_TEST_CMD="/essai mer"` et capturer vers 60-80 s |
| `/aller planete mer` | planete avec mer d'eau liquide, air, qui ne tourne pas en synchrone |
| `/jour`, `/nuit`, `/heure <h>` | regle l'heure locale de l'astre cible (hote) ; refuse en rotation synchrone et le dit |
| `/vol` | descend en vol bas sur l'astre cible (remplace `SPACESPORE_TEST_FLY`) |
| `/mer` | rivage de la mer la plus proche (en vol bas ou a pied) |
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
