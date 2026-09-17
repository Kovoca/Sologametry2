//! **What people do, apart from where they do it.**
//!
//! A trade here has always been an industry wearing a job's name: a
//! "labourer" was whoever worked at a works, a "public servant" whoever the
//! state employed, "office work" everything with a desk in it. So a
//! steelworks employed nobody but machine operators, a hospital nobody but
//! nurses, and there was no accountant, engineer, cleaner or manager
//! anywhere in the world.
//!
//! Real labour statistics are a table, not a list. **Every industry employs
//! a mix of occupations**, and the mix is published: a manufacturer is
//! about half production workers and the other half managers, engineers,
//! clerks, drivers, mechanics, salespeople and accountants; a hospital is a
//! third clinicians, a third care staff and an eighth clerks. This module
//! is that table.
//!
//! ## Where the figures come from
//!
//! `raws/staffing.txt`, read at start-up — **the data file is the source**,
//! so no figure is typed twice. It holds extracts of:
//!
//! - **BLS OEWS, May 2023**, industry-specific occupational employment, for
//!   every sector the economy has — read from the published tables row by
//!   row. A summarising fetch was tried first and invented figures for
//!   several rows, which is why the file says how it was read;
//! - **BLS Employment Projections, National Employment Matrix, 2025 base**,
//!   for farms and fishing, which OEWS does not survey;
//! - **DMDC via the Air & Space Forces Almanac, FY2024**, for the armed
//!   forces, which neither does;
//! - **FBI UCR 2018** for the sworn share of police employment, because
//!   OEWS files police under local government as a whole.
//!
//! ## What an occupation is here
//!
//! The US Standard Occupational Classification's **major groups**, with a
//! few detailed occupations split out where the distinction decides what
//! somebody may do or earns: a physician is not a pharmacist, an electrician
//! holds a ticket a labourer does not, a miner is not a bricklayer, and a
//! farmer who runs the place is not the hand who works it. Each says which
//! codes it covers, so it can be checked against the file.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::person::{Qualification, Skill};

/// **An occupation**: what somebody does, whoever employs them.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Occupation {
    /// 11-0000 less farmers: general, operations and department managers.
    Manager,
    /// 11-9013: farmers, ranchers and other agricultural managers.
    Farmer,
    /// 13-2011: accountants and auditors.
    Accountant,
    /// 13-0000 less accountants: buyers, analysts, HR, loan officers,
    /// compliance, logistics planners.
    BusinessSpecialist,
    /// 15-0000: programmers, analysts, statisticians.
    ComputingSpecialist,
    /// 17-0000: architects, engineers, drafters, engineering technicians.
    Engineer,
    /// 19-0000: life, physical and social scientists and their technicians.
    Scientist,
    /// 21-0000: social workers, counsellors, clergy.
    SocialWorker,
    /// 23-0000: lawyers, judges, paralegals.
    Legal,
    /// 25-0000: teachers, lecturers, librarians, teaching assistants.
    Teacher,
    /// 27-0000: designers, writers, broadcasters, performers, athletes.
    ArtsAndMedia,
    /// 29-1210 and 29-1240: physicians and surgeons.
    Doctor,
    /// 29-1141: registered nurses.
    Nurse,
    /// 29-0000 less doctors and nurses: pharmacists, therapists,
    /// technicians, practical nurses, paramedics.
    HealthTechnician,
    /// 31-0000: nursing assistants, home health and personal care aides.
    CareAssistant,
    /// 33-0000: police, firefighters, correctional officers, security.
    ProtectiveService,
    /// 35-0000: cooks, servers, bar and counter staff.
    FoodService,
    /// 37-0000: janitors, cleaners, groundskeepers.
    Cleaner,
    /// 39-0000: childcare workers, hairdressers, attendants, fitness.
    PersonalCare,
    /// 41-0000: cashiers, retail and wholesale sales, agents.
    Sales,
    /// 43-0000: clerks, secretaries, customer service, dispatchers.
    OfficeClerk,
    /// 45-0000 less fishers: farm hands, graders, forest and logging
    /// workers.
    FarmWorker,
    /// 45-3031: fishing and hunting workers.
    Fisher,
    /// 47-0000 less the three below: carpenters, masons, roofers,
    /// labourers, equipment operators.
    Builder,
    /// 47-2111: electricians.
    Electrician,
    /// 47-2152: plumbers, pipefitters and steamfitters.
    Pipefitter,
    /// 47-5000: extraction workers — miners, drillers, roustabouts.
    Miner,
    /// 49-0000: installation, maintenance and repair.
    Mechanic,
    /// 51-0000: production — operators, assemblers, fabricators, food
    /// processing, plant operators.
    ProductionWorker,
    /// 53-3032 and 53-3033: heavy and light truck drivers.
    Driver,
    /// 53-0000 less drivers: freight movers, stockers, pilots, crews.
    MaterialMover,
    /// The armed forces, enlisted and officers.
    Soldier,
    /// **Somebody in charge of a floor.** Not drawn from any staffing
    /// pattern — every published group already counts its own first-line
    /// supervisors — because here it is what a floor hand is promoted to,
    /// against a count of posts `labour.rs` keeps at a span of control.
    Supervisor,
}

