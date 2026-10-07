# SpaceSpore - Touches et commandes

Clavier AZERTY (les touches sont lues par position physique : Z = W en QWERTY, Q = A, etc.).
Les touches du jeu sont coupees quand on tape dans un champ de texte (chat, guilde, note du dex) ou quand le menu est ouvert.

## Generales (partout dans le jeu)

| Touche | Action |
|---|---|
| Echap | Ouvrir / fermer le menu (ferme d'abord la fenetre ouverte : multijoueur, guilde, dex, dialogue PNJ) |
| F1 | Vaisseau <-> camera libre (sans vaisseau) |
| F2 | Panneau Multijoueur (liste des joueurs, chat) |
| F3 | Masquer / reafficher le panneau de `/stats` |
| F12 | Activer / couper le journal de profilage des astres (debug) |
| Entree | Ecrire dans le chat (Entree = envoyer, Echap = annuler) |
| Tab (dans le chat) | Completer la commande |
| Fleches haut / bas (dans le chat) | Historique des messages |
| Ctrl + Retour arriere (dans le chat) | Effacer un mot |

## Espace (vue orbitale autour de la cible)

| Touche / souris | Action |
|---|---|
| Clic gauche | Selectionner un astre (etoile, planete, lune, galaxie, trou de ver...) |
| Clic droit maintenu + souris | Tourner la camera |
| Molette | Zoom avant / arriere (zoomer sous 1000 sur une planete ou une lune = vol bas) |
| Z / Q / S / D ou fleches | Pres d'une planete, d'une lune ou d'un asteroide (moins de 8 rayons) : piloter le vaisseau (voir Vol) ; ailleurs : tourner la camera |
| P | Cibler une planete du systeme charge (suivante) |
| M | Cibler la lune suivante |
| T | Entrer dans le trou de ver cible |
| H | S'amarrer au hangar d'un autre joueur (porte-vaisseaux) |
| I | Scanner : panneau de l'astre cible |
| K | Dex des decouvertes |
| E | Parler a la faction visee (dialogue PNJ, commerce, colis) |
| L | Afficher / masquer les liens entre galaxies |
| C | Revendiquer / abandonner une etoile (5 max), ou assieger celle d'un autre |
| G | Panneau Guilde |
| F (maintenu) | Tirer sur le vaisseau neutre ou ennemi le plus proche (multijoueur) |
| N | Lampe / phares |

## Camera libre (F1)

| Touche | Action |
|---|---|
| Clic droit maintenu + souris | Regarder |
| Z / Q / S / D ou fleches | Avancer / gauche / reculer / droite |
| Espace | Monter |
| Maj gauche | Descendre |

## Vol bas (au-dessus d'une planete, d'une lune, d'un asteroide)

| Touche / souris | Action |
|---|---|
| Z / S ou fleches haut / bas | Avancer / reculer |
| Q / D ou fleches gauche / droite | Tourner |
| Espace | Monter |
| Ctrl gauche | Descendre |
| Maj gauche | Turbo (x4) |
| Clic droit maintenu + souris | Regarder |
| Molette | Distance de la camera au vaisseau (en la reculant tout en haut : retour a la vue espace) |
| J | Vol suborbital : saut vers le point vise au centre de l'ecran |
| V | Se poser, sous 200 voxels du sol seulement (pente 25 deg au plus, sinon un point plat est propose) ; plus de descente automatique depuis l'orbite |
| L | Point d'atterrissage plat suivant (cercle vert, liste au scanner) |
| F5 | Vue cockpit (1re personne) <-> de derriere |
| N | Phares |
| I / K / E / F / C / G | Comme dans l'espace (scanner, dex, parler, tirer, revendiquer, guilde) |

## A pied (marche sur une planete, une lune, un asteroide)

| Touche / souris | Action |
|---|---|
| Z / S ou fleches haut / bas | Avancer / reculer |
| Q / D ou fleches gauche / droite | Deplacement lateral |
| Espace | Sauter |
| Maj (gauche ou droite) | Courir |
| Souris | Regarder |
| Molette | Distance de la camera (3 a 12 voxels, en 3e personne) |
| F5 | Vue : 1re personne -> de dos -> de face |
| N | Lampe du marcheur |
| V | Remonter dans le vaisseau (embarquement et decollage) |
| I / K | Scanner / dex |

## Panneaux

| Touche | Action |
|---|---|
| I | Scanner (ouvre / ferme) |
| K | Dex (Echap pour fermer) ; dans le dex : Entree = valider la recherche, Ctrl+C / V = copier / coller |
| F2 | Multijoueur |
| G | Guilde (Echap pour fermer) |
| E | Dialogue PNJ (Echap pour fermer) |

## Editeur de modeles voxel (`/editeur` ou bouton du menu)

### Souris

| Souris | Action |
|---|---|
| Clic gauche | Utiliser l'outil (ajouter, retirer, peindre...) ; tirer pour un trait ou un volume |
| Clic droit maintenu | Tourner la camera |
| Clic droit sur une couleur du modele | Remplacer cette couleur partout |
| Clic milieu maintenu | Deplacer la camera |
| Molette | Zoom ; pendant un trace de volume = epaisseur ; pendant la pose d'un bloc = tourner (Maj + molette = taille) |
| Ctrl + molette | Zoom (meme pendant un trace ou une pose) |

### Outils

| Touche | Outil |
|---|---|
| 1 | Ajouter |
| 2 | Retirer |
| 3 | Peindre |
| 4 | Pipette |
| 5 | Boite |
| 6 | Sphere |
| 7 | Cylindre |
| 8 | Ligne |
| 9 | Pot de peinture (remplissage) |
| 0 | Selection |

### Raccourcis

| Touche | Action |
|---|---|
| Ctrl+Z (ou Ctrl+W) | Annuler |
| Ctrl+Y | Refaire |
| Ctrl+S | Enregistrer |
| Ctrl+C / Ctrl+X / Ctrl+V | Copier / couper / coller la selection |
| Suppr | Supprimer la selection |
| R (Maj+R) | Tourner la selection (sens inverse avec Maj) |
| X | Symetrie miroir on / off (pendant la pose d'un bloc : refleter le bloc) |
| C | Coupe : change d'axe (aucune, y, x, z) |
| Page prec. / Page suiv. | Deplacer la coupe (Maj : de 8) |
| P | Apercu de l'animation (lecture / arret) |
| G | Afficher / masquer la grille |
| F | Cadrer la camera sur le modele |
| L | Lumiere d'atelier <-> lumiere du jeu |
| Fleche gauche / droite | Onglet de modele precedent / suivant |
| Echap | Fermer la fenetre ouverte, sinon retourner au jeu |

## Commandes du chat (Entree puis `/commande`, `/aide` pour la liste complete)

`/aller`, `/tp n`, `/stats [n|tout]`, `/profil`, `/graine`, `/heure`, `/temps <facteur>` (hote), `/eclipse [lune]`,
`/impact`, `/comete`, `/ceinture`, `/grotte`, `/relief [forme]`, `/geologie [type]`, `/surplomb`, `/editeur`,
`/echelle k`, `/g message` (chat de guilde).
