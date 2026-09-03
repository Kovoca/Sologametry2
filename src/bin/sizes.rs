//! **What sizes the generator actually produces**, measured rather than
//! assumed.
//!
//!   cargo run --release --bin sizes -- --seed N --target 9000

use scale_sim::polity::Polities;
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn main() {
    let mut seed = 20260828u64;
    let mut target = 9000usize;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--target" => target = it.next().and_then(|v| v.parse().ok()).unwrap_or(target),
            _ => {}
        }
    }

    let world = World::generate(512, 288, seed);
    let pol = Polities::partition(&world, 24);
    let set = Settlements::place(&world, &pol, target);

    let mut pops: Vec<u32> = set.list.iter().map(|s| s.population).collect();
    pops.sort_unstable_by(|a, b| b.cmp(a));
    let total: f64 = pops.iter().map(|&p| p as f64).sum();

    println!("placed {} settlements, {:.2}bn people in them", pops.len(), total / 1e9);
    println!("\nrank      population");
    for r in [1usize, 2, 5, 10, 50, 100, 500, 1000, 2000, 4000, 6000, 8000] {
        if let Some(p) = pops.get(r - 1) {
            println!("  {r:<6}  {p:>12}");
        }
    }
    if let Some(last) = pops.last() {
        println!("  {:<6}  {:>12}   (smallest)", pops.len(), last);
    }

    // How many fall in each real size band.
    let bands: [(&str, u32, u32); 7] = [
        ("hamlet      <100", 0, 100),
        ("village     100-1k", 100, 1_000),
        ("lg village  1k-2.5k", 1_000, 2_500),
        ("small town  2.5k-10k", 2_500, 10_000),
        ("town        10k-50k", 10_000, 50_000),
        ("large town  50k-100k", 50_000, 100_000),
        ("city        100k+", 100_000, u32::MAX),
    ];
    println!("\nby size band");
    for (name, lo, hi) in bands {
        let n = pops.iter().filter(|&&p| p >= lo && p < hi).count();
        let share = 100.0 * n as f64 / pops.len().max(1) as f64;
        println!("  {name:<22} {n:>6}  {share:>5.1}%");
    }

    println!("\nmedian {}", pops[pops.len() / 2]);
}
