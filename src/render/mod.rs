// Renderer: game state → the shellcade canvas (80×24 cells, RGB colors).
//
// `board` paints a game on a fine canvas → `raster` turns each 8 units of it into one
// cell → this module lays the screens out: the solo screen (board and side panel,
// `panel`), the duel screen (two boards around a center panel) and the logo screen
// between duel rounds. Colors are in `palette`.

mod board;
mod palette;
mod panel;
mod raster;

use crackattack_core::frame_data::FrameData;
use crackattack_core::STEPS_PER_SECOND;
use shellcade_kit::prelude::{Frame, COLS, ROWS};

use crate::danger::DangerBars;
use board::{FIELD_COLUMNS, LINES_BELOW_GROUND, LINE_UNITS, TOP_UNIT};
use palette::*;
use raster::{Pen, Writer, LOWER_EIGHTHS};

pub(crate) use panel::game_over_ticks;

/// Lines of the canvas.
const SCREEN_LINES: usize = ROWS as usize;
/// Space between each danger bar and the board.
const DANGER_GAP: i32 = 3;
/// Columns of a board with its danger bars.
const BOARD_SPAN: i32 = 1 + DANGER_GAP + FIELD_COLUMNS as i32 + DANGER_GAP + 1;
/// Solo: space between the right danger bar and the side panel, and the panel's column.
const PANEL_GAP: i32 = 3;
const PANEL_COLUMN: i32 = BOARD_SPAN + PANEL_GAP;
/// Duel: the opponent's board on the right edge, the center panel between the two.
const OTHER_BOARD_COLUMN: i32 = COLS - BOARD_SPAN;
const CENTER_WIDTH: i32 = OTHER_BOARD_COLUMN - BOARD_SPAN;
/// Screen lines of a board, from the top of the playfield to the rising row, and the
/// first one (boards are centered).
const BOARD_LINES: i32 = TOP_UNIT / LINE_UNITS + LINES_BELOW_GROUND;
const FIRST_BOARD_LINE: i32 = (SCREEN_LINES as i32 - BOARD_LINES) / 2;

/// What is drawn around a board besides the game itself.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Hud {
    pub danger: DangerBars,
    /// Chain just reached (×2, ×3…), shown for a moment.
    pub chain: Option<i32>,
    /// Solo: ticks since the game was lost, the logo turning into GAME OVER.
    pub game_over: Option<i32>,
}

/// One danger bar cell for this line: the bar rises from the ground by eighths.
fn write_danger(out: &mut Writer, line: i32, bars: DangerBars) {
    let filled = (bars.height * TOP_UNIT as f32).round() as i32 - line * LINE_UNITS;
    if line < 0 || bars.alpha <= 0.0 || filled <= 0 {
        out.col += 1;
        return;
    }
    let shades = if bars.red { DANGER_RED } else { DANGER_PURPLE };
    let color = if bars.alpha > 0.5 { shades[0] } else { shades[1] };
    let glyph = if filled >= LINE_UNITS { '█' } else { LOWER_EIGHTHS[filled as usize - 1] };
    out.put(glyph, Pen::ink(color));
}

/// How a board is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Normal,
    /// The opponent's, or a lost one: colors faded.
    Muted,
    /// Solo pause: the board hidden behind a message (no planning during the pause).
    Paused,
}

/// A board with its danger bars, `BOARD_SPAN` columns from `col`.
fn write_board(canvas: &mut Frame, col: i32, frame: &FrameData, hud: &Hud, look: Look) {
    let danger = if look == Look::Paused { DangerBars::default() } else { hud.danger };
    let painted = (look != Look::Paused).then(|| board::paint(frame));
    let top_line = TOP_UNIT / LINE_UNITS;
    for i in 0..BOARD_LINES {
        let line = top_line - 1 - i;
        let mut out = Writer::new(canvas, FIRST_BOARD_LINE + i, col);
        if look == Look::Muted {
            out.saturation = MUTED;
        }
        write_danger(&mut out, line, danger);
        out.col += DANGER_GAP;
        match &painted {
            Some(painted) => board::write_line(&mut out, painted, line),
            None => {
                let text = match line - top_line / 2 {
                    0 => "PAUSE",
                    -1 => "P: resume",
                    _ => "",
                };
                out.text(&format!("{:^width$}", text, width = FIELD_COLUMNS), Pen::bold(None));
            }
        }
        out.col += DANGER_GAP;
        write_danger(&mut out, line, danger);
    }
}

/// Writes `text` centered in `width` columns from `col`.
fn write_centered(canvas: &mut Frame, line: i32, col: i32, width: i32, text: &str, pen: Pen) {
    let len = text.chars().count() as i32;
    Writer::new(canvas, line, col + ((width - len) / 2).max(0)).text(text, pen);
}

/// Solo: the board, and the side panel. When `paused`, the board is hidden.
pub(crate) fn compose_solo(canvas: &mut Frame, frame: &FrameData, paused: bool, hud: &Hud) {
    canvas.clear();
    write_board(canvas, 0, frame, hud, if paused { Look::Paused } else { Look::Normal });
    for i in 0..SCREEN_LINES {
        let mut out = Writer::new(canvas, i as i32, PANEL_COLUMN);
        panel::write_panel_line(&mut out, i, SCREEN_LINES, frame, hud);
    }
}

