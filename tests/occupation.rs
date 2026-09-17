//! **Every industry employs a mix of occupations, and the mix is published.**
//!
//! The table in `raws/staffing.txt`, read through `occupation.rs`. Each
//! gate was checked by breaking what it names and requiring it to fail.

use scale_sim::econ::{Doctrine, SiteKind};
use scale_sim::occupation::{
    industry_of_site, jobs_by_industry, jobs_by_occupation, published, Industry, Occupation,
    ACTIVE_DUTY, DEFENCE_CIVILIANS, N_OCCUPATIONS, RAWS, SWORN_SHARE,
};
use scale_sim::slice;

/// **Every industry's shares are shares**: none negative, and together the
/// whole of its jobs.
#[test]
fn every_industry_is_staffed_by_shares_that_add_to_one() {
    for i in Industry::ALL {
        let s = i.staffing();
        assert!(
            s.iter().all(|v| *v >= 0.0),
            "{} has a negative share",
            i.name()
        );
        let total: f64 = s.iter().sum();
        assert!(
            (total - 1.0).abs() < 1e-9,
            "{}'s occupations add to {total:.6} of its jobs",
            i.name()
        );
    }
}

/// **The published figures come through as published.**
///
/// Read against the percentages printed beside each count, which the table
/// is not built from — so a parser taking the wrong column, dropping a row,
/// or sorting a detailed occupation into the wrong group shows here.
/// Tolerances are the rounding and suppression the normalisation absorbs.
#[test]
fn the_published_figures_come_through() {
    use Occupation::*;
    let cases: [(Industry, &[Occupation], f64, f64); 11] = [
        // (industry, occupations summed, published percent, tolerance)
        (Industry::Manufacturing, &[ProductionWorker], 48.80, 0.3),
        (Industry::Manufacturing, &[Accountant], 0.72, 0.05),
        (Industry::Construction, &[Electrician], 7.05, 0.1),
        (Industry::Construction, &[Pipefitter], 4.52, 0.1),
        (Industry::HealthCare, &[Nurse], 12.12, 0.1),
        (Industry::HealthCare, &[Doctor], 3.08, 0.05),
        (Industry::Retail, &[Sales], 48.53, 0.3),
        (Industry::Mining, &[Miner], 28.40, 0.2),
        (Industry::Transport, &[Driver], 22.56, 0.2),
        (Industry::CropFarming, &[Farmer], 16.8, 0.3),
        (Industry::CropFarming, &[FarmWorker], 56.6, 0.4),
    ];
    for (industry, occs, percent, tol) in cases {
        let got: f64 = occs.iter().map(|&o| industry.share(o)).sum::<f64>() * 100.0;
        assert!(
            (got - percent).abs() <= tol,
            "{} comes out {got:.2}% {}, published {percent}%",
            industry.name(),
            occs.iter()
                .map(|o| o.name())
                .collect::<Vec<_>>()
                .join(" and ")
        );
    }
}

/// **A works is not all works hands.** The claim the module exists for:
/// a manufacturer keeps books, a hospital is cleaned, a government has
/// lawyers, and a school employs more than teachers.
#[test]
fn employers_employ_more_than_their_own_trade() {
    use Occupation::*;
    let m = Industry::Manufacturing;
    for o in [Manager, Accountant, Engineer, OfficeClerk, Mechanic, Driver] {
        assert!(
            m.share(o) > 0.005,
            "a manufacturer employs {:.2}% {}s",
            m.share(o) * 100.0,
            o.name()
        );
    }
    assert!(Industry::HealthCare.share(Cleaner) > 0.01);
    assert!(Industry::Government.share(Legal) > 0.02);
    assert!(Industry::Education.share(Teacher) < 0.65);
    assert!(Industry::Education.share(OfficeClerk) > 0.05);
    // And nobody employs what they could not: no farm hands at a bank, no
    // soldiers outside the armed forces.
    assert!(Industry::Finance.share(FarmWorker) < 0.001);
    for i in Industry::ALL {
        if i != Industry::Military {
            assert_eq!(i.share(Soldier), 0.0, "{} employs soldiers", i.name());
        }
    }
}