pub const N_OCCUPATIONS: usize = 33;

impl Occupation {
    /// **Every occupation, in the order `index` gives.**
    pub const ALL: [Occupation; N_OCCUPATIONS] = [
        Occupation::Manager,
        Occupation::Farmer,
        Occupation::Accountant,
        Occupation::BusinessSpecialist,
        Occupation::ComputingSpecialist,
        Occupation::Engineer,
        Occupation::Scientist,
        Occupation::SocialWorker,
        Occupation::Legal,
        Occupation::Teacher,
        Occupation::ArtsAndMedia,
        Occupation::Doctor,
        Occupation::Nurse,
        Occupation::HealthTechnician,
        Occupation::CareAssistant,
        Occupation::ProtectiveService,
        Occupation::FoodService,
        Occupation::Cleaner,
        Occupation::PersonalCare,
        Occupation::Sales,
        Occupation::OfficeClerk,
        Occupation::FarmWorker,
        Occupation::Fisher,
        Occupation::Builder,
        Occupation::Electrician,
        Occupation::Pipefitter,
        Occupation::Miner,
        Occupation::Mechanic,
        Occupation::ProductionWorker,
        Occupation::Driver,
        Occupation::MaterialMover,
        Occupation::Soldier,
        Occupation::Supervisor,
    ];

    /// Where it sits in `ALL`, by exhaustive match, so a new occupation
    /// cannot compile until it has a place.
    pub fn index(self) -> usize {
        use Occupation::*;
        match self {
            Manager => 0,
            Farmer => 1,
            Accountant => 2,
            BusinessSpecialist => 3,
            ComputingSpecialist => 4,
            Engineer => 5,
            Scientist => 6,
            SocialWorker => 7,
            Legal => 8,
            Teacher => 9,
            ArtsAndMedia => 10,
            Doctor => 11,
            Nurse => 12,
            HealthTechnician => 13,
            CareAssistant => 14,
            ProtectiveService => 15,
            FoodService => 16,
            Cleaner => 17,
            PersonalCare => 18,
            Sales => 19,
            OfficeClerk => 20,
            FarmWorker => 21,
            Fisher => 22,
            Builder => 23,
            Electrician => 24,
            Pipefitter => 25,
            Miner => 26,
            Mechanic => 27,
            ProductionWorker => 28,
            Driver => 29,
            MaterialMover => 30,
            Soldier => 31,
            Supervisor => 32,
        }
    }

