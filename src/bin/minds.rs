//! **The same bad year, happening to different people.**
//!
//!   cargo run --release --bin minds
//!   cargo run --release --bin minds -- --seed N --years 3
//!
//! `life` follows one man's purse and `people` follows a cohort's
//! employment. This follows what is going on *inside* them: the mind
//! spec's nine slices, run together on real people in a real town, with
//! one adversity applied to all of them so the differences are theirs.
//!
//! Nothing here is scripted. Every person is drawn from a seed and then
//! the same things happen to them.

use std::time::{SystemTime, UNIX_EPOCH};

use scale_sim::coping::{
    propensities, Circumstances, ControlAppraisal, Defence, Demands, FunctionalDomain,
    FunctionalState, Strain,
};
use scale_sim::growth::{Argued, Doubts, DurableTarget, Framing, ShapesWellbeing};
use scale_sim::id::{Arena, Id};
use scale_sim::mind::{Facet, Value};
use scale_sim::person::{Person, Trade};
use scale_sim::polity::Polities;
use scale_sim::scaling::{promote, Change, Coarse, Standing, What};
use scale_sim::settlement::Settlements;
use scale_sim::world::World;

fn random_seed() -> u64 {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut z = n ^ 0x9E37_79B9_7F4A_7C15;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^ (z >> 31)
}

fn folk(n: u32) -> Id<Person> {
    let mut a: Arena<Person> = Arena::new();
    let mut last = a.add(Person::new("x", Trade::Labourer, 0, 0.0));
    for _ in 0..n {
        last = a.add(Person::new("x", Trade::Labourer, 0, 0.0));
    }
    last
}

/// What the standout parts of somebody are, for a reader who wants to
/// predict what they will do.
fn sketch(m: &scale_sim::mind::Mind) -> String {
    let mut v: Vec<(Facet, f32)> = Facet::ALL.iter().map(|&f| (f, m.person.z(f))).collect();
    v.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
    v.truncate(3);
    v.iter()
        .map(|(f, z)| format!("{f:?} {}{:.1}", if *z >= 0.0 { "+" } else { "" }, z))
        .collect::<Vec<_>>()
        .join(", ")
}

fn state_word(s: FunctionalState) -> &'static str {
    match s {
        FunctionalState::Regulated => "regulated",
        FunctionalState::Strained => "strained",
        FunctionalState::Depleted => "depleted",
        FunctionalState::Impaired => "impaired",
    }
}

