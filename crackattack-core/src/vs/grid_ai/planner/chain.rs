//! Chain preparation: one swap after which a worth-firing cascade is a trigger swap away.

use super::board::*;
use super::cascade::*;

/// What a chain-enabling setup swap buys: the swap plus the enabled payoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChainSetupPlan {
    /// The setup swap `(x,y)`<->`(x+1,y)`, in plan coordinates.
    pub x: usize,
    pub y: usize,
    /// Score of the best cascade this setup enables (attack value + shatter bonus).
    pub score: i32,
}

/// Options of [`plan_chain_setup`].
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ChainSetupOptions {
    /// Minimum chain depth for an enabled cascade to count.
    pub min_chain: i32,
    /// Minimum single-round run for an enabled cascade to count.
    pub min_run: i32,
    /// Score bonus per garbage cell shattered.
    pub shatter_weight: i32,
    /// Also search three-swap constructions (setup, setup, trigger).
    pub lookahead: bool,
    /// Optional seed ranking otherwise equal choices.
    pub tie_seed: Option<u32>,
}

/// The offensive counterpart of [`plan_shatter_setup`]: a bounded two-ply search for one
/// **setup swap** that fires nothing itself but *enables* a worth-firing cascade one swap
/// later. This is how chains are *built* rather than merely noticed: arrange the board so
/// that a single trigger swap starts a cascade whose falls feed further matches.
///
/// Setup swaps are lateral block<->block exchanges (gravity-neutral, so the board stays
/// settled and plans survive re-planning) that complete no run of their own: a swap that
/// clears is a clear, owned by the fire/survival branches. For each candidate setup,
/// trigger swaps are scanned with the static [`makes_run3`] prefilter (exact on a settled
/// board); a block dropped into a fall-through gap can also trigger via gravity, so those
/// few candidates get the full cascade evaluation directly. An enabled cascade counts when
/// it meets the caller's fire thresholds (chain depth / run length), the same bar the fire
/// branch uses, so the enabled trigger is guaranteed to be taken on a later action tick
/// once the setup lands.
///
/// Incremental depth for free: after the setup executes, re-planning either fires the
/// enabled trigger or discovers a *further* enabler, so multi-swap constructions emerge
/// from repeated one-swap planning without a deeper search. With `opts.lookahead`, a
/// **second level** kicks in only when no single enabler exists (about 87% of bank
/// positions, measured): scan setup swaps whose result *contains* a single enabler, a
/// three-swap construction (setup, setup, trigger). The same monotone ladder guarantees
/// progress: a 2-level plan becomes a 1-level plan after its first swap, then a fire.
/// Deterministic: best score wins; an optional seed ranks ties without drawing gameplay
/// RNG. Pure function of the board and options, with no timing.
pub(crate) fn plan_chain_setup(board: &PlanBoard, opts: &ChainSetupOptions) -> Option<ChainSetupPlan> {
    let direct = search_enabler(board, opts);
    if direct.is_some() || !opts.lookahead {
        return direct;
    }
    Lookahead::default().resume(board, opts, usize::MAX).0.flatten()
}

/// The deeper level of [`plan_chain_setup`] in progress, for a caller that spreads it over
/// several calls: the first swaps tried so far and the best one found.
///
/// No single enabler exists: look one level deeper. The candidate move is a setup swap after
/// which a single enabler *does* exist; it inherits that enabler's eventual score. Only runs in
/// enabler-less positions, and the run3 prefilter keeps the inner searches cheap.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Lookahead {
    /// The next first swap to try, as `y * (width - 1) + x`.
    next: usize,
    best: Option<ChainSetupPlan>,
    best_rank: u32,
}

