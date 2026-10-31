// Crack Attack! for shellcade: solo, against the computer, or a duel between the two
// players of a room, on the arcade's 80×24 canvas.
//
//   cargo test                           # natively
//   tools/shellcade-kit play .           # build the wasm artifact and play it
//   tools/shellcade-kit play . --seats 2 # a duel, Ctrl-T switching player
//   tools/shellcade-kit check .          # the conformance harness
//
// The engine (`crackattack-core`) steps 50 times per second. Its ticks follow the room
// clock (`now_unix_nanos`), never the number of wakes: the heartbeat may jitter.
#![forbid(unsafe_code)]

mod danger;
mod duel;
mod menu;
mod render;
mod seat;
mod solo;

use crackattack_core::STEPS_PER_SECOND;
use shellcade_kit::prelude::*;

use duel::Duel;
use menu::{Choice, Menu};
use solo::Solo;

/// One engine tick, in nanoseconds of room time.
const TICK_NANOS: i64 = 1_000_000_000 / STEPS_PER_SECOND as i64;
/// Most ticks caught up in one wake: after a longer stall, the game slows down rather
/// than jump ahead (and stays inside the callback deadline).
const MAX_CATCH_UP: i64 = 10;

struct CrackAttackGame;

impl Game for CrackAttackGame {
    fn meta(&self) -> Meta {
        Meta {
            slug: "crackattack",
            name: "Crack Attack!",
            short_description: "Swap blocks, line up three, chain the combos: alone or in a duel.",
            min_players: 1,
            max_players: 2,
            tags: &["puzzle", "action", "versus"],
            quick_mode_label: "Duel",
            solo_mode_label: "Solo",
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

    fn new_room(&self, cfg: &RoomConfig) -> Box<dyn Handler> {
        Box::new(CrackAttack::new(cfg.mode == Mode::Solo || cfg.capacity <= 1))
    }
}

/// Room time turned into engine ticks.
struct Clock {
    /// Room time of tick 0, and ticks run since.
    origin: i64,
    ticks: i64,
}

impl Clock {
    /// Ticks to run by room time `now`; none while `running` is false (time then stands
    /// still: the next ticks start from now).
    fn due(&mut self, now: i64, running: bool) -> i64 {
        if !running {
            self.origin = now - self.ticks * TICK_NANOS;
            return 0;
        }
        let due = (now - self.origin) / TICK_NANOS - self.ticks;
        if due > MAX_CATCH_UP {
            self.origin += (due - MAX_CATCH_UP) * TICK_NANOS;
        }
        let due = due.clamp(0, MAX_CATCH_UP);
        self.ticks += due;
        due
    }
}

/// What the room is doing.
enum Stage {
    /// Nobody joined yet.
    Empty,
    /// One player, choosing a game (and in a room for two, waiting for an opponent).
    Menu(Menu),
    Solo(Solo),
    Duel(Duel),
}

/// One room. All its state lives here (the host may snapshot the room).
struct CrackAttack {
    /// A solo room: no opponent will come.
    solo_room: bool,
    stage: Stage,
    /// Players in seat order: the first two play, a later one watches.
    players: Vec<Player>,
    clock: Clock,
    canvas: Frame,
    /// The screen last sent to each seat (the solo screen goes to everyone, as seat 0's):
    /// most ticks change nothing visible, and are not sent.
    shown: [Vec<Cell>; 2],
    ended: bool,
}

/// The same player, across reconnections: by account, a guest by connection.
fn same_player(a: &Player, b: &Player) -> bool {
    if a.account_id.is_empty() || b.account_id.is_empty() {
        a.conn == b.conn
    } else {
        a.account_id == b.account_id
    }
}

impl CrackAttack {
    fn new(solo_room: bool) -> Self {
        Self {
            solo_room,
            stage: Stage::Empty,
            players: Vec::new(),
            clock: Clock { origin: 0, ticks: 0 },
            canvas: Frame::new(),
            shown: [Vec::new(), Vec::new()],
            ended: false,
        }
    }

    fn seat_of(&self, p: &Player) -> Option<usize> {
        self.players.iter().position(|q| same_player(p, q))
    }

    /// Runs the ticks due by room time `now`.
    fn catch_up(&mut self, now: i64) {
        let running = !self.ended
            && match &self.stage {
                Stage::Solo(solo) => !solo.paused(),
                Stage::Duel(duel) => !duel.suspended(),
                Stage::Empty | Stage::Menu(_) => false,
            };
        for _ in 0..self.clock.due(now, running) {
            match &mut self.stage {
                Stage::Solo(solo) => solo.tick(),
                Stage::Duel(duel) => duel.tick(),
                Stage::Empty | Stage::Menu(_) => {}
            }
        }
    }

    /// The game chosen in the menu.
    fn start(&mut self, r: &mut Room, choice: Choice) {
        let seed = r.rng_u64() as u32;
        self.stage = match choice {
            Choice::Solo => Stage::Solo(Solo::new(seed)),
            Choice::Computer(level) => Stage::Duel(Duel::against_computer(seed, self.players[0].display_name(), level)),
        };
        self.clock = Clock { origin: r.now_unix_nanos(), ticks: 0 };
    }

    /// Ends the room with its result.
    fn finish(&mut self, r: &mut Room) {
        if self.ended {
            return;
        }
        self.ended = true;
        let rankings = match &self.stage {
            Stage::Solo(solo) => self.players.first().map(|player| PlayerResult {
                player: player.clone(),
                metric: solo.last.score as i64,
                rank: 1,
                status: Status::Finished,
            }).into_iter().collect(),
            // A game against the computer is not a result for the leaderboard.
            Stage::Duel(duel) if duel.against_computer_on() => Vec::new(),
            Stage::Duel(duel) => duel.rankings(&self.players),
            Stage::Empty | Stage::Menu(_) => Vec::new(),
        };
        r.end(&Outcome { rankings });
    }

    /// Sends each seat its screen, if it changed.
    fn render(&mut self, r: &mut Room) {
        match &self.stage {
            Stage::Empty => {}
            Stage::Menu(menu) => {
                menu.compose(&mut self.canvas, !self.solo_room);
                if changed(&mut self.shown[0], &self.canvas) {
                    r.identical(&self.canvas);
                }
            }
            Stage::Solo(solo) => {
                solo.compose(&mut self.canvas);
                if changed(&mut self.shown[0], &self.canvas) {
                    r.identical(&self.canvas);
                }
            }
            Stage::Duel(duel) if duel.against_computer_on() => {
                duel.compose(0, &mut self.canvas);
                if changed(&mut self.shown[0], &self.canvas) {
                    r.identical(&self.canvas);
                }
            }
            Stage::Duel(duel) => {
                for side in 0..self.players.len().min(2) {
                    duel.compose(side, &mut self.canvas);
                    if changed(&mut self.shown[side], &self.canvas) {
                        r.send(&self.players[side], &self.canvas);
                    }
                }
            }
        }
    }
}

/// Whether `canvas` differs from the screen `shown`, which then becomes it.
fn changed(shown: &mut Vec<Cell>, canvas: &Frame) -> bool {
    let cells: Vec<Cell> = (0..ROWS).flat_map(|row| (0..COLS).map(move |col| canvas.get(row, col))).collect();
    let same = |a: &Cell, b: &Cell| {
        (a.rune, a.cp2, a.cp3, a.fg, a.bg, a.attr, a.cont) == (b.rune, b.cp2, b.cp3, b.fg, b.bg, b.attr, b.cont)
    };
    if cells.len() == shown.len() && cells.iter().zip(shown.iter()).all(|(a, b)| same(a, b)) {
        return false;
    }
    *shown = cells;
    true
}

impl Handler for CrackAttack {
    fn on_start(&mut self, _r: &mut Room) {}

    fn on_join(&mut self, r: &mut Room, p: Player) {
        self.catch_up(r.now_unix_nanos());
        if let Some(seat) = self.seat_of(&p) {
            // Back (a new connection, or a revived room).
            self.players[seat] = p;
            if let Stage::Duel(duel) = &mut self.stage {
                duel.rejoin(seat);
            }
        } else {
            self.players.push(p);
            match self.stage {
                Stage::Empty => self.stage = Stage::Menu(Menu::new()),
                Stage::Menu(_) if !self.solo_room => {
                    let names = [0, 1].map(|seat| self.players[seat].display_name());
                    self.stage = Stage::Duel(Duel::new(r.rng_u64() as u32, names));
                    self.clock = Clock { origin: r.now_unix_nanos(), ticks: 0 };
                }
                // A game going on: the newcomer watches.
                Stage::Menu(_) | Stage::Solo(_) | Stage::Duel(_) => {}
            }
        }
        // Everyone gets the whole screen again.
        self.shown = [Vec::new(), Vec::new()];
        self.render(r);
    }

    fn on_leave(&mut self, r: &mut Room, p: Player) {
        let Some(seat) = self.seat_of(&p) else { return };
        self.catch_up(r.now_unix_nanos());
        match &mut self.stage {
            // Nobody to wait for; a spectator just goes.
            Stage::Duel(duel) if duel.against_computer_on() => {
                if seat == 0 {
                    self.finish(r);
                }
            }
            Stage::Duel(duel) if seat < 2 => {
                if duel.suspended() {
                    // Nobody left to play.
                    self.finish(r);
                } else {
                    // The duel stands still: the one who stays sees the tally, and may
                    // end it with a key.
                    duel.leave(seat);
                }
            }
            _ if seat == 0 => self.finish(r),
            // A spectator.
            _ => {}
        }
        self.render(r);
    }

    fn on_input(&mut self, r: &mut Room, p: Player, input: Input) {
        let Some(seat) = self.seat_of(&p) else { return };
        self.catch_up(r.now_unix_nanos());
        match &mut self.stage {
            // The choices are for the room for one; in a room for two, the menu only waits.
            Stage::Menu(menu) if seat == 0 && self.solo_room => {
                if let Some(choice) = menu.input(input) {
                    self.start(r, choice);
                }
            }
            Stage::Solo(solo) if seat == 0 => {
                if solo.game_over_shown() {
                    self.finish(r);
                } else {
                    solo.input(input);
                }
            }
            // Against the computer, side 1 is the AI: a second player only watches.
            Stage::Duel(duel) if seat < 2 && !(duel.against_computer_on() && seat == 1) => {
                if duel.suspended() {
                    self.finish(r);
                } else {
                    duel.input(seat, input);
                }
            }
            _ => {}
        }
        self.render(r);
    }

    fn on_wake(&mut self, r: &mut Room) {
        let ticks = self.clock.ticks;
        self.catch_up(r.now_unix_nanos());
        if self.clock.ticks != ticks {
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
        let mut clock = Clock { origin: 0, ticks: 0 };
        assert_eq!(clock.due(5 * TICK_NANOS, true), 5);
        // The same instant again: nothing more.
        assert_eq!(clock.due(5 * TICK_NANOS, true), 0);
        // A long stall: a bounded catch-up, then the clock goes on from there.
        assert_eq!(clock.due(1000 * TICK_NANOS, true), MAX_CATCH_UP);
        assert_eq!(clock.due(1001 * TICK_NANOS, true), 1);
    }

    #[test]
    fn a_stopped_clock_resumes_where_it_was() {
        let mut clock = Clock { origin: 0, ticks: 0 };
        clock.due(3 * TICK_NANOS, true);
        assert_eq!(clock.due(500 * TICK_NANOS, false), 0);
        assert_eq!(clock.due(501 * TICK_NANOS, true), 1);
        assert_eq!(clock.ticks, 4);
    }

    #[test]
    fn players_are_recognized_by_account_or_connection() {
        let player = |account: &str, conn: &str| Player { account_id: account.into(), conn: conn.into(), ..Player::default() };
        assert!(same_player(&player("a", "1"), &player("a", "2")));
        assert!(!same_player(&player("a", "1"), &player("b", "1")));
        assert!(same_player(&player("", "1"), &player("", "1")));
        assert!(!same_player(&player("", "1"), &player("", "2")));
    }
}
