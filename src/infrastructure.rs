//! What infrastructure costs to build and to keep.
//!
//! Spec B.5 and C.4. Roads already exist in `network`; this is what they
//! cost, which is a different and more consequential question.
//!
//! **Terrain multiplies cost far more than distance does.** A kilometre of
//! two-lane rural highway across grassland runs to about two million; the
//! same kilometre through mountains is fifteen, because it is cuts,
//! bridges and tunnels rather than earthworks. Ten kilometres of tunnel
//! can cost more than two hundred kilometres of plains road. All figures
//! below are approximate real-world values, in millions of currency units
//! per kilometre, and are there to be checked against.
//!
//! The consequence that matters for the simulation is a trap: serving a
//! marginal region costs *more* per kilometre and there are *fewer* people
//! to pay for it. That is why remote country stays poorly connected, why
//! states under-invest in it, and why C1.3's "difficult terrain" is an
//! opportunity for anyone who does not want to be governed.

use crate::network::{Network, Road};
use crate::polity::{Polities, UNCLAIMED};
use crate::region::KM_PER_CELL;
use crate::world::{Biome, World};

/// Capital cost of one kilometre of road across a given biome, in
/// millions. Real figures for two-lane rural highway.
pub fn cost_per_km(biome: Biome, river: bool) -> f64 {
    let ground = match biome {
        Biome::Grassland | Biome::Savanna | Biome::Beach => 2.0,
        Biome::Desert | Biome::Shrubland => 3.0,
        Biome::Forest => 3.0,
        Biome::Rainforest => 5.0, // clearing, drainage, and it grows back
        Biome::Taiga => 4.5,
        Biome::Swamp => 7.0, // piling and drainage
        Biome::Tundra => 8.0, // permafrost heaves whatever you lay on it
        Biome::Mountain => 15.0, // cuts, bridges, tunnels
        Biome::Snowcap => 25.0,
        // Not built on; crossings are handled as bridges below.
        Biome::Ocean | Biome::Shallows => 40.0,
    };
    // A watercourse means a bridge, and bridges are where road money goes.
    if river {
        ground + 6.0
    } else {
        ground
    }
}

/// A trunk route is wider, stronger and graded for heavy traffic; a track
/// is barely more than a formation. Multiplier on the base cost.
pub fn class_multiplier(road: Road) -> f64 {
    match road {
        Road::Highway => 2.6,
        Road::Road => 1.0,
        Road::Track => 0.35,
        Road::None => 0.0,
    }
}

/// Annual maintenance as a share of capital value.
///
/// Two to four per cent is the real range. Freeze-thaw is the great
/// destroyer of roads — water gets into the surface, freezes, and lifts it
/// — so cold country pays the top of the range and warm dry country the
/// bottom. Heavy rain does its own damage.
pub fn maintenance_rate(temperature: f32, rainfall_rank: f32) -> f64 {
    let freeze_thaw = if (0.10..0.35).contains(&temperature) {
        // Crossing zero repeatedly is far worse than staying frozen.
        0.018
    } else if temperature < 0.10 {
        0.010
    } else {
        0.004
    };
    let wet = 0.004 * rainfall_rank.clamp(0.0, 1.0) as f64;
    0.018 + freeze_thaw + wet
}

/// What a nation's road network is worth and what it costs to keep.
#[derive(Clone, Debug, Default)]
pub struct RoadAccount {
    /// Kilometres of road, by class.
    pub highway_km: f64,
    pub road_km: f64,
    pub track_km: f64,
    /// Replacement value, in millions.
    pub capital: f64,
    /// What a full maintenance programme costs each year, in millions.
    pub upkeep_per_year: f64,
    /// Cells of road that cross difficult ground — mountain, swamp, ice.
    pub hard_going_km: f64,
}

impl RoadAccount {
    pub fn total_km(&self) -> f64 {
        self.highway_km + self.road_km + self.track_km
    }

    /// Upkeep per head of population per year. The number that decides
    /// whether a country can afford the network it has: a sparse
    /// population spread over hard country pays many times what a dense
    /// one on a plain does for the same connectivity.
    pub fn upkeep_per_capita(&self, population: f64) -> f64 {
        if population <= 0.0 {
            return 0.0;
        }
        self.upkeep_per_year * 1.0e6 / population
    }
}

/// Value and upkeep of the roads inside one polity's territory.
pub fn road_account(
    world: &World,
    net: &Network,
    pol: &Polities,
    polity: u16,
) -> RoadAccount {
    let mut a = RoadAccount::default();

    for i in 0..world.biomes.len() {
        if pol.owner[i] != polity {
            continue;
        }
        let class = net.road[i];
        let mult = class_multiplier(class);
        if mult <= 0.0 {
            continue;
        }

        // One cell of road is one cell-width of it.
        let km = KM_PER_CELL;
        match class {
            Road::Highway => a.highway_km += km,
            Road::Road => a.road_km += km,
            Road::Track => a.track_km += km,
            Road::None => {}
        }

        let per_km = cost_per_km(world.biomes[i], net.navigable[i] || world.river[i]);
        let capital = per_km * mult * km;
        a.capital += capital;

        if matches!(
            world.biomes[i],
            Biome::Mountain | Biome::Snowcap | Biome::Swamp | Biome::Tundra
        ) {
            a.hard_going_km += km;
        }

        let rain = net_rain_rank(world, i);
        a.upkeep_per_year +=
            capital * maintenance_rate(world.temperature.data[i], rain);
    }
    a
}

/// Rainfall at a cell as a 0..1 rank against the wettest ground on the
/// planet. Cheap stand-in for the ranked field the biome pass builds.
fn net_rain_rank(world: &World, i: usize) -> f32 {
    let max = world
        .rainfall
        .data
        .iter()
        .copied()
        .fold(0.0f32, f32::max)
        .max(1e-6);
    world.rainfall.data[i] / max
}

/// Road accounts for every polity holding territory, indexed by polity id.
pub fn all_accounts(world: &World, net: &Network, pol: &Polities) -> Vec<RoadAccount> {
    let mut out = vec![RoadAccount::default(); pol.list.len()];
    let max = world.rainfall.data.iter().copied().fold(0.0f32, f32::max).max(1e-6);

    for i in 0..world.biomes.len() {
        let owner = pol.owner[i];
        if owner == UNCLAIMED {
            continue;
        }
        let class = net.road[i];
        let mult = class_multiplier(class);
        if mult <= 0.0 {
            continue;
        }
        let a = &mut out[owner as usize];
        let km = KM_PER_CELL;
        match class {
            Road::Highway => a.highway_km += km,
            Road::Road => a.road_km += km,
            Road::Track => a.track_km += km,
            Road::None => {}
        }
        let per_km = cost_per_km(world.biomes[i], net.navigable[i] || world.river[i]);
        let capital = per_km * mult * km;
        a.capital += capital;
        if matches!(
            world.biomes[i],
            Biome::Mountain | Biome::Snowcap | Biome::Swamp | Biome::Tundra
        ) {
            a.hard_going_km += km;
        }
        a.upkeep_per_year += capital
            * maintenance_rate(world.temperature.data[i], world.rainfall.data[i] / max);
    }
    out
}
