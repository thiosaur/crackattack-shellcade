//! The swaps worth looking at on a planning board: every legal swap that triggers a cascade,
//! with that cascade evaluated and its distance from the cursor. The strategic scan, the
//! opportunistic play and the timing play during a fall all start from this list and differ
//! only in what they value.

use super::controller::AiController;
use super::planner::{evaluate_swap, Cascade, PlanBoard, PLAN_BLOCKED, PLAN_GARBAGE};
use super::view::SwapPlan;

/// A swap that triggers something, with its simulated outcome.
pub(super) struct Candidate {
    pub(super) plan: SwapPlan,
    pub(super) cascade: Cascade,
    /// Cursor cells to walk.
    pub(super) dist: i32,
}

/// Scan the board bottom-up, left to right. `excluded` drops a swap before anything else is
/// checked (the swap to avoid, the chain's zone).
///
/// Three different questions hide behind "legal", and `legal` answers only the caller's:
/// - *possible in the simulation*: the two cells are not fixed and one holds a block (`can_swap`
///   on the plan board; what the strategic scan asks, on a settled board);
/// - *executable now*: the simulator's own check on the live game (`can_swap_now`; what the
///   plays during motion ask, because the plan board drops the moving cells);
/// - *reachable by the cursor*: the cursor stops below `SAFE`. The strategic scan does not check
///   it; a swap above that row could only be walked toward in vain. Measured never to occur
///   (journal S21, `x_reachable`), so left as is, but a behaviour change would start here.
pub(super) fn trigger_swaps(
    board: &PlanBoard,
    cursor: (i32, i32),
    excluded: impl Fn(SwapPlan) -> bool,
    legal: impl Fn(SwapPlan) -> bool,
) -> Vec<Candidate> {
    let mut found = Vec::new();
    for by in 0..board.height as i32 {
        for x in 0..board.width as i32 - 1 {
            let plan = SwapPlan { x, y: by + 1 };
            if excluded(plan) || !legal(plan) {
                continue;
            }
            let a = board.get(x as usize, by as usize);
            let c = board.get(x as usize + 1, by as usize);
            if a == c || [a, c].iter().any(|&v| v == PLAN_GARBAGE || v == PLAN_BLOCKED) {
                continue; // no-op / illegal
            }
            let cascade = evaluate_swap(board, x as usize, by as usize);
            if cascade.chain_depth == 0 {
                continue; // triggers nothing
            }
            let dist = (x - cursor.0).abs() + (plan.y - cursor.1).abs();
            found.push(Candidate { plan, cascade, dist });
        }
    }
    found
}

/// Boards whose triggering swaps are kept: the strategic scan, the plays during motion and the
/// retries of `plan_meanwhile` look at a few boards, the same ones tick after tick while the
/// board falls or wakes up.
const CACHED_BOARDS: usize = 4;

/// The part of `trigger_swaps` that depends on the board alone: every swap that triggers a
/// cascade, evaluated, in scan order. The caller's filters and the distance are applied on
/// reading. Purely an optimisation, like `Memo`: the boards are compared whole, so the result
/// is the one a fresh scan would give.
pub(super) struct SwapCache {
    /// Most recently used first.
    boards: Vec<(PlanBoard, Vec<(SwapPlan, Cascade)>)>,
}

impl SwapCache {
    pub(super) fn new() -> Self {
        Self { boards: Vec::new() }
    }

    fn swaps(&mut self, board: &PlanBoard) -> &[(SwapPlan, Cascade)] {
        match self.boards.iter().position(|(b, _)| b == board) {
            Some(i) => self.boards[..=i].rotate_right(1),
            None => {
                let swaps = trigger_swaps(board, (0, 0), |_| false, |_| true)
                    .into_iter()
                    .map(|c| (c.plan, c.cascade))
                    .collect();
                self.boards.truncate(CACHED_BOARDS - 1);
                self.boards.insert(0, (board.clone(), swaps));
            }
        }
        &self.boards[0].1
    }
}

impl AiController {
    /// `trigger_swaps`, read from the swap cache.
    pub(super) fn trigger_swaps(
        &self,
        board: &PlanBoard,
        cursor: (i32, i32),
        excluded: impl Fn(SwapPlan) -> bool,
        legal: impl Fn(SwapPlan) -> bool,
    ) -> Vec<Candidate> {
        let mut cache = self.swap_cache.borrow_mut();
        cache
            .swaps(board)
            .iter()
            .filter(|(plan, _)| !excluded(*plan) && legal(*plan))
            .map(|&(plan, cascade)| {
                let dist = (plan.x - cursor.0).abs() + (plan.y - cursor.1).abs();
                Candidate { plan, cascade, dist }
            })
            .collect()
    }
}
