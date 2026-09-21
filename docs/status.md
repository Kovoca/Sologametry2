# Scale Sim — status and plan

**The authoritative record of what is built, what is not, and what order
the rest goes in.** Where this file and the specifications disagree, this
file describes the repository and the specs describe the target.

It exists so that neither the owner nor an assistant has to hold the state
of the project in their head. `CLAUDE.md` records the *reasoning* and the
calibration figures; `docs/session-log.md` records how it got here; this
file records **where it is and where it is going**.

---

## Refresh the numbers before trusting them

A number somebody types goes out of date silently, in whichever direction
flatters. Every figure in the header below is produced by a command, and the
command is given so it can be re-run rather than believed.

```bash
git rev-parse --short HEAD
ls src/*.rs | wc -l ; ls src/bin/*.rs | wc -l
cat src/*.rs src/bin/*.rs tests/*.rs | wc -l
cargo test --release 2>&1 | grep -E "^test result" \
  | awk '{gsub(/;/,""); p+=$4; f+=$6; i+=$8; n++} \
         END {print "binaries:", n, "passed:", p, "failed:", f, "ignored:", i}'
```

Do not pipe the suite through `tail` to see the summary — it truncates away
the per-binary results the total is built from. That mistake was made while
writing this file.

| | | measured |
|---|---|---|
| Commit | `744f18d` who pays for medicine | 2026-09-20 |
| Source | 68 modules, 18 diagnostic binaries, 70 test files | 2026-09-20 |
| Lines | ~117,000 including tests | 2026-09-20 |
| Tests | 91 binaries, **918 passed, 0 failed, 0 ignored** | 2026-09-20 |
| Build | clean; `cargo clippy --all-targets` clean | 2026-09-20 |

**The full release suite takes over ten minutes**, which is worth knowing
before planning a change that touches every module — and the travel-time
work below had to run it three times.

The total is measured across every binary including doctests. Counting
`#[test]` attributes statically instead gives a smaller number and misses
the doctests — which is where this project keeps its `compile_fail` proofs,
so it misses precisely the ones that matter most.

---

## The goal

From `docs/scale-sim-design-doc.md`: a single-player, solo-built Rust
simulation where the player **drops into a procedurally generated universe
as a single individual and shapes it to the degree they are able** — which
means: if they have the means they can shape as they please, and if they do
not, they cannot. Nothing is scaled to them.

- **Interaction is Dwarf Fortress adventure mode / CDDA.** You stand
  somewhere, you see and hear what is near you, you talk to people, you pick
  things up, take them apart and make things out of them.
- **Scale target** is individual to intergalactic; **build order starts
  planetary** and expands outward. Space is deferred on purpose.
- **A modern technological baseline**, not a medieval growth arc.
- Tarn Adams' four principles: do not overplan; model independent underlying
  elements and let results emerge; nothing exists unless it has
  player-perceptible impact; base the model on real-world analogs.
- This project's own additions: **calibrate against real published figures
  rather than taste**, and **delete a mechanism and require its gate to go
  red** before believing the gate tests anything.

---

## The scope, as stated by the owner

Recorded here because it is the target the tables below are scored against,
and because it existed only in conversation before this.

| | wanted |
|---|---|
| **Interaction** | DF adventure mode / CDDA: walk, look, talk, take, make. "To the degree they're able" means means, not permission |
| **Economy** | realistic micro **and** macro. Finance, banks, **stock markets** |
| **Industry** | factories producing goods through **labour and machinery**; refining, so oil and chemical; **assembly lines and machinery as multiple tiles working in tandem** |
| **Networks** | electrical grids, **water systems**, **telecoms** |
| **Vehicles** | individual to cargo to **mining equipment** |
| **Marine** | boats from individual to **industrial cargo and fishing**; **oil rigs** |
| **Jobs** | **made through need**: factories, farmers, miners, fishermen, businessmen, office workers, accountants; **employment by company size**, small to large to international; **the military**; **government agencies, ministers, mayors** *(owner, 2026-09-16)* |

Bold is what does not exist yet. The rest is built or substantially built.

---

## Where each area actually stands

**Built** means it exists and is gated by tests. **Partial** means the
mechanism exists but not the scope wanted. **Absent** means not started.

### World

| area | state | what exists | what is missing |
|---|---|---|---|
| Terrain, climate, hydrology | **built** | elevation, erosion, rivers, lakes, rainfall, temperature, water table | — |
| Geology | **built** | rock provinces, ore/coal/petroleum grades, soil fertility and depth | — |
| Biota | **built** | Miami-model NPP, biomass, game, timber, six crops, seven livestock | `biota.game` has no consumer; nothing hunts |
| Polities and settlement | **built** | emergent borders, siting on reachable water, real rank-size, villages | no history sim; polities never consolidate |
| Roads and networks | **built** | traffic-derived classes, passes and tunnels, maintenance deficit | rail, ports as places, airfields |
| Scale ladder | **built** | region cell to locality to plot to tile, arithmetic asserted | — |

### Economy

| area | state | what exists | what is missing |
|---|---|---|---|
| Commodities and production | **built** | 18 commodities, 32 recipes, 21 site kinds, conserved ledger | more of the tree: limestone, aggregate, copper |
| Prices and trade | **built** | cost separated from price, nine typed value kinds, arbitrage bounds | — |
| Freight and logistics | **built** | Dijkstra routing, cargo in transit, road capacity booked, **transit time reads the road surface leg by leg** | multi-drop rounds; the quay-to-town leg is priced, not ridden; road *condition* raises cost but not yet time |
| Money | **built** | one write path, conserved, four account kinds, unpaid tracked by reason | firm-to-firm unpaid is the largest open category |
| Banking and credit | **built** | loans create deposits, capital adequacy binds, real credit tiers | central bank, policy rate, bonds |
| **Stock markets** | **absent** | firm *form* is known — sole trader, partnership, company | shares, valuation, issuance, an exchange, ownership registers |
| The state | **built** | one per nation, real tax capacity, public payroll, services | provinces — no rung between town and nation |
| The border | **built** | import and export parity, duty hook, exchange rate, capital account | a currency per nation; reserves |
| Labour and services | **built** | recipe-derived headcount, contracts, rotas, promotion, sectors, **skill reaching what a works makes** | retail headcount about 4.9 points short of real; skill does not yet reach scrap |
| Households | **built** | basket by need, repair/reuse/second-hand, utility arrears, scrap | no "eating cheaply"; buys on credit nobody extended |

### Things

| area | state | what exists | what is missing |
|---|---|---|---|
| Materials | **built** | 39 materials, properties, hazard, recycling routes | — |
| Items | **built** | definition/instance split, seven content kinds, full BOMs, quality apart from condition | — |
| Crafting | **built** | capability-with-a-figure tools, three kinds of time, 13 outcomes, batch faults | — |
| Work in progress | **built** | real intermediate objects, identity rules, interruption per process | — |
| Teardown and scrap | **built** | nine intentions, joint table, recovery grades, real scrap prices | — |
| Scheduling | **built** | benches booked by the **minute**, capacity, outage per process | **nothing drives its diary** — durations exist, a tick to spend them in does not |
| **Machines as tiles** | **absent** | seven fixtures, all retail: till, shelving, rack, dock, counter, chiller | footprint, input and output ends, conveyors, a line in tandem |
| **Weapons** | **absent** | a rifle exists as an item with a real bill of materials | combat, projectiles, wounds |
| Fields and hazards | **absent** | — | fire, smoke, gas, flooding |

### Infrastructure

| area | state | what exists | what is missing |
|---|---|---|---|
| Electrical grid | **built** | five levels, N-1 transmission, faults sized by level, merit order, spares, mutual aid | routing to every meter; sub-day clearing |
| **Water and sewer** | **absent** | the water *table* is real ground truth; "sewer" is a Z level under a street | mains, treatment, pressure, drainage as graphs |
| **Telecoms** | **absent** | one `bool`, `comms_up`, gating fault reporting | nodes, backhaul, coverage, capacity |

### Vehicles and craft