    pub fn name(self) -> &'static str {
        use Occupation::*;
        match self {
            Manager => "manager",
            Farmer => "farmer",
            Accountant => "accountant",
            BusinessSpecialist => "business specialist",
            ComputingSpecialist => "computing specialist",
            Engineer => "engineer",
            Scientist => "scientist",
            SocialWorker => "social worker",
            Legal => "legal",
            Teacher => "teacher",
            ArtsAndMedia => "arts and media",
            Doctor => "doctor",
            Nurse => "nurse",
            HealthTechnician => "health technician",
            CareAssistant => "care assistant",
            ProtectiveService => "protective service",
            FoodService => "food service",
            Cleaner => "cleaner",
            PersonalCare => "personal care",
            Sales => "sales",
            OfficeClerk => "office clerk",
            FarmWorker => "farm worker",
            Fisher => "fisher",
            Builder => "builder",
            Electrician => "electrician",
            Pipefitter => "pipe fitter",
            Miner => "miner",
            Mechanic => "mechanic",
            ProductionWorker => "production worker",
            Driver => "driver",
            MaterialMover => "material mover",
            Soldier => "soldier",
            Supervisor => "supervisor",
        }
    }

    /// **The skill the work practises**, which is what improves by doing it
    /// and what decides how well it is done.
    pub fn skill(self) -> Skill {
        use Occupation::*;
        match self {
            Manager | Supervisor => Skill::Management,
            Farmer | FarmWorker => Skill::Husbandry,
            Accountant => Skill::Accounting,
            BusinessSpecialist => Skill::Commerce,
            ComputingSpecialist => Skill::Computing,
            Engineer => Skill::Engineering,
            Scientist => Skill::Science,
            SocialWorker => Skill::Counselling,
            Legal => Skill::Law,
            Teacher => Skill::Teaching,
            ArtsAndMedia => Skill::Artistry,
            // A pharmacist, a radiographer and a paramedic practise the
            // same clinical science a doctor does, to a shallower depth.
            Doctor | HealthTechnician => Skill::Medicine,
            Nurse | CareAssistant => Skill::Nursing,
            ProtectiveService => Skill::Protection,
            FoodService => Skill::Catering,
            Cleaner => Skill::Cleaning,
            PersonalCare => Skill::Tending,
            Sales => Skill::Retail,
            OfficeClerk => Skill::Clerical,
            Fisher => Skill::Fishing,
            Builder => Skill::Building,
            Electrician => Skill::Wiring,
            Pipefitter => Skill::Pipework,
            Miner => Skill::Mining,
            Mechanic => Skill::Mechanics,
            ProductionWorker => Skill::Machining,
            Driver => Skill::Driving,
            MaterialMover => Skill::Handling,
            Soldier => Skill::Soldiering,
        }
    }

    /// **The level the work wants**, below which somebody is not up to it
    /// and is paid accordingly. Set by the depth of the training the work
    /// takes — a week of being shown, an apprenticeship, a degree, six years
    /// and a foundation programme. Designed, on CDDA's 0-10.
    pub fn wants_level(self) -> u8 {
        use Occupation::*;
        match self {
            Doctor => 7,
            Engineer | Scientist | Legal => 6,
            Manager | Accountant | ComputingSpecialist | Nurse | Electrician | Pipefitter => 5,
            Farmer | BusinessSpecialist | SocialWorker | Teacher | HealthTechnician | Builder
            | Mechanic | Supervisor => 4,
            ArtsAndMedia | CareAssistant | ProtectiveService | Miner | Driver | Soldier => 3,
            PersonalCare | OfficeClerk | FarmWorker | Fisher | ProductionWorker => 2,
            FoodService | Cleaner | Sales | MaterialMover => 1,
        }
    }

    /// **What the work requires before anybody will have you**, from the BLS
    /// Employment Projections' *typical entry-level education*, read onto
    /// the three steps this model has: a bachelor's degree or more is a
    /// degree; an apprenticeship, a certificate or an associate degree is
    /// vocational; a high school diploma or nothing is school.
    pub fn qualification(self) -> Qualification {
        use Occupation::*;
        match self {
            Manager | Accountant | BusinessSpecialist | ComputingSpecialist | Engineer
            | Scientist | SocialWorker | Legal | Teacher | ArtsAndMedia | Doctor | Nurse => {
                Qualification::Degree
            }
            HealthTechnician | CareAssistant | Builder | Electrician | Pipefitter | Mechanic
            | Driver => Qualification::Vocational,
            Farmer | ProtectiveService | FoodService | Cleaner | PersonalCare | Sales
            | OfficeClerk | FarmWorker | Fisher | Miner | ProductionWorker | MaterialMover
            | Soldier | Supervisor => Qualification::School,
        }
    }

    /// **How the work is held**: the chance of full-time, part-time and
    /// casual. Carried over from the trade each occupation replaces until
    /// occupation-level figures are read in — offices salaried and
    /// permanent, public service steady, clinical work salaried with a bank
    /// and agency share, a ticket being what lets you work for yourself,
    /// hospitality and retail where the insecurity lives.
    pub fn employment_mix(self) -> (f64, f64, f64) {
        use Occupation::*;
        match self {
            Manager | Accountant | BusinessSpecialist | ComputingSpecialist | Engineer
            | Scientist | Legal | OfficeClerk => (0.88, 0.10, 0.02),
            Teacher | SocialWorker | ProtectiveService => (0.70, 0.28, 0.02),
            // An enlistment is a contract for years.
            Soldier => (1.0, 0.0, 0.0),
            Doctor => (0.92, 0.06, 0.02),
            Nurse | HealthTechnician => (0.72, 0.18, 0.10),
            CareAssistant => (0.55, 0.28, 0.17),
            FoodService | PersonalCare => (0.35, 0.36, 0.29),
            Sales | Cleaner => (0.30, 0.58, 0.12),
            Builder => (0.60, 0.08, 0.32),
            // Freelance, like a ticketed trade. Designed.
            ArtsAndMedia | Electrician | Pipefitter => (0.45, 0.07, 0.48),
            Driver => (0.60, 0.10, 0.30),
            Farmer | FarmWorker | Fisher | Miner | Mechanic | ProductionWorker | MaterialMover => {
                (0.85, 0.10, 0.05)
            }
            Supervisor => (0.90, 0.08, 0.02),
        }
    }

    /// **What the work's week looks like.**
    pub fn week(self) -> Week {
        use Occupation::*;
        match self {
            Manager | Accountant | BusinessSpecialist | ComputingSpecialist | Engineer
            | Scientist | Legal | OfficeClerk | Teacher => Week::Weekdays,
            SocialWorker => Week::MostlyWeekdays,
            Doctor | Nurse | HealthTechnician | CareAssistant | ProtectiveService | Soldier => {
                Week::EveryDay
            }
            Electrician | Pipefitter | Mechanic => Week::SiteWithCallOuts,
            Sales | FoodService | PersonalCare | ArtsAndMedia => Week::OpenSevenDays,
            Farmer | FarmWorker | Fisher | Miner | ProductionWorker | MaterialMover | Builder
            | Driver | Cleaner | Supervisor => Week::Rota,
        }
    }

    /// **How much of the work is done where whoever decides can see it.** A
    /// driver is on the road and a fisher at sea; everybody else works in
    /// front of the people who promote them.
    pub fn in_sight(self) -> f64 {
        match self {
            Occupation::Driver | Occupation::Fisher => 0.25,
            _ => 1.0,
        }
    }

    /// **Median hourly pay**, OEWS May 2023, from the national table in the
    /// raws.
    ///
    /// For a group, the group's median; where a detailed occupation carries
    /// its own, that. Three are not published as medians and say what stands
    /// in: physicians' median is above the survey's $115 an hour ceiling, so
    /// their mean is used; fishers are not published nationally, so the
    /// farming, fishing and forestry median is; and the armed forces are not
    /// in the survey at all, so a soldier is the midpoint of an E-1 and an
    /// E-5's Regular Military Compensation — pay, housing, subsistence and
    /// the tax advantage, which is what a civilian buys out of a wage —
    /// from DOD's January 2026 tables, three years on from the rest.
    pub fn median_hourly(self) -> f64 {
        use Occupation::*;
        let national = wages();
        let median = |code: &str| {
            national
                .get(code)
                .and_then(|w| w.median_hourly)
                .unwrap_or_else(|| panic!("no median hourly wage for {code} in the raws"))
        };
        match self {
            Manager => median("11-0000"),
            Farmer => median("11-9013"),
            Accountant => median("13-2011"),
            BusinessSpecialist => median("13-0000"),
            ComputingSpecialist => median("15-0000"),
            Engineer => median("17-0000"),
            Scientist => median("19-0000"),
            SocialWorker => median("21-0000"),
            Legal => median("23-0000"),
            Teacher => median("25-0000"),
            ArtsAndMedia => median("27-0000"),
            Doctor => {
                national
                    .get("29-1210")
                    .and_then(|w| w.mean_annual)
                    .expect("physicians' mean wage in the raws")
                    / HOURS_IN_A_PAID_YEAR
            }
            Nurse => median("29-1141"),
            HealthTechnician => median("29-0000"),
            CareAssistant => median("31-0000"),
            ProtectiveService => median("33-0000"),
            FoodService => median("35-0000"),
            Cleaner => median("37-0000"),
            PersonalCare => median("39-0000"),
            Sales => median("41-0000"),
            OfficeClerk => median("43-0000"),
            FarmWorker | Fisher => median("45-0000"),
            Builder => median("47-0000"),
            Electrician => median("47-2111"),
            Pipefitter => median("47-2152"),
            Miner => median("47-5000"),
            Mechanic => median("49-0000"),
            ProductionWorker => median("51-0000"),
            Driver => {
                // Heavy and light, by how many of each there are.
                let heavy = national.get("53-3032").expect("heavy truck drivers");
                let light = national.get("53-3033").expect("light truck drivers");
                (heavy.employment * median("53-3032") + light.employment * median("53-3033"))
                    / (heavy.employment + light.employment)
            }
            MaterialMover => median("53-0000"),
            Soldier => (SOLDIER_RMC_E1 + SOLDIER_RMC_E5) / 2.0 / HOURS_IN_A_PAID_YEAR,
            Supervisor => median("51-1011"),
        }
    }

    /// **Days of food a day's work buys**, the scale this model pays in.
    ///
    /// Pinned where it always was: a production worker at six, the figure
    /// the labourer carried — so the rent, which is set off that wage, and
    /// every cost of production, which reads it as a ratio, do not move.
    /// Every other occupation is placed against it by the ratio of real
    /// median pay, so what a doctor earns against a cook is the American
    /// figure rather than a guess.
    pub fn days_of_food_a_day(self) -> f64 {
        PRODUCTION_WORKER_DAYS_OF_FOOD * self.median_hourly()
            / Occupation::ProductionWorker.median_hourly()
    }
}

