//! Scale Sim world generation.
//!
//! Pipeline steps 1-3 of the design doc: elevation, independent climate
//! fields, and biomes that emerge from how those fields interact. Nothing
//! past biome classification lives here yet.

pub mod biota;
pub mod building;
pub mod econ;
pub mod field;
pub mod geology;
pub mod ground;
pub mod hydrology;
pub mod id;
pub mod infrastructure;
pub mod labour;
pub mod locality;
pub mod logistics;
pub mod memory;
pub mod mind;
pub mod money;
pub mod network;
pub mod noise;
pub mod person;
pub mod polity;
pub mod populace;
pub mod region;
pub mod rng;
pub mod services;
pub mod settlement;
pub mod state;
pub mod slice;
pub mod townplan;
pub mod travel;
pub mod vehicle;
pub mod world;
