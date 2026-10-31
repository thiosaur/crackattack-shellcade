// The menu at the start of a room: solo, or a duel against the computer at one of its
// three levels. In a room for two, it is also the waiting screen: an opponent who
// arrives before a choice starts a duel.

use crackattack_core::ai_difficulty::AIDifficulty;
use shellcade_kit::prelude::{Frame, Input, Key};

use crate::render::{self, Tone};

/// What the player chose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Choice {
    Solo,
    Computer(AIDifficulty),
}

/// The hard level (Cortex) is left out: one of its decisions can take over the 100 ms a
/// callback is allowed in wasm (3.6 ms natively at worst, against 0.2 ms for Sigma), and
/// the host then kills the room.
const CHOICES: [Choice; 3] = [
    Choice::Solo,
    Choice::Computer(AIDifficulty::Easy),
    Choice::Computer(AIDifficulty::Medium),
];

impl Choice {
    fn label(self) -> String {
        match self {
            Choice::Solo => "Solo".to_string(),
            Choice::Computer(level) => format!("Computer: {}", computer_name(level)),
        }
    }
}

/// The computer as an opponent: its level's name, as in the graphical version.
pub(crate) fn computer_name(level: AIDifficulty) -> String {
    let difficulty = match level {
        AIDifficulty::Easy => "easy",
        AIDifficulty::Medium => "medium",
        AIDifficulty::Hard => "hard",
    };
    format!("{} ({})", level.name(), difficulty)
}

pub(crate) struct Menu {
    selected: usize,
}

impl Menu {
    pub(crate) fn new() -> Self {
        Self { selected: 0 }
    }

    /// Arrows (or W/S) move through the choices; Enter or Space picks one.
    pub(crate) fn input(&mut self, input: Input) -> Option<Choice> {
        match input {
            Input::Key(Key::Up) | Input::Char('w' | 'W') => {
                self.selected = (self.selected + CHOICES.len() - 1) % CHOICES.len();
            }
            Input::Key(Key::Down) | Input::Char('s' | 'S') => {
                self.selected = (self.selected + 1) % CHOICES.len();
            }
            Input::Key(Key::Enter) | Input::Char(' ') => return Some(CHOICES[self.selected]),
            _ => {}
        }
        None
    }

    /// The logo, then the choices; `waiting` for an opponent, it says so.
    pub(crate) fn compose(&self, canvas: &mut Frame, waiting: bool) {
        let header = if waiting { ("Waiting for an opponent... or play:", Tone::Strong) } else { ("Play:", Tone::Strong) };
        let mut lines = vec![(header.0.to_string(), header.1), (String::new(), Tone::Plain)];
        for (i, choice) in CHOICES.iter().enumerate() {
            lines.push(if i == self.selected {
                (format!("> {} <", choice.label()), Tone::Won)
            } else {
                (choice.label(), Tone::Plain)
            });
        }
        lines.push(("Arrows choose  Enter play  Esc leave".to_string(), Tone::Hint));
        render::compose_logo_screen(canvas, &lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_go_round_the_choices() {
        let mut menu = Menu::new();
        assert_eq!(menu.input(Input::Key(Key::Enter)), Some(Choice::Solo));
        menu.input(Input::Key(Key::Up));
        assert_eq!(menu.input(Input::Char(' ')), Some(Choice::Computer(AIDifficulty::Medium)));
        menu.input(Input::Key(Key::Down));
        menu.input(Input::Key(Key::Down));
        assert_eq!(menu.input(Input::Key(Key::Enter)), Some(Choice::Computer(AIDifficulty::Easy)));
        assert_eq!(menu.input(Input::Key(Key::Left)), None);
    }
}
