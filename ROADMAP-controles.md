# Feuille de route — P. Contrôles (entrées, touches, manette, confort)

Source : bloc **P** de `RAPPORT-ameliorations.md` (idées 207 à 214) + idées 44, 45, 47, 49, 50 du même rapport.
Les phases s'appellent **CTL-n** (le « P » est déjà pris par l'approche planétaire de la 0.13).

Objectif : que **chaque action du jeu** passe par une couche unique « action → touche / bouton », réglable par
le joueur, utilisable au clavier, à la souris et à la manette, avec des réglages séparés pour chaque mode
(vue galaxie, vol, vol bas, marche, éditeur).

---

## 1. Point de départ (code du 05/10/2026)

- Les touches sont **en dur** dans ~125 endroits : `KeyCode::…` partout (`main.rs`, `surface.rs`, `ui.rs`,
  `editeur/view.rs`, `net_ui.rs`…). Les plus utilisées : Échap, Maj, Ctrl, ZQSD (W/A/S/D physiques), flèches,
  Espace, X, F, C, V, P, L, G ; une seule touche chacune pour K (dex), I (scanner), H (amarrage), J (saut
  suborbital), M, N (phares), T, R, E, F1 à F5, F12.
- Seuls réglages d'entrée : `mouse_sensitivity`, `scroll_speed`, `keyboard_speed`, `invert_y`
  (`GameSettings`). Une seule valeur pour tous les modes (`main.rs:1307` et `:1386` multiplient par des
  constantes différentes).
- Clavier **AZERTY** de l'auteur ; le code utilise les touches **physiques** (`KeyCode`) pour ZQSD, bon pour les
  claviers étrangers, mais les noms affichés dans l'aide ne suivent pas la disposition.
- Aucune manette (aucun `Gamepad` dans `src`). Aucun menu de rebind. `net_ui::Field` bloque les touches du jeu
  quand un champ de texte est actif (bon mécanisme à garder).
