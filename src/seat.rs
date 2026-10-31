// One player at a board: the keys they pressed, and what is drawn around their board
// (danger bars, the chain just reached).

use crackattack_core::gameplay_event::GameplayEvent;
use crackattack_core::input::{INPUT_ADVANCE, INPUT_DOWN, INPUT_LEFT, INPUT_RIGHT, INPUT_SWAP, INPUT_UP};
use crackattack_core::{FrameData, InputSnapshot};
use shellcade_kit::prelude::{Input, Key};

use crate::danger::DangerAnimator;
use crate::render::Hud;

/// How long the chain just reached stays on screen (ticks).
const CHAIN_SHOW_TICKS: i32 = 75;

pub(crate) struct Seat {
    /// Keys pressed since the last tick (`INPUT_*` bits), and those of the last tick,
    /// for the engine's edge detection.
    keys: u8,
    prev_state: u8,
    danger: DangerAnimator,
    /// Chain just reached, and ticks left to show it.
    chain: Option<(i32, i32)>,
}

impl Seat {
    pub(crate) fn new() -> Self {
        Self { keys: 0, prev_state: 0, danger: DangerAnimator::new(), chain: None }
    }

    /// A key for the engine, if `input` is one.
    pub(crate) fn press(&mut self, input: Input) {
        self.keys |= map_input(input);
    }

    /// Engine keys (`INPUT_*` bits) for the next tick: the AI, in tests.
    #[cfg(test)]
    pub(crate) fn press_keys(&mut self, keys: u8) {
        self.keys |= keys;
    }

    /// Keys pressed so far do not count (pause, end of a round).
    pub(crate) fn release_keys(&mut self) {
        self.keys = 0;
        self.prev_state = 0;
    }

    /// The input of the next tick: the keys pressed since the last one.
    pub(crate) fn next_input(&mut self) -> InputSnapshot {
        let mut input = InputSnapshot::new();
        input.state = std::mem::take(&mut self.keys);
        input.pressed_this_frame = input.state & !self.prev_state;
        input.prev_state = self.prev_state;
        self.prev_state = input.state;
        input
    }

    /// Follows a tick of the game: danger bars, chains.
    pub(crate) fn follow(&mut self, frame: &FrameData) {
        self.danger.step(&frame.ui);
        self.chain = self.chain.map(|(n, left)| (n, left - 1)).filter(|&(_, left)| left > 0);
        for event in &frame.gameplay_events {
            if let GameplayEvent::ChainProgressed(e) = event {
                self.chain = Some((e.chain_level, CHAIN_SHOW_TICKS));
            }
        }
    }

    pub(crate) fn hud(&self) -> Hud {
        Hud { danger: self.danger.bars(), chain: self.chain.map(|(n, _)| n), game_over: None }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_to_the_engine_inputs() {
        assert_eq!(map_input(Input::Char(' ')), INPUT_SWAP);
        assert_eq!(map_input(Input::Char('W')), INPUT_UP);
        assert_eq!(map_input(Input::Key(Key::Enter)), INPUT_ADVANCE);
        assert_eq!(map_input(Input::Char('x')), 0);
    }

    #[test]
    fn a_key_counts_for_one_tick() {
        let mut seat = Seat::new();
        seat.press(Input::Char(' '));
        let first = seat.next_input();
        assert_eq!((first.state, first.pressed_this_frame), (INPUT_SWAP, INPUT_SWAP));
        assert_eq!(seat.next_input().state, 0);
    }
}
