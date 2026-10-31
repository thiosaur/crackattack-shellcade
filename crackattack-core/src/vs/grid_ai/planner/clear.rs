//! A plain clear a few swaps away: when no single swap clears anything, the shortest sequence of
//! swaps after which one does. Nothing is fired for its value; this is the "do something
//! useful" fallback for a board that is stuck (the stack is high and the board cannot be raised).

use super::board::*;
use super::cascade::*;
use super::garbage::SetupPlan;

/// The legal swaps of a board that change it: `(x, y)` in plan coordinates.
fn movable_swaps(board: &PlanBoard) -> Vec<(usize, usize)> {
    let mut swaps = Vec::new();
    for y in 0..board.height {
        for x in 0..board.width - 1 {
            if can_swap(board, x, y) && board.get(x, y) != board.get(x + 1, y) {
                swaps.push((x, y));
            }
        }
    }
    swaps
}

/// `board` after the swap, with everything in the air landed.
fn after_swap(board: &PlanBoard, x: usize, y: usize) -> PlanBoard {
    let mut b = board.clone();
    let tmp = b.get(x, y);
    b.set(x, y, b.get(x + 1, y));
    b.set(x + 1, y, tmp);
    settled(&b)
}

/// Work of a full cascade simulation, in swaps examined (measured about ten times as long).
const SIMULATION_WORK: usize = 10;

/// The most blocks a last swap clears from `board` (0 when none triggers anything), adding its
/// work to `work` (swaps examined, `SIMULATION_WORK` per simulation). The board is settled
/// (`after_swap`). If it holds no run either, a swap of two blocks moves nothing else and
/// clears only through a run it completes itself, which `makes_run3` sees: only the swaps that
/// pass it, and those with an empty cell (a block may fall), are simulated.
fn best_clear(board: &PlanBoard, work: &mut usize) -> i32 {
    debug_assert_eq!(*board, settled(board));
    let quiet = find_matches(board, &mut vec![false; board.cell.len()]).0 == 0;
    let mut best = 0;
    for (x, y) in movable_swaps(board) {
        *work += 1;
        if quiet && board.get(x, y) >= 0 && board.get(x + 1, y) >= 0 && !makes_run3(board, x, y) {
            continue;
        }
        *work += SIMULATION_WORK;
        best = best.max(evaluate_swap(board, x, y).total_cleared);
    }
    best
}

/// The first swap of the shortest sequence (at most `max_swaps` swaps, the last one clearing
/// something) on a board where no single swap clears. Among sequences of the same length, the one
/// that clears most; equal ones are separated by the seeded rank. `cost` is the sequence length.
pub(crate) fn plan_clear_setup(board: &PlanBoard, max_swaps: i32, tie_seed: Option<u32>) -> Option<SetupPlan> {
    ClearSearch::default().resume(board, max_swaps, tie_seed, usize::MAX).0.flatten()
}

/// [`plan_clear_setup`] in progress, for a caller that spreads it over several calls: the
/// sequences of two swaps are searched at once, those of three from one first swap to the next.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ClearSearch {
    /// Whether the two-swap level found nothing (the three-swap one is under way).
    deeper: bool,
    /// The next first swap of the three-swap level, as an index in the board's movable swaps.
    next: usize,
    best: Option<(i32, u32, (usize, usize))>,
}

impl ClearSearch {
    /// Go on until `budget` work is spent (see `best_clear`; a first swap started is finished):
    /// the answer once done, and the work spent.
    pub(crate) fn resume(
        &mut self,
        board: &PlanBoard,
        max_swaps: i32,
        tie_seed: Option<u32>,
        budget: usize,
    ) -> (Option<Option<SetupPlan>>, usize) {
        let mut spent = 0;
        let first_swaps = movable_swaps(board);
        let consider = |best: &mut Option<(i32, u32, (usize, usize))>, cleared: i32, first: (usize, usize)| {
            let rank = seeded_rank(tie_seed, &[first.0, first.1, 0x434c_5352]);
            if best.map_or(true, |(c, r, _)| cleared > c || (cleared == c && rank < r)) {
                *best = Some((cleared, rank, first));
            }
        };
        if !self.deeper {
            if max_swaps >= 2 {
                for &(x, y) in &first_swaps {
                    let cleared = best_clear(&after_swap(board, x, y), &mut spent);
                    if cleared > 0 {
                        consider(&mut self.best, cleared, (x, y));
                    }
                }
                if let Some((_, _, (x, y))) = self.best {
                    return (Some(Some(SetupPlan { x, y, cost: 2 })), spent);
                }
            }
            if max_swaps < 3 {
                return (Some(None), spent);
            }
            self.deeper = true;
        }
        while let Some(&first) = first_swaps.get(self.next) {
            if spent >= budget {
                return (None, spent);
            }
            self.next += 1;
            let b1 = after_swap(board, first.0, first.1);
            for (x, y) in movable_swaps(&b1) {
                let cleared = best_clear(&after_swap(&b1, x, y), &mut spent);
                if cleared > 0 {
                    consider(&mut self.best, cleared, first);
                }
            }
        }
        (Some(self.best.map(|(_, _, (x, y))| SetupPlan { x, y, cost: 3 })), spent)
    }
}