| area | state | what exists | what is missing |
|---|---|---|---|
| Road vehicles | **built** | 13 part kinds, capability from parts, structure severs, centre of mass, real drag | ordinary cars, doors and windows, driving, collision with the world |
| **Mining equipment** | **absent** | `LandGear` is the only working attachment | excavators, loaders, draglines, haul trucks |
| **Boats** | **absent** | sea freight is a share of cargo value; berth depth reads the sea floor | hulls, holds, nets, engines — any vessel as an object |
| **Oil rigs** | **absent** | oil fields are land sites; `offshore` means wind turbines | platforms, derricks, offshore logistics |

### Mind and people

| area | state | what exists | what is missing |
|---|---|---|---|
| Personality and emotion | **built** | measured Big Five substrate, appraisal, concerns apart from episodes | — |
| Memory and witness | **built** | provenance, reconstruction, line of sight and acoustics | — |
| Needs, relations, speech | **built** | semantic satisfaction, eight-dimension relations, no telepathy (compile-proofed) | — |
| Growth, coping, scaling | **built** | doubt before change, strain ladder, fidelity-invariant advance | — |
| Conversation | **built** | assembled from memory, witness, relations, coping; no privileged channel for the player | — |
| Population | **built** | sampled cohorts, ageing, births, education, inheritance | a sampled pocket is not the household pool |

### Persistence and spine

| area | state | what exists | what is missing |
|---|---|---|---|
| Durable identity | **built** | `Id<T>` arenas, `Key<T>` registry, seven rules each sabotage-gated | — |
| Save format | **built** | hand-rolled, explicit wire codes, floats as bits, four stores, crash points tested | — |
| Ground overlay | **built** | per-layer changes, base hash, rebase refusal | — |
| Economy codec | **built** | every enum variant named, impossible worlds refused | — |
| The root | **partial** | one clock, one day order, **four of eight parts saveable**: clock, seed, ground, economy | the item store, the people, buildings and utilities, vehicles and work orders |
| **Play** | **absent** | generators and 16 inspection binaries | **no player, no commands, no turn loop, no save loop** |

---

## One clock, a tick nothing has chosen, and a diary nothing drives

**This section said "there are two clocks" and that was the wrong name for
it.** A 24-hour and a 12-hour clock are one scheme presented two ways, and
minutes are not a rival unit to seconds — in CDDA a turn is a second *and*
every action carries a duration measured in those turns, so a craft, a
butchering or a long read runs over many turns and can be interrupted.
Durations attached to actions are the same clock at a different magnitude,
which is what this project already has:

```text
game.rs      day: u64      private, no setter, `advance` the only mover
schedule.rs  minutes       setup, paid and owner minutes, fatigue over a
                           540-minute day, tool wear per minute, spoilage
                           after N minutes, outages split at the exact minute

35 minutes of oven      = 2,100 ticks at a second
a 540-minute working day = 32,400 ticks
```

So `schedule.rs` is not a competing clock. It is a set of **CDDA-shaped
activity durations already written** — and nothing advances its diary. What
is missing is a conversion and a driver, not a reconciliation.

The economy meanwhile has **no sub-day time at all**: every "minute" in
`econ.rs` is prose in a doc comment, and the only sub-day structure in the
whole world model is `day % 7` for the weekday.

What actually has to be decided is narrower than it first looked:

- **The base tick.** A second, or near it — a conversation, a fight, a door,
  a tile of walking all live there. The 24h/12h clock is display.
- **Durations on actions**, which `craft.rs` and `schedule.rs` already carry
  and which only need expressing in ticks.
- **What the rest of the world does meanwhile** — the one genuinely open
  question. The player spends 2,100 ticks at an oven and the economy cannot
  step 2,100 times. CDDA answers this with the reality bubble: a small region
  live, everything outside caught up on return. **This project has already
  built the proof that catching up is safe** — `scaling.rs` and
  `coping::advance` assert that an analytic advance over an interval equals
  that many daily steps, split at every discontinuity, with the one
  irremovable divergence named and its direction gated. Built for minds,
  never asked of the economy.
- **The day order becomes a time-of-day order**, and this is the piece that
  stays real work. `game.rs` encodes two rules learned the hard way: shops
  trade before anybody counts who worked, and a service is paid before wages
  fall due. Once the player can act at 14:30, "has the shop restocked yet?"
  has an answer and those become clock rules. A modelling decision, not a
  refactor.
- **The grid wants an intermediate rate on its own merits.** Merit order
  clears every half hour in life, and the whole point of a merit order is
  that the marginal plant moves through the day. Clearing once a day means
  the evening peak does not exist.
- **Most of the model must not move to the tick rate**, and not for
  performance — because it would be meaningless. There is no wheat price at
  14:30:07.

**And the cost of a day is measured, and roughly cubic in towns** — 16
markets 1.4 s per 100 days, 40 gives 14.1 s, 80 gives 116.7 s, 160 gives
1,489.5 s. That is what blocks the regional rung, and it constrains how much
of the world can be live at once under any clock.

---

## Tracked gaps

Real defects, each visible in a test or measurable in a binary.

1. **A person is joined, and nothing in the running world talks to one.**
   `Person` carries a `Mind`, a `Memory` and a directed map of
   `Relationship` — so the man with a trade is the man with convictions,
   the man who remembers, and the man who has an opinion of you.
   `converse::ask_of` takes the person rather than three arguments a caller
   promised were his, which is the defect closed.

   **What remains is a caller.** `converse` is exercised only by tests,
   because there is still no player and no turn loop, and nothing in
   production reads `Person::mind` — it is the prerequisite the
   value-gated social skills sit on. The type now enforces the join; the
   running world does not yet use it.

   **And nothing derives a town's values.** `Mind::draw` takes a culture
   and every person is drawn against a blank one. `custom.rs` derives
   *norms* from the ground — size, remoteness, climate, trade — and says
   outright it is deliberately mundane, grand moral questions being
   `mind::Value`. There is no equivalent for those, so a place whose people
   hold nothing in common is not yet a culture.

   What it was, recorded because the shape is worth keeping:

   *There was no whole person. Two halves sharing only an `Id<Person>` by
   convention, and nothing constructing both for the same individual:*

   | | holds | lives in |
   |---|---|---|
   | `person::Person` | trade, skill, practice, money, housing, employment, household | `populace.rs`'s arena, sampled per market |
   | `mind::Mind` | personality, **values**, memory, relations, needs, coping, growth | nothing — `Mind::draw` is called by tests and `bin/minds` |

   `converse::ask` takes `Id<Person>` handles for speaker and listener *and*
   a `&Mind`, `&Memory` and `&Relationship` as separate arguments — so the
   two halves being the same individual is **a caller's promise rather than
   a type**. `GameState` owns `folk: Option<Populace>` and its own header
   says minds "still live where they were". `populace.rs`, `labour.rs` and
   `game.rs` mention `Mind` nowhere at all.

   **The consequence is the one that matters for the design.** The NPC you
   would walk up to and lie to does not exist: the one with a job, a wage
   and a house has no values, no memory and no relationships, and the one
   that can be lied to is employed by nobody, houses nobody and is paid by
   nobody. It also blocks value-gated social skills outright — DF gates
   Liar on a low regard for truth, and a `Person` has no values to gate on.

   `Mind::draw(rng, culture)` makes the join feasible, and `custom.rs`
   already derives a culture from the place. What it cannot be is *redrawn
   on demand*: a mind accumulates memory, concerns and growth entries that
   are not derivable from a seed, which `scaling.rs` already records —
   *"a seed is not sufficient save state"*.

   *The consequence: the person you could walk up to and lie to did not
   exist — the one with a job and a house had no values, and the one that
   could be lied to was employed by nobody.*

