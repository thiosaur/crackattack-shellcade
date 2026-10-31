# Crack Attack! pour shellcade

Crack Attack! sur [shellcade](https://github.com/shellcade/kit), l'arcade en
terminal jouée par SSH : un module wasm (`wasm32-wasip1`) qui dessine sur la
grille de 80×24 cellules de l'arcade.

Ce dépôt emporte tout ce dont il a besoin et ne dépend pas du dépôt principal.

| Dossier | Contenu |
|---|---|
| `crackattack-core/` | Le moteur du jeu, **copié sans modification** du dépôt principal (`rust/`, commit `f1ecbc6`). On n'y touche pas : une évolution du moteur se reprend par une nouvelle copie. |
| `src/` | Le jeu shellcade : la boucle (`lib.rs`), l'affichage compact du terminal porté sur la grille de l'arcade (`render/`), les barres de danger (`danger.rs`). |
| `smoke.yaml` | Les écrans scriptés que la CI du catalogue montre en aperçu. |

## Essayer

```sh
# Une fois : le CLI shellcade-kit (v2.17.1), dans tools/ (ignoré par git).
mkdir -p tools && curl -fsSL -o tools/kit.tar.gz \
  https://github.com/shellcade/kit/releases/download/v2.17.1/shellcade-kit_2.17.1_darwin_arm64.tar.gz \
  && tar -xzf tools/kit.tar.gz -C tools shellcade-kit && rm tools/kit.tar.gz

cargo test                    # les tests, en natif
tools/shellcade-kit play .    # construit le wasm et lance une partie dans ce terminal
tools/shellcade-kit check .   # le banc de conformité (latence, mémoire, déterminisme)
tools/shellcade-kit smoke .   # les écrans de smoke.yaml, dans smoke-out/
```

## Commandes

Flèches ou WASD : déplacer le curseur · Espace : échanger · Entrée : faire
monter la pile · P : pause · Échap : quitter (géré par l'arcade).

## Licence

GPL-3.0-or-later, comme le jeu d'origine (Crack Attack! 1.1.14, GPL v2 ou
ultérieure, de Daniel Nelson). Voir `LICENSE`.
