//! Scale Sim world generation.
//!
//! Pipeline steps 1-3 of the design doc: elevation, independent climate
//! fields, and biomes that emerge from how those fields interact. Nothing
//! past biome classification lives here yet.

pub mod econ;
pub mod field;
pub mod geology;
pub mod hydrology;
pub mod network;
pub mod noise;
pub mod polity;
pub mod rng;
pub mod settlement;
pub mod slice;
pub mod world;