2. **Firm-to-firm supply goes unpaid**, about 2.6e11 over 700 days — the
   largest unpaid category, ahead of households at the till.
   **And the shape of it is worse than the figure**, on a reviewer's
   point, 2026-09-20. `Treasury::pay` caps a transfer at the payer's
   balance, adds the shortfall to a counter, and **the goods have already
   moved**. So nothing anywhere records a receivable: the supplier is
   poorer, the buyer holds the stock, and conservation is satisfied
   throughout — which is exactly how money can conserve while the
   transactions are wrong. It then propagates, because a supplier that was
   not paid cannot meet payroll and lays people off, and that is the
   unemployment the bands are being read against. **Until every delivery
   has a named commercial outcome — paid, owed, or an internal transfer —
   matching a calibration band is weak evidence, because a missing payment
   can be offsetting another error.**
   **The design, after a reviewer corrected the first one**, 2026-09-20.
   The first sketch was one edge, `(debtor, creditor) -> amount`, on the
   argument that a single record cannot disagree with itself. It cannot,
   and **it also cannot carry terms**: two deliveries of 100 on day 0 net
   30 and 200 on day 20 net 50 are 100 due on day 30, and collapsing them
   into "300 owed" destroys that. So the authoritative record is **per
   invoice** — a durable key, both parties, the delivery it came from, the
   amount, the invoice date, the due date and what has been settled
   against it — with both firms' positions *derived* by walking invoices,
   which keeps the one-record property and adds terms. Obligations merge
   only where terms and due date are identical.

   - **Credit is agreed before the goods move**, which is the reverse of
     the first sketch. If goods transfer and an obligation appears
     whenever payment fails, **every supplier is a compulsory lender**.
     The purchase decision settles what is payable now, what credit the
     supplier will extend, whether existing arrears cut that off, and
     therefore **what quantity actually transfers** — and the inventory
     move, the cash and the invoice commit together.
   - **Outstanding and overdue are different states.** A net-30 invoice
     is an ordinary trade asset before day 30 and arrears after it, and
     it is arrears that restrict further supply.
   - **A receivable is not cash**, and this is the part most likely to go
     wrong. The supplier correctly books a sale and an asset and still
     cannot make payroll, so working capital, collections, partial
     payment, restricted credit and firms that do not recover all have to
     exist — otherwise the unpaid counter simply becomes an unbounded
     debt stock and the defect has moved rather than closed. A collection
     reduces the invoice and **is not revenue a second time**; writing a
     debt down as doubtful is the creditor's loss and **does not forgive
     the debtor**.
   - **The transition can manufacture its own crisis.** Delaying receipts
     against unchanged opening cash is a startup liquidity shock that is
     an artefact of the change, so whatever opening balance or financing
     carries it is stated explicitly rather than absorbed.
   - **It stays at the commercial layer.** `Treasury::pay` carries wages,
     tax, public spending, premiums and claims; a shortfall there must
     **not** silently become trade credit. Only a firm-to-firm goods
     purchase takes the new path.
   - **Terms and DSO are not interchangeable settings.** Net 30 is
     contractual and is an input; DSO measures collection performance and
     depends on customer behaviour, business mix and the calculation
     method, so it is an **output the run reports** and is compared
     against — never a dial. Setting invoices to clear at 37 days would
     reproduce the chosen number and test nothing, and the two figures
     quoted here (about 37 days domestic, a cross-industry median nearer
     56) need their country, period, sector coverage and method before
     they are targets at all.

   **What closes this item is behaviour, not a smaller counter.** A
   purchase without cash or authorised credit cannot move unsupported
   goods; two invoices between one pair keep different due dates; a
   partial payment reduces the right balance exactly once; an insolvent
   buyer loses further unsecured supply; save and reload preserve
   invoices, partial settlements and subsequent outcomes; and the soak
   reports credit sales, collections, outstanding, overdue and losses
   **separately**.

   **Built, measured, and deliberately not wired in**, 2026-09-20.
   `src/credit.rs` is the record — invoices with terms, due dates, partial
   settlement, doubtful debts, per-pair limits, oldest-due-first
   collection, and a purchase decision that refuses what is neither paid
   for nor lent — with eight gates in `tests/credit.rs` and nine sabotages
   all red. What it is not is connected to `distribute`, and the reason is
   a measurement. Seed 7 over two years, against 6.4% unemployment and
   five days of food cover: cutting credit one day past due gave **82.6%**
   unemployment and 0.04 days of cover; on stop at sixty days, 47.9%; with
   a working-capital floor, 59.2% and a debt stock of 9.7e12. **With the
   limit made infinite — nothing refused at all — it still reached 51.0%**,
   which is what rules the refusals out as the cause and names collections
   instead.

   **So the unpaid counter is load-bearing**, and that is the finding. This
   economy functions because firms take goods they cannot pay for; the day
   a delivery requires cash or agreed credit, the circuit that was being
   papered over fails in the open. **Item 3 is therefore the prerequisite
   and not a neighbour of this one** — a household buying on credit nobody
   extended is the same defect one layer down, and until it is closed
   there is nothing for a supplier to be paid out of. One correction was
   kept from the attempt because it is right regardless: **ordinary
   lateness does not stop supply.** Net-30 terms against a DSO of 37-56
   days means the average invoice is settled after its due date and the
   supplier goes on supplying; what stops an account is serious arrears,
   60-90 days in real practice.
3. **A household buys the basket whatever its balance.** `consume_households`
   records the shortfall as unpaid, so a poor town eats like a rich one on
   credit nobody extended. Fixing it moves hunger.

   **Built, measured, and not in the tree**, 2026-09-21 — the same standing
   as the shutdown rule and the other changes this project has tried and
   reverted with the measurement kept. The rule
   is *over a counter you pay; on a bill you can fall behind* — groceries
   are paid for or not taken, while the power (`utility.rs` bills a month in
   arrears) and the hospital send an invoice. What a household gives up
   when it is short comes from the published **income elasticity** of each
   thing, read off the Consumer Expenditure Survey by income quintile
   (drugs 0.26, meat 0.40, food at home 0.53, household durables 1.29), so
   **Engel's law is an output** and the surprise is the real one: medicine
   is the least income-elastic thing a household buys. Six gates in
   `tests/counter.rs`, seven sabotages all red. Measured over five years on
   three worlds: world 23's unemployment peak 18.6% -> 15.4% and its end
   16.0% -> 13.4%, worlds 7 and 11 unmoved, no famine anywhere, and
   household purchases unpaid down 34% on the 400-day counter measurement.

   **What blocks it is five gates across two fixtures, on one root**, and
   the reason is worth more than the change. On the two-town slice food
   settles at **635 against a cost of 908** — a demand-deficient glut,
   because the customers have run out of money, which fails the spec's own
   acceptance test that an undisturbed economy settles at cost. And
   `carriers_have_nothing_to_do_in_a_country_that_is_already_even` runs
   four hundred days on `slice::symmetric`, which by then holds
   **96.4% of its money abroad** and its households hold 0.0% — a purse of
   2,850 against a daily basket of 108,759, or **a fortieth of one day's
   shopping**. Nothing that reads a purse can be judged in a country with
   no money in it: an 11% purse difference between the towns becomes a 40%
   difference in days of cover, because purchases are purse-proportional by
   construction. The drain is the trade deficit this project already
   documents, and on a generated world `exchange.rs` answers it; on this
   fixture nothing does.

   **And the fixture's own defects are being cleared one at a time.** Its
   power station opened on twenty thousand tonnes of coal with nothing to
   refill it — a four-hundred-day run burns most of that pile — so it has a
   colliery now, sized on what the station actually draws, and `bin/
   symmetry` reports nothing diverging beyond float noise. The goods depot
   is the same defect in money rather than in coal: one town wants 82.2 t
   of retail goods a day, which lands at about 75,200, while the most grain
   an export-agriculture nation is *built* to sell — three times its own
   milling need, `region.rs`'s own ceiling — earns about 27,500. **A
   country that imports all its manufactured goods cannot pay for them by
   exporting grain**, and this one exports nothing at all.

   So the prerequisite is the fixtures' own money circuit, and the order is
   **item 3 waits on that**. The work is on the `counter-at-the-till`
   branch, deliberately not green, with the measurement in its commit
   message. Three real defects were found on the way and
   two are shipped; the third — nothing crosses the country for less than
   the carriage — is held with this change, because the condition that
   produces one is only reached when a household's purse decides what it
   buys.
