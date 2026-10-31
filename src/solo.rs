// Solo: one game, as in the terminal version, until the stack tops out.

use crackattack_core::{FrameData, GameState};
use shellcade_kit::prelude::{Frame, Input};

use crate::render;
use crate::seat::Seat;

pub(crate) struct Solo {
    game: GameState,
    pub(crate) last: FrameData,
    seat: Seat,
    paused: bool,
    /// Ticks since the game was lost: the logo turns into GAME OVER.
    game_over: Option<i32>,
}

impl Solo {
    pub(crate) fn new(seed: u32) -> Self {
        let mut solo = Self { game: GameState::new(seed), last: FrameData::default(), seat: Seat::new(), paused: false, game_over: None };
        // The first frame, before the first tick.
        solo.tick();
        solo
    }

    /// The clock stands still.
    pub(crate) fn paused(&self) -> bool {
        self.paused
    }

    /// One tick: the game, or once it is lost, the GAME OVER animation.
    pub(crate) fn tick(&mut self) {
        if self.last.game_over {
            if !self.game_over_shown() {
                self.game_over = Some(self.game_over.map_or(0, |t| t + 1));
            }
            return;
        }
        self.last = self.game.step(&self.seat.next_input());
        self.seat.follow(&self.last);
    }

    /// GAME OVER is fully shown: the next key ends the game.
    pub(crate) fn game_over_shown(&self) -> bool {
        self.game_over.is_some_and(|t| t >= render::game_over_ticks())
    }

    /// A key from the player: P pauses; keys pressed during the GAME OVER animation do
    /// not count.
    pub(crate) fn input(&mut self, input: Input) {
        if self.last.game_over {
            return;
        }
        if matches!(input, Input::Char('p' | 'P')) {
            self.paused = !self.paused;
            // The keys pressed during the pause must not count as held after it.
            self.seat.release_keys();
        } else if !self.paused {
            self.seat.press(input);
        }
    }

    pub(crate) fn compose(&self, canvas: &mut Frame) {
        let hud = render::Hud { game_over: self.game_over, ..self.seat.hud() };
        render::compose_solo(canvas, &self.last, self.paused, &hud);
    }
}
