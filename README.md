# Crack Attack! pour shellcade

Ce dépôt est le jeu **Crack Attack!** adapté à [shellcade](https://github.com/shellcade/kit),
l'arcade en terminal à laquelle on joue par SSH. C'est un jeu de blocs à
aligner : on échange deux blocs voisins pour en faire des lignes de trois ou
plus, qui disparaissent, pendant que la pile monte. Les réactions en chaîne
rapportent, et en duel elles envoient des déchets à l'adversaire.

On y joue seul, contre l'ordinateur, ou à deux. Le jeu est compilé en un module
wasm (`wasm32-wasip1`) que l'arcade fait tourner et qui dessine sur sa grille
de 80×24 cellules.

Le dépôt est autonome : il contient le moteur du jeu, repris de la réécriture
en Rust de Crack Attack!, et la couche shellcade autour.

| Dossier | Contenu |
|---|---|
| `crackattack-core/` | Le moteur du jeu : règles, physique des blocs, mode duel, IA. |
| `src/` | Le jeu shellcade : la salle et l'horloge (`lib.rs`), le menu (`menu.rs`), la partie solo (`solo.rs`), le duel (`duel.rs`), les touches d'un joueur (`seat.rs`), l'affichage (`render/`, `danger.rs`). |
| `smoke.yaml`, `smoke-solo.yaml` | Des parties scriptées dont les écrans servent d'aperçu et de contrôle. |

## Les parties

L'arcade propose « Solo » (une place) ou « Duel » (deux places).

- **Salle Solo :** un menu propose une partie seule ou un duel contre
  l'ordinateur, à trois niveaux : Logic (facile), Sigma (moyen), Cortex
  (difficile). Q ramène au menu depuis une partie.
- **Salle Duel :** on attend un adversaire, et le duel commence quand il
  arrive. Une personne de plus dans la salle regarde la partie.

Un duel se joue en manches. Chacun voit son plateau à gauche et celui de
l'autre à droite. Entre deux manches, la suivante part quand les deux joueurs
ont appuyé sur Entrée. Si un joueur s'en va, le duel attend son retour, ou
l'autre y met fin d'une touche.

Au classement compte le meilleur score d'une manche, ou celui de la partie en
solo. Un duel contre l'ordinateur ne compte pas.

## Commandes

Flèches ou WASD : déplacer le curseur · Espace : échanger · Entrée : faire
monter la pile · P : pause (en solo) · Q : retour au menu (salle Solo) ·
Échap : quitter.

## Essayer en local

```sh
# Une fois : le CLI shellcade-kit (v2.17.1), dans tools/ (ignoré par git).
mkdir -p tools && curl -fsSL -o tools/kit.tar.gz \
  https://github.com/shellcade/kit/releases/download/v2.17.1/shellcade-kit_2.17.1_darwin_arm64.tar.gz \
  && tar -xzf tools/kit.tar.gz -C tools shellcade-kit && rm tools/kit.tar.gz

tools/shellcade-kit play . --seats 1   # la salle Solo et son menu
tools/shellcade-kit play .             # une salle Duel ; Ctrl-T passe d'un joueur à l'autre
cargo test                             # les tests, en natif
tools/shellcade-kit check .            # le banc de conformité (latence, mémoire, déterminisme)
tools/shellcade-kit smoke .            # les écrans de smoke.yaml, dans smoke-out/
tools/shellcade-kit smoke . -script smoke-solo.yaml
```

## Licence

GPL-3.0-or-later, comme le jeu d'origine (Crack Attack! 1.1.14, de Daniel
Nelson, GPL v2 ou ultérieure). Voir `LICENSE`.