4. **A sampled person's pocket is not the household pool.** Promoting
   somebody to detail creates their savings. The reification problem, not an
   accounting one.
5. **Profit is still about half of household income** against a real ~40%,
   because firms pay no rent, interest or depreciation — all three fall into
   the residual.
6. **Retail headcount about 4.9 points short**, because shop fixtures are
   sized on commodity tonnage and real retail follows customers and floor.
7. **The water table is too deep in the typical cell** — median 85 m against
   a far shallower reality. It cuts about 60% of candidate settlement sites.
8. **`biota.game` has no consumer.** Generated, and nothing hunts it.
9. **No history sim.** Polities are partitioned geographically and never
   consolidate into great powers.
10. **The trade multiples are the floor of the observed band**, not its
   middle; recentring at 8 days of food is a change to be measured.
11. **`GameState` saves four of eight parts**, measured by serialising rather
    than typed in.
12. **A town with no reachable quay imports at the cheapest possible
    price.** `inland_leg` returns 0.0 when `nearest_quay` is `None` — and
    `None` there means *unreachable*, not *free*, so the most cut-off town
    in the world lands its goods as though the dock were in its own high
    street. Meanwhile `worth_exporting` requires the town's **own** quay, so
    the same country can never sell anything: a guaranteed one-way drain.
    The same shape as the `freight_between` fallback, pointing the other
    way. Both fixtures are in exactly this state, which is why they cannot
    pay their way — and fixing the rule without giving them a quay would
    starve their canneries of tinplate, so it belongs with that decision.
    *(Appended rather than inserted: item numbers are referenced from
    `src/` and from `CLAUDE.md`, so inserting one renames every later
    defect — a position read as a name, which this project has a rule
    about.)*

---

## The plan

Ordered by what unblocks the most, and by what would have to be built twice
if taken out of order.

**The owner's decision, 2026-09-16: the world functions without the player
first.** *"Getting the world to function as it would in game without the
player is the first step of the journey."* People buy food, furnishings and
property, learn things, have families; factories and machines produce goods;
trades and construction happen — all of it with nobody at the controls. The
player comes after, into a world that is already running. The clock and the
player below move behind this phase accordingly.

### Phase A — the world runs itself

Two facts decide the order, both measured on 2026-09-16:

- **The item layer is not in the living world.** 156 items with full bills
  of materials exist, and `basket.rs` models a household buying, mending and
  discarding them — but `standard_catalogue()` is called only by
  `bin/basket` and `bin/make`. `populace.rs`, `person.rs`, the economy and
  `game.rs` never touch it, and `GameState::items` is filled by nothing. The
  running world is 18 commodities in tonnes.
- **Items are authored as Rust.** All 156 are hand-written in one function,
  `item.rs:962-2200`, about 1,240 lines before their bills of materials. The
  owner's stated scope is **thousands of items**, which as code is tens of
  thousands of lines, a recompile and a ten-minute suite per item, and
  nothing the owner can add or correct without a programmer. CDDA's content
  is JSON and DF's is raw text for exactly this reason. The safety net
  already exists — `bom::validate`, and item keys derived from names rather
  than position — so moving 156 now is cheap and moving a thousand is not.

### What the soak found, 2026-09-16, and what it changed

`cargo run --release --bin soak` ran the integrated world for the first time
— several nations, their people, the root's own day, nothing commanded.
Five years on seed 7: **unemployment 6.2% to 64.3%, homelessness 1.2% to
18.8%, households' money down 81%**, with money and tonnage conserved every
day. It went abroad: households -4.15e10, abroad +4.72e10, net -0.016.

**The cause is the trade deficit**, and it is a calibration fault: every
town imports 30% of its steel, machinery and timber, so the world imports
2.2x what it exports against the US's 1.29x.

**The spiral is that nobody can respond.** No unemployment benefit, no
emigration, no changing trade, no eating cheaply — and every state held
flat through a depression, 8.26e9 to 8.23e9, where a real one's deficit
widens automatically. The one response that exists, `leave_town`, is a
stand-in from early testing with typed thresholds (15 points less
unemployment, 20% cheaper food), and it correctly finds nowhere to go when
every town is at 64%.

**The owner's correction: NPCs function as players do.** Facing no work,
a person fills a niche, learns a new skill, retrains — which is why
schools, colleges and universities matter. That is spec **A4, marked
SETTLED and entirely unbuilt**: opportunities as side-effects of other
lives, pushed rather than scanned; goals with hysteresis; persistent plans
from task templates; four reconsideration triggers; failure as a normal
outcome; expiring reservations; a hard think budget.

**Education is what makes retraining possible, and adults have none.**
Only children enter it, at sixteen — *"the adults are given their skills;
the children must go and get them."* And schools are public posts counted
against population, not places with an intake anybody can apply to.

**Revised order for Phase A**, one change at a time, soak after each:

**A0.1 The A4 core. — BUILT, 2026-09-16.** `src/planner.rs`: leads pushed
by word of mouth and notices, hysteresis, persistent plans, the four
triggers, forgotten failures, expiring holds on openings, a think budget.
One template: take up a trade you are already qualified for. Gated in
`tests/planner.rs`, each gate checked by deleting its mechanism. Building it
exposed and fixed three older faults, each measured on three worlds: shifts
took two days; the sample was seeded from national shares instead of each
town's posts (half shop workers, doctors with no work); and the skill feed
trusted whoever happened to be sampled. Seed 7's year-five homelessness
went 15.8% to 3-4%. The planner's own effect is small and mixed in a world
already seeded where the work is, as it should be: 0.6-0.8% change trade a
year against a real ~10%.

**A0.1b Jobs made through need — IN PROGRESS, owner's order 2026-09-16.**
Before education, because retraining means something only once there are
real occupations to train for.
1. *Industries and the occupations they employ* — **built.**
   `src/occupation.rs` over `raws/staffing.txt`: 24 industries, 32
   occupations, from BLS OEWS May 2023, the EP matrix, DMDC and FBI UCR.
   A nation's workforce lands within a point of the US in most occupations;
   sales, drivers and material movers are half or less (distribution), and
   builders double.
2. *People hold occupations* — **built.** Work is offered by every
   employer to every occupation its staffing includes; pay is OEWS medians
   placed against the production worker at six days of food; the gate is
   typical entry education; 18 new skills. The skill feed now shrinks a
   small sample toward the ordinary. Homelessness falls in all three soak
   worlds. **Next inside this step:** the bottom of the pay scale sits under
   the 6-10 days-of-food band (food service 4.4), which is the recentring
   to measure on its own.
3. *Employers by size* — sole traders to multinationals, from published
   size distributions.
4. *Positions* — ministers, departments, mayors, councils: posts one person
   holds.

**Fixing what is wrong before moving on — owner's order, 2026-09-17.**
1. *The drain abroad* — **fixed.** Countries were built to make 75% of their
   goods and 70% of their steel, with no exports on the other side. Built to
   make what they need; hands follow the work done; a capital-account bug
   fixed. Households gain money in all three soak worlds.
   **And the unemployment statistic — fixed.** It was not firms selling
   under cost; it was seven places where goods or services changed hands
   with nobody paid: power, builders' materials, trade between towns, meat
   with no counter to sell it, health staff on the state's payroll twice,
   a tax levied on tills that could not pay it, and hospitals buying
   medicine out of their wage bill. Workforce unemployment ends at 11.1%,
   10.4% and 14.4% (`bin/soak`, seeds 7, 11, 23), every band in range at
   the end. **Still open:** builders meet 46-63% of payroll; hospital
   supplies cost 0.7-1.4x hospital payroll against a real third, which
   belongs with pay (item 2); world 7 lost live-cattle exports that only
   existed because nobody could buy meat, and its households end 17% down.
   A rule for where traders deliver was tried and rejected (CLAUDE.md).