/// What a production worker's day buys, which everything else is placed
/// against. The labourer's figure, unchanged.
pub const PRODUCTION_WORKER_DAYS_OF_FOOD: f64 = 6.0;

/// A full-time paid year in hours, which is how OEWS turns an hourly wage
/// into an annual one.
pub const HOURS_IN_A_PAID_YEAR: f64 = 2080.0;

/// Regular Military Compensation, January 2026 *(DOD Selected Military
/// Compensation Tables, via CRS IF10532)*: an E-1 private and an E-5
/// sergeant.
pub const SOLDIER_RMC_E1: f64 = 60_810.0;
pub const SOLDIER_RMC_E5: f64 = 89_148.0;

/// **The shape of a working week.**
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Week {
    /// Monday to Friday and nothing else: offices and schools.
    Weekdays,
    /// Weekdays, with a share of the weekend covered.
    MostlyWeekdays,
    /// A rota that does not care what day it is: hospitals, police, the
    /// armed forces.
    EveryDay,
    /// Weekday site hours, and call-outs at the weekend.
    SiteWithCallOuts,
    /// Open seven days and busiest at the weekend.
    OpenSevenDays,
    /// Shifts, quieter on a Sunday: works, farms, mines, depots.
    Rota,
}

/// **One occupation's line in the national table.**
#[derive(Copy, Clone, Debug)]
pub struct NationalWage {
    pub employment: f64,
    pub median_hourly: Option<f64>,
    pub mean_annual: Option<f64>,
}

