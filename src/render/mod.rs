// Renderer: FrameData → the shellcade canvas (80×24 cells, RGB colors).
//
// `board` paints the game on a fine canvas → `raster` turns each 8 units of it into one
// cell → `compose` (here) lays the screen out: danger bars, board, side panel (`panel`).
// Colors are in `palette`.

mod board;
mod palette;
mod panel;
mod raster;

use crackattack_core::frame_data::FrameData;
use shellcade_kit::prelude::{Frame, ROWS};

use crate::danger::DangerBars;
use board::{FIELD_COLUMNS, LINES_BELOW_GROUND, LINE_UNITS, TOP_UNIT};
use palette::{DANGER_PURPLE, DANGER_RED};
use raster::{Pen, Writer, LOWER_EIGHTHS};

pub(crate) use panel::game_over_ticks;

/// Lines of the canvas.
const SCREEN_LINES: usize = ROWS as usize;
/// Space between each danger bar and the board.
const DANGER_GAP: i32 = 3;
/// Space between the right danger bar and the side panel.
const PANEL_GAP: i32 = 3;
/// First column of the board, and of the side panel: left of it are the board and its
/// danger bars.
const BOARD_COLUMN: i32 = 1 + DANGER_GAP;
const PANEL_COLUMN: i32 = BOARD_COLUMN + FIELD_COLUMNS as i32 + DANGER_GAP + 1 + PANEL_GAP;

/// What is drawn around the board besides the game itself.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Hud {
    pub danger: DangerBars,
    /// Chain just reached (×2, ×3…), shown for a moment.
    pub chain: Option<i32>,
    /// Ticks since the game was lost: the logo turns into GAME OVER.
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

/// Draws the screen on `canvas`. When `paused`, the board is hidden (no planning during
/// the pause) and a message replaces it.
pub(crate) fn compose(canvas: &mut Frame, frame: &FrameData, paused: bool, hud: &Hud) {
    canvas.clear();
    let danger = if paused { DangerBars::default() } else { hud.danger };
    let painted = (!paused).then(|| board::paint(frame));
    let top_line = TOP_UNIT / LINE_UNITS;
    let pause_line = top_line / 2;
    let board_lines = (top_line + LINES_BELOW_GROUND) as usize;
    // The board, centered.
    let first_board_line = (SCREEN_LINES - board_lines) / 2;

    for i in 0..SCREEN_LINES {
        let line = top_line - 1 - (i as i32 - first_board_line as i32);
        if (-LINES_BELOW_GROUND..top_line).contains(&line) {
            let mut out = Writer::new(canvas, i as i32, 0);
            write_danger(&mut out, line, danger);
            out.col += DANGER_GAP;
            match &painted {
                Some(painted) => board::write_line(&mut out, painted, line, frame.play_offset_y),
                None => {
                    let text = match line - pause_line {
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
        let mut out = Writer::new(canvas, i as i32, PANEL_COLUMN);
        panel::write_panel_line(&mut out, i, SCREEN_LINES, frame, hud);
    }
}
