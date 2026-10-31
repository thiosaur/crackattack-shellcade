// Crack Attack! for shellcade: one player, the game of the terminal version on the
// arcade's 80×24 canvas.
//
//   cargo test                           # natively
//   tools/shellcade-kit play .           # build the wasm artifact and play it
//   tools/shellcade-kit check .          # the conformance harness
//
// The engine (`crackattack-core`) steps 50 times per second. Its ticks follow the room
// clock (`now_unix_nanos`), never the number of wakes: the heartbeat may jitter.
#![forbid(unsafe_code)]

mod danger;
mod render;

use crackattack_core::gameplay_event::GameplayEvent;
use crackattack_core::input::{INPUT_ADVANCE, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_SWAP, INPUT_UP};
use crackattack_core::{FrameData, GameState, InputSnapshot, STEPS_PER_SECOND};
use shellcade_kit::prelude::*;

/// One engine tick, in nanoseconds of room time.
const TICK_NANOS: i64 = 1_000_000_000 / STEPS_PER_SECOND as i64;
/// Most ticks caught up in one wake: after a longer stall, the game slows down rather
/// than jump ahead (and stays inside the callback deadline).
const MAX_CATCH_UP: i64 = 10;
/// How long the chain just reached stays in the side panel (ticks).
const CHAIN_SHOW_TICKS: i32 = 75;

struct CrackAttackGame;

impl Game for CrackAttackGame {
    fn meta(&self) -> Meta {
        Meta {
            slug: "crackattack",
            name: "Crack Attack!",
            short_description: "Swap blocks, line up three, chain the combos before the stack tops out.",
            min_players: 1,
            max_players: 1,
            tags: &["puzzle", "action"],
            leaderboard: Some(Leaderboard {
                metric_label: "score",
                direction: Direction::HigherBetter,
                aggregation: Aggregation::BestResult,
                format: MetricFormat::Integer,
            }),
            // The engine's own rate: a wake per tick when the host allows it.
            heartbeat_ms: (1000 / STEPS_PER_SECOND) as u16,
            ..Meta::DEFAULT
        }
    }

    fn new_room(&self, _cfg: &RoomConfig) -> Box<dyn Handler> {
        Box::new(CrackAttack::new())
    }
}

/// One room: one game. All its state lives here (the host may snapshot the room).
struct CrackAttack {
    canvas: Frame,
    /// The screen last sent: most ticks change nothing visible, and are not sent.
    shown: Vec<Cell>,
    player: Option<Player>,
    /// Created when the player joins, from the room's seed.
    game: Option<GameState>,
    last: FrameData,
    /// Room time of tick 0, and ticks run since (game, then game-over animation).
    origin: i64,
    ticks: i64,
    paused: bool,
    /// Keys pressed since the last tick (`INPUT_*` bits), and those of the last tick,
    /// for the engine's edge detection.
    keys: u8,
    prev_state: u8,
    danger: danger::DangerAnimator,
    /// Chain just reached, and ticks left to show it.
    chain: Option<(i32, i32)>,
    hud: render::Hud,
    ended: bool,
}

impl CrackAttack {
    fn new() -> Self {
        Self {
            canvas: Frame::new(),
            shown: Vec::new(),
            player: None,
            game: None,
            last: FrameData::default(),
            origin: 0,
            ticks: 0,
            paused: false,
            keys: 0,
            prev_state: 0,
            danger: danger::DangerAnimator::new(),
            chain: None,
            hud: render::Hud::default(),
            ended: false,
        }
    }

    /// Runs the ticks due by room time `now`.
    fn catch_up(&mut self, now: i64) {
        if self.paused || self.ended {
            // Time stands still: the next ticks start from now.
            self.origin = now - self.ticks * TICK_NANOS;
            return;
        }
        let due = (now - self.origin) / TICK_NANOS - self.ticks;
        if due > MAX_CATCH_UP {
            self.origin += (due - MAX_CATCH_UP) * TICK_NANOS;
        }
        for _ in 0..due.clamp(0, MAX_CATCH_UP) {
            self.tick();
        }
    }

    /// One tick: the game, or once it is lost, the GAME OVER animation.
    fn tick(&mut self) {
        self.ticks += 1;
        if self.last.game_over {
            if !self.game_over_shown() {
                self.hud.game_over = Some(self.hud.game_over.map_or(0, |t| t + 1));
            }
            return;
        }
        let Some(game) = &mut self.game else { return };
        let mut input = InputSnapshot::new();
        input.state = std::mem::take(&mut self.keys);
        input.pressed_this_frame = input.state & !self.prev_state;
        input.prev_state = self.prev_state;
        self.prev_state = input.state;

        self.last = game.step(&input);
        self.danger.step(&self.last.ui);
        self.chain = self.chain.map(|(n, left)| (n, left - 1)).filter(|&(_, left)| left > 0);
        for event in &self.last.gameplay_events {
            if let GameplayEvent::ChainProgressed(e) = event {
                self.chain = Some((e.chain_level, CHAIN_SHOW_TICKS));
            }
        }
        self.hud = render::Hud { danger: self.danger.bars(), chain: self.chain.map(|(n, _)| n), game_over: None };
    }