/// The national section of the raws, with its wages.
pub fn wages() -> &'static BTreeMap<String, NationalWage> {
    static CELL: OnceLock<BTreeMap<String, NationalWage>> = OnceLock::new();
    CELL.get_or_init(|| {
        let money = |w: Option<&str>| {
            w.filter(|t| t.starts_with('$')).and_then(|t| {
                t.trim_start_matches('$')
                    .replace(',', "")
                    .parse::<f64>()
                    .ok()
            })
        };
        let mut out = BTreeMap::new();
        let mut in_national = false;
        for line in RAWS.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("== ") {
                in_national = rest.starts_with("national");
                continue;
            }
            if !in_national {
                continue;
            }
            let words: Vec<&str> = t.split_whitespace().collect();
            if words.len() < 2 || !is_soc(words[0]) {
                continue;
            }
            let Ok(employment) = words[1].replace(',', "").parse::<f64>() else {
                continue;
            };
            out.insert(
                words[0].to_string(),
                NationalWage {
                    employment,
                    median_hourly: money(words.get(2).copied()),
                    mean_annual: money(words.get(3).copied()),
                },
            );
        }
        out
    })
}

/// **The United States' mix of occupations**, for a town with no employers
/// to read one from. OEWS leaves out farms and the armed forces, so this
/// does too.
pub fn national_mix() -> &'static [f64; N_OCCUPATIONS] {
    static CELL: OnceLock<[f64; N_OCCUPATIONS]> = OnceLock::new();
    CELL.get_or_init(|| {
        from_counts(
            published()
                .get("national")
                .expect("the national table in the raws"),
        )
    })
}

/// **What an employer makes or does**, as the statistics classify it.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Industry {
    /// NAICS 111.
    CropFarming,
    /// NAICS 112, livestock and aquaculture.
    AnimalFarming,
    /// NAICS 113.
    Forestry,
    /// NAICS 114.
    Fishing,
    /// NAICS 21: mines, quarries, oil and gas.
    Mining,
    /// NAICS 22: power, water, gas.
    Utilities,
    /// NAICS 23.
    Construction,
    /// NAICS 31-33.
    Manufacturing,
    /// NAICS 42.
    Wholesale,
    /// NAICS 44-45.
    Retail,
    /// NAICS 48-49: carriers, warehouses, couriers.
    Transport,
    /// NAICS 51: publishing, broadcasting, telecoms, data.
    Information,
    /// NAICS 52: banks, insurers, brokers.
    Finance,
    /// NAICS 54: lawyers, accountants, engineers, consultants.
    Professional,
    /// NAICS 55: head offices.
    HeadOffices,
    /// NAICS 56: staffing, cleaning and security contractors, call
    /// centres, waste.
    AdminSupport,
    /// NAICS 61, public and private schools and colleges.
    Education,
    /// NAICS 62, public and private hospitals, clinics and care.
    HealthCare,
    /// NAICS 6244: nurseries and day care.
    ChildCare,
    /// NAICS 71.
    Recreation,
    /// NAICS 72: hotels, restaurants, bars.
    Accommodation,
    /// OEWS sector 99: federal, state and local government, without their
    /// schools, hospitals or the postal service.
    Government,
    /// Police forces: sworn officers and the civilians who support them.
    LawEnforcement,
    /// The armed forces and their civilian staff.
    Military,
}

pub const N_INDUSTRIES: usize = 24;

impl Industry {
    pub const ALL: [Industry; N_INDUSTRIES] = [
        Industry::CropFarming,
        Industry::AnimalFarming,
        Industry::Forestry,
        Industry::Fishing,
        Industry::Mining,
        Industry::Utilities,
        Industry::Construction,
        Industry::Manufacturing,
        Industry::Wholesale,
        Industry::Retail,
        Industry::Transport,
        Industry::Information,
        Industry::Finance,
        Industry::Professional,
        Industry::HeadOffices,
        Industry::AdminSupport,
        Industry::Education,
        Industry::HealthCare,
        Industry::ChildCare,
        Industry::Recreation,
        Industry::Accommodation,
        Industry::Government,
        Industry::LawEnforcement,
        Industry::Military,
    ];