/// The big message of the duel's center panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Banner {
    /// 3, 2, 1, GO!
    Countdown(&'static str),
    Won,
    Lost,
}

/// One side of a duel, as its player sees it.
pub(crate) struct DuelSide<'a> {
    pub frame: &'a FrameData,
    pub hud: Hud,
    pub name: &'a str,
    pub wins: i32,
}

/// What a duel player sees: their board on the left, the opponent's on the right (faded),
/// the round, the tally and their game info in between.
pub(crate) struct DuelView<'a> {
    pub own: DuelSide<'a>,
    pub other: DuelSide<'a>,
    pub round: i32,
    pub banner: Option<Banner>,
}

/// Duel: the player's board, the center panel, the opponent's board.
pub(crate) fn compose_duel(canvas: &mut Frame, view: &DuelView) {
    canvas.clear();
    let own_look = if view.banner == Some(Banner::Lost) { Look::Muted } else { Look::Normal };
    write_board(canvas, 0, view.own.frame, &view.own.hud, own_look);
    write_board(canvas, OTHER_BOARD_COLUMN, view.other.frame, &view.other.hud, Look::Muted);
    // Under each board, whose it is.
    let names_line = FIRST_BOARD_LINE + BOARD_LINES + 1;
    write_centered(canvas, names_line, 0, BOARD_SPAN, "You", Pen::bold(None));
    write_centered(canvas, names_line, OTHER_BOARD_COLUMN, BOARD_SPAN, view.other.name, Pen::ink(HINT_COLOR));

    let center = |canvas: &mut Frame, line: i32, text: &str, pen: Pen| {
        write_centered(canvas, line, BOARD_SPAN, CENTER_WIDTH, text, pen);
    };
    // The game's name in the logo's colors.
    let title = BOARD_SPAN + (CENTER_WIDTH - 13) / 2;
    let mut out = Writer::new(canvas, 0, title);
    out.text("CRACK ", Pen::bold(Some(LOGO_COLORS[0])));
    out.text("ATTACK!", Pen::bold(Some(LOGO_COLORS[1])));

    center(canvas, 2, &format!("Round {}", view.round), Pen::default());
    let tally = |side: &DuelSide, name: &str| format!("{:<20}{:>3}", truncate(name, 20), side.wins);
    center(canvas, 4, &tally(&view.own, "You"), Pen::bold(None));
    center(canvas, 5, &tally(&view.other, view.other.name), Pen::default());
    let seconds = view.own.frame.ui.clock_time / STEPS_PER_SECOND;
    center(canvas, 7, &format!("{:<9}{:>6}", "Score", view.own.frame.score), Pen::default());
    center(canvas, 8, &format!("{:<9}{:>3}:{:02}", "Time", seconds / 60, seconds % 60), Pen::default());
    if let Some(n) = view.own.hud.chain {
        center(canvas, 10, &format!("Chain x{}!", n), Pen::bold(Some(CHAIN_COLOR)));
    }
    if let Some(n) = view.other.hud.chain {
        center(canvas, 11, &format!("{} x{}", truncate(view.other.name, 12), n), Pen::ink(HINT_COLOR));
    }
    match view.banner {
        Some(Banner::Countdown(digit)) => center(canvas, 14, digit, Pen::bold(Some(CHAIN_COLOR))),
        Some(Banner::Won) => center(canvas, 14, "YOU WIN!", Pen::bold(Some(WIN_COLOR))),
        Some(Banner::Lost) => center(canvas, 14, "YOU LOSE", Pen::bold(Some(LOSS_COLOR))),
        None => {}
    }
    center(canvas, 22, "Arrows move  Space swap", Pen::ink(HINT_COLOR));
    center(canvas, 23, "Enter raise  Esc leave", Pen::ink(HINT_COLOR));
}

fn truncate(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

/// How a line of the logo screen is written.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Tone {
    Plain,
    Strong,
    Won,
    Lost,
    Hint,
}

/// The logo screen, between duel rounds and while waiting for an opponent: the big logo,
/// then `lines` centered below it.
pub(crate) fn compose_logo_screen(canvas: &mut Frame, lines: &[(String, Tone)]) {
    canvas.clear();
    let col = (COLS - panel::logo_width() as i32) / 2;
    let first = 1;
    for row in 0..panel::logo_lines() {
        let mut out = Writer::new(canvas, first + row as i32, col);
        panel::write_logo_line(&mut out, row, None);
    }
    let mut line = first + panel::logo_lines() as i32 + 1;
    for (text, tone) in lines {
        let pen = match tone {
            Tone::Plain => Pen::default(),
            Tone::Strong => Pen::bold(None),
            Tone::Won => Pen::bold(Some(WIN_COLOR)),
            Tone::Lost => Pen::bold(Some(LOSS_COLOR)),
            Tone::Hint => Pen::ink(HINT_COLOR),
        };
        write_centered(canvas, line, 0, COLS, text, pen);
        line += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_boards_and_the_center_panel_fit_the_canvas() {
        assert_eq!(BOARD_SPAN, 27);
        assert!(CENTER_WIDTH >= 23);
        assert!(FIRST_BOARD_LINE + BOARD_LINES + 1 < 22);
        assert!(panel::logo_width() as i32 <= COLS);
    }
}
