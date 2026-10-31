# Crack Attack! pour shellcade

Crack Attack! sur [shellcade](https://github.com/shellcade/kit), l'arcade en
terminal jouée par SSH : un module wasm (`wasm32-wasip1`) qui dessine sur la
grille de 80×24 cellules de l'arcade.

Ce dépôt emporte tout ce dont il a besoin et ne dépend pas du dépôt principal.

| Dossier | Contenu |
|---|---|
| `crackattack-core/` | Le moteur du jeu, **copié sans modification** du dépôt principal (`rust/`, commit `f1ecbc6`). On n'y touche pas : une évolution du moteur se reprend par une nouvelle copie. |
| `src/` | Le jeu shellcade : la salle et l'horloge (`lib.rs`), la partie solo (`solo.rs`), le duel (`duel.rs`), les touches et l'habillage d'un joueur (`seat.rs`), l'affichage compact du terminal porté sur la grille de l'arcade (`render/`), les barres de danger (`danger.rs`). |
| `smoke.yaml` | Les écrans scriptés d'un duel, que la CI du catalogue montre en aperçu ; `smoke-solo.yaml` pour le solo. |

## Solo et duel

Une salle solo lance une partie seule. Une salle à deux places (partie rapide
ou invitation) attend un adversaire sur l'écran du logo ; `S` lance un solo en
attendant.

Le duel reprend le mode VS du moteur (manches, compte à rebours 3-2-1, déchets
envoyés à l'autre, égalités). Les deux parties tournent dans le même module, au
même tick : pas de réseau, donc ni décalage ni désynchronisation. Chacun voit
son plateau à gauche et celui de l'adversaire à droite, aux couleurs
désaturées. Entre deux manches, l'écran du logo donne le résultat et le score ;
la manche suivante part quand les deux ont appuyé sur Entrée. Un joueur qui part
fige le duel : il le reprend en revenant, ou l'autre y met fin d'une touche.

Au classement : le meilleur score d'une manche (le score d'une partie en solo).

## Essayer

```sh
# Une fois : le CLI shellcade-kit (v2.17.1), dans tools/ (ignoré par git).
mkdir -p tools && curl -fsSL -o tools/kit.tar.gz \
  https://github.com/shellcade/kit/releases/download/v2.17.1/shellcade-kit_2.17.1_darwin_arm64.tar.gz \
  && tar -xzf tools/kit.tar.gz -C tools shellcade-kit && rm tools/kit.tar.gz

cargo test                    # les tests, en natif
tools/shellcade-kit play .    # construit le wasm et lance une partie dans ce terminal (S : solo)
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