/// **Police are mostly sworn and the armed forces mostly uniformed**, and
/// the rest of each is staffed like government.
#[test]
fn police_and_armed_forces_are_composed_as_published() {
    let gov = Industry::Government;
    let police = Industry::LawEnforcement.share(Occupation::ProtectiveService);
    let expected = SWORN_SHARE + (1.0 - SWORN_SHARE) * gov.share(Occupation::ProtectiveService);
    assert!(
        (police - expected).abs() < 1e-9,
        "police are {:.1}% protective service, expected {:.1}%",
        police * 100.0,
        expected * 100.0
    );
    assert!(police > 0.70);

    let soldiers = Industry::Military.share(Occupation::Soldier);
    let uniformed = ACTIVE_DUTY / (ACTIVE_DUTY + DEFENCE_CIVILIANS);
    assert!(
        (soldiers - uniformed).abs() < 1e-9,
        "the armed forces are {:.1}% soldiers, expected {:.1}%",
        soldiers * 100.0,
        uniformed * 100.0
    );
    assert!((0.60..0.70).contains(&soldiers));
}

/// **The raws file is the only copy.** The three figures that could not be
/// read as table rows are constants, and they must still be the figures the
/// file records.
#[test]
fn the_constants_are_the_figures_in_the_raws() {
    assert!(RAWS.contains("sworn officers: 70.4% of all law enforcement personnel"));
    assert_eq!(SWORN_SHARE, 0.704);
    assert!(RAWS.contains("total active duty (DMDC table): 1,280,652"));
    assert_eq!(ACTIVE_DUTY, 1_280_652.0);
    assert!(RAWS.contains("civilian full-time equivalents (thousands): 726"));
    assert_eq!(DEFENCE_CIVILIANS, 726_000.0);
}

/// **A suppressed figure is not a zero, and it is not read as one.** The
/// retail table does not release its scientists; the file says `(8)`, and
/// that row must be absent rather than parsed.
#[test]
fn suppressed_figures_are_left_out() {
    let retail = published().get("44-45").expect("retail is in the raws");
    assert!(!retail.contains_key("19-0000"));
    assert_eq!(retail.get("41-0000").copied(), Some(7_560_970.0));
    // And prose is not data.
    assert!(published().get("military,").is_none());
}

/// **Every kind of works belongs to the industry it is.** The match is
/// exhaustive; this checks the answers are sane rather than merely present.
#[test]
fn every_kind_of_works_has_its_industry() {
    let cases = [
        (SiteKind::Farm, Industry::CropFarming),
        (SiteKind::Pasture, Industry::AnimalFarming),
        (SiteKind::Steelworks, Industry::Manufacturing),
        (SiteKind::OilField, Industry::Mining),
        (SiteKind::PowerPlant, Industry::Utilities),
        (SiteKind::Hospital, Industry::HealthCare),
        (SiteKind::Shop, Industry::Retail),
        (SiteKind::Depot, Industry::Wholesale),
        (SiteKind::Builders, Industry::Construction),
    ];
    for (kind, want) in cases {
        assert_eq!(industry_of_site(kind), want, "{kind:?}");
    }
}

/// **Sorting jobs into occupations creates and loses none.**
#[test]
fn spreading_jobs_over_occupations_keeps_every_job() {
    let e = slice::build(Doctrine::Prudent);
    let by_industry = jobs_by_industry(&e);
    let by_occupation = jobs_by_occupation(&e);
    assert!(by_industry.iter().flatten().sum::<f64>() > 0.0);
    for (town, (i, o)) in by_industry.iter().zip(by_occupation.iter()).enumerate() {
        let (a, b): (f64, f64) = (i.iter().sum(), o.iter().sum());
        assert!(
            (a - b).abs() <= 1e-9 * a.max(1.0),
            "town {town} has {a:.3} jobs by industry and {b:.3} by occupation"
        );
        assert_eq!(o.len(), N_OCCUPATIONS);
    }
}