impl Lookahead {
    /// Go on until `budget` work is spent (see `counted_search_enabler`; an inner search started
    /// is finished): the answer once every first swap has been tried (`None` inside when none
    /// enables anything), and the work spent.
    pub(crate) fn resume(
        &mut self,
        board: &PlanBoard,
        opts: &ChainSetupOptions,
        budget: usize,
    ) -> (Option<Option<ChainSetupPlan>>, usize) {
        let mut spent = 0;
        let mut b2 = board.clone();
        let row = board.width - 1;
        while self.next < board.height * row {
            let (sx, sy) = (self.next % row, self.next / row);
            let a = board.get(sx, sy);
            let c = board.get(sx + 1, sy);
            let setup = a >= 0 && c >= 0 // setups are block<->block only
                && map_flavor_to_base_flavor(a) != map_flavor_to_base_flavor(c) // no-op
                && !makes_run3(board, sx, sy); // that's a clear, not a setup
            if !setup {
                self.next += 1;
                continue;
            }
            if spent >= budget {
                return (None, spent);
            }
            self.next += 1;
            b2.cell.copy_from_slice(&board.cell);
            b2.set(sx, sy, c);
            b2.set(sx + 1, sy, a);
            let enabled = counted_search_enabler(&b2, opts, &mut spent);
            let rank = seeded_rank(opts.tie_seed, &[0x4c4f4f4b, sx, sy]);
            if let Some(enabled) = enabled {
                let better = match &self.best {
                    None => true,
                    Some(b) => {
                        enabled.score > b.score
                            || (enabled.score == b.score && opts.tie_seed.is_some() && rank > self.best_rank)
                    }
                };
                if better {
                    self.best = Some(ChainSetupPlan {
                        x: sx,
                        y: sy,
                        score: enabled.score,
                    });
                    self.best_rank = rank;
                }
            }
        }
        (Some(self.best), spent)
    }
}

/// Work of a full cascade simulation, in trigger positions examined (measured about ten times
/// as long).
const SIMULATION_WORK: usize = 10;

/// The single-enabler search behind [`plan_chain_setup`] (one setup + trigger).
pub(crate) fn search_enabler(board: &PlanBoard, opts: &ChainSetupOptions) -> Option<ChainSetupPlan> {
    counted_search_enabler(board, opts, &mut 0)
}

/// `search_enabler`, adding its work to `work`: trigger positions examined, and
/// `SIMULATION_WORK` per cascade simulated.
fn counted_search_enabler(board: &PlanBoard, opts: &ChainSetupOptions, work: &mut usize) -> Option<ChainSetupPlan> {
    let mut best: Option<ChainSetupPlan> = None;
    let mut best_rank = 0u32;
    let mut b2 = board.clone();

    for sy in 0..board.height {
        for sx in 0..board.width - 1 {
            let a = board.get(sx, sy);
            let c = board.get(sx + 1, sy);
            if a < 0 || c < 0 {
                continue; // setups are block<->block only
            }
            if map_flavor_to_base_flavor(a) == map_flavor_to_base_flavor(c) {
                continue; // no-op
            }
            if makes_run3(board, sx, sy) {
                continue; // that's a clear, not a setup
            }
            // Apply the setup on the scratch board (restored from `board` each time).
            b2.cell.copy_from_slice(&board.cell);
            b2.set(sx, sy, c);
            b2.set(sx + 1, sy, a);

            for ty in 0..b2.height {
                for tx in 0..b2.width - 1 {
                    *work += 1;
                    if tx == sx && ty == sy {
                        continue; // undoing the setup is pointless
                    }
                    if !can_swap(&b2, tx, ty) {
                        continue;
                    }
                    let ta = b2.get(tx, ty);
                    let tc = b2.get(tx + 1, ty);
                    if ta >= 0 && tc >= 0 && map_flavor_to_base_flavor(ta) == map_flavor_to_base_flavor(tc) {
                        continue; // no-op
                    }
                    // A block swapped over a fall-through gap triggers via gravity, which
                    // the static prefilter can't see: evaluate those in full.
                    let fall_through = (ta >= 0 && tc < 0 && ty > 0 && b2.get(tx + 1, ty - 1) < 0)
                        || (tc >= 0 && ta < 0 && ty > 0 && b2.get(tx, ty - 1) < 0);
                    if !fall_through && !makes_run3(&b2, tx, ty) {
                        continue;
                    }
                    *work += SIMULATION_WORK;
                    let cas = evaluate_swap(&b2, tx, ty);
                    if cas.chain_depth < opts.min_chain && cas.max_round < opts.min_run {
                        continue;
                    }
                    let score = attack_value(&cas) + cas.garbage_shattered * opts.shatter_weight;
                    let rank = seeded_rank(opts.tie_seed, &[0x43484149, sx, sy]);
                    let better = match &best {
                        None => true,
                        Some(b) => score > b.score || (score == b.score && opts.tie_seed.is_some() && rank > best_rank),
                    };
                    if better {
                        best = Some(ChainSetupPlan { x: sx, y: sy, score });
                        best_rank = rank;
                    }
                }
            }
        }
    }
    best
}
