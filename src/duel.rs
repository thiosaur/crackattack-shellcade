// Duel: the two players of the room, each in a session of the engine's VS mode (rounds,
// countdown, garbage, ties), linked through memory. Both run in this module, stepped at
// the same tick: no network, no lag, no desync.
//
// Between rounds, each player consents to the next one (the logo screen); the round
// starts for both at the same tick, once both have. Against the computer, side 1 is the
// engine's AI, always ready.

use std::cell::RefCell;
use std::rc::Rc;

use crackattack_core::ai_difficulty::AIDifficulty;
use crackattack_core::frame_data::CountdownDigit;
use crackattack_core::vs::{GridAutoplay, Opponent, RoundOutcome, SessionPhase, VsSession};
use crackattack_core::FrameData;
use shellcade_kit::prelude::{Frame, Input, Key, Player, PlayerResult, Status};

use crate::render::{self, Banner, DuelSide, DuelView, Tone};
use crate::seat::Seat;

/// What the two sides pass each other: the garbage sent to each, and when each lost
/// (its game tick).
#[derive(Default)]
struct Exchange {
    garbage: [Vec<(i32, i32, i32)>; 2],
    lost_at: [Option<i32>; 2],
}

/// The opponent of side `side`: the other side, through the exchange.
struct Link {
    side: usize,
    exchange: Rc<RefCell<Exchange>>,
    name: String,
}

impl Link {
    fn other(&self) -> usize {
        1 - self.side
    }
}

impl Opponent for Link {
    fn game_start(&mut self) {
        let mut exchange = self.exchange.borrow_mut();
        exchange.lost_at[self.other()] = None;
        exchange.garbage[self.side].clear();
    }

    /// The other side steps on its own, in `Duel::tick`.
    fn time_step(&mut self) {}

    fn send_garbage(&mut self, height: i32, width: i32, flavor: i32) {
        self.exchange.borrow_mut().garbage[self.other()].push((height, width, flavor));
    }

    fn recv_garbage(&mut self) -> Vec<(i32, i32, i32)> {
        std::mem::take(&mut self.exchange.borrow_mut().garbage[self.side])
    }

    /// For the level lights of the graphical version: not drawn here.
    fn grid_level(&mut self) -> i32 {
        0
    }

    fn notify_local_loss(&mut self, time_step: i32) {
        self.exchange.borrow_mut().lost_at[self.side].get_or_insert(time_step);
    }

    fn has_opponent_lost(&self) -> bool {
        self.exchange.borrow().lost_at[self.other()].is_some()
    }

    fn opponent_loss_timestamp(&self) -> Option<i32> {
        self.exchange.borrow().lost_at[self.other()]
    }

    fn opponent_name(&self) -> &str {
        &self.name
    }
}

pub(crate) struct Duel {
    sessions: [VsSession<Link>; 2],
    last: [FrameData; 2],
    seats: [Seat; 2],
    names: [String; 2],
    /// Consent to the next round.
    ready: [bool; 2],
    /// Result of each side's last round, for the logo screen (the engine forgets it once
    /// the next round is prepared).
    last_won: [Option<bool>; 2],
    /// Best round score of each side: the leaderboard's metric.
    best: [i32; 2],
    /// The side that left: the duel stands still until they come back, or the one who
    /// stayed ends it.
    away: Option<usize>,
    /// Against the computer: its level, and the AI playing side 1 (a new one each round).
    computer: Option<(AIDifficulty, GridAutoplay)>,
    seed: u32,
}

impl Duel {
    /// Both play boards dealt from the same seed. Round 1 starts at once.
    pub(crate) fn new(seed: u32, names: [String; 2]) -> Self {
        let exchange = Rc::new(RefCell::new(Exchange::default()));
        let sessions = [0, 1].map(|side| {
            let link = Link { side, exchange: Rc::clone(&exchange), name: names[1 - side].clone() };
            let mut session = VsSession::new(seed, link);
            session.set_needs_ready_prompt(true);
            // Side 1 steps second: when both top out at the same tick, it sees side 0's
            // loss and its own at the same time, and decides the tie for both.
            session.set_win_ties(side == 1);
            session
        });
        let mut duel = Self {
            sessions,
            last: [FrameData::default(), FrameData::default()],
            seats: [Seat::new(), Seat::new()],
            names,
            ready: [false; 2],
            last_won: [None; 2],
            best: [0; 2],
            away: None,
            computer: None,
            seed,
        };
        duel.start_round();
        duel
    }

