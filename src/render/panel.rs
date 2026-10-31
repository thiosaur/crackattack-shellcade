// Side panel, right of the board: game info, the chain just reached, the logo and the
// keys.

use crackattack_core::constants::STEPS_PER_SECOND;
use crackattack_core::frame_data::FrameData;

use super::palette::{CHAIN_COLOR, HINT_COLOR, LOGO_COLORS};
use super::raster::{Pen, Writer, LOWER_EIGHTHS};
use super::Hud;

/// The game's compact logo (`logo.txt`): one block per `#`, the two words separated by a
/// blank line.
const LOGO: &str = include_str!("../logo.txt");
/// What the logo turns into when the game is lost: same layout (two words, same colors).
const GAME_OVER: &str = include_str!("../game-over.txt");
/// Game over: the logo turns into GAME OVER column after column, from the left, each
/// flipping like a block of a shattered garbage (it flattens, then the new one grows).
/// Ticks between two columns, and for one column's flip.
const FLIP_STAGGER: i32 = 2;
const FLIP_TICKS: i32 = 8;

/// The game's keys (mapped in `lib.rs`), as the panel reminds them: (key, action).
/// Esc belongs to the arcade: it leaves the game.
const KEYS: [(&str, &str); 5] =
    [("Arrows", "move"), ("Space", "swap"), ("Enter", "raise"), ("P", "pause"), ("Esc", "leave")];

/// Panel lines (from the top) of the game info, the chain popup and the logo; the keys
/// take the last `KEY_LINES`.
const TIME_LINE: usize = 1;
const BEST_LINE: usize = 2;
const CHAIN_LINE: usize = 4;
const LOGO_LINE: usize = 6;
const KEY_LINES: usize = 2;

/// Writes the side panel's line `i` (0 = top) of `lines`: game info at the top, the chain
/// just reached below it, the logo, the keys at the bottom (once the game is lost and
/// GAME OVER shown, a prompt instead).
pub(super) fn write_panel_line(out: &mut Writer, i: usize, lines: usize, frame: &FrameData, hud: &Hud) {
    let seconds = frame.ui.clock_time / STEPS_PER_SECOND;
    let first_key_line = lines - KEY_LINES;
    if i == 0 {
        out.text(&format!("Score  {}", frame.score), Pen::bold(None));
    } else if i == TIME_LINE {
        out.text(&format!("Time   {}:{:02}", seconds / 60, seconds % 60), Pen::default());
    } else if i == BEST_LINE {
        let best = match frame.stats.max_chain_level {
            n if n >= 2 => format!("Best   x{}", n),
            _ => "Best   -".to_string(),
        };
        out.text(&best, Pen::default());
    } else if i == CHAIN_LINE {
        if let Some(n) = hud.chain {
            out.text(&format!("Chain x{}!", n), Pen::bold(Some(CHAIN_COLOR)));
        }
    } else if (LOGO_LINE..LOGO_LINE + art().colors.len()).contains(&i) {
        let row = i - LOGO_LINE;
        out.text(&logo_line(row, hud.game_over), Pen::ink(art().colors[row]));
    } else if hud.game_over.is_some_and(|t| t >= game_over_ticks()) {
        if i == first_key_line {
            out.text("Press any key", Pen::bold(None));
        }
    } else if let Some(keys) = i.checked_sub(first_key_line).and_then(|n| KEYS.chunks(KEYS.len().div_ceil(KEY_LINES)).nth(n)) {
        let text = keys.iter().map(|(key, action)| format!("{} {}", key, action)).collect::<Vec<_>>().join("  ");
        out.text(&text, Pen::ink(HINT_COLOR));
    }
}

/// The logo and GAME OVER, cell by cell (`true` = a block), with the color of each line.
struct Art {
    logo: Vec<Vec<bool>>,
    game_over: Vec<Vec<bool>>,
    colors: Vec<u8>,
}

fn art() -> &'static Art {
    static ART: std::sync::OnceLock<Art> = std::sync::OnceLock::new();
    ART.get_or_init(|| {
        let cells = |text: &str| -> Vec<Vec<bool>> {
            text.lines().map(|line| line.chars().map(|c| c == '#').collect()).collect()
        };
        // The words are separated by a blank line; each has its color.
        let mut word = 0;
        let colors = LOGO
            .lines()
            .map(|line| {
                if line.trim().is_empty() {
                    word = (word + 1).min(LOGO_COLORS.len() - 1);
                }
                LOGO_COLORS[word]
            })
            .collect();
        Art { logo: cells(LOGO), game_over: cells(GAME_OVER), colors }
    })
}

/// Ticks from the loss until GAME OVER is fully shown.
pub(crate) fn game_over_ticks() -> i32 {
    let art = art();
    let width = art.logo.iter().chain(&art.game_over).map(Vec::len).max().unwrap_or(0);
    (width as i32 - 1).max(0) * FLIP_STAGGER + FLIP_TICKS
}

/// Line `row` of the logo; `game_over` ticks after a loss, turning into GAME OVER.
fn logo_line(row: usize, game_over: Option<i32>) -> String {
    let art = art();
    let filled = |cells: &[Vec<bool>], col: usize| cells.get(row).and_then(|l| l.get(col)).copied().unwrap_or(false);
    let width = |cells: &[Vec<bool>]| cells.get(row).map_or(0, Vec::len);
    match game_over {
        None => (0..width(&art.logo)).map(|col| if filled(&art.logo, col) { '█' } else { ' ' }).collect(),
        Some(t) => (0..width(&art.logo).max(width(&art.game_over)))
            .map(|col| {
                // Into its flip: the logo's cell shrinks to the bottom (8, 6, 4, 2 eighths),
                // then GAME OVER's grows (2, 4, 6, 8).
                let p = t - col as i32 * FLIP_STAGGER;
                let (on, eighths) = if p < FLIP_TICKS / 2 {
                    (filled(&art.logo, col), 8 - 2 * p.max(0))
                } else {
                    (filled(&art.game_over, col), (2 * (p - FLIP_TICKS / 2 + 1)).min(8))
                };
                match (on, eighths) {
                    (false, _) => ' ',
                    (true, 8) => '█',
                    (true, n) => LOWER_EIGHTHS[n as usize - 1],
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_turns_into_game_over() {
        assert!(logo_line(0, None).contains('█'));
        assert_eq!(logo_line(0, Some(0)), logo_line(0, None));
        let over: String = GAME_OVER.lines().next().unwrap().chars().map(|c| if c == '#' { '█' } else { ' ' }).collect();
        assert_eq!(logo_line(0, Some(game_over_ticks())).trim_end(), over.trim_end());
    }

    #[test]
    fn the_logo_and_keys_fit_the_panel() {
        let logo_end = LOGO_LINE + art().colors.len();
        assert!(logo_end <= super::super::SCREEN_LINES - KEY_LINES);
    }
}