2. *The lowest pay under the 6-10 days-of-food band* — **fixed.** The band
   was right; the anchor sat on its floor. A production worker was pinned at
   six days of food a day, so every occupation paid less fell under it — food
   service 4.4, sales 5.1. Measured, a production worker's eight hours buys
   **nine**: $180.72 (OEWS May 2025, 51-0000) against $20.12 of food a
   person a day ($2.51tn of US food spending in 2025, USDA ERS, over
   341,784,857 people, Census V2025). Food service comes out at 6.7 on the
   same arithmetic and sales 7.4, so the band holds. No cost moved — pay
   reaches costs as a ratio to the reference — and the rent, which is 30% of
   a production worker's day against a real 26%, rose with it. In the soak
   (seeds 7, 11, 23) homelessness and hunger fall in all three worlds,
   households' money improves in all three, and home ownership goes
   0.5/4.1/1.6% to 4.5/11.4/6.7% of the sample.
   **And it found an older defect: a person bought nothing but flour.** No
   meat, no goods, no power, no chemist — everything `econ` has debited its
   own households for all along — so honest pay left so much over that 71%
   of a cohort owned a house outright after 25 years against a real 26%.
   A person now buys the rest of a head's basket, **after the rent and out
   of what is left above a month of everything**: charged before the rent it
   put 11-17% of three worlds on the street, because people shopped their
   way out of a roof. Over 25 years ownership lands at **25.0% against a
   real 26%**, everybody housed, owners paid 15.4 days of food against 9.2
   (real owner households earn about 1.9x renters), and a house costs 6.1
   years of a production worker's pay against a real 7.1.
   **And the readout found a money printer older than the change, now
   diagnosed:** a driver holds **29-203 million days of food** in every soak
   world, baseline included, against a production worker's seven thousand.
   His log says exactly what he does — buy 24.1 t of medicine in Ashcombe
   for 88,512, sell it in Caldleigh for 177,546, **every day for 1,095
   days** at about a 100% margin. Two faults, and only the second is his:
   **the gap never closes**, because medicine is made in one town and wanted
   in all of them and neither bulk mechanism reaches it (`trade` is pairwise
   and priced, a haulier decides on days of cover) — which this file already
   exempts by name from the no-arbitrage gate; and **his pocket is outside
   the ledger**, so what he gains nobody pays, which is the known
   reification gap made visible rather than a new leak. Fixing the first is
   an end-to-end merchant for a commodity no shop sells, and it is the same
   instruction as item 9: the economy works on supply and demand, so a gap
   that wide has to be closed by somebody entering the trade. Next.
   **Still open:** missing one rent payment puts somebody out the same day —
   there is no arrears or notice for a tenancy where `utility.rs` has the
   lot for a power bill — which is what world 7's 4.8% homeless month is;
   world 23's unemployment ends 2.5 points worse because **poverty was
   staffing its mines** — trade changes fall from 82 to 37 over five years
   as people stop being desperate, and its steelworks and iron mine run at
   58% and 53% against 101% and 92%, which is item 6 (mobility at 1.2% a
   year against a real tenth) biting; a house
   is 10.6-13.7 years of a production worker's pay in a run world and 5.6 in
   a fresh one against a real 7.1 ($332,700 median home, Census 2020-24),
   and item 9 is what decides that level; and `econ::day_rate_here` still
   charges every firm six days of food a hand where the all-occupations
   median is 9.7 — the next change, and one that moves money from profit
   into payroll.
3. *Supervisors* — **fixed.** A supervisor is a rank inside a crew, not an
   occupation; each kind of work carries supervisors at its published crew
   size (OEWS: 7 to a supervisor in construction up to 22 among drivers and
   loaders), posts come from the town's jobs, the best-regarded with two
   years at the work are made up, and a world starts with its crews run.
   Management is reached by a degree or by running a crew. Supervisors end
   at 5.6%, 3.9%, 3.6% of the sample against room for 5.3% (real 5.1%);
   managers 8.1%, 7.7%, 8.0% against 7.5% (`bin/soak`, seeds 7, 11, 23).
   **And the work is reviewed** once a year: commended hands are considered
   for a post after one year instead of two; a warning, then a supervisor
   put back into the crew or a hand let go, and the worst let go at once.
   19% commended, 4% warned, 1.2-1.6% of the sample let go a year,
   supervisors put back 0-0.9% a year — bounded by federal removals
   (GAO-18-48) and all US layoffs and discharges (JOLTS).
   **Still open:** posts nobody inside qualifies for go unfilled (no outside
   hiring); no chief executive or middle management until employers own
   several sites; misconduct is not a route to dismissal yet.
4. Contract mixes from occupation-level figures.
5. Distribution: warehouses and van drivers missing; builders doubled.
6. Occupational mobility 1% a year against 10%.
7. *Rank structures, as data a player can write* — owner's notes,
   2026-09-17. A military is **its structure**: which ranks it has, in what
   order and under what names, how they group (enlisted, NCOs, warrant
   officers, officers, general officers), how many of each it carries, and
   **the standards** for moving between them — time in the rank and in
   service, schooling, performance, and posts that must be free. Real forces
   differ in all of it, so each nation's default comes from its culture;
   and **a player who comes to command a force can replace it with their
   own**, so rank structures are authored data (the raws format the owner
   chose for items), never tables in the code, and every one — shipped or
   player-written — goes through a validator the way `bom::validate`
   checks an item: a ladder with no bottom rung, a promotion to a rank that
   does not exist, or a standard nobody can ever meet is refused with the
   reason. NATO's OR-1..OR-9 and OF-1..OF-10 codes are the common frame a
   national or invented structure maps onto, so the rest of the model can
   ask what a rank *is* whatever it is called.
   Verified so far: US officers including warrant officers are about 18% of
   the force, warrant officers 9% of the officer corps, company grade 56%,
   field grade 35%, general and flag officers under 0.4% (CRS IF10685, DMDC
   August 2024). To verify from primary sources before building: UK
   officers ~20%, a Russian officer share once put near a third and the
   long absence of a professional NCO corps, the PLA's officers, NCOs and
   conscripts, and each force's promotion standards.