    pub fn index(self) -> usize {
        use Industry::*;
        match self {
            CropFarming => 0,
            AnimalFarming => 1,
            Forestry => 2,
            Fishing => 3,
            Mining => 4,
            Utilities => 5,
            Construction => 6,
            Manufacturing => 7,
            Wholesale => 8,
            Retail => 9,
            Transport => 10,
            Information => 11,
            Finance => 12,
            Professional => 13,
            HeadOffices => 14,
            AdminSupport => 15,
            Education => 16,
            HealthCare => 17,
            ChildCare => 18,
            Recreation => 19,
            Accommodation => 20,
            Government => 21,
            LawEnforcement => 22,
            Military => 23,
        }
    }

    pub fn name(self) -> &'static str {
        use Industry::*;
        match self {
            CropFarming => "crop farming",
            AnimalFarming => "livestock farming",
            Forestry => "forestry",
            Fishing => "fishing",
            Mining => "mining",
            Utilities => "utilities",
            Construction => "construction",
            Manufacturing => "manufacturing",
            Wholesale => "wholesale",
            Retail => "retail",
            Transport => "transport",
            Information => "information",
            Finance => "finance",
            Professional => "professional services",
            HeadOffices => "head offices",
            AdminSupport => "administrative support",
            Education => "education",
            HealthCare => "health care",
            ChildCare => "child care",
            Recreation => "recreation",
            Accommodation => "accommodation and food",
            Government => "government",
            LawEnforcement => "law enforcement",
            Military => "armed forces",
        }
    }

    /// The section of `raws/staffing.txt` this industry is read from, if it
    /// is read directly rather than composed.
    fn section(self) -> Option<&'static str> {
        use Industry::*;
        Some(match self {
            CropFarming => "111",
            AnimalFarming => "112",
            Forestry => "113",
            Fishing => "114",
            Mining => "21",
            Utilities => "22",
            Construction => "23",
            Manufacturing => "31-33",
            Wholesale => "42",
            Retail => "44-45",
            Transport => "48-49",
            Information => "51",
            Finance => "52",
            Professional => "54",
            HeadOffices => "55",
            AdminSupport => "56",
            Education => "61",
            HealthCare => "62",
            ChildCare => "6244",
            Recreation => "71",
            Accommodation => "72",
            Government => "99",
            LawEnforcement | Military => return None,
        })
    }

    /// **Who this industry employs**, as shares of its jobs that sum to
    /// one, indexed by `Occupation::index`.
    pub fn staffing(self) -> &'static [f64; N_OCCUPATIONS] {
        &table()[self.index()]
    }

    /// The share of this industry's jobs held by one occupation.
    pub fn share(self, o: Occupation) -> f64 {
        self.staffing()[o.index()]
    }
}

/// **Sworn officers as a share of all police employment**: 70.4% in 2018
/// *(FBI UCR, Police Employee Data)*. The civilians — clerks, dispatchers,
/// jailers, mechanics — are staffed like the rest of government.
pub const SWORN_SHARE: f64 = 0.704;

/// **Active-duty strength, FY2024** *(DMDC)*: 1,280,652, against 726,000
/// appropriated-fund civilian full-time equivalents. The civilians are
/// staffed like the rest of government.
pub const ACTIVE_DUTY: f64 = 1_280_652.0;
pub const DEFENCE_CIVILIANS: f64 = 726_000.0;

/// The published data, embedded so the file is the only copy.
pub const RAWS: &str = include_str!("../raws/staffing.txt");

fn is_soc(code: &str) -> bool {
    let b = code.as_bytes();
    b.len() == 7
        && b[2] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 2 || c.is_ascii_digit())
}

/// **Every section of the file, as SOC code → employment.**
///
/// A line counts if it starts with a SOC code followed by a number; a
/// suppressed figure — `(7)` for a share too small to print, `(8)` for one
/// not released — is left out, and so is every line that is prose.
pub fn published() -> &'static BTreeMap<String, BTreeMap<String, f64>> {
    static CELL: OnceLock<BTreeMap<String, BTreeMap<String, f64>>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut out: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
        let mut section: Option<String> = None;
        for line in RAWS.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("== ") {
                section = rest.split_whitespace().next().map(str::to_string);
                continue;
            }
            let Some(key) = section.as_ref() else {
                continue;
            };
            let mut words = t.split_whitespace();
            let (Some(code), Some(count)) = (words.next(), words.next()) else {
                continue;
            };
            if !is_soc(code) {
                continue;
            }
            let Ok(v) = count.replace(',', "").parse::<f64>() else {
                continue;
            };
            out.entry(key.clone())
                .or_default()
                .insert(code.to_string(), v);
        }
        out
    })
}

