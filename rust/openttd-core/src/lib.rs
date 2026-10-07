//! Rust game logic for OpenTTD that does not depend on the C++ code base.
//!
//! Everything here can be tested with `cargo test` alone. Code that can affect
//! the game state must be deterministic: see the Rust section of `CLAUDE.md`.

pub mod math;
