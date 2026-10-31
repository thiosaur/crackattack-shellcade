// From the fine canvas to screen cells: each cell shows 8 vertical units with at most
// two colors, split at an eighth (▁▂▃▄▅▆▇█).

use shellcade_kit::prelude::{Cell, Frame, ATTR_BOLD, ATTR_REVERSE, ATTR_UNDERLINE};

use super::board::{FIELD_COLUMNS, LOWEST_UNIT, TOP_UNIT};
use super::palette::{rgb, tint, CURSOR_COLOR};

/// Eighth blocks, from 1/8 to 7/8 of the height, drawn from the bottom.
pub(super) const LOWER_EIGHTHS: [char; 7] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇'];
/// Weight of a cursor unit when a cell picks its two colors (a whole line's worth).
const CURSOR_WEIGHT: usize = 8;

/// Fine paint buffer: a color (or nothing) per screen column and vertical unit of the
/// board.
pub(super) struct Canvas {
    cells: Vec<Option<u8>>,
    /// Glyphs drawn over the board at (column, line), in a color: dying blocks and the
    /// cursor brackets.
    pub(super) marks: Vec<(usize, i32, char, u8)>,
    /// Dot places (column, line) to leave blank: the cells of a swap, which a sliding
    /// block crosses.
    pub(super) no_dots: Vec<(usize, i32)>,
    /// The dots of the playable area: (line, glyph), one per row.
    pub(super) dots: Vec<(i32, char)>,
}

impl Canvas {
    pub(super) fn new() -> Self {
        Self {
            cells: vec![None; FIELD_COLUMNS * (TOP_UNIT - LOWEST_UNIT) as usize],
            marks: Vec::new(),
            no_dots: Vec::new(),
            dots: Vec::new(),
        }
    }

    pub(super) fn mark(&self, col: usize, line: i32) -> Option<(char, u8)> {
        self.marks.iter().find(|m| m.0 == col && m.1 == line).map(|m| (m.2, m.3))
    }

    pub(super) fn get(&self, col: usize, unit: i32) -> Option<u8> {
        if col >= FIELD_COLUMNS || !(LOWEST_UNIT..TOP_UNIT).contains(&unit) {
            return None;
        }
        self.cells[(unit - LOWEST_UNIT) as usize * FIELD_COLUMNS + col]
    }

    /// Paints columns `cols` over units `[bottom, top)`, clipped to the canvas.
    pub(super) fn fill(&mut self, cols: std::ops::RangeInclusive<usize>, bottom: i32, top: i32, color: u8) {
        for unit in bottom.max(LOWEST_UNIT)..top.min(TOP_UNIT) {
            let row = (unit - LOWEST_UNIT) as usize * FIELD_COLUMNS;
            for col in cols.clone().filter(|&col| col < FIELD_COLUMNS) {
                self.cells[row + col] = Some(color);
            }
        }
    }
}

/// One cell for 8 stacked units (`units[0]` at the bottom): the split (lower color,
/// upper color, height of the lower part in eighths) that matches the most units.
pub(super) fn split_line(units: &[Option<u8>; 8]) -> (Option<u8>, Option<u8>, usize) {
    // Most cells are one color, or two (a block under its gap): the split is exact.
    let k = units.iter().position(|u| *u != units[0]).unwrap_or(8);
    if units[k..].iter().all(|u| *u == units[k.min(7)]) {
        return if k == 8 { (units[0], units[0], 8) } else { (units[0], units[k], k) };
    }
    // The cursor counts more than anything else.
    let weight = |u: &Option<u8>| if *u == Some(CURSOR_COLOR) { CURSOR_WEIGHT } else { 1 };
    let most_common = |part: &[Option<u8>]| -> Option<u8> {
        let mut best = (0, None);
        for candidate in part {
            let n: usize = part.iter().filter(|u| *u == candidate).map(weight).sum();
            if n > best.0 {
                best = (n, *candidate);
            }
        }
        best.1
    };
    let mut best = (usize::MAX, None, None, 0);
    for k in 0..=8 {
        let lower = most_common(&units[..k]);
        let upper = most_common(&units[k..]);
        let errors: usize = units[..k].iter().filter(|u| **u != lower).map(weight).sum::<usize>()
            + units[k..].iter().filter(|u| **u != upper).map(weight).sum::<usize>();
        if errors < best.0 {
            best = (errors, lower, upper, k);
        }
    }
    (best.1, best.2, best.3)
}