- La souris : clic gauche / droit / molette selon le mode ; le curseur est capturé en vol bas et à pied
  (`Surface::release_cursor` pour l'éditeur).

## 2. Règles

1. **Aucun `KeyCode` direct** dans la logique du jeu après CTL-1 : on lit une `Action` (`Input<Action>`).
2. Les **réglages d'entrée** sont dans un fichier à part (`saves/controls.json`), pas dans `settings.json` du
   monde : ils suivent le joueur d'un monde à l'autre.
3. Une action peut avoir plusieurs liaisons (touche + bouton de manette) ; un conflit dans le **même contexte**
   est refusé avec un message, dans deux contextes différents il est permis.
4. Rien ne change **par défaut** : les touches actuelles restent la configuration de départ.
5. Les noms de touches affichés (aide, infobulles, tutoriel) viennent de la **liaison réelle**.
6. Toute entrée reste utilisable avec une seule main (option) et sans maintenir de touche (option « basculer »).
7. Sans nouvelle dépendance si possible : Bevy 0.15 fournit `Gamepad` ; une caisse d'actions (`leafwing-input-manager`)
   seulement si la couche maison devient trop lourde (à justifier dans la PR).

## 3. Les phases

| Phase | Contenu | Taille |
|---|---|---|
| **CTL-1. Couche d'actions** | Énumération `Action` (≈ 80 actions) regroupées par **contexte** (`Galaxie`, `Vol`, `VolBas`, `Marche`, `Editeur`, `Menu`, `Chat`) ; ressource `Bindings` (action → liste de `Binding`) ; `ActionState` mis à jour en PreUpdate ; chaque `KeyCode::…` du jeu remplacé par une lecture d'action. Contexte actif = fonction de l'état (`AppState`, `Surface::phase`, champ de texte actif). Aucun changement visible. | L |
| **CTL-2. Menu des touches** | Fenêtre « Contrôles » dans Options : liste par contexte, clic sur une ligne puis touche pour rebinder, bouton « Rétablir », détection des conflits, export / import (`saves/controls.json`), préréglages (AZERTY, QWERTY, ZQSD gauche, **une main**). Noms de touches localisés. | M |
| **CTL-3. Réglages par mode** | Sensibilité, lissage, inversion Y, accélération, zone morte, **séparés** pour : vue galaxie, vol, vol bas, marche, éditeur ; molette : vitesse de zoom et sens ; option « basculer au lieu de maintenir » (Maj, accroupi, phares) ; clic maintenu ou double-clic pour cibler ; délai de double-clic. | M |
| **CTL-4. Manette** | `Gamepad` Bevy : joystick gauche = déplacement, droit = vue, gâchettes = monter / descendre / vitesse, croix = actions rapides, boutons = interagir, scanner, dex. Zones mortes et courbes réglables, **vibration** (rentrée, chocs, séismes) en option, **curseur virtuel** pour les menus (ou navigation par focus), branchement / débranchement à chaud, plusieurs manettes (la première gagne). Détection de la disposition (Xbox / PlayStation / Switch) pour les icônes. | L |
| **CTL-5. Interaction unique et barre rapide** | Touche **« Interagir »** contextuelle (monter dans le vaisseau, ouvrir une porte, ramasser, amarrer H, parler) avec **invite** à l'écran (« [E] Monter ») ; **barre d'actions rapides** 1 à 9 (outils, instruments, modules) configurable, molette pour changer ; invite qui suit la liaison réelle. | M |
| **CTL-6. HUD et aide** | Fenêtre d'aide des raccourcis (liste générée depuis `Bindings`, filtrée par contexte), mémo à l'écran quelques secondes au changement de mode, **historique des messages `NOTIFY`**, **version et build** dans le menu et sur les captures, échelle de l'interface, HUD configurable (masquer des blocs, mode photo sans HUD). | M |
| **CTL-7. Accessibilité des entrées** | Mode **une main** (actions regroupées d'un côté), touches collantes (Maj / Ctrl sans maintenir), répétition réglable, **réduction des secousses** de caméra (séismes, chocs, rentrée), **réduction des flashs** (éclairs, éruptions), taille et contraste du texte, sous-titres des sons (quand D5 / P9 existent). | M |
| **CTL-8. Pause et contrôle du temps (solo)** | `Pause` qui fige l'horloge du monde en solo (`WorldClock`) et la physique, indicateur « horloge x60 » quand `/temps` est actif, pas de pause en multijoueur (message). Raccourci « retour au vaisseau » sûr, annuler une saisie dans le chat. | S |
| **CTL-9. Palette de commandes** | Ctrl+K : recherche floue de **toutes** les actions et commandes de chat (`COMMAND_HELP`), exécution directe, historique ; complète le Tab du chat. | S |
| **CTL-10. Souris avancée** | Pas de capture quand une fenêtre est ouverte, sensibilité à la molette par palier, **clic droit maintenu** pour la caméra libre, mode « clic pour viser » pour tablettes / pavé tactile, pas de dérive à la reprise du curseur après l'éditeur. | S |

Ordre : **CTL-1 → 2 → 3** (sans rien changer pour le joueur qui ne touche à rien) ; **CTL-5 et 6** juste après ;
**CTL-4** (manette) quand les actions sont stables ; CTL-7 à 10 dans n'importe quel ordre.

## 4. Détail de CTL-1 (la phase qui compte)

- `controls.rs` : `enum Context`, `enum Action` (dérive `Hash`, `Serialize`), `Bindings::default()` qui **reproduit
  exactement** les touches actuelles (inventaire à faire avec `grep KeyCode::`).
- Table de migration : `KeyCode::KeyC` dans le code → `Action::Interagir` / `Action::EditeurCouper`… selon
  le fichier ; un test vérifie que **chaque action a au moins une liaison** et qu'**aucun conflit** n'existe
  dans un contexte avec les liaisons par défaut.
- `ActionState` : `pressed`, `just_pressed`, `just_released`, `axis(Action)` (valeur -1..1 : clavier = 0 / ±1,
  manette = analogique) pour que le vol marche avec les deux sans code différent.
- Le contexte `Chat` / champ de texte (`net_ui::Field`) **masque** tous les autres contextes (comme aujourd'hui).
- Les systèmes lisent `Res<ActionState>` ; les tests peuvent **injecter** des actions (utile pour
  `SPACESPORE_TEST_*` et pour les scénarios de `ROADMAP-technique.md` TECH-9).

## 5. Mesures et tests

- Test unitaire : sérialisation / désérialisation de `Bindings`, conflits, préréglages.
- Test de non-régression : une table « ancienne touche → action » vérifiée à la compilation (un `match` exhaustif).
- Capture : fenêtre « Contrôles » (CTL-2) et invite d'interaction (CTL-5), avec le monde et le contexte
  vérifiés (règles de `TESTS-JEU.md`).
- Test manuel manette : 360 / PS5 / Switch Pro, branchement à chaud, déconnexion en plein vol.

## 6. Questions

| # | Question | Proposition |
|---|---|---|
| Q1 | Caisse d'actions ou couche maison ? | Maison (≈ 400 lignes), caisse seulement si les combinaisons (chords) deviennent nécessaires. |
| Q2 | Disposition par défaut pour un clavier non AZERTY ? | Garder les touches physiques ZQSD (= WASD sur QWERTY) ; l'aide affiche le bon nom. |
| Q3 | La manette doit-elle pouvoir tout faire, éditeur compris ? | Jeu complet oui ; éditeur de modèles : non au début (souris obligatoire). |
| Q4 | Pause en multijoueur ? | Non (horloge de l'hôte, `follow_host_clock`). |
| Q5 | Plusieurs profils de contrôles ? | Oui : « profils » nommés (clavier, manette, une main). |

## 7. Prompts (à coller dans une nouvelle session, un par phase)

**CTL-1** : « Lis `CLAUDE.md` et `ROADMAP-controles.md`. Fais l'inventaire de tous les `KeyCode::` et
`MouseButton::`, crée `controls.rs` (`Context`, `Action`, `Bindings`, `ActionState`) avec les liaisons par défaut
identiques aux touches actuelles, remplace chaque lecture directe par une action, ajoute les tests. Comportement
inchangé. PR non fusionnée. »

**CTL-2** : « Lis §3 CTL-2. Ajoute la fenêtre Contrôles (rebind, conflits, préréglages, export / import
`saves/controls.json`). Capture de la fenêtre. »

**CTL-3** : « Sépare les réglages de sensibilité par mode (galaxie, vol, vol bas, marche, éditeur), ajoute lissage,
zone morte, « basculer au lieu de maintenir » ; menu Options. »

**CTL-4** : « Ajoute la manette (`Gamepad` Bevy) : déplacement, vue, gâchettes, croix, vibration, curseur ou focus
pour les menus, icônes par disposition ; tests de branchement à chaud. »

**CTL-5 à CTL-10** : un prompt par phase sur le modèle « Lis `ROADMAP-controles.md` §3 CTL-n, implémente, teste,
capture, PR ».
