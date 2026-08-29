//! Scale Sim world generation.
//!
//! Pipeline steps 1-3 of the design doc: elevation, independent climate
//! fields, and biomes that emerge from how those fields interact. Nothing
//! past biome classification lives here yet.

pub mod field;
pub mod noise;
pub mod rng;
pub mod world;