    fn game_over_shown(&self) -> bool {
        self.hud.game_over.is_some_and(|t| t >= render::game_over_ticks())
    }

    /// Ends the room with the score.
    fn finish(&mut self, r: &mut Room) {
        if self.ended {
            return;
        }
        self.ended = true;
        let rankings = self.player.iter().map(|player| PlayerResult {
            player: player.clone(),
            metric: self.last.score as i64,
            rank: 1,
            status: Status::Finished,
        });
        r.end(&Outcome { rankings: rankings.collect() });
    }

    fn render(&mut self, r: &mut Room) {
        if self.game.is_none() {
            return;
        }
        render::compose(&mut self.canvas, &self.last, self.paused, &self.hud);
        let cells: Vec<Cell> = (0..ROWS).flat_map(|row| (0..COLS).map(move |col| (row, col)))
            .map(|(row, col)| self.canvas.get(row, col))
            .collect();
        let same = |a: &Cell, b: &Cell| {
            (a.rune, a.cp2, a.cp3, a.fg, a.bg, a.attr, a.cont) == (b.rune, b.cp2, b.cp3, b.fg, b.bg, b.attr, b.cont)
        };
        if cells.len() == self.shown.len() && cells.iter().zip(&self.shown).all(|(a, b)| same(a, b)) {
            return;
        }
        r.identical(&self.canvas);
        self.shown = cells;
    }
}

/// The engine key for an input, `INPUT_*` bits (0 for none). Letters work in either case.
fn map_input(input: Input) -> u8 {
    match input {
        Input::Key(Key::Up) => INPUT_UP,
        Input::Key(Key::Down) => INPUT_DOWN,
        Input::Key(Key::Left) => INPUT_LEFT,
        Input::Key(Key::Right) => INPUT_RIGHT,
        Input::Key(Key::Enter) => INPUT_ADVANCE,
        Input::Char(c) => match c.to_ascii_lowercase() {
            'w' => INPUT_UP,
            's' => INPUT_DOWN,
            'a' => INPUT_LEFT,
            'd' => INPUT_RIGHT,
            ' ' => INPUT_SWAP,
            _ => 0,
        },
        _ => 0,
    }
}

impl Handler for CrackAttack {
    fn on_start(&mut self, _r: &mut Room) {}

    fn on_join(&mut self, r: &mut Room, p: Player) {
        if self.game.is_none() {
            self.game = Some(GameState::new(r.rng_u64() as u32));
            self.origin = r.now_unix_nanos();
            self.player = Some(p);
            // The first frame, before the first tick.
            self.tick();
        }
        // A new connection (or a revived room) gets the whole screen.
        self.shown.clear();
        self.render(r);
    }

    fn on_leave(&mut self, r: &mut Room, _p: Player) {
        self.finish(r);
    }

    fn on_input(&mut self, r: &mut Room, _p: Player, input: Input) {
        self.catch_up(r.now_unix_nanos());
        if self.last.game_over {
            // Keys pressed during the animation do not count.
            if self.game_over_shown() {
                self.finish(r);
            }
            return;
        }
        if matches!(input, Input::Char('p' | 'P')) {
            self.paused = !self.paused;
            // The keys pressed during the pause must not count as held after it.
            self.keys = 0;
            self.prev_state = 0;
        } else if !self.paused {
            self.keys |= map_input(input);
        }
        self.render(r);
    }

    fn on_wake(&mut self, r: &mut Room) {
        let ticks = self.ticks;
        self.catch_up(r.now_unix_nanos());
        if self.ticks != ticks {
            self.render(r);
        }
    }
}

shellcade_kit::shellcade_game!(CrackAttackGame);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_follow_room_time_not_wakes() {
        let mut room = CrackAttack::new();
        room.game = Some(GameState::new(7));
        room.catch_up(5 * TICK_NANOS);
        assert_eq!(room.ticks, 5);
        // The same instant again: nothing more.
        room.catch_up(5 * TICK_NANOS);
        assert_eq!(room.ticks, 5);
        // A long stall: a bounded catch-up, then the clock goes on from there.
        room.catch_up(1000 * TICK_NANOS);
        assert_eq!(room.ticks, 5 + MAX_CATCH_UP);
        room.catch_up(1001 * TICK_NANOS);
        assert_eq!(room.ticks, 6 + MAX_CATCH_UP);
    }

    #[test]
    fn the_pause_stops_the_clock() {
        let mut room = CrackAttack::new();
        room.game = Some(GameState::new(7));
        room.catch_up(3 * TICK_NANOS);
        room.paused = true;
        room.catch_up(500 * TICK_NANOS);
        room.paused = false;
        room.catch_up(501 * TICK_NANOS);
        assert_eq!(room.ticks, 4);
    }

    #[test]
    fn keys_map_to_the_engine_inputs() {
        assert_eq!(map_input(Input::Char(' ')), INPUT_SWAP);
        assert_eq!(map_input(Input::Char('W')), INPUT_UP);
        assert_eq!(map_input(Input::Key(Key::Enter)), INPUT_ADVANCE);
        assert_eq!(map_input(Input::Char('x')), 0);
    }
}