/// How a cell is drawn: colors (256-color indices, `None` = terminal default) and
/// attributes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Pen {
    pub fg: Option<u8>,
    pub bg: Option<u8>,
    pub reverse: bool,
    pub bold: bool,
    pub underline: bool,
}

impl Pen {
    pub(super) fn ink(fg: u8) -> Self {
        Self { fg: Some(fg), ..Self::default() }
    }

    pub(super) fn bold(fg: Option<u8>) -> Self {
        Self { fg, bold: true, ..Self::default() }
    }

    fn cell(self, rune: char, saturation: f32) -> Cell {
        let attr = [(self.bold, ATTR_BOLD), (self.reverse, ATTR_REVERSE), (self.underline, ATTR_UNDERLINE)]
            .iter()
            .filter(|(on, _)| *on)
            .fold(0, |attr, (_, bit)| attr | bit);
        let color = |c: Option<u8>| c.map_or(Default::default(), |c| tint(rgb(c), saturation));
        Cell { rune, fg: color(self.fg), bg: color(self.bg), attr, ..Cell::default() }
    }
}

/// Writes cells on one line of the frame, left to right.
pub(super) struct Writer<'a> {
    frame: &'a mut Frame,
    row: i32,
    pub(super) col: i32,
    /// Colors' saturation: 1 as they are, less for a muted board (see `MUTED`).
    pub(super) saturation: f32,
}

impl<'a> Writer<'a> {
    pub(super) fn new(frame: &'a mut Frame, row: i32, col: i32) -> Self {
        Self { frame, row, col, saturation: 1.0 }
    }

    pub(super) fn put(&mut self, rune: char, pen: Pen) {
        self.frame.set(self.row, self.col, pen.cell(rune, self.saturation));
        self.col += 1;
    }

    pub(super) fn text(&mut self, text: &str, pen: Pen) {
        for rune in text.chars() {
            self.put(rune, pen);
        }
    }
}

/// The pen of one cell from its split: two colors at most, foreground for the lower
/// part. An empty upper part over a color takes reverse video, so the glyph (lower part)
/// is drawn in the terminal background.
pub(super) fn split_cell(lower: Option<u8>, upper: Option<u8>, k: usize) -> (char, Pen) {
    let (lower, upper, k) = match (lower, upper) {
        // One color over the whole height.
        _ if k == 0 => (upper, upper, 8),
        _ if k == 8 || lower == upper => (lower, lower, 8),
        _ => (lower, upper, k),
    };
    match (lower, upper) {
        (None, None) => (' ', Pen::default()),
        (Some(c), Some(u)) if k == 8 || c == u => ('█', Pen::ink(c)),
        (Some(c), Some(u)) => (LOWER_EIGHTHS[k - 1], Pen { bg: Some(u), ..Pen::ink(c) }),
        (Some(c), None) => (LOWER_EIGHTHS[k - 1], Pen::ink(c)),
        (None, Some(u)) => (LOWER_EIGHTHS[k - 1], Pen { reverse: true, ..Pen::ink(u) }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uniform_line_is_one_full_color() {
        assert_eq!(split_line(&[Some(3); 8]).2 % 8, 0);
        let (lower, upper, k) = split_line(&[None; 8]);
        assert_eq!((lower, upper), (None, None));
        assert!(k == 0 || k == 8);
    }

    #[test]
    fn a_boundary_splits_at_the_right_eighth() {
        let mut units = [None; 8];
        for u in units.iter_mut().take(3) {
            *u = Some(5);
        }
        assert_eq!(split_line(&units), (Some(5), None, 3));
        let mut two = [Some(1); 8];
        for u in two.iter_mut().skip(6) {
            *u = Some(2);
        }
        assert_eq!(split_line(&two), (Some(1), Some(2), 6));
    }

    #[test]
    fn an_empty_lower_part_takes_reverse_video() {
        assert_eq!(split_cell(Some(5), Some(5), 3), ('█', Pen::ink(5)));
        assert_eq!(split_cell(None, Some(9), 3), ('▃', Pen { reverse: true, ..Pen::ink(9) }));
        assert_eq!(split_cell(Some(5), None, 7).0, '▇');
    }
}