/// **Sort one industry's published counts into occupations**, and scale
/// them to shares of its jobs.
///
/// Normalised over what is printed rather than divided by the published
/// total, because suppressed rows and rounding leave the printed groups a
/// fraction short of it — and a staffing pattern whose shares add to 0.998
/// quietly loses a fifth of a per cent of every employer's jobs.
fn from_counts(c: &BTreeMap<String, f64>) -> [f64; N_OCCUPATIONS] {
    use Occupation::*;
    let g = |code: &str| c.get(code).copied().unwrap_or(0.0);
    let less =
        |group: &str, parts: &[&str]| (g(group) - parts.iter().map(|p| g(p)).sum::<f64>()).max(0.0);
    let mut s = [0.0f64; N_OCCUPATIONS];
    s[Farmer.index()] = g("11-9013");
    s[Manager.index()] = less("11-0000", &["11-9013"]);
    s[Accountant.index()] = g("13-2011");
    s[BusinessSpecialist.index()] = less("13-0000", &["13-2011"]);
    s[ComputingSpecialist.index()] = g("15-0000");
    s[Engineer.index()] = g("17-0000");
    s[Scientist.index()] = g("19-0000");
    s[SocialWorker.index()] = g("21-0000");
    s[Legal.index()] = g("23-0000");
    s[Teacher.index()] = g("25-0000");
    s[ArtsAndMedia.index()] = g("27-0000");
    s[Doctor.index()] = g("29-1210") + g("29-1240");
    s[Nurse.index()] = g("29-1141");
    s[HealthTechnician.index()] = less("29-0000", &["29-1210", "29-1240", "29-1141"]);
    s[CareAssistant.index()] = g("31-0000");
    s[ProtectiveService.index()] = g("33-0000");
    s[FoodService.index()] = g("35-0000");
    s[Cleaner.index()] = g("37-0000");
    s[PersonalCare.index()] = g("39-0000");
    s[Sales.index()] = g("41-0000");
    s[OfficeClerk.index()] = g("43-0000");
    s[Fisher.index()] = g("45-3031");
    s[FarmWorker.index()] = less("45-0000", &["45-3031"]);
    s[Electrician.index()] = g("47-2111");
    s[Pipefitter.index()] = g("47-2152");
    s[Miner.index()] = g("47-5000");
    s[Builder.index()] = less("47-0000", &["47-2111", "47-2152", "47-5000"]);
    s[Mechanic.index()] = g("49-0000");
    s[ProductionWorker.index()] = g("51-0000");
    s[Driver.index()] = g("53-3032") + g("53-3033");
    s[MaterialMover.index()] = less("53-0000", &["53-3032", "53-3033"]);
    normalise(s)
}

fn normalise(mut s: [f64; N_OCCUPATIONS]) -> [f64; N_OCCUPATIONS] {
    let total: f64 = s.iter().sum();
    if total > 0.0 {
        for v in s.iter_mut() {
            *v /= total;
        }
    }
    s
}

/// **The whole table**, built once.
fn table() -> &'static Vec<[f64; N_OCCUPATIONS]> {
    static CELL: OnceLock<Vec<[f64; N_OCCUPATIONS]>> = OnceLock::new();
    CELL.get_or_init(|| {
        let data = published();
        let read = |i: Industry| {
            let key = i.section().expect("a directly published industry");
            let counts = data
                .get(key)
                .unwrap_or_else(|| panic!("raws/staffing.txt has no section {key}"));
            from_counts(counts)
        };
        let government = read(Industry::Government);
        Industry::ALL
            .iter()
            .map(|&i| match i {
                Industry::LawEnforcement => {
                    // Sworn officers, and civilians staffed like the rest
                    // of government.
                    let mut s = government.map(|v| v * (1.0 - SWORN_SHARE));
                    s[Occupation::ProtectiveService.index()] += SWORN_SHARE;
                    normalise(s)
                }
                Industry::Military => {
                    let uniformed = ACTIVE_DUTY / (ACTIVE_DUTY + DEFENCE_CIVILIANS);
                    let mut s = government.map(|v| v * (1.0 - uniformed));
                    s[Occupation::Soldier.index()] += uniformed;
                    normalise(s)
                }
                _ => read(i),
            })
            .collect()
    })
}

