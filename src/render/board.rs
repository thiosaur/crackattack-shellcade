// The board: its geometry, and blocks, garbage, cursor and dots painted on a fine grid
// (8 units per row, one screen line). Blocks are two columns wide and solid (▇, an eighth
// of gap on top), and the whole stack rises by whole rows above a fixed ground line.

use crackattack_core::constants::*;
use crackattack_core::frame_data::FrameData;
use crackattack_core::garbage::GARBAGE_FLAVOR_GRAY;
use crackattack_core::BlockState;

use super::palette::*;
use super::raster::{split_cell, split_line, Canvas, Pen, Writer};

/// Fine vertical units per screen line (one per eighth of a cell), and per row.
pub(super) const LINE_UNITS: i32 = 8;
pub(super) const ROW_UNITS: i32 = LINE_UNITS;
/// Screen columns per cell: a gap column (cursor brackets, garbage seams) then the block.
pub(super) const CELL_COLUMNS: usize = 3;
const BLOCK_COLUMNS: usize = CELL_COLUMNS - 1;
/// Width of the playfield in columns (with the closing gap column on the right).
pub(super) const FIELD_COLUMNS: usize = PLAY_WIDTH * CELL_COLUMNS + 1;
/// Lines shown below the ground: the rising row, not pushed up yet.
pub(super) const LINES_BELOW_GROUND: i32 = ROW_UNITS / LINE_UNITS;
/// Vertical span painted, in units: from the bottom of the rising row to the top of the
/// playfield.
pub(super) const LOWEST_UNIT: i32 = -ROW_UNITS;
pub(super) const TOP_UNIT: i32 = SAFE_HEIGHT as i32 * ROW_UNITS;

/// Dying block: a small square, which shrinks during the second half of the delay.
const DYING_GLYPHS: [char; 2] = ['■', '▪'];
/// How long a garbage flashes once hit (ticks).
const SHATTER_FLASH_TICKS: i32 = 8;
/// Last ticks before a block of a shattered garbage pops: it flips like a card, from the
/// garbage color to its own (as the graphical version, `4 * INTERNAL_POP_DELAY / 5`).
const POP_ROTATE_TIME: i32 = 4 * INTERNAL_POP_DELAY / 5;
/// Thinnest a flipping block gets, as a fraction of a block.
const FLIP_MIN_HEIGHT: f32 = 0.2;
/// How far two swapping blocks move apart vertically where they cross (units): about a
/// third of a row.
const SWAP_LIFT_UNITS: i32 = (ROW_UNITS * 5 + 8) / 16;

/// Marks the playable area: a dot at the center of each empty cell. The glyph and line
/// are picked so the dot lands closest to the center.
const DOTS: [char; 3] = ['.', '·', '˙'];
/// Height of each dot's middle within its cell, in units from the bottom (8 = top),
/// measured with Menlo.
const DOT_HEIGHTS: [f32; 3] = [2.5, 4.2, 6.5];

/// Bottom of a row in units above the ground, from its grid row, the stack rise and the
/// sub-row fall offset. Row 1 sits on the ground when the stack has not risen. Snapped
/// to whole lines, so the gaps between blocks always show.
pub(super) fn bottom_unit(grid_y: i32, sub_grid_y: i32, play_offset_y: f32) -> i32 {
    let rows = grid_y as f32 - 1.0 + play_offset_y + sub_grid_y as f32 / STEPS_PER_GRID as f32;
    // A row reaches a line only once it has fully risen into it (the margin absorbs
    // float error).
    (rows * ROW_UNITS as f32 / LINE_UNITS as f32 + 1e-3).floor() as i32 * LINE_UNITS
}

