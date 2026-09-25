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

   **Cash on delivery, built and measured, 2026-09-24**, with the profit
   sweep fixed so a firm keeps its capital. A delivery between firms moves
   only as much as the buyer can pay for, goods and carriage, in
   `distribute`, `trade` and `consign`, and a works draws only the power
   its till covers (`Economy::payable_on_delivery`). On the viable fixture
   it does exactly what it should: every hand-off between firms paid for
   five years, and a works emptied of cash receiving nothing on a promise.
   **Across the suite it turns 17 gates red, and on three worlds it
   collapses them**: unemployment at the end 92%, 34% and 78% (worlds 7,
   11, 23) against 10.6, 10.4 and 11.1 on master; households' money in
   world 7 from 3.5e10 to 3.4e8; hunger up to 13% and hospital supply to
   nought in a working nation. Firm-to-firm supply unpaid goes to about
   1e-7 — the counter closes and the economy behind it stops.

   **Why, measured on the durability nation**, and it is not the rule:
   hospitals and builders are paid at cost **for what they used**, after
   the day's deliveries. Once any shortfall from their payers drains one
   to nought it cannot buy supplies, so it uses none, so it is paid for
   none — the loop with no way in that builders already fell into once.
   And their payers are short, because the household bills raised after
   the counter are outside the counter's budget (item 3, below): in that
   nation households take in about 267M a day, the counter takes 166M,
   and a 91.8M service bill is paid 58.6M. Every builder's yard ended
   holding nought of a 74.5M capital with the cement works sitting full.

   **So it is on the `pay-on-delivery` branch, not shipped**, with a gate
   (`a_works_receives_only_what_it_can_pay_for`) that is green there and
   red here — on master the drained cannery takes 124.8 t and leaves
   36,852 unpaid. What it waits for: providers paid for what they are
   commissioned to do rather than at cost for what they happened to use,
   and the household bills brought inside the household's budget.
   **Half of the first is done, 2026-09-24: providers price over a
   margin.** The owner's point — things charged at cost explain the
   failures; there is usually a profit margin. A builder bills its direct
   costs over the **15.46%** gross margin of US engineering and
   construction firms and a hospital over the **39.10%** of hospital
   chains *(Damodaran, Margins by Sector (US), January 2026; for chains,
   cost of goods is 60.9% of sales, about salaries and supplies)*. Empty
   every builder and hospital in a country and every one is paying all it
   owes within two months; at cost 4 of 5 builders and 5 of 5 hospitals are
   still short (`tests/margins.rs`, red at either margin set to nought).
   They are still paid for what they *used* rather than what they were
   commissioned to do.
   - **It found the state overdrawing.** `tax_and_spend` sized the tax on
     its own copy of the hospital bill, without the margin, while the
     payment read the new bill — so every state paid the margin out of an
     overdraft `Treasury::pay` allows by design, and three worlds' states
     ended five years 3.5-5.8e10 below nought, the money reaching
     households as hospital dividends. One function now
     (`Economy::hospital_bill`), and the gate this file recorded as
     missing: **a state does not spend what it did not raise**
     (`tests/health.rs`), red at 50 days of outgoings with the second copy
     put back.
   - **Two money gates were on knife edges**, each corrected to its claim
     and recorded in `tests/money.rs`: the circuit's drift was measured
     against households' own balance, which a transfer between domestic
     holders moves (now the domestic circuit, same size of bar); and "firms
     do not hoard" was 5% of all money *including abroad's*, passing by
     0.2% at cost (now days of outgoings under sixty — firms hold 15-34).
     Neither drift assertion catches a capital pay-out or a steady firm
     drain, both measured; conservation and each holder's own gate do.
   - **Five years, worlds 7 / 11 / 23**: firm-to-firm supply unpaid
     1.06e11 / 2.67e10 / 4.53e10 -> 6.95e10 / 2.37e10 / 4.01e10; household
     purchases unpaid 7.85 / 4.57 / 3.93e10 -> 8.93 / 5.29 / 4.18e10;
     mean unemployment 8.76 / 7.00 / 8.84% -> 8.65 / 7.21 / 8.58%; months
     of unemployment outside the band by head 7 / 1 / 10 -> 5 / 3 / 8;
     worst hunger month 4.1 / 0.2 / 3.1% -> 3.4 / 0.2 / 3.4%; states flat
     in both. World 23's builders meet 91% of payroll against 98%: they
     pay profit out in towns that can pay and miss payroll in towns that
     cannot.
   - **So the second half now bites harder**: a dearer bill for the same
     work comes out of the same purse the counter has already spent, which
     is why households are shorter at the till in all three worlds.
   - **And it does not rescue cash on delivery**, measured by replaying
     the branch's one commit over the margins (worlds 7 / 11 / 23, five
     years): unemployment peaks at 98 / 85 / 82% against 15.5 / 13.1 /
     15.2% on master, every hospital ends the run idle, households end
     holding 1e7 / 2.5e9 / 7.5e7. It starts in the second month — world 7
     goes 8.6% to 15.8% unemployed by day 60 — with food cover under two
     days and food at three times its cost, and the money goes abroad
     (+3.2e10 in world 7) and into firms. So the missing half is the
     other one: firms here buy only with cash in hand, where real firms
     buy on agreed terms (`credit.rs`, not wired), and the household bills
     are outside the household's budget.
   - **Corrected on review, 2026-09-24.** The margins are margins on
     revenue, reproduced by dividing by one less the margin — markups of
     18.29% and 64.20% on the billed cost, not "15.46% and 39.10% over
     direct costs" as the report said. And they were the wrong margins: a
     gross margin pays for expenses below cost of goods that no firm here
     has, so all of it became profit (builders paid out 9.1% of revenue,
     hospitals 25%). Every operating cost incurred — payroll, inputs used,
     power, carriage — is now billed over the **operating** margin, 6.49% and
     13.36%. The emptied-provider gate then showed the margin was never the
     fix: billed their whole cost, providers recover with no margin at all
     (9 of 10 stuck with wages and materials alone at no margin, 2 of 10
     with the margins, none with the whole cost either way). Part of that
     recovery rides on unpaid bills being forgotten rather than owed. The
     state gate became the review's two: books that reconcile and never go
     below nought (no state borrowing exists to record), and this model's
     balanced-budget rule kept as its own gate; a third allocates each
     hospital bill once across state, insurer and patient, with the share a
     state does not fund now recorded as owed rather than vanishing.
     *(Overstated, and corrected on the next review: "recorded" meant a
     shortfall on the day's tally, gone the next morning — not a debt. It
     is a persistent obligation since the budgeting change, below.)*
     Five years, worlds 7 / 11 / 23, operating margins over every cost
     against the gross-margin version: mean unemployment 8.65 / 7.21 / 8.58%
     -> 8.71 / 7.23 / 8.51%; household purchases unpaid 8.93 / 5.29 / 4.18e10
     -> 8.48 / 4.61 / 4.02e10; firm-to-firm supply unpaid 6.95 / 2.37 / 4.01e10
     -> 9.18 / 2.46 / 4.45e10; months of unemployment outside the band by
     head 5 / 3 / 8 -> 11 / 2 / 10; states flat in both. The aggregate
     changes are small in these three worlds — an observation, not something
     a calibration correction is owed; a correction can move an economy a
     long way.
   - **The worse firm-to-firm shortfall, located and not explained.**
     `bin/soak` now splits failed payments between firms by which kind of
     works failed to pay which. On world 7's final year, the gross-margin
     version against this one, nearly all of the rise is **shops failing to
     pay other shops for restock** — the retail relay — 1.56e10 -> 3.41e10;
     every other pair moved within about 15%. A shop's cash is what the
     counter brought in, which is the part household budgeting rewrites, so
     the cause is left for after it rather than guessed. World 7's
     unemployment months out of band (5 -> 11 by head) are not traced; the
     same run is where to start.
   - **Two sampled-person gates were reading cliffs and are now
     comparisons.** The price-shock gate passed at cost only because the
     man starved (it accepted *evicted or dead* and he was never evicted);
     it now claims only that **this outage costs this worker shifts and food
     while it lasts**, comparing the same man with and without the fault over
     the days the country is dark — 185 days worked against 299, 29 days of
     food against 80 — and records that neither the lost shifts in the dark
     nor the wage lag is shown to be the road. The shared-roof gate's 5% bar sat between readings that had
     moved to 1.7% and -28%; it now runs the town again without the
     equivalence scale — 1.017 against 0.717.
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
   this nation is *permitted* to sell — three times its own milling need,
   `region.rs`'s own ceiling — earns about 27,500. **This fixture's allowed
   merchandise exports cannot finance its specified imports at those
   quantities and prices**, and it exports nothing at all. Stated more
   generally than that it was wrong: merchandise exports need not match
   imports, and a real country closes the gap with services, transfers,
   investment income or borrowing — which `exchange.rs` already models as a
   funded deficit. This fixture has none of those channels, so the gap has
   nowhere to go, and the honest consequence of that absence is constrained
   purchasing or a recorded financing problem rather than a country quietly
   losing its money.

   **Five blocking gates are now one**, 2026-09-21, and the fixtures were
   the whole of it. Both had a goods depot and no goods industry — they
   landed retail goods every day with nothing to sell — and the symmetric
   one also had a power station with no colliery. With both repaired,
   `tests/equilibrium.rs`, `tests/exchange.rs` and `tests/shipment.rs` are
   green under the counter change and `tests/economy.rs` is down to one.

   **The one that is left is a dormitory town with no income.** Bexley has
   twenty-six thousand people and one shop with 0.00 staff: over forty days
   it takes **no payroll at all**, earns 1.8M of profit from that shop and
   pays out 5.7M, so its households hold **21,744 against Ashford's
   15,000,000**. With households buying only what they can pay for its shop
   cannot sell, its food prices at 677 against a cost of 900, and the gap
   to Ashford's 903 is 226 against a freight of 45.

   **Those figures were measured on a fixture in a 52% blackout, and two
   gap figures quoted from them measured different things** —
   reconciled 2026-09-23 with `cargo run --release --bin bexley`:

   | | "97,500 a day" | "3,233 a day" | master now, days 0-39 |
   |---|---|---|---|
   | code | counter branch, grid undersized | the same | grid sized off its load |
   | window | days 0-39, averaged | **day 41 alone** | days 0-39, averaged |
   | spending means | cash actually paid | cash actually paid, **capped by an empty purse** | wanted and paid, apart |
   | income | 46k profit | 18.5k profit | 42.8k profit |
   | paid out | 27.6k purchases + **115k power at the price cap** | 0 purchases + 21,744.7 power | 34.6k purchases + 2.2k power |
   | net | -97k: an opening purse of 3.9M spent down | -3.2k | **+6.0k** |

   - **Neither was desired spending.** A transfer records what moved,
     which `Treasury::pay` caps at the balance; the day-41 power line
     equals the purse the night before to the penny.
   - **The 97,500 was a rate of spending down an endowment**, and nearly
     all of it was electricity at 104 times cost.
   - **The 3,233 was one cash-constrained day**, and single days are noisy
     on their own account: day 40 on master shows a deficit of 19,459
     against a 40-day surplus of 6,000, because profit arrives in lumps.
   - **What Bexley wants and does not get is not money.** Of 132.6k a day
     wanted, 95.8k is meat, remedies and retail goods that no site in the
     fixture supplies — recorded as not on any shelf — and **nothing** is
     on a shelf and unaffordable. Food is bought in full.

   **And the fixture goes dark on day 253.** Its station opened on twenty
   thousand tonnes of coal with no colliery — the endowment already
   removed from `slice::symmetric`, never removed from `slice::build` — so
   from then on electricity sits at the 12,000 cap, the cannery stops, and
   Bexley's income and spending are both nought. That is the whole country
   dark rather than one town declining.

   The ownership rule is not at fault: only the farm, at 169 staff, is a
   company whose profit is spread nationally, and the mill at 1.3, the
   cannery at 23, the station at 0.41 and both shops are sole traders or
   partnerships whose profit stays where they stand — which is what this
   project already establishes about who owns a business. Founding a
   service sector was tried, since `region.rs` does it over every economy
   it builds and these fixtures never did: Bexley's payroll went from
   nothing to 1.07M over forty days, it bought the services too, and its
   purse ended at 215 rather than 21,744. Reverted with the measurement
   kept.

   **The structural fact is that Bexley eats food made in Ashford and has
   nothing whatever to sell**, and how a dormitory town earns is a
   modelling question rather than a defect with an obvious fix.

   **Shipped, 2026-09-23**, with every gate green: 937 tests across 97
   files, plus the strengthened cap gate and the two fixture-role gates.
   *(What that cap gate proves, stated exactly, 2026-09-24: a dark grid
   prices at its cap and not beyond; load goes unserved and the mills stop;
   a country with its fuel exhausted and nothing to restart from stays
   dark; and **given fuel the test puts there**, a system that can
   black-start recovers inside ten days. That last is recovery after
   supplied fuel, not autonomous recovery — the fixture holds no reserve and
   has no supply that does not itself need power. Black-start capability is
   now explicit (`Grid::black_start`, on for every grid built), and a
   second gate shows fuel alone does not restart a system without it.)*
   Five years on three worlds against master, the change moves
   homelessness at the end by +0.3, 0.0 and +0.8 points (worlds 7, 11,
   23) and leaves what a house costs unchanged. World 23's house at
   **526 years of a production worker's pay is not this change** — master
   reads 529.4 on the same world — and is recorded below as its own
   defect.

   **The whole comparison, re-run in full, 2026-09-24**, because the first
   one read homelessness at the end and left unemployment and money
   unassessed. Five years each, `ba829eb` (master before the change)
   against `b9b4cd7` (after; nothing between them touches a generated world
   but this change and the freight guard shipped with it):

   | worlds 7 / 11 / 23 | before | after |
   |---|---|---|
   | homeless, mean over the run | 0.12 / 0.00 / 4.90% | 0.21 / 0.00 / 5.50% |
   | homeless at the end | 0.0 / 0.0 / 4.2% | 0.3 / 0.0 / 5.0% |
   | unemployment, mean (workforce) | 13.0 / 8.9 / 13.3% | **12.2 / 9.0 / 12.0%** |
   | months outside its band | 12 / 0 / 22 | **3 / 0 / 3** |
   | households' money at the end | 3.86 / 3.49 / 4.74e10 | 4.22 / 3.62 / 4.47e10 |
   | hungry, worst month | 2.5 / 0.2 / 4.7% | the same |

   **So the change leaves a world that works better and a few more people
   on the street in the one world that already had them** — in world 23
   about four of 640 sampled people over the run. Why, from world 23's own
   tables, and it is not the counter reaching them directly: **a sampled
   person does not buy through the counter at all** — their food, their
   rent and their other outgoings come out of their own pocket, which is
   not the household pool (gap 4). The change reaches them through work
   and prices only:
   - **Income**: pay per day did not fall in the trades that carry the
     homelessness — food service 7.3 → 7.4 days of food a day, office
     clerks 10.0 → 10.6, production workers 9.4 → 9.9 — but **days worked
     did**: clerks 75.3 → 72.2%, production 85.8 → 82.8%, cleaners 82.6 →
     78.0%. Why those days fell is not traced: a shop selling less to a
     thin purse rosters fewer shifts, which is one route, but clerks and
     production workers are mostly not shop staff.
   - **Rent** is a fixed share of a production worker's day, so it did not
     move with it; notices fell 3.3 → 2.3% of renters a year and evictions
     rose 1.8 → 1.9%.
   - **Consumption** moved the other way: food ends at 0.75 of its cost
     against 0.84, and savings in days of food rose in most trades.
   Homelessness rose in exactly the trades whose days fell — food service
   6.0 → 8.2%, clerks 3.3 → 5.7%, production 3.9 → 5.9% — which is fewer
   paid days meeting a rent that did not change. Kept as an observed effect;
   it is not by itself a case against the counter, and one world of 640
   people cannot separate four people from sampling noise.

   **And the last blocking gate was not the counter change at all**,
   2026-09-22. `tests/durability.rs::a_worn_out_town_is_a_cheap_town` went
   red, and the cause was the third defect this branch carried — *nothing
   crosses the country for less than the carriage*, `if freight > paid {
   continue; }` in `distribute`. It was already recorded here as ungated
   and unreachable: "measured 5,958 hauls over four hundred days and found
   no offender even with its mechanism deleted". That was the whole of the
   warning and it was read as harmless rather than as what it was.

   It is not weaker than the half-the-value rule it claimed to be weaker
   than, because `carriage_for` floors a consignment at a quarter-lorry, so
   for a small order the freight is fixed while the value scales — and what
   it refused was a works topping up its daily ration down a long road. A
   refusal on day zero is then permanent: a works that bought nothing had
   no outgoings and no staff, so `distribute_profits` sized its reserve on
   thirty days of one wage and swept its opening balance to households.
   Measured on one nation, three of five cement works were bankrupt on day
   one holding about two hundred each, cement's cost halved from 190.24 to
   102.86 because only the near kilns still contributed, three towns'
   builders' yards were dry inside a month, and their fabric sat at
   0.31-0.39 after twenty years with the building trade fully funded.

   Removed, and the fixture it was written for reports nothing diverging
   beyond float noise and raises no inter-town consignment under one tonne
   over four hundred days. A replacement floor at a thousandth of the order
   was measured and **not** shipped: both its sabotages stayed green.

   **What it left open is fixed, 2026-09-24:** `distribute_profits` sized
   working capital on *today's* outgoings, so any one-day interruption to a
   firm's buying stripped it permanently — reproduced on a viable works
   with no guard involved, a cannery at 995,729 paid down to 156 five days
   after its flour was cut. A firm now keeps the larger of its paid-in
   capital and forty-five days of its **rated** operation; see CLAUDE.md,
   "What a firm was given is not profit".

   **What the counter does not reach, found 2026-09-24.** It costs the
   basket over the counter against the purse, and nothing else — so the
   bills raised after it are still taken whatever the balance: the
   private service sector's (`run_the_service_sector`), the builders'
   and the hospital door's (`pay_for_services`), and insurance premiums.
   Measured on the nation `tests/durability.rs` builds, over its second
   year: households took in 267M a day and paid out 266M; the counter took
   166M first, leaving 58.6M against a service bill of 91.8M (33.2M
   unpaid), and 8.18M of the builders' 8.60M. It is the same defect as the
   counter's, one bill later, and it is why cash on delivery cannot ship
   (item 2).
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
    **Fixed, 2026-09-24.** `inland_leg` returns one of three answers —
    `Some(0)` where the town is itself a gateway, the road's cost where one
    is reachable, and `None` where none is — and `None` makes import parity
    infinite and export parity minus infinity. The gateway depends on the
    mode (`econ::Gateway`): a quay for a sea shipment, a frontier post for
    a land-border one (`Market::frontier`), and exports may leave through
    either, so a country with no sea can sell as well as buy. The fixtures
    with no sea have explicit frontier posts; a generated town that cannot
    reach any coast when its country is built is given one
    (`post_frontiers_where_no_coast_is_reached`), which is what it had
    before by accident; a town cut off *later* gets nothing free. Both
    handle a tonne at the same per-tonne charge for now — what a land
    crossing really costs is not read yet. Gates in `tests/gateways.rs`, all
    three red under the old rules.

    Two things the suite found on the way. **A world is founded twice** —
    each nation built on its own, then folded in with its neighbours — so
    posts added to a landlocked nation's towns survived into a world where
    they could reach a neighbour's quay, and an inland town was loading
    ships; the founding step decides the flag now rather than adding to it.
    **And an unreachable parity is not a cost.** A merchant with no gateway
    has an infinite parity and lands nothing, and priced as a cost, nought
    times infinity is not a number: in an eight-nation world a town cut
    off by a winter pass turned money into NaN by day fifty, and in a
    release build, with the conservation check compiled out, a test ran for
    over half an hour instead of four minutes. Such a merchant prices
    nothing now, and the gate requires the treasury to conserve and every
    price to stay a number. Five-year soak on worlds 7/11/23
    identical to the baseline, month by month, because every town in them
    reaches a quay.
