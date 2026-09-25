//! `cargo run --release --bin recipes`
//!
//! **What each recipe's inputs cost at the reference prices against what
//! its output is worth at them.** A works pays for its inputs and is paid
//! for its output at the same wholesale share of the price, so at the
//! reference that share cancels and this ratio is the margin its prices
//! leave it before any scarcity. Above one, a recipe loses money at its own
//! reference prices; well under one, the day's prices have room to move
//! before it does.
//!
//! The labour column is value added at `VALUE_ADDED_AN_HOUR` — wage, plant,
//! overhead and margin — which is what a cost is built on and not what a
//! works pays out in wages.
use scale_sim::econ::{Commodity, RECIPES, VALUE_ADDED_AN_HOUR};

fn main() {
    println!(
        "{:<24} {:>9} {:>8} {:>8} {:>9} {:>7} {:>8}",
        "recipe", "inputs", "power", "labour", "output", "in/out", "all/out"
    );
    for r in RECIPES.iter() {
        let inputs: f64 = r.inputs.iter().map(|&(c, q)| q * c.base_cost()).sum();
        let power = r.power * Commodity::Electricity.base_cost();
        let labour = r.labour * VALUE_ADDED_AN_HOUR;
        let output: f64 = r.outputs.iter().map(|&(c, q)| q * c.base_cost()).sum();
        let ins: Vec<String> = r
            .inputs
            .iter()
            .map(|&(c, q)| format!("{q} {c:?}"))
            .collect();
        let outs: Vec<String> = r
            .outputs
            .iter()
            .map(|&(c, q)| format!("{q} {c:?}"))
            .collect();
        let share = |x: f64| if output > 0.0 { x / output } else { 0.0 };
        println!(
            "{:<24} {:>9.1} {:>8.1} {:>8.1} {:>9.1} {:>7.2} {:>8.2}   {} -> {}",
            r.name,
            inputs,
            power,
            labour,
            output,
            share(inputs),
            share(inputs + power + labour),
            ins.join(" + "),
            outs.join(" + "),
        );
    }
}