/// Paints blocks, garbage and the cursor on the fine grid.
pub(super) fn paint(frame: &FrameData) -> Canvas {
    let mut canvas = Canvas::new();
    let rise = frame.play_offset_y;
    canvas.dots = dot_lines(rise);

    // Garbage first: the blocks a shattered garbage turns into are painted over it.
    for garbage in &frame.garbage {
        let flashing = garbage.shatter_progress > 0.0
            && garbage.shatter_progress * (SHATTER_TIME as f32) < SHATTER_FLASH_TICKS as f32;
        let color = garbage_color(garbage.flavor, flashing);
        let bottom = bottom_unit(garbage.grid_y, garbage.sub_grid_y, rise);
        let left = garbage.grid_x.max(0) as usize * CELL_COLUMNS + 1;
        let right = (garbage.grid_x + garbage.width).max(1) as usize * CELL_COLUMNS - 1;
        // One slab: the seams between its cells are filled.
        canvas.fill(left..=right, bottom, bottom + garbage.height * ROW_UNITS - 1, color);
    }

    let swap_factor = frame.swapper.swap_factor;
    for block in &frame.blocks {
        let x = block.grid_x as usize;
        if x >= PLAY_WIDTH || block.grid_y < 0 {
            continue;
        }
        let flavor = (block.flavor.max(0) as usize).min(COLORS.len() - 1);
        let state = BlockState::from_bits_truncate(block.state);
        let bottom = bottom_unit(block.grid_y, block.sub_grid_y, rise);
        if state.contains(BlockState::DYING) {
            // A small square on the line of the cell, in its middle column.
            let second_half = block.animation_countdown * 2 < DYING_DELAY;
            canvas.marks.push((
                x * CELL_COLUMNS + CELL_COLUMNS / 2,
                (bottom + ROW_UNITS / 2).div_euclid(LINE_UNITS),
                DYING_GLYPHS[second_half as usize],
                DYING_COLOR,
            ));
            continue;
        }
        let mut color = if block.grid_y == 0 { DIM_COLORS[flavor] } else { COLORS[flavor] };
        // One unit of gap on top keeps stacked blocks apart.
        let (mut low, mut high) = (bottom, bottom + ROW_UNITS - 1);
        let mut left = (x * CELL_COLUMNS + 1) as i32;
        if state.contains(BlockState::AWAKING) {
            // From a shattered garbage.
            let remaining = block.pop_countdown - frame.time_step;
            let under_slab = canvas.get(left as usize + 1, bottom + ROW_UNITS / 2).is_some();
            if block.pop_countdown != 0 && remaining >= POP_ROTATE_TIME && under_slab {
                // Still inside its garbage, which is drawn (and flashes) as one slab.
                continue;
            }
            let (awaking_color, height) = awaking_look(remaining, block.pop_countdown, block.pop_color, color);
            color = awaking_color;
            let shrink = ((ROW_UNITS - 1) as f32 * (1.0 - height) / 2.0).round() as i32;
            low += shrink;
            high -= shrink;
            if low >= high {
                continue;
            }
        } else if state.contains(BlockState::SWAPPING) {
            // Slides toward the other cell, the block going right in the upper part of
            // the row and the one going left in the lower part, so both stay visible
            // where they cross.
            let rightward = state.contains(BlockState::SWAP_DIRECTION);
            let other = if rightward { x + 1 } else { x.wrapping_sub(1) };
            for cell in [x, other].into_iter().filter(|&c| c < PLAY_WIDTH) {
                canvas.no_dots.push((cell * CELL_COLUMNS + CELL_COLUMNS / 2, bottom.div_euclid(LINE_UNITS)));
            }
            let shift = (swap_factor * CELL_COLUMNS as f32).round() as i32;
            let lift = ((swap_factor * std::f32::consts::PI).sin() * SWAP_LIFT_UNITS as f32).round() as i32;
            if rightward {
                left += shift;
                low += lift;
            } else {
                left -= shift;
                high -= lift;
            }
        }
        let left = left.max(0) as usize;
        canvas.fill(left..=left + BLOCK_COLUMNS - 1, low, high, color);
    }

    // Cursor last, over garbage seams: brackets in the gap columns around its two cells,
    // on the line of its row.
    let cursor_x = frame.swapper.grid_x.max(0) as usize;
    let line = bottom_unit(frame.swapper.grid_y, 0, rise).div_euclid(LINE_UNITS);
    let (left, right) = (cursor_x * CELL_COLUMNS, (cursor_x + 2) * CELL_COLUMNS);
    canvas.marks.extend([(left, line, '[', CURSOR_COLOR), (right, line, ']', CURSOR_COLOR)]);
    canvas
}

fn garbage_color(flavor: i32, flashing: bool) -> u8 {
    match (flavor == GARBAGE_FLAVOR_GRAY, flashing) {
        (true, false) => GARBAGE_GRAY_COLOR,
        (true, true) => GARBAGE_GRAY_FLASH_COLOR,
        (false, false) => GARBAGE_COLOR,
        (false, true) => GARBAGE_FLASH_COLOR,
    }
}