    /// Against the computer at `level`: the player is side 0.
    pub(crate) fn against_computer(seed: u32, player: String, level: AIDifficulty) -> Self {
        let mut duel = Self::new(seed, [player, crate::menu::computer_name(level)]);
        duel.computer = Some((level, GridAutoplay::new(level, seed)));
        duel
    }

    /// Against the computer (not a result for the leaderboard).
    pub(crate) fn against_computer_on(&self) -> bool {
        self.computer.is_some()
    }

    /// Both sessions into the countdown of the round prepared, at the same tick.
    fn start_round(&mut self) {
        for side in 0..2 {
            self.sessions[side].do_start_new_round();
            self.seats[side].release_keys();
            self.last[side] = self.sessions[side].player_game.frame_data();
        }
        self.ready = [false; 2];
        if let Some((level, ai)) = &mut self.computer {
            // The seed only varies the AI's choices between equal moves.
            *ai = GridAutoplay::new(*level, self.seed.wrapping_add(self.sessions[1].round_number as u32));
        }
    }

    /// A player left: nothing moves.
    pub(crate) fn suspended(&self) -> bool {
        self.away.is_some()
    }

    pub(crate) fn leave(&mut self, side: usize) {
        self.away.get_or_insert(side);
    }

    /// A player is back: the duel goes on where it stood.
    pub(crate) fn rejoin(&mut self, side: usize) {
        if self.away == Some(side) {
            self.away = None;
            self.seats[side].release_keys();
        }
    }

    fn between_rounds(&self, side: usize) -> bool {
        matches!(self.sessions[side].phase(), SessionPhase::WaitingBothReady | SessionPhase::WaitingRemote)
    }

    pub(crate) fn tick(&mut self) {
        if self.suspended() {
            return;
        }
        if let Some((_, ai)) = &mut self.computer {
            // Always ready, and playing as the engine's computer opponent does.
            self.ready[1] = true;
            if self.sessions[1].phase() == SessionPhase::Playing {
                self.seats[1].press_keys(ai.update(&self.sessions[1].player_game));
            }
        }
        if (0..2).all(|side| self.between_rounds(side) && self.ready[side]) {
            self.start_round();
        }
        for side in 0..2 {
            let session = &mut self.sessions[side];
            let input = self.seats[side].next_input();
            let frame = session.time_step(&input);
            // Lifecycle events are for the graphical version's sounds and effects.
            session.pending_events.clear();
            if let Some(outcome) = session.round_outcome {
                self.last_won[side] = Some(outcome == RoundOutcome::PlayerWon);
            }
            self.best[side] = self.best[side].max(frame.score);
            self.seats[side].follow(&frame);
            self.last[side] = frame;
        }
    }

    pub(crate) fn input(&mut self, side: usize, input: Input) {
        if self.between_rounds(side) {
            if matches!(input, Input::Key(Key::Enter) | Input::Char(' ')) {
                self.ready[side] = true;
            }
            return;
        }
        if self.sessions[side].phase() == SessionPhase::Playing {
            self.seats[side].press(input);
        }
    }

    /// What side `side` sees.
    pub(crate) fn compose(&self, side: usize, canvas: &mut Frame) {
        let other = 1 - side;
        let wins = |s: usize| self.sessions[s].session_wins;
        let tally = format!("You {} - {} {}", wins(side), wins(other), self.names[other]);
        if self.away.is_some() {
            let lines = [
                (format!("{} left.", self.names[other]), Tone::Strong),
                (tally, Tone::Plain),
                (String::new(), Tone::Plain),
                ("Waiting for them to come back".to_string(), Tone::Hint),
                ("Any key: end the duel".to_string(), Tone::Strong),
            ];
            render::compose_logo_screen(canvas, &lines);
            return;
        }
        if self.between_rounds(side) {
            let result = match self.last_won[side] {
                Some(true) => ("YOU WIN!".to_string(), Tone::Won),
                _ => ("YOU LOSE".to_string(), Tone::Lost),
            };
            let prompt = if self.ready[side] {
                (format!("Waiting for {}...", self.names[other]), Tone::Hint)
            } else {
                ("Enter: play again".to_string(), Tone::Strong)
            };
            let leave = if self.against_computer_on() { "Q: menu  Esc: leave" } else { "Esc: leave" };
            let lines = [result, (tally, Tone::Plain), (String::new(), Tone::Plain), prompt, (leave.to_string(), Tone::Hint)];
            render::compose_logo_screen(canvas, &lines);
            return;
        }
        let session = &self.sessions[side];
        let banner = match session.phase() {
            SessionPhase::Countdown | SessionPhase::Playing => session.countdown_info().map(|info| {
                Banner::Countdown(match info.digit {
                    CountdownDigit::Three => "3",
                    CountdownDigit::Two => "2",
                    CountdownDigit::One => "1",
                    CountdownDigit::Go => "GO!",
                })
            }),
            SessionPhase::Resolving | SessionPhase::ShowResult => match session.round_outcome {
                Some(RoundOutcome::PlayerWon) => Some(Banner::Won),
                Some(RoundOutcome::PlayerLost) => Some(Banner::Lost),
                None => None,
            },
            _ => None,
        };
        let view = DuelView {
            own: DuelSide { frame: &self.last[side], hud: self.seats[side].hud(), name: &self.names[side], wins: wins(side) },
            other: DuelSide { frame: &self.last[other], hud: self.seats[other].hud(), name: &self.names[other], wins: wins(other) },
            round: session.round_number,
            banner,
            menu_key: self.against_computer_on(),
        };
        render::compose_duel(canvas, &view);
    }

