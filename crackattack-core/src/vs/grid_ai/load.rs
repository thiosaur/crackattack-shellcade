//! The AI's load: time per decision over hard-against-hard duels, and a fingerprint of every
//! decision (an optimisation that must not change the play keeps it).

use crate::ai_difficulty::AIDifficulty;
use crate::input::InputSnapshot;
use crate::vs::{GridOpponent, SessionPhase, VsSession};
use std::time::{Duration, Instant};

use super::controller::{decision_seed, AiController};
use super::tuning::Adaptive;
use super::GridAutoplay;

/// `cargo test --release -p crackattack-core ai_load -- --ignored --nocapture`; `LOAD_SEEDS`
/// duels (default 10), `LOAD_LEVEL=easy|medium|hard` (default hard), `LOAD_TUNING` overrides
/// (`name=value,...`) for both players.
#[test]
#[ignore]
fn ai_load() {
    let seeds: u32 = std::env::var("LOAD_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
    let level = match std::env::var("LOAD_LEVEL").as_deref() {
        Ok("easy") => AIDifficulty::Easy,
        Ok("medium") => AIDifficulty::Medium,
        _ => AIDifficulty::Hard,
    };
    let spec = std::env::var("LOAD_TUNING").unwrap_or_default();
    let mut adaptive = Adaptive::for_difficulty(level);
    adaptive.survival = adaptive.survival.with_overrides(&spec).unwrap();
    adaptive.aggressive = adaptive.aggressive.with_overrides(&spec).unwrap();
    let mut calls: Vec<Duration> = Vec::new();
    let mut fingerprint: u64 = 0xcbf2_9ce4_8422_2325;
    for seed in 1..=seeds {
        let mut session = VsSession::new(seed, GridOpponent::with_adaptive(level, adaptive, seed + 1000));
        let mut ai = GridAutoplay { controller: AiController::with_adaptive(adaptive, Some(decision_seed(seed, 1))) };
        let mut prev = 0u8;
        for _ in 0..50 * 300 {
            let mut input = InputSnapshot::new();
            if session.phase() == SessionPhase::Playing {
                let start = Instant::now();
                input.state = ai.update(&session.player_game);
                calls.push(start.elapsed());
                fingerprint = (fingerprint ^ input.state as u64).wrapping_mul(0x100_0000_01b3);
            }
            input.pressed_this_frame = input.state & !prev;
            input.prev_state = prev;
            prev = input.state;
            session.time_step(&input);
            if !matches!(session.phase(), SessionPhase::Playing | SessionPhase::Countdown) {
                break;
            }
        }
    }
    let total: Duration = calls.iter().sum();
    calls.sort();
    let pct = |p: f64| calls[((calls.len() as f64 - 1.0) * p) as usize];
    let over = |us: u128| calls.iter().filter(|d| d.as_micros() > us).count();
    println!("{} {spec}: decisions fingerprint {fingerprint:016x}", level.name());
    println!(
        "ticks {}  mean {:?}  p50 {:?}  p99 {:?}  p99.9 {:?}  max {:?}  over 1ms {}  over 0.5ms {}",
        calls.len(),
        total / calls.len() as u32,
        pct(0.5),
        pct(0.99),
        pct(0.999),
        calls[calls.len() - 1],
        over(1000),
        over(500)
    );
}