/// Look of a block born from a shattered garbage: (color, height as a fraction of a
/// block). It wears the garbage color until its pop, then flips: it flattens, turns to
/// its own color halfway, and grows back. `remaining` = ticks until the pop;
/// `pop_countdown` 0 means the pop is over.
fn awaking_look(remaining: i32, pop_countdown: i32, pop_color: i32, color: u8) -> (u8, f32) {
    if pop_countdown == 0 || remaining <= 0 {
        return (color, 1.0);
    }
    if remaining >= POP_ROTATE_TIME {
        return (garbage_color(pop_color, false), 1.0);
    }
    let progress = 1.0 - remaining as f32 / POP_ROTATE_TIME as f32;
    // Never flat: an empty cell would show its dot for a moment.
    let height = (1.0 - 2.0 * progress).abs().max(FLIP_MIN_HEIGHT);
    let shown = if progress < 0.5 { garbage_color(pop_color, false) } else { color };
    (shown, height)
}

/// The dots of the playable area for this rise: one per row, on the line and with the
/// glyph that land closest to the center of its cells. The playable area is the rows the
/// cursor can reach (1 to `SAFE_HEIGHT - 1`); the row above stays clear: the stack rises
/// into it and garbage arrives through it.
fn dot_lines(rise: f32) -> Vec<(i32, char)> {
    (1..SAFE_HEIGHT as i32)
        .filter_map(|row| {
            let center = bottom_unit(row, 0, rise) as f32 + (ROW_UNITS - 1) as f32 / 2.0;
            // The closest (line, glyph): in the line holding the center or the one below.
            let home = (center as i32).div_euclid(LINE_UNITS);
            let (line, glyph) = [home, home - 1]
                .into_iter()
                .flat_map(|l| (0..DOTS.len()).map(move |g| (l, g)))
                .min_by(|a, b| {
                    let off = |(l, g): (i32, usize)| ((l * LINE_UNITS) as f32 + DOT_HEIGHTS[g] - center).abs();
                    off(*a).total_cmp(&off(*b))
                })?;
            (0..TOP_UNIT / LINE_UNITS).contains(&line).then_some((line, DOTS[glyph]))
        })
        .collect()
}

/// Dot glyph for column `col` of `line`, if a cell center of the playable area falls
/// there (`dots` from `dot_lines`).
fn dot_at(dots: &[(i32, char)], col: usize, line: i32) -> Option<char> {
    if col % CELL_COLUMNS != CELL_COLUMNS / 2 || col >= PLAY_WIDTH * CELL_COLUMNS {
        return None;
    }
    dots.iter().find(|d| d.0 == line).map(|d| d.1)
}

