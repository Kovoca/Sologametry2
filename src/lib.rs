//! Scale Sim world generation.
//!
//! Pipeline steps 1-3 of the design doc: elevation, independent climate
//! fields, and biomes that emerge from how those fields interact. Nothing
//! past biome classification lives here yet.

pub mod bank;
pub mod basket;
pub mod befall;
pub mod biota;
pub mod bom;
pub mod building;
pub mod census;
pub mod consequence;
pub mod converse;
pub mod coping;
pub mod craft;
pub mod custom;
pub mod econ;
pub mod field;
pub mod fitted;
pub mod game;
pub mod geology;
pub mod ground;
pub mod growth;
pub mod hydrology;
pub mod id;
pub mod infrastructure;
pub mod item;
pub mod labour;
pub mod locality;
pub mod logistics;
pub mod material;
pub mod memory;
pub mod mind;
pub mod money;
pub mod needs;
pub mod network;
pub mod noise;
pub mod patch;
pub mod person;
pub mod power;
pub mod polity;
pub mod populace;
pub mod region;
pub mod relations;
pub mod rng;
pub mod services;
pub mod save;
pub mod scrap;
pub mod schedule;
pub mod scaling;
pub mod settlement;
pub mod social;
pub mod state;
pub mod slice;
pub mod teardown;
pub mod townplan;
pub mod travel;
pub mod utility;
pub mod vehicle;
pub mod wip;
pub mod witness;
pub mod world;
