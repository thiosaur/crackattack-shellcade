//! The cascade simulator: gravity, runs of three, shattering, repeated until stable. A pure
//! function of the board; it approximates the engine (see the module doc of `planner`).

use crate::constants::PLAY_WIDTH;

use super::board::*;

/// The outcome of simulating one candidate swap's cascade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cascade {
    /// Match rounds triggered (about the combo multiplier; round 1 isn't a chain link).
    pub chain_depth: i32,
    /// Total blocks eliminated across all rounds (about the combo magnitude).
    pub total_cleared: i32,
    /// Largest single round's elimination (drives width garbage).
    pub max_round: i32,
    /// Garbage cells shattered.
    pub garbage_shattered: i32,
}

/// Settle every block straight down onto the floor / garbage / the block below.
pub(super) fn apply_gravity(b: &mut PlanBoard) {
    for column in b.cell.chunks_exact_mut(b.height) {
        // `write` is the next free row from the bottom of the current segment.
        // Garbage and blocked cells are fixed (blocks rest on them), so they reset the segment.
        let mut write = 0;
        for y in 0..column.len() {
            let v = column[y];
            if v == PLAN_GARBAGE || v == PLAN_BLOCKED {
                write = y + 1; // blocks above land on top of the garbage
            } else if v >= 0 {
                if write != y {
                    column[write] = v;
                    column[y] = PLAN_EMPTY;
                }
                write += 1;
            }
            // empty cells are skipped; their slot is reused by a falling block above
        }
    }
}

/// The board once everything in the air has landed (nothing matched yet).
pub(crate) fn settled(board: &PlanBoard) -> PlanBoard {
    let mut b = board.clone();
    apply_gravity(&mut b);
    b
}

/// Find every cell in a 3+ run (horizontal or vertical) of matching flavours.
/// Marks them in `marked` (a mask indexed by `x*height+y`, of the board's size, cleared here)
/// and returns their count and the largest single run.
pub(super) fn find_matches(b: &PlanBoard, marked: &mut [bool]) -> (i32, i32) {
    marked.fill(false);
    let (w, h, cell) = (b.width, b.height, &b.cell[..]);
    // Two neighbours of one run: blocks of the same flavour, gray excepted (`flavor_match`).
    let joins = |p: i16, q: i16| p >= 0 && q >= 0 && flavor_match(p, q);
    let mut count = 0;
    let mut largest = 0;
    // The run ending before `end` (exclusive), `run` cells long, every `step` cells.
    let mut close = |end: usize, run: usize, step: usize, marked: &mut [bool]| {
        if run >= 3 {
            largest = largest.max(run as i32);
            for k in 1..=run {
                let idx = end - k * step;
                if !marked[idx] {
                    marked[idx] = true;
                    count += 1;
                }
            }
        }
    };
    // Horizontal runs.
    for y in 0..h {
        let mut run = 1;
        for x in 1..w {
            if joins(cell[x * h + y], cell[(x - 1) * h + y]) {
                run += 1;
            } else {
                close(x * h + y, run, h, marked);
                run = 1;
            }
        }
        close(w * h + y, run, h, marked);
    }
    // Vertical runs.
    for x in 0..w {
        let column = x * h;
        let mut run = 1;
        for y in 1..h {
            if joins(cell[column + y], cell[column + y - 1]) {
                run += 1;
            } else {
                close(column + y, run, 1, marked);
                run = 1;
            }
        }
        close(column + h, run, 1, marked);
    }
    (count, largest)
}

/// Remove garbage cells 4-adjacent to any matched cell; return how many. Removed on the spot:
/// whether a cell touches a match does not depend on the garbage already removed.
pub(super) fn shatter_adjacent(b: &mut PlanBoard, marked: &[bool]) -> i32 {
    let mut shattered = 0;
    let h = b.height;
    for x in 0..b.width {
        for y in 0..h {
            if b.get(x, y) != PLAN_GARBAGE {
                continue;
            }
            let touches = (x > 0 && marked[(x - 1) * h + y])
                || (x + 1 < b.width && marked[(x + 1) * h + y])
                || (y > 0 && marked[x * h + (y - 1)])
                || (y + 1 < h && marked[x * h + (y + 1)]);
            if touches {
                b.cell[x * h + y] = PLAN_EMPTY;
                shattered += 1;
            }
        }
    }
    shattered
}

