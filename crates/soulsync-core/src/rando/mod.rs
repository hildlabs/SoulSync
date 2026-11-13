//! SoulSync's own randomizer, in the making.
//!
//! A port of Universal Pokémon Randomizer ZX 4.6.1 (GPL-3.0) for the games
//! of generation 5. It keeps to the original's order of steps and draws
//! random numbers the same way, so that with the same seed both make the
//! same game; that is how each ported step is checked.

pub mod gen5;
pub mod lz;
pub mod random;
pub mod text;
