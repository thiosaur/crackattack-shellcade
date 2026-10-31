# Crack Attack! for shellcade

This repository is the game **Crack Attack!** for [shellcade](https://github.com/shellcade/kit),
the terminal arcade you play over SSH. It is a block-matching game: you swap
two neighbouring blocks to line up three or more, which then disappear, while
the stack keeps rising. Chain reactions score big, and in a duel they send
garbage to your opponent.

You can play alone, against the computer, or against another player. The game
is compiled to a wasm module (`wasm32-wasip1`) that the arcade runs; it draws
on the arcade's 80×24 cell grid.

The repository is self-contained: it holds the game engine, taken from the Rust
rewrite of Crack Attack!, and the shellcade layer around it.

| Folder | Contents |
|---|---|
| `crackattack-core/` | The game engine: rules, block physics, duel mode, AI. |
| `src/` | The shellcade game: the room and its clock (`lib.rs`), the menu (`menu.rs`), the solo game (`solo.rs`), the duel (`duel.rs`), a player's keys (`seat.rs`), the display (`render/`, `danger.rs`). |
| `smoke.yaml`, `smoke-solo.yaml` | Scripted games whose screens serve as previews and checks. |

## Playing

The arcade offers "Solo" (one seat) or "Duel" (two seats).

- **Solo room:** a menu offers a game on your own or a duel against the
  computer, at three levels: Logic (easy), Sigma (medium), Cortex (hard).
  Q goes back to the menu from a game.
- **Duel room:** you wait for an opponent, and the duel starts when they
  arrive. Anyone else in the room watches.

A duel is played in rounds. Each player sees their own board on the left and
the opponent's on the right. Between rounds, the next one starts once both
players have pressed Enter. If a player leaves, the duel waits for them to come
back, or the other player ends it with a key.

The leaderboard keeps the best score of a round, or of the game in solo. A duel
against the computer does not count.

## Controls

Arrows or WASD: move the cursor · Space: swap · Enter: raise the stack ·
P: pause (solo) · Q: back to the menu (Solo room) · Esc: leave.

## Trying it locally

```sh
# Once: the shellcade-kit CLI (v2.17.1), in tools/ (ignored by git).
mkdir -p tools && curl -fsSL -o tools/kit.tar.gz \
  https://github.com/shellcade/kit/releases/download/v2.17.1/shellcade-kit_2.17.1_darwin_arm64.tar.gz \
  && tar -xzf tools/kit.tar.gz -C tools shellcade-kit && rm tools/kit.tar.gz

tools/shellcade-kit play . --seats 1   # the Solo room and its menu
tools/shellcade-kit play .             # a Duel room; Ctrl-T switches between the players
cargo test                             # the tests, natively
tools/shellcade-kit check .            # the conformance bench (latency, memory, determinism)
tools/shellcade-kit smoke .            # the screens of smoke.yaml, in smoke-out/
tools/shellcade-kit smoke . -script smoke-solo.yaml
```

The download above is the macOS arm64 build; other platforms are on the
[kit's releases page](https://github.com/shellcade/kit/releases/tag/v2.17.1).

## License

GPL-3.0-or-later, like the original game (Crack Attack! 1.1.14, by Daniel
Nelson, GPL v2 or later). See `LICENSE`.