/// **Which industry a kind of works belongs to.**
///
/// Exhaustive, so a new kind of site cannot compile until somebody has said
/// what it is. A butcher here is a meat plant, not a shop counter, and a
/// depot lands and sells on goods from abroad, which is wholesale.
pub fn industry_of_site(kind: crate::econ::SiteKind) -> Industry {
    use crate::econ::SiteKind::*;
    match kind {
        Farm => Industry::CropFarming,
        Pasture => Industry::AnimalFarming,
        Forestry => Industry::Forestry,
        Mine | IronMine | OilField => Industry::Mining,
        PowerPlant => Industry::Utilities,
        Builders => Industry::Construction,
        Butcher | Mill | Factory | Works | Steelworks | Cracker | MachineWorks | CementWorks
        | Pharma | ChemicalWorks => Industry::Manufacturing,
        Hospital => Industry::HealthCare,
        Shop => Industry::Retail,
        Depot => Industry::Wholesale,
    }
}

/// **Which industries a private service sector is**, and in what shares.
///
/// Offices are not one industry: professional services, administrative
/// support, finance, publishing and telecoms, and head offices, weighted by
/// their published employment *(OEWS May 2023: 10.75M, 9.50M, 6.26M,
/// 3.04M, 2.77M)*.
pub fn industries_of_sector(sector: crate::services::Sector) -> &'static [(Industry, f64)] {
    use crate::services::Sector::*;
    match sector {
        Construction => &[(Industry::Construction, 1.0)],
        Hospitality => &[(Industry::Accommodation, 1.0)],
        Recreation => &[(Industry::Recreation, 1.0)],
        Office => &[
            (Industry::Professional, 10_747_310.0),
            (Industry::AdminSupport, 9_496_560.0),
            (Industry::Finance, 6_258_320.0),
            (Industry::Information, 3_039_950.0),
            (Industry::HeadOffices, 2_771_010.0),
        ],
    }
}

/// **Which industry a public service is.** State schools and hospitals are
/// counted with education and health care, as the statistics count them.
pub fn industry_of_service(service: crate::state::Service) -> Industry {
    use crate::state::Service::*;
    match service {
        Education => Industry::Education,
        Health => Industry::HealthCare,
        Administration => Industry::Government,
        Safety => Industry::LawEnforcement,
        Defence => Industry::Military,
        Family => Industry::ChildCare,
    }
}

/// **The jobs an economy has, by industry**, in each town.
///
/// Every source that already counts its own people, sorted by what it is: a
/// works by its kind, a shop's fixtures, the private services by sector, the
/// state's posts by service, and a carrier's drivers where its depot is.
pub fn jobs_by_industry(econ: &crate::econ::Economy) -> Vec<[f64; N_INDUSTRIES]> {
    use crate::econ::SiteKind;
    let n = econ.markets.len();
    let mut out = vec![[0.0f64; N_INDUSTRIES]; n];
    let grid = econ.grid.capacity();
    for site in econ.ledger.sites.iter() {
        if site.market >= n {
            continue;
        }
        let industry = industry_of_site(site.kind).index();
        if let Some(b) = &site.fitted {
            out[site.market][industry] += b.staff();
        }
        let Some(r) = site.recipe else { continue };
        let rated = match site.kind {
            SiteKind::PowerPlant => grid,
            _ => site.throughput,
        };
        out[site.market][industry] +=
            crate::labour::rated_headcount(rated, crate::econ::RECIPES[r].labour);
    }
    if let Some(svc) = econ.services.as_ref() {
        for m in 0..n {
            for sector in crate::services::Sector::ALL {
                let parts = industries_of_sector(sector);
                let weight: f64 = parts.iter().map(|p| p.1).sum();
                for &(i, w) in parts {
                    out[m][i.index()] += svc.posts_in(m, sector) * w / weight;
                }
            }
        }
    }
    for m in 0..n {
        if let Some(gov) = econ.government(m) {
            for service in crate::state::Service::ALL {
                // Counted at the hospital, with the works.
                if service.staffed_at_its_sites() {
                    continue;
                }
                out[m][industry_of_service(service).index()] += gov.posts_for(econ, m, service);
            }
        }
    }
    // **A carrier's drivers are its drivers**, and the firm employs clerks,
    // loaders and mechanics besides — so its jobs are the drivers over the
    // share of transport jobs that drive.
    if let Some(freight) = econ.logistics.as_ref() {
        let driving = Industry::Transport.share(Occupation::Driver).max(1e-9);
        for c in freight.carriers.iter() {
            if c.home < n {
                out[c.home][Industry::Transport.index()] += c.drivers() / driving;
            }
        }
    }
    out
}

/// **The jobs an economy has, by occupation**: every industry's jobs spread
/// over the people it employs.
pub fn jobs_by_occupation(econ: &crate::econ::Economy) -> Vec<[f64; N_OCCUPATIONS]> {
    jobs_by_industry(econ)
        .iter()
        .map(|town| {
            let mut out = [0.0f64; N_OCCUPATIONS];
            for i in Industry::ALL {
                let jobs = town[i.index()];
                if jobs <= 0.0 {
                    continue;
                }
                for (o, share) in i.staffing().iter().enumerate() {
                    out[o] += jobs * share;
                }
            }
            out
        })
        .collect()
}