/// The board's cells for screen line `line` (0 = just above the ground), from the
/// painted canvas: blocks, cursor, dying squares and the dots of empty cells. The line
/// just above the ground is underlined: the ground takes no line of its own.
pub(super) fn write_line(out: &mut Writer, canvas: &Canvas, line: i32) {
    let base = line * LINE_UNITS;
    let ground = line == 0;
    for col in 0..FIELD_COLUMNS {
        let (glyph, mut pen) = if let Some((glyph, color)) = canvas.mark(col, line) {
            (glyph, Pen::ink(color))
        } else {
            let units: [Option<u8>; 8] = std::array::from_fn(|u| canvas.get(col, base + u as i32));
            let dot = (units.iter().all(Option::is_none) && !canvas.no_dots.contains(&(col, line)))
                .then(|| dot_at(&canvas.dots, col, line))
                .flatten();
            match dot {
                Some(dot) => (dot, Pen::ink(DOT_COLOR)),
                None => {
                    let (lower, upper, k) = split_line(&units);
                    split_cell(lower, upper, k)
                }
            }
        };
        if ground {
            // The underline takes the text color: gray where the cell has none.
            pen.underline = true;
            pen.fg = pen.fg.or(Some(DOT_COLOR));
        }
        out.put(glyph, pen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crackattack_core::{GameState, InputSnapshot};

    /// A frame with a single block, on row 3 of column 1, and no garbage.
    fn lone_block_frame(state: u8) -> FrameData {
        let mut game = GameState::new(7);
        let mut frame = game.step(&InputSnapshot::new());
        let mut block = frame.blocks.iter().find(|b| b.grid_y == 3).unwrap().clone();
        block.grid_x = 1;
        block.state = state;
        frame.blocks = vec![block];
        frame.garbage.clear();
        frame.swapper.grid_y = 10;
        frame
    }

    /// Units of column `col` painted in `color`.
    fn painted(canvas: &Canvas, col: usize, color: u8) -> Vec<i32> {
        (LOWEST_UNIT..TOP_UNIT).filter(|&u| canvas.get(col, u) == Some(color)).collect()
    }

    #[test]
    fn the_stack_moves_by_whole_rows() {
        assert_eq!(bottom_unit(1, 0, 0.0), 0);
        assert_eq!(bottom_unit(1, 0, 0.99), 0);
        assert_eq!(bottom_unit(1, 0, 1.0), ROW_UNITS);
        assert_eq!(bottom_unit(0, 0, 0.3), -ROW_UNITS);
    }

    #[test]
    fn dots_mark_each_cell_of_the_playable_area() {
        let dots = dot_lines(0.0);
        let count = (0..TOP_UNIT / LINE_UNITS)
            .flat_map(|line| (0..FIELD_COLUMNS).map(move |col| (col, line)))
            .filter(|&(col, line)| dot_at(&dots, col, line).is_some())
            .count();
        assert_eq!(count, PLAY_WIDTH * (SAFE_HEIGHT - 1));
    }

    #[test]
    fn a_block_is_two_columns_with_a_gap_on_top() {
        let frame = lone_block_frame(0);
        let color = COLORS[frame.blocks[0].flavor as usize];
        let canvas = paint(&frame);
        let bottom = bottom_unit(3, 0, frame.play_offset_y);
        for col in [CELL_COLUMNS + 1, CELL_COLUMNS + 2] {
            assert_eq!(painted(&canvas, col, color), (bottom..bottom + ROW_UNITS - 1).collect::<Vec<_>>());
        }
        assert!(painted(&canvas, CELL_COLUMNS, color).is_empty());
    }

    #[test]
    fn a_dying_block_is_a_small_square_at_its_cell_center() {
        let mut frame = lone_block_frame(BlockState::DYING.bits());
        frame.blocks[0].animation_countdown = DYING_DELAY;
        let canvas = paint(&frame);
        let line = bottom_unit(3, 0, frame.play_offset_y) / LINE_UNITS;
        assert_eq!(canvas.mark(CELL_COLUMNS + CELL_COLUMNS / 2, line), Some(('■', DYING_COLOR)));
    }

    #[test]
    fn swapping_blocks_slide_and_cross_one_above_the_other() {
        let mut frame = lone_block_frame((BlockState::SWAPPING | BlockState::SWAP_DIRECTION).bits());
        frame.swapper.swap_factor = 0.5;
        let color = COLORS[frame.blocks[0].flavor as usize];
        let bottom = bottom_unit(3, 0, frame.play_offset_y);
        let canvas = paint(&frame);
        // Going right: halfway, in the upper part of the row.
        let col = CELL_COLUMNS + 1 + 2;
        assert_eq!(painted(&canvas, col, color), (bottom + SWAP_LIFT_UNITS..bottom + ROW_UNITS - 1).collect::<Vec<_>>());
    }

    #[test]
    fn a_shattered_garbage_block_flips_from_garbage_to_its_color() {
        let mut frame = lone_block_frame(BlockState::AWAKING.bits());
        let color = COLORS[frame.blocks[0].flavor as usize];
        frame.blocks[0].pop_color = 0;
        let col = CELL_COLUMNS + 2;
        let mut look = |remaining: i32| {
            frame.blocks[0].pop_countdown = frame.time_step + remaining;
            let canvas = paint(&frame);
            (painted(&canvas, col, GARBAGE_COLOR).len(), painted(&canvas, col, color).len())
        };
        let full = (ROW_UNITS - 1) as usize;
        assert_eq!(look(POP_ROTATE_TIME + 5), (full, 0));
        let (garbage, own) = look(POP_ROTATE_TIME / 4);
        assert!(garbage == 0 && own > 0 && own < full);
    }

    #[test]
    fn the_cursor_brackets_its_two_cells() {
        let mut game = GameState::new(7);
        let frame = game.step(&InputSnapshot::new());
        let canvas = paint(&frame);
        let x = frame.swapper.grid_x as usize;
        let line = bottom_unit(frame.swapper.grid_y, 0, frame.play_offset_y) / LINE_UNITS;
        assert_eq!(canvas.mark(x * CELL_COLUMNS, line), Some(('[', CURSOR_COLOR)));
        assert_eq!(canvas.mark((x + 2) * CELL_COLUMNS, line), Some((']', CURSOR_COLOR)));
    }
}