8. *Eating out* — owner's note, 2026-09-17. Every meal here is bought as
   tins at a market; real Americans spend **58.5%** of their food money
   away from home ($4,485 a head against $3,187 at home, USDA ERS Food
   Expenditure Series, 2023). Restaurants employ, today, as a private
   service paid only for wages, and buy no food at all. They want to buy
   food from suppliers and sell meals, so a share of household food goes
   through a kitchen and the shops sell less. Belongs with 5: supplying
   restaurants is foodservice distribution. Measure the cost structure
   (food's share of a menu price) from a real source before building.
   **Owner's notes, same day: it comes down to the individual and their own
   choices.** Nobody eats out at a population rate. Some people eat out
   because they cannot cook or will not — a `Catering` skill already
   exists to say who can — and some because they are tired after work or a
   partner does the cooking; for others a restaurant is a treat, a classy
   place or a special occasion rather than a meal. And **keeping a reserve
   of food at home** is a practice of the person, commoner where there is
   room and money to buy ahead — the owner's own, and a homeowner's more
   than a renter's — so a household spends less by buying in bulk and
   seldom. `Person::larder` is where that lives. Leads to check against
   primary sources before building: 28% of Americans say they cannot cook
   (Tufts Health & Nutrition Letter); time and after-work fatigue lead the
   reasons people give for not cooking more (HelloFresh, 2025); cooking
   skill by age and sex in the UK National Diet and Nutrition Survey; 4.9%
   of homeowners against 15.5% of renters food insecure (US Census, 2015
   American Housing Survey); full-service dining increasingly kept for
   celebrations as its prices rise (National Restaurant Association, 2025).
   **And owner's note, same day: a meal has to carry the price of what went
   into it.** If the primary food is plentiful the ingredients are cheap,
   and what a fast-food counter and a restaurant charge should follow. The
   mechanism already exists and wants no new idea: a kitchen is a works
   with a recipe, and `econ` propagates how far an input has moved from its
   own reference cost down every stage that uses it, which is how a rich
   seam already reaches the price of a tin. What has to be right is the
   *share*: of a 2024 US food dollar, food services took **38.6 cents**
   and crops and livestock together **5.8** (USDA ERS Food Dollar Series),
   so halving the price of grain moves a menu price by a few per cent and
   not by half — which is exactly why eating out did not get cheaper when
   commodity prices fell, and the model should reproduce that rather than
   passing the whole fall through. Verify the restaurant cost structure
   (food, labour, occupancy as shares of a menu price) from a primary
   source before building.
   **And the owner's eye caught a calibration gap on the way, 2026-09-17:
   the farm takes too much of what a household pays for food.** The chain
   is grain 220 a tonne, flour 340, tinned food 900, so about a **quarter**
   of a grocery price is the farm — against a real **6.7 cents of the whole
   US food dollar** (crops 2.5, livestock 3.3, forestry and fishing 0.9)
   and roughly a tenth of a food-at-home dollar once food service's 38.6
   cents is taken out; processing takes 16.1 and wholesale and retail 20.1
   (USDA ERS Food Dollar Series, 2024). The primary end is right — real
   wheat really is about $220 a tonne — and it is the **processing and
   retail end that is compressed**, which this file already records as the
   currency being pinned to the food chain with the dear end squeezed. The
   consequence is that a world needs more agriculture per mouth than a
   real one, and it will matter as soon as a kitchen buys ingredients and
   charges for a meal, because the food share of a menu price is what
   decides whether cheap grain reaches the counter.
9. *Housing on supply and demand* — **half built, 2026-09-20.** What is
   built: a town's **buildable land** is measured off the world the way
   Saiz measures it (land, not water, under 15% slope, within 49 km), and
   the price and the rent answer people against it rather than against
   population. That replaced a land term which **saturated for every town
   a generated world contains**, so a house cost 1.543e4 in all sixteen
   towns of one world. Measured: 1,093-4,821 people per buildable km2, a
   4.41x spread, giving a 6.35x spread in price and 2.65x in rent.
   **The calibration is fitted, not validated**, and the first write-up
   said otherwise: two exponents against two observed spreads is zero
   degrees of freedom, and the price-to-rent spread is the quotient of
   the other two by construction. The independent check is the *level*,
   and it half fails — median 6.5 years of a production worker's pay
   against a real 7.1, dearest town 25.9 against a real metro ceiling
   near 11-12, because real expensive cities pay more and here pay is
   national.
   **What is still missing is the half the instruction names as a
   shortage.** There is no stock of dwellings, nobody competes for one,
   and building more changes nothing — and it cannot be built yet,
   because a town's population is fixed at world generation, so demand
   has no way to move. Stock, construction that answers price, and
   migration are the pieces, in that dependency order.

   *Original instruction, 2026-09-17:*
   **owner's instruction:
   the economy works on supply and demand, and that goes for everything.
   A housing shortage means higher prices; somewhere people want to live
   means higher prices.** Nothing about housing answers demand today. A
   house costs its bill of materials plus a land term read off the town's
   population (`econ::house_price`), and a rent is a flat 30% of a
   production worker's day times how well the town is kept
   (`person::rent_per_day`) — so no town has a number of dwellings, nobody
   competes for one, an empty town and a crowded one price alike, and
   building more changes nothing. What the model already has to build it
   on: builders are a real sector with a real payroll, `building.rs` knows
   what a dwelling costs in cement, steel and timber, `townplan` decides
   how many plots and storeys a place has, and `settlement.rs` knows why
   anybody wants to be there. What it needs is a **stock** of dwellings a
   town actually holds, households competing for it, a price that clears,
   and construction that answers the price — which is also what makes the
   item below mean anything, because a shortage is what public housing is
   built for. Anchors to verify before building: the US homeowner vacancy
   rate 1.2% and rental 7.3% (Census HVS, Q2 2026 — a tight market and a
   loose one side by side), median home value $332,700 and median gross
   rent $1,413 (Census, 2020-24), and the long-run finding that housing
   supply is inelastic where land and permission are scarce, which is why
   the same house costs three times as much in one city as another.
10. *Missing a payment has consequences, and there is room to negotiate* —
   **owner's instruction, 2026-09-20: mortgages; missing rent should carry
   penalties like real life, and so should missing a credit card payment;
   more than one missed payment should put somebody out; and there is room
   for negotiation with a landlord — extra time, or work in lieu given
   somebody's trade.** Today a tenancy has no arrears at all: miss one
   day's rent and you are on the street that evening, where `utility.rs`
   has the whole apparatus for a *power* bill — a month of usage, three
   weeks to pay, a notice, and a winter rule. Verified anchors to build on:
   - **Rent.** 2.35M eviction filings against 38.4M renter households and
     898,479 evictions in 2016 — **6.1% filed on, 2.3% put out, and about
     38% of filings end in an eviction** *(Eviction Lab, national
     estimates)*; filing rates by state today run 2-12% a year, and 24% in
     Atlanta *(Eviction Lab tracker, 2026)*. So a notice is common and
     being put out is not, which is exactly the ladder that is missing.
   - **Mortgage.** Foreclosure **cannot legally begin until 120 days
     behind** *(CFPB, Reg X)*, servicers must offer loss mitigation — which
     is the negotiation, formalised — and the grace period before a late
     fee is about **15 days** *(CFPB; the statutory figure for high-cost
     mortgages)*. Delinquency on residential mortgages at commercial banks
     is **1.86%** *(Federal Reserve, 2026 Q2)*.
   - **Credit cards.** Delinquency **2.85%** *(same release)*. The CFPB's
     $8 late-fee cap was **vacated in April 2025**, so the safe harbours
     stand at **$30 for a first late payment and $41 after**, with a
     typical fee around $32 *(CFPB)*. A penalty rate and a charge-off at
     180 days are the rest of that ladder.
   - **Work in lieu** has a legal form worth copying: **repair and deduct**,
     where a tenant fixes what the landlord will not and takes it off the
     rent, capped in most states at about a month's rent. `Person::trade`
     already says whether somebody can do the work, and `building.rs`
     already prices it.
   Built in that order: arrears and eviction first, because it is the open
   defect; then negotiation; then a mortgage, which `bank.rs` already has
   the credit tiers and refusals for; then revolving credit.
   **Arrears, notice, negotiation and eviction — done, 2026-09-20.** Rent
   falls due once a month (the owner's correction: it is a bill, not a
   daily drip); a missed month is a late fee of 5% and a notice of 14 days;
   the notice running out or a second missed month is when the landlord
   decides — **work in lieu** where the tenant's trade can do it (repair
   and deduct, capped at a month), **time** for somebody in work and well
   thought of, and out for the rest. Homelessness in the soak falls from
   peaks of 3.8/0.3/2.8% to 0.9/0.0/0.5% and ends at nought in all three.
   **Still light by an order of magnitude:** 0.0-0.8% of renters are served
   notice a year against a real 6.1% and 0.0-0.2% put out against 2.3%,
   because a household here meets no shocks — no medical bill, no car off
   the road, no debt to service — which the next two items supply.
   **Shocks and insurance — done, 2026-09-20**, the owner's note that
   insurance is where this comes in. A vehicle is damaged (4.16% a year,
   22% of its value) or its owner damages somebody else (3.3% a year, a
   quarter of a year's pay); cover is a monthly premium derived from those
   —  expected claims over the 56% of a premium that comes back as claims
   (ISO/NAIC via III) — and an uninsured owner who cannot find the repair
   loses the vehicle. **It did not move the eviction rate**, and that is
   the finding: a vehicle is bought out of savings, so shocks here land on
   the better-off, who all buy cover. What the poorest still meet is
   nothing at all — illness (lost days, not bills: this world has a
   tax-funded health service), a cooker that dies, a rent that rises.
   **And there is no insurer**: a premium reaches nobody and a claim is
   paid by nobody. The owner's point — an insurance company is a firm with
   premiums in, claims out and staff who process them, and `services.rs`
   already carries finance and insurance at 3.4% of employment.
   **The insurer as a firm — done, 2026-09-20**, the owner's note that an
   insurance company handles all this and so there is another workflow and
   claims to process. Insurance is its own service sector now: households
   pay premiums (`Why::Premium`), the insurer pays claims back
   (`Why::Claim`) at the derived loss ratio, and it employs people —
   **sized by the claims there are to settle rather than by a share of
   population**. The chain is all real: 0.87 vehicles a head (FHWA 2024),
   11.41 claims per hundred vehicles a year (ISO), one adjuster per 110
   claims with two clerks to every three adjusters, and an agent or
   underwriter per 735 policies (OEWS May 2025). It comes out at **0.64% of
   everybody in work against a real 0.72%**, 47% of them settling claims,
   and nothing at the end of that chain is typed in.
   **Still open:** a claim is settled the day it happens — there is no
   queue, so an understaffed insurer never takes three weeks to pay, which
   is the workflow half of the owner's point; and the person layer's
   premiums and claims are still its own money rather than the insurer's,
   which is the old reification gap.
   **Next: the mortgage** (monthly, with the biweekly option that pays a
   loan down faster — the owner's note, 2026-09-20 — since 26 half-payments
   is 13 monthly ones and the extra one comes off the principal), then
   revolving credit and its penalties.
11. *Everything has a workflow* — **owner's instruction, 2026-09-20: any
   and everything has a workflow; factories have different inputs and
   workflows to reach their end products, with or without human
   interaction, and which it is depends on the product.** This names a seam
   that has been open since the crafting layer was built. **The model has
   two production models and they have never met.** `craft.rs`, `wip.rs`
   and `schedule.rs` hold real workflows: named operations, three kinds of
   time (labour, machine occupancy, and unattended transformation that
   needs nobody), tools chosen by capability, what an interruption does to
   each kind of step, batch setup, and first-pass yield. `econ.rs` holds a
   recipe: inputs to output at a rated throughput with **one labour-hours
   figure**. So today:
   - **automation cannot be expressed in the economy.** A works run by
     robots and a works run by hands are the same recipe with a different
     number typed in, and nothing can say a step needs nobody;
   - **a blackout stops a whole site**, where the workflow layer already
     knows that the cure carries on, the handsaw still cuts and only the
     powered step stops;
   - **services have no workflow at all** — an insurance claim, a hospital
     admission, a checkout — except a shop, whose staff come off fixtures
     with real throughputs, which is the proto-version and the one that
     works (a till serves 25 customers an hour, so a till is 1.4 people).
   Order to build it in: a works' labour comes from its workflow rather
   than a single figure; then which steps need power and which need hands;
   then service workflows, starting with claims, because that sector now
   has a real claims volume to process and a queue is the thing that makes
   an understaffed insurer take three weeks to pay; then automation as a
   property of the workflow, so a factory can be built with hands or
   without and the difference shows in both the payroll and the output.
12. *Who pays for medicine* — **owner's instruction, 2026-09-20: medical
   care depends on the government too; there are variants — the United
   States, then the EU and Canada where it is subsidised, then countries
   like Russia, China and North Korea.** **Built, for who pays.**
   Every nation on every planet ran the British arrangement: the state
   paid the hospitals in full and nobody else paid anything, so an illness
   could not cost a household a penny and a country could not contain an
   uninsured man.

   Read from the World Bank's API over the WHO Global Health Expenditure
   Database, 2022, as a share of current health expenditure — government /
   other private / out of pocket: **United Kingdom 82.1 / 3.5 / 14.4**,
   Germany 80.5 / 9.1 / 10.4, France 75.3 / 15.5 / 9.3, Canada 71.0 / 14.0
   / 15.0, Russia 70.8 / 1.6 / 27.6, China 57.7 / 10.7 / 31.6, **United
   States 55.2 / 33.8 / 11.0**, India 40.4 / 15.1 / 44.5. Health as a
   share of GDP: US 16.5%, Germany 12.4, France 11.8, UK 11.1, Canada
   11.1, Russia 6.9, China 5.9, India 3.4. Out of pocket per person:
   US $1,381, Canada $935, UK $735, Germany $650, France $449, Russia
   $299, China $239, India $36.

   `state::HealthSystem` is four archetypes off those figures, chosen per
   nation — capacity fixes the range and politics picks inside it, which
   is honest because every country in the table paying a third of its own
   medicine in cash is a lower-capacity state. A hospital's bill is split
   three ways; the state taxes only for the share it carries; and what a
   health service delivers is what somebody paid for rather than what was
   budgeted.

   **Measured, and the finding points at an older defect.** `bin/soak`
   prints the policy against what was actually settled, and the shortfall
   is the patient at the door every time: against policies of 78/12/10,
   77/9/14 and 55/34/11, the money came to **83/13/4, 81/10/9 and
   62/36/2**. The state pays its share in full and so do the insurers;
   households cannot, because what they owe a hospital competes with
   everything else they owe and item 3's circuit does not close. So the
   more of a country's medicine is found at the door, the less of it gets
   paid for — right in direction, and resting on a known defect.

   **What it does not model, and the American case is the one it matters
   most for.** The split is an *aggregate*: every household in a town pays
   the same share of the same bill. The whole distinguishing feature of a
   private system is the **distribution** — 8% of Americans have no cover
   at all, medical debt runs to roughly $220bn, and about two thirds of
   personal bankruptcies cite medical causes — and the hardship measure
   says so plainly: the share of the population facing out-of-pocket costs
   over a tenth of the household budget is **6.8% in the United States
   against 7.5% in Britain**, which are the same number, while China is
   33.6% and India 30.9%. So the mean is not what differs among rich
   countries; who carries it is. That wants a household's own cover and an
   illness that happens to a person, which is the next piece.
   Also not modelled: **the level**. A US-archetype country spends the
   same on health as a British one here, where really it spends half again
   as much — and the reason is prices rather than more care, since real
   physician and nurse densities are broadly similar and the US has
   *fewer* doctors per head. A price level per system is its own change.
   North Korea is named in the instruction and is not in any of these
   sources; nothing is claimed about it.

13. *Public residence clusters* — **owner's note, 2026-09-17.** From Marko
   Kloos's *Terms of Enlistment* (Frontlines, 2013): most of an
   overpopulated Earth is warehoused in vast **public residence
   clusters** — welfare housing on subsistence rations, violent and
   hopeless — and the ways out are a colony lottery or **enlisting**,
   after which the same soldiers are sent back to police the blocks they
   grew up in. Worth having because it is a coherent answer to a question
   this model will soon be able to ask: what a state does with people its
   economy has no work for. The pieces are nearly all here — a state that
   employs and pays (`state.rs`), a garrison and a soldier's pay, housing
   that can be owned, rented or nowhere, unemployment that reaches 14% in
   the soak, and `building.rs`'s tenement at 4-6 storeys — and what is
   missing is the state *building and letting* dwellings, welfare as
   **rations in kind rather than money**, and enlistment as a way out of a
   place rather than a job like any other. Calibrate against the real
   thing rather than the fiction, and verify each figure from a primary
   source first: what share of a population lives in public housing
   (Hong Kong's is the high case, the US projects and the UK's council
   estates the familiar ones), in-kind welfare (SNAP), and whether
   soldiers really are recruited from the poorest — the American evidence
   is that recruits come from the middle of the income distribution rather
   than the bottom, which is the opposite of the "poverty draft" the
   fiction assumes, and the model should reproduce whichever the figures
   say.

**A0.2 Education as places.** Colleges and universities with intakes an
adult can apply to, so *retrain* is a plan: a place, years at reduced or no
earnings, a qualification, then work.

**A0.3 Move for a known vacancy**, replacing `leave_town`'s thresholds.

**A0.4 Fill a niche** — `basket.rs` already records unmet demand; posted as
an opportunity, somebody can start that business.

**Measured separately, because they are institutions and calibration rather
than choices:** each state's unemployment benefit, scaled by `Capacity`; and
the import share that makes the world buy 2.2x what it sells.

*Items as data files (below) waits: content added to a world that falls
apart in five years is more to fall apart. The owner chose raws as the
format.*

**A1. Items as data files.** A human-writable format, hand-parsed to keep
the minimal-dependency rule, every loaded definition put through
`bom::validate` exactly as the built-in ones are today. What makes content
authorable by the owner.

**A2. The basket joins the living world.** Sampled households actually buy,
own, wear out, repair and discard real items, paid through the money ledger
and carried in `GameState::items`. "Buys furnishings" stops being a demo.

**A3. The mind runs daily.** `Person` now carries a mind, a memory and
relationships; nothing in `populace` drives needs, coping or the day's
memories yet.

**A4. Then property, construction and social life** — including the
value-gated social skills (Liar gated on `Value::Truth`, as DF does) — on
top of a population that is already living.

---

### Later — the player and the clock

### 1. The clock, and the player after it — two jobs, not one

**This section said "one job, not two" and that was wrong**, on a
reviewer's correction, 2026-09-20. The reasoning was that a player who
acts forces the tick decision, which is true and is not the dependency.
**Autonomous workers already need every piece of it**: a shift that
starts, a machine reserved, a process that completes, a vehicle loaded,
an arrival somewhere. `schedule.rs` holds all of those durations in
minutes today and nothing advances its diary. So the real dependency is a
**headless scheduler**, and the player is a later consumer of the same
action rules rather than the reason to build them.

**And it does not mean stepping everything every second.** An oven
schedules a completion and is interrupted if the power fails; a truck
schedules an arrival and revises it when something relevant changes. What
has to be decided is the unit and the event queue, not an update rate for
the world.

**`scaling.rs` is groundwork and not a transferable guarantee.** Its
invariance contract was established for one mind under constant pressure,
split at *that mind's* own discontinuities. Interacting economic
processes have discontinuities that belong to other agents — a delivery
landing, a shortage biting, an outage, a cancellation — and nothing here
establishes that a catch-up stops at those. The contract has to be
re-established for them rather than reused.

You still cannot have a player who acts without deciding what a tick is,
and the tick decides what everything above it is allowed to assume.

Deliver: a base tick of about a second; the durations `craft.rs` and
`schedule.rs` already hold expressed in it; a reality bubble with catch-up
outside it, built on `scaling.rs`'s invariance contract; the day order
re-expressed as a time of day; and a player who can stand on a tile, move,
look, talk and pick things up.

**Measure first** — what actually has to run per second, per minute, per day.
A guess about a hot loop is not evidence: memoising `share_out` looked
obvious and took 80 markets from 116.7 s to 206 s.

### 2. Machines as tile-placed objects

Unlocks factories, refineries and assembly lines in one move, and makes four
already-built modules pay off — `craft`, `schedule`, `wip` and `ground`.
Needs the clock first, because a line is stations passing work along in
minutes. It also gives water and telecom nodes somewhere to hang.

### 2b. Compliance — the missing half of every rule in the model

The thread running through the owner's scope statements, and it is **one
concept rather than three**. In each case the physical half is built and
the half where somebody decides whether to do it properly is absent:

| | the model has | the model lacks |
|---|---|---|
| production and efficiency | durations, throughput, **skill reaching output** | skill reaching *scrap* — `craft.rs` has the yields, nothing consumes them |
| travel, and rules that keep it safer | transit days, four real regulations | the choice to break them, and anyone watching |
| officials doing their job | `Capacity` — what a state can reach | an office, discretion, and its abuse |

**Every road rule in this model is obeyed automatically, and a rule obeyed
automatically is not a rule — it is a constant.** Four are already in and
each costs throughput, which is the point of them: an artic limited to
90 km/h when it has the engine for 126; nine hours' driving as the legal
maximum against seven as a working day; 2.55 m of legal width; and the
oversize ladder that makes a borrowed transformer take three weeks rather
than three days. Nobody can run over hours, nobody can be overweight, and
there is no enforcement to evade.

`custom.rs::will_bend(regard_for_law, dutifulness, gain, chance_seen)` is
already that primitive, with certainty deterring and severity deliberately
not a term. It was written for fly-tipping and is wired to nothing with
money in it.

**And the shape of it is a distribution, not a type** *(the owner's
correction, and it matters)*:

- **Rules are generally followed and bent sometimes.** The ordinary case is
  compliance; breaking is the exception, and `will_bend`'s `gain` gate
  already says nobody bends a rule for nothing.
- **A criminal is a derived label, not a stored flag** — somebody whose
  rate of breaking is high and whose offences are serious. That is exactly
  `relations.rs`'s existing discipline, where `friend = true` is stored
  nowhere and friend, rival, creditor and employer can all be true at once.
- **Or somebody who bends enough to avoid jail by keeping it hidden**,
  which is a high-gain low-visibility offence and is the same arithmetic as
  an official with discretion. One mechanism, both cases.
- **Severity decides the response, not the deterrence.** The existing rule
  — certainty deters and severity mostly does not — is about whether
  somebody does it. What severity settles is what happens when they are
  caught: a ticket, a court, a cell. Three different consequences with
  three different costs to a life, and none of them a deterrent term.
- **The motive is ordinary.** Somebody speeding is late, or impatient — not
  wicked, and not a criminal. The `gain` has to come from the situation
  rather than from character, or the model has only villains.

So: enforcement density scaled by `Capacity`, `will_bend` where there is
money, and a severity table for consequences. **This is what makes offices
and regime change possible**, and it is why it sits before them.

### 3. The save loop

The root saves four of eight parts. The item store and the people are the two
that make a session resumable at all.

### 4. Boats, rigs and mining plant

Extensions of `vehicle.rs` rather than a new system: capability from parts
already works, and `fitted.rs` already makes parts individual objects that
wear out and get replaced. A trawler is frames, engine, hull, hold and nets.

### 5. Water and telecom networks

`power.rs`'s five-level hierarchy with faults sized by where they happen is
the template, and it is a good one. Both are graphs with condition and decay,
which `state-and-economy-spec.md` already asks for.

### 6. Stock markets, and a currency per nation

Both need what precedes them. A tradeable share needs firms to be valued
entities. A currency needs the per-nation state — now built — plus a second
money, a conversion on every cross-border payment, and a conservation rule
spanning both. **The tonnage ledger must not change**: tonnage is physics,
not an institution, and one ledger for the planet is what makes a cargo
leaving one country the same tonnes arriving in another.

### 7. Making a day cheap enough for a region

Roughly cubic in towns. Until it comes down, `Nations::build` holds 16
markets out of 3,000 generated settlements, and a regional rung would be a
field nothing reads.

---

## Calibration anchors

Every figure in the model is anchored to a published or measured real value.
The full list is in `CLAUDE.md`; these are the ones most of the rest hangs
off.

| | real value | used for |
|---|---|---|
| Earth land mean temperature | 8.5 °C | climate calibration |
| Earth land mean precipitation | 715 mm | rainfall scale |
| Miami model NPP | land mean ~700 g/m²/yr | all vegetation |
| French–Schultz | 22 kg/ha/mm, 80 mm loss | crop yield |
| BF-BOF steel | 1.4 t ore + 0.8 t coal, 200–300 kWh | the whole metal chain |
| World crude steel | ~230 kg/head/yr | goods composition |
| Labour share of value added | 51.8% of US GDP, 55–60% OECD | the wage–price link |
| Wage curve elasticity | −0.1 | pay against local unemployment |
| US current account | in deficit every year since 1976 | the exchange-rate band |
| BIS FX turnover | $7.5tn/day against ~$66bn/day of trade | why only the *real* rate is modelled |
| UK road freight | 1.6 bn t, 150 bn t-km, ~94 km mean haul | freight sizing |
| Dock turnaround | 45–60 min, one at each end | the fixed half of carriage |
| Building Bulletin 103 | 350 m² + 4.1 m²/pupil | school size |
| Hospital | 47.5 m²/bed | hospital size |
| Government employment | UK 17%, US 14%, FR 21% | state size |
| Public staffing ratios | health, education, admin 1 in 45 each | the public payroll |
| Graduate ability | +0.67 SD | education gate |
| Modified OECD scale | 1.0 / 0.5 / 0.3 | household costs |
| Nursery cost | 65% of a median wage | childcare |
| Customer minutes lost | UK 35/yr, DE 12, US ~90 | grid reliability |
| Big Five heritability | h² 0.40, parent–offspring ≈0.20 | personality inheritance |
| Personality stability | r ≈0.68 observed at reliability 0.80 | the twenty-year calibration |
