//! `jnr-core`: the deterministic game simulation of Oblatenjagd.
//!
//! This crate owns all game state and advances it in fixed 1/60 s ticks.
//! It is pure: no macroquad, no floats in simulation state or logic
//! (fixed-point only), no `HashMap`/`HashSet`, no time, filesystem, threads
//! or global RNG. These rules are checked by the architecture lint in
//! `cargo xtask check`. See `docs/architecture.md`.

/// Simulation ticks per second. `Game::step` advances exactly one tick.
pub const TICKS_PER_SECOND: u32 = 60;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_runs_at_60_hz() {
        assert_eq!(TICKS_PER_SECOND, 60);
    }
}
