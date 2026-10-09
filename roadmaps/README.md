# Feuilles de route

Les documents de suivi du projet, rangés en deux dossiers. Restent à la racine : `CLAUDE.md` (instructions courtes), `ARCHITECTURE.md` (détail technique par système),
`TESTS-JEU.md` (règles de test) et `TOUCHES.md` (touches du jeu).

## `fait/` — terminé

| Fichier | Contenu | Publié |
|---|---|---|
| `ROADMAP-0.10.md` | Génération des étoiles et des planètes (phases 0 à 9) | v0.10.0 |
| `ROADMAP-0.10-sauvegarde.md` | Ancienne version de la feuille de route 0.10 | — |
| `ROADMAP-0.11.md` | Horloge, jour / nuit, saisons, voxels 3D, grottes, ceintures, comètes, météo (le bloc D est passé en 0.14) | v0.11.x |
| `ROADMAP-0.12-editeur.md` | Éditeur de modèles voxel (E0 à E8) | v0.12.0 |
| `ROADMAP-0.12-correctifs.md` | Correctifs C1 à C8 (ex « 0.11.4 correctifs ») | v0.12.0 |
| `RAPPORT-echelle-E1.md` | Étude d'échelle : choix de k = 16 | v0.13 |
| `RAPPORT-generation-terrain.md` | Rapport sur le terrain plat, l'eau, le détail (base des blocs E, T, O de la 0.13) | v0.13 |

## `a-faire/` — en cours ou à venir

| Fichier | Contenu | État |
|---|---|---|
| `ROADMAP-0.13.md` | Mondes : E, T, O, P (approche planétaire, v0.13.4) et le dex **faits** ; C, V1, V2, V4 faits ; reste V3 (vue inclinée) ; L (aliens, terraformation) **déplacé dans la 0.18** | en cours |
| `ROADMAP-0.13.5-correctifs.md` | 20 bugs et petites idées, phases F0 à F10 (releases 0.13.5 à 0.13.7) | à faire (prochaine) |
| `ROADMAP-0.14.md` | Le monde qui vit (bloc D), mondes exceptionnels, événements | à faire |
| `prompt0.14.md` | Banque d'idées de planètes (source de la 0.14) | source |
| `A-FAIRE-PLUS-TARD.md` | Bloc D de la 0.11, musique, pollution | à faire |
| `ROADMAP-0.16.md` | Amélioration des mondes (systèmes, histoire, vie, géantes, cailloux) | à faire |
| `analyse-planetes/` | Audit de la génération, 5 analyses d'IA (source de la 0.16) | source |
| `ROADMAP-0.17-debug-opti.md` | Débogage et optimisation (ex 0.20, renumérotée le 07/10/2026) | à faire |
| `prompt0.17.md` | Questions à ChatGPT sur le benchmark (source de la 0.17) | source |
| `ROADMAP-0.18-capitales.md` | Aliens et terraformation (bloc L venu de la 0.13 : 0.18.1, 0.18.2), puis capitales : une par race extraterrestre sur sa planète natale, une capitale galactique multi-races ; croissance selon progression et ressources ; versions 0.18.1 à 0.18.11 | à faire |
| `RAPPORT-ameliorations.md` | 230 idées d'amélioration (source des feuilles de route thématiques) | source |
| `ROADMAP-technique.md`, `ROADMAP-controles.md`, `ROADMAP-astres.md`, `ROADMAP-mondes-surfaces.md`, `ROADMAP-personnage.md`, `ROADMAP-instruments.md`, `ROADMAP-rendu-ambiance.md`, `ROADMAP-contenu.md` | Feuilles de route thématiques | à faire |
| `IDEES-constructions.md` | Banque d'idées de constructions | idées |

Quand une feuille de route est terminée et publiée, la passer de `a-faire/` à `fait/` (avec `git mv`) et mettre à
jour ce tableau et les chemins dans `ARCHITECTURE.md`.