/// Run the cascade on `b` (mutated): settle gravity, clear 3+ runs, repeat until stable.
/// A board that triggers nothing returns an all-zero cascade.
pub(super) fn evaluate_cascade(b: &mut PlanBoard) -> Cascade {
    evaluate_cascade_in(b, &mut vec![false; b.cell.len()])
}

/// `evaluate_cascade` with the match mask given (the board's size; its contents do not matter).
fn evaluate_cascade_in(b: &mut PlanBoard, marked: &mut [bool]) -> Cascade {
    let mut result = Cascade::default();
    loop {
        apply_gravity(b);
        let (count, largest) = find_matches(b, marked);
        if count == 0 {
            break;
        }
        result.chain_depth += 1;
        result.total_cleared += count;
        result.max_round = result.max_round.max(largest);
        result.garbage_shattered += shatter_adjacent(b, marked);
        for (idx, &m) in marked.iter().enumerate() {
            if m {
                b.cell[idx] = PLAN_EMPTY;
            }
        }
    }
    result
}

/// Settle the board without a swap and report the cascade gravity alone creates.
pub(crate) fn evaluate_gravity(board: &PlanBoard) -> Cascade {
    evaluate_cascade(&mut board.clone())
}

/// Simulate the cascade after swapping `(x,y)`<->`(x+1,y)` on a copy of `board`.
/// Returns the chain depth, blocks cleared, largest round, and garbage shattered.
pub(crate) fn evaluate_swap(board: &PlanBoard, x: usize, y: usize) -> Cascade {
    let mut b = board.clone();
    let tmp = b.get(x, y);
    b.set(x, y, b.get(x + 1, y));
    b.set(x + 1, y, tmp);
    evaluate_cascade(&mut b)
}

/// Attack value of a cascade, in units of "garbage rows sent", per the generator: each
/// chain link past the first ships a full-width row (`multiplier - 1`), and a combo whose
/// total exceeds 3 ships width garbage (about `total - 3` cells, a fraction of a row).
/// Chains dominate, exactly as in real play.
pub(crate) fn attack_value(c: &Cascade) -> i32 {
    let chain_rows = (c.chain_depth - 1).max(0);
    let width_cells = (c.total_cleared - 3).max(0);
    chain_rows * PLAY_WIDTH as i32 + width_cells
}

/// Whether swapping `(x,y)`<->`(x+1,y)` immediately completes a 3+ run, checked statically
/// against post-swap flavours (base-flavour matching). On a settled board (which a
/// `readPlanBoard` snapshot always is, and which gravity-neutral setup swaps preserve) this
/// is **exact** for a cascade's first round when the swap doesn't drop a block, so it
/// serves as a cheap prefilter that lets the full [`evaluate_swap`] run only on genuine
/// trigger candidates.
pub(super) fn makes_run3(b: &PlanBoard, x: usize, y: usize) -> bool {
    let va = b.get(x, y);
    let vb = b.get(x + 1, y);
    // Post-swap value at a cell (the two swapped cells exchange).
    let val = |cx: usize, cy: usize| -> i16 {
        if cy == y && cx == x {
            vb
        } else if cy == y && cx == x + 1 {
            va
        } else {
            b.get(cx, cy)
        }
    };
    let matches = |v: i16, base: i16| v >= 0 && map_flavor_to_base_flavor(v) == base;

    for (ax, v) in [(x, vb), (x + 1, va)] {
        if v < 0 {
            continue; // the cell became empty: anchors nothing
        }
        let base = map_flavor_to_base_flavor(v);
        let mut run = 1;
        let mut cx = ax;
        while cx > 0 && matches(val(cx - 1, y), base) {
            run += 1;
            cx -= 1;
        }
        cx = ax + 1;
        while cx < b.width && matches(val(cx, y), base) {
            run += 1;
            cx += 1;
        }
        if run >= 3 {
            return true;
        }
        run = 1;
        let mut cy = y;
        while cy > 0 && matches(val(ax, cy - 1), base) {
            run += 1;
            cy -= 1;
        }
        cy = y + 1;
        while cy < b.height && matches(val(ax, cy), base) {
            run += 1;
            cy += 1;
        }
        if run >= 3 {
            return true;
        }
    }
    false
}