fn main() {
    let mut seed = random_seed();
    let mut years = 3u64;
    let mut cast = 6usize;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--years" => years = it.next().and_then(|v| v.parse().ok()).unwrap_or(3).max(1),
            "--folk" => cast = it.next().and_then(|v| v.parse().ok()).unwrap_or(6).clamp(2, 20),
            "--help" | "-h" => {
                println!("usage: minds [--seed N] [--years N] [--folk N]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other:?}");
                std::process::exit(2);
            }
        }
    }

    // --- a real place -------------------------------------------------
    println!("generating world (seed {seed})...");
    let world = World::generate(512, 288, seed);
    let polities = Polities::partition(&world, 24);
    let settlements = Settlements::place(&world, &polities, 6000);
    let Some(&(nation, _)) = polities.ranked().first() else {
        eprintln!("no nations on this world");
        std::process::exit(1);
    };
    let mut here = settlements
        .list
        .iter()
        .filter(|s| polities.owner[s.cell] == nation)
        .collect::<Vec<_>>();
    here.sort_by(|a, b| b.population.cmp(&a.population));
    let Some(town) = here.first() else {
        eprintln!("that nation has no towns");
        std::process::exit(1);
    };
    let (tx, ty) = (town.cell % world.width, town.cell / world.width);
    println!(
        "\n{} people, in a town of {} at ({tx}, {ty}) — {:?}\n",
        cast, town.population, world.biomes[town.cell]
    );

    // The culture they were raised in. Their own convictions depart from
    // it, which is what leaves room for a heretic.
    let culture = vec![
        (Value::Family, 30),
        (Value::Law, 25),
        (Value::Loyalty, 20),
        (Value::Craftsmanship, 15),
        (Value::Peace, 10),
    ];

    // --- the same bad year --------------------------------------------
    //
    // Everybody loses their work on day 200 and it does not come back.
    // Nothing else is arranged: what follows is theirs.
    let last_day = years * 365;
    let mut people: Vec<Coarse> = Vec::new();
    for i in 0..cast {
        let mut c = Coarse::new(folk(i as u32), seed ^ (i as u64 + 1) * 0x9E37, 0);
        c.perceived_control =
            ControlAppraisal { source: 0.55, consequences: 0.6, own_response: 0.5 };
        c.support_expected = 0.6;
        people.push(c);
    }

    let schedule = vec![
        Change {
            day: 200,
            what: What::StressorBegins(Standing {
                severity: 0.75,
                since: 200,
                worsens_if_ignored: 0.7,
                // Out of work, and no way back into it: this is the
                // finding, not a punishment. Recovery from losing work is
                // incomplete on average even after re-employment.
                actual: scale_sim::coping::ActualControl {
                    source: 0.05,
                    consequences: 0.35,
                    exit: 0.1,
                    means: 0.3,
                },
            }),
        },
        Change { day: 420, what: What::SupportChanges(0.25) },
    ];

    let demands = Demands { work: 0.8, caregiving: 0.5, social: 0.4, self_care: 0.4 };

    for (i, c) in people.iter_mut().enumerate() {
        let me = promote(c, &culture, 0);

        // The blow lands on the well-being layer, which is where the
        // literature that measured it actually looked.
        c.growth.shaped_wellbeing(ShapesWellbeing::LostWork, 1.0, -1.0, 200);
        c.appraise(200, 0, 0.75, 200);
        let tol = me.mind.stress.tolerance;
        c.advance_through(&me.mind, tol, &schedule);
        c.advance_to(last_day, &me.mind, tol);

        let now = promote(c, &culture, last_day);
        let circumstances = Circumstances {
            severity: c.pressure(),
            company: c.support_expected > 0.2,
            ..Default::default()
        };
        let ranked = propensities(&now.mind, &c.perceived_control, &circumstances, c.strain.debt);

        println!("--- person {} ----------------------------------------", i + 1);
        println!("  is            {}", sketch(&now.mind));
        println!(
            "  reaches for   {:?}, then {:?}",
            ranked[0].0, ranked[1].0
        );
        println!("  settled on    {:?}", c.habits.strongest());

        // Not one number: which part of a life is failing.
        let parts: Vec<String> = FunctionalDomain::ALL
            .iter()
            .map(|&d| {
                format!(
                    "{d:?} {}",
                    state_word(c.strain.functioning_in(d, &demands, &Defence::default()))
                )
            })
            .collect();
        println!("  functioning   {}", parts.join(", "));
        println!(
            "  carrying      debt {:.2}, {} days severely impaired over {} episode(s)",
            c.strain.debt, c.strain.history.lifetime_days, c.strain.history.episode_count
        );
        // **Two layers, and this is the whole point of keeping them
        // apart.** Losing work is a life-satisfaction result, so it lands
        // there and permanently; whether it also wrecks how somebody
        // *functions* is a separate question with a different answer per
        // person.
        println!(
            "  life feels    {:+.2} SD (permanent), traits moved {:+.2} z",
            now.mind.wellbeing_baseline,
            now.z(Facet::Anxiety) - me.z(Facet::Anxiety)
        );
        println!(
            "  believes now  {:.2} control over it, from {:.2}",
            c.perceived_control.source, 0.55
        );

        if c.strain.state >= FunctionalState::Depleted {
            let breaks = Strain::crisis_propensities(&now.mind, &circumstances);
            println!("  if it breaks  {:?}", breaks[0].0);
        }

        let shaped = c.growth.what_shaped(DurableTarget::WellbeingBaseline, last_day);
        if let Some((cause, v)) = shaped.first() {
            println!("  because of    {cause:?} ({v:+.2})");
        }
        println!();
    }

    // --- and one thing that is argued about ---------------------------
    //
    // Somebody credible, with something new to say, once a week, for as
    // long as the run lasts. Doubt before change.
    println!("=== being argued with, weekly, about the law =============\n");
    for (i, c) in people.iter_mut().enumerate().take(3) {
        let mut me = promote(c, &culture, last_day);
        let held_before = me.mind.conviction(Value::Law);
        let mut d = Doubts::new();
        let mut moved = 0;
        let mut dismissed = 0;
        for day in 0..last_day {
            if day % 7 != 0 {
                continue;
            }
            match d.argued(
                &mut me.mind,
                Value::Law,
                -40,
                1.0,
                Framing { credible: 0.9, novelty: 1.0, ..Default::default() },
                day,
            ) {
                Argued::Moved => moved += 1,
                Argued::Dismissed => dismissed += 1,
                _ => {}
            }
        }
        let after = me.mind.conviction(Value::Law);
        let culture_says = me
            .mind
            .values
            .iter()
            .find(|v| v.topic == Value::Law)
            .map(|v| v.heterodoxy())
            .unwrap_or(0);
        println!(
            "  person {}: held {held_before:+} -> {after:+} over {} years \
             ({moved} time(s) it gave, {dismissed} waved away), now {culture_says} from the culture",
            i + 1,
            years
        );
    }

    println!(
        "\n{years} years, one adversity, no scripts.\n\
         Everybody carries the same permanent loss of life satisfaction, because that is\n\
         what losing work is measured to do. Whether it also broke how they function is\n\
         a different question, and the answer is what they reached for."
    );
}