13. **World 23 prices a house at about 527 years of a production
    worker's pay.** Measured by `cargo run --release --bin soak -- --seed
    23 --nations 4 --years 5 --each 40`: **529.4 on master before the
    counter change and 526.5 after it**, against 21.3 and 15.0 on worlds 7
    and 11 and a real 7.1. The same world ends with 4-5% of its sample
    homeless where the other two end at nought. Pay in its trade table is
    ordinary — 7-47 days of food a day — so it is the price that has run,
    not the wage that has collapsed. **What runs it is not established**:
    a house is its bill of materials at the local price of cement, steel
    and timber, times the land term, so one of those four is the place to
    look, and none has been read. Named rather than diagnosed.
    **Traced, 2026-09-24, and not recalibrated.** The raw terms, world 23
    after five years (`bin/soak` prints them town by town now):
    - **It is one town.** Caldleigh, 9.7M people, prices a house at
      1.334e7 against a production worker's 1,357 a year: **9,826 years**.
      The other fifteen towns run 3.8 to 34.9, averaging 14.0 weighted by
      people, and the median town is **8.2** against a real 7.1. The 526 is
      a population-weighted mean of a ratio, and Caldleigh is 97.5% of it.
    - **The denominator is ordinary**: 1,357 is inside the 1,330-2,106 the
      other towns pay, in the same model money.
    - **The numerator is a valuation, not a sale.** `house_price` is the
      dwelling's bill of materials at local prices over the 45% materials
      are of a build, worn by condition, plus land; it is also the price a
      sampled person pays to buy outright. There are no transactions at any
      other price and no asking prices. The dwelling is the same bill in
      every town — 16.7 t of cement, 3.27 t of steel, 3.80 t of timber.
    - **Its materials are ordinary and its land is not.** Structure 12,512;
      land 1,064.6 times that. Caldleigh sits in mountain country (relief
      428 m/km) with 3.5% of its reach buildable, so its people press on
      **36,133 per buildable km²** against 900-4,100 elsewhere — a
      Dhaka-like density, so the pressure itself is plausible.
    - **The mechanism** is `0.664 x pressure^2.55` (pressure 18.1 here,
      clamped at 20), an exponent fitted where towns sit within about a
      factor of two of an ordinary density. Extrapolated to eighteen times
      it makes land 99.9% of the price.
    - **What looks wrong, for a decision rather than a tuning:** a dwelling
      is priced on a house-sized share of land at any density, where at
      thirty-six thousand to the square kilometre people live in flats and
      share a plot; and a land share of 99.9% is beyond any real market.
      Either answer is a change to the model, not to a constant, and
      neither is made.
    - In passing: the diagnostic's "buildable" column can read 102.6%,
      because a disc of 29 whole cells is 2.6% larger than the circle it
      approximates. The model's figure is the cells'; only the column's
      denominator is the circle.
    **Fixed, 2026-09-24: past a point a town builds up.** The owner's
    answer to the decision above: where land gets scarce, towns build high
    rises rather than suburbs. The fitted land curve now holds only until
    land would be **59.3%** of a dwelling's value — the 99th percentile of
    US counties for a single-family home *(FHFA, Davis, Larson, Oliner and
    Shui, 1,054 counties 2012-2022; median 22.9%, 90th percentile 39.5%)*.
    That is pressure **1.36**, about 2,700 people to the buildable km².
    Past it the ground is bid no further, and the structure costs more for
    being stacked, at the height elasticity of construction cost of
    **0.25** *(Ahlfeldt and McMillen, REStat 2018, Chicago: about 25% up to
    five floors, rising with height and passing 100% for super-tall — so
    towers are understated here, and said so)*. The rent carries the same
    height premium on its structure. `Economy::pressure_where_towns_build_up`,
    `ground_pressure`, `height_premium`.
    - **Caldleigh** after five years: 30.8 years of pay, land 1.46 times the
      structure and the building 1.91 times dearer for its height (9,826
      years before). A 76 m² dwelling at a Hong Kong-like density.
    - **It also caught the ordinary top end.** Nine of the 48 towns in the
      three soak worlds sit past the switch — the capitals at 2.0-2.4, where
      land had been 67-86% of the price. The same towns now run 17.7-22.6
      years in world 23 against 34.9 before, and a fresh world's dearest
      ordinary towns read 8.9-9.3 against a real dearest US metro of about
      11-12. The documented overshoot at the top of the housing gate — the
      dearest towns at 23-26 years where the real ones stop near 11-12 —
      came from the same extrapolation.
    - **Five years, worlds 7 / 11 / 23**, before and after (`bin/soak`):
      a house, population-weighted, 20.5 / 14.8 / 526.4 years of pay became
      **12.8 / 10.4 / 13.0**; the median town 13.2 / 7.1 / 8.2 became
      11.8 / 7.1 / 8.1. World 23's homeless: worst month **6.9% to 0.8%**,
      at the end **4.4% to 0.0%**, and outside its band in 59 of 61 months
      before and none after. Worst hunger month 4.5 / 0.2 / 4.7% became
      4.1 / 0.2 / 3.1%. Owned 1.7 / 11.9 / 7.8% became 5.8 / 10.6 / 9.4%.
    - **One thing moves the other way in all three worlds**, and it is
      recorded as observed rather than explained: mean unemployment over
      the run is 0.07-0.25 points higher (8.54 / 6.93 / 8.59% became
      8.76 / 7.00 / 8.84%) and the worst month about a point higher. House
      prices and rents reach only the sampled people, whose pockets are
      outside the economy's ledger, so any effect on the works comes back
      through the skill feed and the planner. The direction fits what item
      3 already records — poverty was staffing the mines — and it is not
      established that that is the route.
    - Gate: `tests/housing.rs` asks the ground question of both regimes — a
      loose town bids for ground (halving it dearens a house by more than
      half, a rent by less), the tightest town builds up (halving its ground
      still costs, by less than ground would), and a town squeezed to a
      thousandth of its ground keeps land at or under 59.3%. Red under each
      of: the ground uncapped, height free, and a rent carrying no height.
    - **Uxhaven, fixed on review:** a surveyed zero — nothing within reach
      under the 15% slope cut — is read at the pressure ceiling, 12.2 years
      of pay against 5.9 as an "ordinary" town. The model cannot yet tell
      *no undeveloped land left* from *no footprint at all*: nothing records
      which ground is built on, and steep ground is priced, never forbidden.
    - **Corrected on review:** 59.3% is a distribution statistic used as a
      chosen approximation, not an identified switching threshold; an
      elasticity of 0.25 is 19% per doubling of height, not the 25% the
      report said; and "building up" changes only the valuation — nothing is
      built, and additional space needing funded construction is a named
      gap.
14. **A merchant at a quay cannot see the demand inland.** An importer
    decides whether to land on its **own town's** price against import
    parity. A steel stockholder at a port with no steel demand of its own
    therefore never lands a tonne, however short the works up the road are:
    measured on the first layout of `slice::viable`, with the stockholder
    at Seaton's quay and the cannery in Harwick, the cannery ran on its
    opening tinplate and stopped on day 50. Real import merchants land
    against their customers' orders wherever the customers are. The
    fixture puts its cannery at the port instead, so it does not reach
    this; the generated worlds give every town its own stockholder, which
    hides it the same way.
15. **Every maker of a good sells the same good.** *(Owner's note,
    2026-09-24: "there is usually a profit margin... also a reason why
    there's genuine products and aftermarket items.")* A commodity here has
    one price per market, so a tonne of machinery from any works is a
    perfect substitute for a tonne from any other, and the only thing that
    decides who sells is cost and carriage. Real markets carry a brand or
    maker's margin that differs by reputation — a part made to the original
    design and sold under the maker's name against a cheaper copy that fits
    the same hole — and that difference is how a known maker earns more
    than its costs and how a cheap one wins on price. The item layer already
    records the half this needs (`craft.rs` keeps who made a thing and how
    well; `bom.rs` keeps what is in it) and nothing connects it to a price.
    No figures are read for this yet.
    **Two-way trade does not wait on it**, corrected on review, 2026-09-24:
    a coast can land grain while an inland district sells grain over a
    nearby border with no brands anywhere. Measured over 300 days, worlds 7
    / 11 / 23: nations both import and export steel (3 / 4 / 4 of them, and
    in the same town over the year), and none both imports and exports
    grain, though the world as a whole lands 3.8e7 t and ships 1.9e6 t. The
    decision is per town, against the town's own band, so nothing nets it
    nationally. **What blocks the geographic case is the gateway rule**:
    exports leave only from a town that is itself a gateway, and a frontier
    post is given only to a town that can reach no coast at all — so an
    inland district near a border, which can reach some distant quay, has no
    crossing to sell through.

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

**The money sequence, agreed on review, 2026-09-24**, in this order:

1. **Pricing equations and cost definitions** — done: providers bill every
   operating cost incurred over the source's operating margin; housing's
   switch is labelled a chosen approximation and a valuation only.
2. **Household budgeting.** One budget, drawn up before anything is bought:
   known commitments first — rent, premiums, debt payments, authorised
   construction or maintenance — then discretionary spending from what is
   left; unexpected medical bills through cover and any permitted debt; and
   unmet demand, deferred work and unpaid obligations recorded separately.
   The payer has to be the right one: a landlord's repair is not a tenant's
   builder's bill, and an insured patient does not pay the insurer's part.
   Forecast wages may inform the plan and are never spendable before they
   are paid or lent against.
   **Done in part, 2026-09-24.** Every bill a household, a state or an
   insurer is presented with pays what cash covers and **owes the rest** —
   a persistent obligation (`credit::Book` on `Economy::obligations`) with a
   named debtor, creditor and bill, net-30 terms, paid down oldest first by
   later payments against that same obligation, saved and reloaded, and
   ended only by payment or an explicit, dated charge-off (households, 180
   days past due; a designed policy anchored on FFIEC's open-end rule). A
   retried bill is not a second debt. The counter reads the day's known
   bills and makes room for them out of the goods whose spending rises
   faster than income, never out of food, meat or remedies. A state cannot
   borrow and can owe: it pays hospitals only from cash it holds, funds its
   staff before its arrears, and a weak state's arrears grow on the book.
   Unmet demand (`went_without`), obligations (the book) and charge-offs
   (`charged_off_by`) are separate records. **Not done**: deferral —
   builders' work and a day's services are done before the bill, so a town
   that cannot afford them owes rather than goes without; a policy does not
   lapse unpaid; the landlord/tenant split; rent, which lives only on a
   sampled person. Gates in `tests/obligations.rs`, each red under its
   sabotage. Five years on worlds 7 / 11 / 23: no household, state or
   insurer shortfall is forgotten any more (it was 9.3 / 4.9 / 4.3e10 a
   year); households held back 3.9 / 3.2 / 3.1e10 of goods for known bills
   and ended owing 2.9e9 / 7.3e8 / 3.4e9; **and unemployment rose** — mean
   11.8 / 9.8 / 11.6% to 14.7 / 11.6 / 13.8%, outside its band by head
   29 / 26 / 39 months of 61 against 11 / 2 / 10 — because the money now
   paying for services came out of goods, and the works that make them ran
   65 / 47 / 42% of rating against 83 / 74 / 81%. That is the circuit not
   closing, now visible as idle factories rather than unpaid services; it
   is not fixed by this change and is not to be tuned away.
3. **Bounded trade credit.** The invoices in `credit.rs`, connected through
   agreed limits, due dates, collection and default, authorised before the
   goods move — not bookkeeping on unlimited compulsory lending.
   **Built and measured, 2026-09-24, on the `trade-credit` branch — not
   shippable.** Every payment a firm makes is paid, owed on the book, or
   refused before the goods move, under either rule: trade credit (net 30,
   sixty days of planned outlay per supplier, on stop at 60 days past due)
   or cash on delivery (goods and consignments move only as far as they are
   paid for). Wages owed are debts due that day and paid first; a works in
   arrears keeps a day's planned outlay before paying suppliers (without it
   the viable fixture's mill stood idle 211 days); no dividend while owing;
   firms' debts charged off at 180 days past due. Seven gates in
   `tests/trade_credit.rs`, nine sabotages, all red. See step 4 for what it
   measured. **Verification on this branch**, clean build, 2026-09-24: 943
   tests pass and 4 fail — `a_state_that_does_not_pay_for_medicine_does_
   not_tax_for_it`, `people_share_a_roof_and_that_is_most_of_how_they_
   afford_one`, `the_viable_fixture_earns_its_living_and_names_what_goes_
   unpaid` and `the_wage_price_loop_settles`, not examined one by one.
   `tests/nations.rs` run test by test: 21 of 23 pass, and the two
   twenty-year road-decay runs had not finished after twenty minutes each,
   because a collapsed economy's book of debts grows every day. Master's
   suite the same day: 963 passed, none failed.
4. **Reassess**, on a small controlled economy: the existing behaviour,
   budgeting alone, credit alone, and both — reading consumption and
   production beside cash, overdue debt and external flows. A recovery that
   rests on unpaid invoices growing for ever has not fixed anything.
   **Done, 2026-09-24, and the answer is prices.** `cargo run --release
   --bin money_rules` (on the branch) runs the four on `slice::viable` for
   two years: trade credit keeps the works at 62% of rating, 110 t of food a
   day and 17% unemployment against cash on delivery's 32%, 31 t and 29%,
   with 9.2e5 charged off; budgeting makes no difference there because that
   fixture has no service bills. **On worlds 7 / 11 / 23 all four collapse**
   (five-year soaks, `--spend-first` and `--cash-on-delivery` on the
   branch's `bin/soak`): mean unemployment 37-60% against the committed
   model's 12-15%, 58-90% at the end, food at 2.5-3.7 times its cost, the
   states emptied, 1.2-3.3e12 owed at the end with over 90% of it overdue,
   and 0.4-1.3e12 charged off in the final year. Credit is the less bad
   rule; nothing is forgotten in any of them. *(The cash-on-delivery runs
   were rerun: the first set was built from a stale build that still
   carried a sabotage — a file restored from a backup keeps its older
   timestamp and cargo does not recompile. Same conclusion.)* **Traced**:
   the first to fail
   are the machine works, all on stop by day 150 in world 7. Steel priced as
   scarce (1,572-2,997 against a cost of 626-877) and machinery as a glut
   (434-491 against 621-702), 0.72 t of steel a tonne, so a works paid 2.6
   times what it took in. An output's price is built from its inputs'
   *costs*, deliberately, so a shortage does not compound down a chain —
   and so a processor buys at the shortage price and sells at the
   cost-built one. The forgotten tally paid the gap: 9.8 / 3.4 / 4.0e10 a
   year of firms' failed payments in the committed model. **Next is
   pricing, not credit**: a works' price able to answer what its inputs
   actually cost it, or its output falling when it cannot cover them —
   which is the landed cost that does not reach the price (experiments L
   and M) and the question already on this list of why prices settle
   under cost at all. Measure first: across the worlds, per kind of works,
   the margin between what its inputs cost it and what its output fetches,
   with credit on, to see how many chains invert and how often.
   **Measured, 2026-09-24, on the committed model** (`bin/soak` now ends
   with each kind of works on an accrual basis, at the day's prices, and
   each commodity's price against its cost; `bin/recipes` gives the
   reference ratios). Four kinds of works are billed more than they bill:
   chemical works 1.77-2.25 times, machine works 1.40-1.53, mills
   1.32-1.46, crackers 0.77-1.76; a chemical works' inputs alone cost more
   than its output in every monthly sample of every world. Their recipes
   are sound at the reference (inputs 35%, 58%, 87%, 72% of output). The
   prices are not: coal, oil, plastics, timber, meat and remedies run 1.4
   to 2.8 times their cost, cement, livestock and machinery sit on the 0.7
   floor, flour and food at 0.74-0.78 — the same in every world, because
   nothing a works makes answers its price. See CLAUDE.md, "Which chains
   invert, measured". **With credit on** (this branch's default, budget
   and trade credit, same worlds): the same pattern, amplified — coal 6.2
   to 10.2 times its cost, oil 3.5-4.9, electricity 4.3-4.8, food 3.4-3.7;
   mills billed 1.10-1.50 times what they bill and losing at the day's
   prices in 64-93% of samples, chemical works 1.00-3.42, crackers
   1.04-1.95, machine works 1.31-2.07; and shops at exactly 1.00 in all
   three worlds.

**Kept on that path, not shown closed:** the production-to-shelf
integration test (three gates on master in `tests/production_to_shelf.rs`;
the fourth, *a works receives only what it can pay for*, is on the
`pay-on-delivery` branch and red here), and the unreachable-freight defect
(item 12 above has gates, which do not yet establish it closed once
households budget and firms trade on credit).

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