    /// The room's result: the most rounds won first; the best round score is the
    /// leaderboard's metric. A player who left did not finish.
    pub(crate) fn rankings(&self, players: &[Player]) -> Vec<PlayerResult> {
        let mut order: Vec<usize> = (0..players.len().min(2)).collect();
        order.sort_by_key(|&side| (std::cmp::Reverse(self.sessions[side].session_wins), self.away == Some(side)));
        order
            .iter()
            .enumerate()
            .map(|(rank, &side)| PlayerResult {
                player: players[side].clone(),
                metric: self.best[side] as i64,
                rank: rank as u16 + 1,
                status: if self.away == Some(side) { Status::Dnf } else { Status::Finished },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn duel() -> Duel {
        Duel::new(42, ["Ann".to_string(), "Bob".to_string()])
    }

    /// Ticks until both sides are between rounds; panics if it takes too long.
    fn play_round(duel: &mut Duel) -> usize {
        for ticks in 1..60_000 {
            duel.tick();
            if (0..2).all(|side| duel.between_rounds(side)) {
                return ticks;
            }
        }
        panic!("the round never ended");
    }

    #[test]
    fn a_round_starts_with_the_countdown_for_both() {
        let mut duel = duel();
        duel.tick();
        for side in 0..2 {
            assert_eq!(duel.sessions[side].phase(), SessionPhase::Countdown);
        }
    }

    #[test]
    fn a_round_has_one_winner_and_one_loser() {
        let mut duel = duel();
        // Nobody plays: both stacks top out at the same tick, the tie goes to one side.
        play_round(&mut duel);
        let won = duel.last_won;
        assert!(won == [Some(true), Some(false)] || won == [Some(false), Some(true)], "{:?}", won);
        assert_eq!(duel.sessions[0].session_wins + duel.sessions[1].session_wins, 1);
    }

    #[test]
    fn the_next_round_waits_for_both_players() {
        let mut duel = duel();
        play_round(&mut duel);
        duel.input(0, Input::Key(Key::Enter));
        for _ in 0..10 {
            duel.tick();
        }
        assert!(duel.between_rounds(1));
        duel.input(1, Input::Char(' '));
        duel.tick();
        for side in 0..2 {
            assert_eq!(duel.sessions[side].phase(), SessionPhase::Countdown);
            assert_eq!(duel.sessions[side].round_number, 2);
        }
    }

    #[test]
    fn the_combos_of_one_side_drop_garbage_on_the_other() {
        use crackattack_core::ai_difficulty::AIDifficulty;
        use crackattack_core::vs::GridAutoplay;
        let mut duel = duel();
        let mut ai = GridAutoplay::new(AIDifficulty::Hard, 42);
        let mut received = false;
        for _ in 0..30_000 {
            if duel.sessions[0].phase() == SessionPhase::Playing {
                let keys = ai.update(&duel.sessions[0].player_game);
                duel.seats[0].press_keys(keys);
            }
            duel.tick();
            received |= !duel.last[1].garbage.is_empty();
            if duel.between_rounds(0) {
                break;
            }
        }
        assert!(received, "no garbage reached side 1");
        // The AI outlasts a player who does nothing.
        assert_eq!(duel.last_won, [Some(true), Some(false)]);
    }

    #[test]
    fn the_computer_plays_side_1_and_is_always_ready() {
        let mut duel = Duel::against_computer(42, "Ann".to_string(), AIDifficulty::Hard);
        assert_eq!(duel.names[1], "Cortex (hard)");
        play_round(&mut duel);
        // The computer beats a player who does nothing.
        assert_eq!(duel.last_won, [Some(false), Some(true)]);
        for _ in 0..10 {
            duel.tick();
        }
        assert!(duel.between_rounds(0), "the next round waits for the player only");
        duel.input(0, Input::Key(Key::Enter));
        duel.tick();
        assert_eq!(duel.sessions[0].phase(), SessionPhase::Countdown);
        assert_eq!(duel.sessions[1].round_number, 2);
    }

    #[test]
    fn garbage_goes_to_the_other_side() {
        let mut duel = duel();
        let mut link = Link { side: 0, exchange: Rc::clone(&duel.sessions[0].opponent.exchange), name: String::new() };
        link.send_garbage(1, 6, 0);
        assert_eq!(duel.sessions[1].opponent.recv_garbage(), vec![(1, 6, 0)]);
        assert!(duel.sessions[0].opponent.recv_garbage().is_empty());
    }

    #[test]
    fn a_player_who_leaves_and_comes_back_resumes_the_round() {
        let mut duel = duel();
        for _ in 0..300 {
            duel.tick();
        }
        duel.leave(1);
        let frozen = duel.sessions[0].player_game.time_step;
        for _ in 0..100 {
            duel.tick();
        }
        assert_eq!(duel.sessions[0].player_game.time_step, frozen);
        duel.rejoin(1);
        duel.tick();
        assert_eq!(duel.sessions[0].player_game.time_step, frozen + 1);
        assert_eq!(duel.sessions[0].phase(), SessionPhase::Playing);
    }

    #[test]
    fn the_player_who_stays_ranks_first_at_equal_wins() {
        let mut duel = duel();
        duel.leave(0);
        let players = [Player { handle: "Ann".into(), ..Player::default() }, Player { handle: "Bob".into(), ..Player::default() }];
        let rankings = duel.rankings(&players);
        assert_eq!(rankings[0].player.handle, "Bob");
        assert_eq!(rankings[1].status, Status::Dnf);
    }
}

/// A look at a duel with garbage flying (not a test of anything): the screen of the side
/// receiving it, to smoke-out/duel.ansi. `cargo test duel_snapshot -- --ignored`, then
/// `cat smoke-out/duel.ansi`.
#[cfg(test)]
mod snapshot {
    use super::*;
    use crackattack_core::ai_difficulty::AIDifficulty;
    use crackattack_core::vs::GridAutoplay;
    use shellcade_kit::prelude::{ATTR_BOLD, ATTR_REVERSE, ATTR_UNDERLINE, COLS, ROWS};
    use std::fmt::Write;

    #[test]
    #[ignore]
    fn duel_snapshot() {
        let mut duel = Duel::new(42, ["Ann".to_string(), "Bob".to_string()]);
        let mut ais = [GridAutoplay::new(AIDifficulty::Hard, 1), GridAutoplay::new(AIDifficulty::Easy, 2)];
        for _ in 0..30_000 {
            for side in 0..2 {
                if duel.sessions[side].phase() == SessionPhase::Playing {
                    let keys = ais[side].update(&duel.sessions[side].player_game);
                    duel.seats[side].press_keys(keys);
                }
            }
            duel.tick();
            if duel.last[1].garbage.len() >= 2 {
                break;
            }
        }
        let mut canvas = Frame::new();
        duel.compose(1, &mut canvas);
        let mut out = String::new();
        for row in 0..ROWS {
            for col in 0..COLS {
                let cell = canvas.get(row, col);
                let mut params = vec!["0".to_string()];
                for (bit, code) in [(ATTR_BOLD, "1"), (ATTR_UNDERLINE, "4"), (ATTR_REVERSE, "7")] {
                    if cell.attr & bit != 0 {
                        params.push(code.into());
                    }
                }
                for (color, base) in [(cell.fg, 38), (cell.bg, 48)] {
                    if color.is_set() {
                        let (r, g, b) = color.rgb_vals();
                        params.push(format!("{};2;{};{};{}", base, r, g, b));
                    }
                }
                let _ = write!(out, "\x1b[{}m{}", params.join(";"), cell.rune);
            }
            out.push_str("\x1b[0m\n");
        }
        std::fs::create_dir_all("smoke-out").unwrap();
        std::fs::write("smoke-out/duel.ansi", out).unwrap();
    }
}
