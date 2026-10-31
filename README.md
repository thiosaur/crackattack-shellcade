# Crack Attack! pour shellcade

Crack Attack! sur [shellcade](https://github.com/shellcade/kit), l'arcade en
terminal jouée par SSH : un module wasm (`wasm32-wasip1`) qui dessine sur la
grille de 80×24 cellules de l'arcade.

Ce dépôt emporte tout ce dont il a besoin et ne dépend pas du dépôt principal.

| Dossier | Contenu |
|---|---|
| `crackattack-core/` | Le moteur du jeu, **copié sans modification** du dépôt principal (`rust/`, commit `f1ecbc6`). On n'y touche pas : une évolution du moteur se reprend par une nouvelle copie. |
| `src/` | Le jeu shellcade : la salle et l'horloge (`lib.rs`), le menu (`menu.rs`), la partie solo (`solo.rs`), le duel, contre un joueur ou l'ordinateur (`duel.rs`), les touches et l'habillage d'un joueur (`seat.rs`), l'affichage compact du terminal porté sur la grille de l'arcade (`render/`), les barres de danger (`danger.rs`). |
| `smoke.yaml` | Les écrans scriptés d'un duel, que la CI du catalogue montre en aperçu ; `smoke-solo.yaml` pour le solo. |

## Solo, contre l'ordinateur, duel

Le choix entre seul et à deux se fait dans l'arcade : elle propose « Solo »
(une place) et « Duel » (partie rapide, ou salle privée sur invitation).

Dans une salle Solo, un menu sous le logo propose le solo ou un duel contre
l'ordinateur (l'IA du moteur : Logic, facile, Sigma, moyen, ou Cortex,
difficile). Dans une salle Duel, le logo n'annonce que l'attente d'un adversaire :
le duel commence quand il arrive. Un troisième venu regarde.

En local, `tools/shellcade-kit play .` ouvre une salle à deux places (l'attente) ;
`--seats 1` ouvre la salle Solo et son menu.

Cortex est ici une version allégée de celui du dépôt principal : sans le jeu
pendant les chutes, sans les chaînes préparées en trois coups ni les combos
x8/x10. Il est aussi fort en duel (51,8 % sur 500 parties contre le Cortex
complet) et calcule bien moins (p99 de 106 à 44 µs par décision, pire tick
0,43 ms en natif). Le Cortex complet dépassait parfois les 100 ms qu'un appel a
le droit de prendre en wasm, et l'arcade coupait la salle ; l'allégé passe les
mêmes essais. Le réglage est dans `crackattack-core/src/vs/grid_ai/tuning.rs`
(journal S46 à S49 du dépôt principal).

Le duel reprend le mode VS du moteur (manches, compte à rebours 3-2-1, déchets
envoyés à l'autre, égalités). Les deux parties tournent dans le même module, au
même tick : pas de réseau, donc ni décalage ni désynchronisation. Chacun voit
son plateau à gauche et celui de l'adversaire à droite, aux couleurs
désaturées. Entre deux manches, l'écran du logo donne le résultat et le score ;
la manche suivante part quand les deux ont appuyé sur Entrée. Un joueur qui part
fige le duel : il le reprend en revenant, ou l'autre y met fin d'une touche.

Au classement : le meilleur score d'une manche (le score d'une partie en solo).
Une partie contre l'ordinateur ne compte pas.

## Essayer

```sh
# Une fois : le CLI shellcade-kit (v2.17.1), dans tools/ (ignoré par git).
mkdir -p tools && curl -fsSL -o tools/kit.tar.gz \
  https://github.com/shellcade/kit/releases/download/v2.17.1/shellcade-kit_2.17.1_darwin_arm64.tar.gz \
  && tar -xzf tools/kit.tar.gz -C tools shellcade-kit && rm tools/kit.tar.gz

cargo test                    # les tests, en natif
tools/shellcade-kit play .    # construit le wasm et lance le menu dans ce terminal
tools/shellcade-kit play . --seats 2   # un duel ; Ctrl-T passe d'un joueur à l'autre
tools/shellcade-kit check .   # le banc de conformité (latence, mémoire, déterminisme)
tools/shellcade-kit smoke .   # les écrans de smoke.yaml, dans smoke-out/
tools/shellcade-kit smoke . -script smoke-solo.yaml
```

## Commandes

Flèches ou WASD : déplacer le curseur · Espace : échanger · Entrée : faire
monter la pile · P : pause (en solo) · Échap : quitter (géré par l'arcade).

## Licence

GPL-3.0-or-later, comme le jeu d'origine (Crack Attack! 1.1.14, GPL v2 ou
ultérieure, de Daniel Nelson). Voir `LICENSE`.
