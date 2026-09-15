# Research Context — Terra Classic Forex / EUTC (compiled 2026-09-12)

Everything discovered from primary sources during the design of this implementation, consolidated so no future contributor has to re-derive it. All claims verified against the linked sources on the compile date. Companion docs: [SPEC](./SPEC.md) · [ARCHITECTURE](./ARCHITECTURE.md) · [USER-STORIES](./USER-STORIES.md) · [DIAGRAMS](./DIAGRAMS.md).

---

## 1. Proposal 12209 — the on-chain record (verified via LCD API)

| Field | Value |
|---|---|
| Title | **Lunc Forex Genesis + EUTC Repeg** |
| Type | `TextProposal` (cosmos.gov v1beta1) — **requests no funds** |
| Submitted / voting end | 2025-12-13 → 2025-12-20 |
| Deposit | 5,000,000,000,000 `uluna` (50B LUNC) |
| Result | **PASSED** — yes ≈ 490.0B · no ≈ 81.4B · veto ≈ 43.0B · abstain ≈ 52.7B LUNC |
| Full memo | "Begin development of Collateralized Stablecoin Module, test and oracle integration. Development is done for free. New prop will be put up for liquidity investment." |
| Linked discussion | [discourse.luncgoblins.com/t/lunc-forex-genesis-eutc-repeg/290](https://discourse.luncgoblins.com/t/lunc-forex-genesis-eutc-repeg/290) |

**What this authorized:** development work only — free of charge, no deployment mandate, no treasury spend. Liquidity seeding was explicitly deferred to a *future* proposal, which as of the docs' 2026-06-01 verification and our 2026-09-12 check **has not been filed**.

## 2. What the proposal actually says (from the Discourse source text)

- **Scope:** "exclusively for the construction of the CSM system: multi-asset collateral, stablecoin issuance, automated swaps, LUNC buyback vault logic." Framed as **"the replacement of the old market module"** (the algorithmic one that failed with UST).
- **Collateral:** EUTC ↔ EURC 1:1; EUTC ↔ USDC via EUR/USD rate with **0.5% premium**.
- **Fees:** **1.5% mint + 1.5% redeem, paid in-kind** (same stablecoin) → secondary reserve.
- **Buybacks:** fees → periodic LUNC purchases on **Garuda (GDEX) / Terraswap / Terraport** ("whichever gives best price"), **"no governance needed, no human involvement"** → LUNC into a **permanent vault**, called "a permanent LUNC accumulation engine."
- **Redeem cap:** **10% of total supply per day** (the proposal's central anti-run mechanism; justifies why 0.5% premium suffices and "100.5% collateralization becomes safe").
- **Kill switch:** **multisig-controlled** ("5-of-9 is common"), pauses redemptions, "never block user balances," auto-resume after governance vote.
- **Oracles:** on-chain fiat prices (USD, EUR, GBP, JPY, CAD, SGD, AUD, CNY "and more"), **30s refresh**.
- **Vault infra:** maintain ~101.5% ratios, track outstanding supply, auto-rebalance mint caps; **5% USDC/EURC differential** triggers portfolio rebalancing.
- **Chain revenue narrative:** 0.5% native swap tax + 0.2–0.3% DEX LP fees + 1.5% CSM fees → LUNC accumulation.
- **Team:** presented by **Nicolas Boulay** (Lunc Cookie validator / Luncverse / Luncdash), with **StrathCole** ("tremendous help… will help deploy CSM after approval") and **Mark Mcdonnell** (forex/liquidity guidance).
- **Funding request:** none for code; liquidity injection from community pool to be proposed later for EUTC/LUNC, EUTC/USTC, EUTC/USDC pools.

## 3. The vault-vs-burn controversy (unresolved)

During the discussion, community members (notably "Rocko") demanded buyback LUNC go to the **burn wallet**, calling the permanent vault post-Do-Kwon suspicious ("if they go to a permanent vault, it's a no"). The proposer argued vault LUNC is **rebalancing collateral** ("Collateral is KING… it makes absolutely zero sense to burn your collateral"), that burning "destroys marketcap" and that existing pool taxes (0.4%/trade) already fund burns. The prop passed **with the vault design**, but the objection was never formally conceded or resolved — press articles since (e.g., KuCoin) describe buybacks as "permanently burning LUNC," repeating the community's wish rather than the proposal's text. **Status: open governance question; disposition of vaulted LUNC needs an explicit vote before any real funds accumulate.**

## 4. EUTC history — why "Repeg"

The title's "Repeg" and the discussion confirm EUTC **pre-existed** as a community-issued euro-pegged token that **lost its peg** (de-pegged, illiquid, "practically untradeable" per later coverage). The CSM is the rescue mechanism: re-back the euro unit with external collateral. What the legacy supply/holders get is **nowhere specified** — the single biggest open design item (see §10). Original issuance/depeg dates are not documented in any indexed source (gap noted honestly).

## 5. Workstream split (decisive clarification)

**MM2 (Market Module 2.0) and FOREX/CSM are separate proposals for separate assets:**

| | MM2 | FOREX / CSM |
|---|---|---|
| Asset | **USTC** (via `uusd` swaps) | **TC fiat family: EUTC first**, WONTC/YENTC later |
| Backing | LUNC-side accumulator + tax redirect | External collateral (EURC/USDC); **no LUNC in the peg** |
| Home | In-core Go module — classic-terra/core **#664** (v15 upgrade), developed in [`Market-Module-2-0/core`](https://github.com/Market-Module-2-0/core) branch `feat/mm-implementation` (companion PRs #2 Farrell23 hardening, #3 lunctoken activation/validation) | **This workspace** — cw20 contracts |
| Status (2026-09-12) | Open PR since June, updated Sept 9, simulation gaps flagged (TWAP/daily-cap) | PoC implemented, 14/14 tests |

**Project decision:** the two remain **fully decoupled** — no dependency, no shared code, no tracking of #664's fate.

**Team progress reality check:** StrathCole's public work is chain-infra (core fork, oracle-go, indexer-go); Boulay has no public LUNC code; **no CSM/EUTC application-layer code exists anywhere** except this workspace (verified via GitHub-wide searches, both accounts, orgs, core PRs/issues). The "developers are building the Collateralized Market Module" press line refers to MM2, not the CSM.

## 6. Chain burn tax — verified mechanics (affects economics, not design)

From `classic-terra/core` v4 source (`x/tax`) + live LCD queries:

- **Live rate: 1.5%** (`/terra/tax/v1beta1/params` → `burn_tax_rate: 0.015`, confirmed on publicnode + hexxagon 2026-09-12). Code genesis default is 0.1% (`x/treasury DefaultTaxRate`) — governance raised it; the legacy treasury endpoint returns 0 (rate moved to `x/tax`).
- **What's taxed:** bank denoms **except** `uluna` (`sdk.DefaultBondDenom` skip in `x/tax/types/compute.go ComputeTaxes`) **and except IBC denoms**. So **LUNC transfers are never burn-taxed**; `uusd`/`ueur`-style denoms are, clamped by **per-denom TaxCap** (cap binds on small transfers).
- **What's not reachable:** **CW20 tokens** (EUTC/WON/… are not bank denoms) and wasm executes — the wasm tax hook in `x/tax/handlers/wasm_msg_server.go` is **commented out** (pass-through). Even if re-enabled, it taxes `msg.funds`, which are empty in cw20 `Send` hook flows.
- **Settlement:** post-handler → `ProcessTaxSplits` (default 90% burn / 10% community pool) → treasury epoch proceeds.
- **CSM impact:** our flows are structurally tax-free; only exposure is native-denom legs inside a real DEX router during buybacks. The **0.5% native swap tax** (market module) *does* apply to native-denom swaps — relevant if EUTC ever becomes a native denom (see §8).

## 7. Ecosystem readiness

- Chain: cosmos-sdk v0.53.x, wasmd **v0.61.8**, wasmvm **3.0.3** (`core/go.mod`) — CosmWasm 2.1-generation contracts deployable today; one governance upload prop per contract batch.
- DEXs for buybacks: Garuda/GDEX (0.3%: 0.2% LP + 0.1% dex), Terraswap (0.3% to LPs), Terraport (0.3%) — per the proposal.
- Tax machinery: `x/taxexemption` governance-managed address-pair exemptions exist if ever needed.

## 8. Deployment path: standalone wasm vs in-core module

**Decision: standalone wasm repo** (see ARCHITECTURE §2). The steelmanned case for the alternative — proposing FOREX **inside `classic-terra/core`** — for the record:

1. **Native denom issuance.** The pre-collapse Terra had *native* stables (`uusd`, `ueur`…). Re-issuing EUTC as a native bank denom (`ueur`-class) restores that model: free bank transfers, universal wallet/explorer support, IBC-native, and the 0.5% native swap tax flow — none of which cw20 gets.
2. **Repeg of legacy supply may require it.** Contracts can only convert holders who opt in (claim-based, slow tail, stranded dust). A one-time **upgrade-handler store migration** could re-map every legacy EUTC balance chain-wide instantly — something *only* in-core code can do. If governance wants universal conversion rather than opt-in, this is the decisive technical argument.
3. **Burn disposition without trust.** Buyback LUNC could be burned **at consensus level**, settling the vault-vs-burn controversy in code rather than by multisig promise.
4. **No 650k wasm gas floor.** Native ops cost ~10–50k gas vs ≥650k per wasm execute; material for a high-volume, small-transfer stablecoin.
5. **Governance-native parameters.** Params live in the standard param store — changed by ordinary governance props, no contract-admin multisig keys to lose or capture.
6. **One trust root.** Validators run the code; no separate admin key can pause/migrate anything. Post-Do-Kwon, "the chain itself issues the stablecoin" is the strongest possible credibility story.
7. **Synchronous composability.** Direct market-module swap integration, atomic multi-op flows, no submessage round-trips.

**Why standalone still wins for now:** none of those advantages are needed for a *correct first deployment* (the CSM's assets are themselves cw20s; native-denom issuance can come later via migration); wasm ships in weeks vs a hard-fork cycle measured in months of validator review; the core repo's PR queue is congested and #664's review pace illustrates it; and the repeg-bridge question (§10) can be answered opt-in-first in wasm, with the in-core migration as a later promotion if governance demands universality. **Revisit triggers:** governance chooses universal repeg; native-denom issuance is mandated; gas economics prove material; or burn-at-consensus is required.

## 9. Where things stand (2026-09-12)

- This workspace: CSM reference implementation, **14/14 integration tests**, full doc set, known gaps E1 (buyback settlement trusts router-reported amount) and E2 (collateral ratio counts vault only) documented in ARCHITECTURE §5.
- Upstream: nothing to PR into (greenfield); engagement path is the Discourse thread / governance forum, not a code PR.
- MM2: explicitly out of scope; not tracked.

## 10. Legacy supply data — **CORRECTED 2026-09-12: they are native denoms, not cw20s**

An initial working assumption (cw20 contracts, needs a wasm scan) was **wrong**. The entire pre-collapse forex family lives on as **native bank denoms** — no contract addresses, queryable directly. Live supplies from `/cosmos/bank/v1beta1/supply` (publicnode LCD, 2026-09-12, 254 denoms total):

| Denom | Asset | Supply (µ-units) | Units (÷10⁶) |
|---|---|---:|---:|
| `uusd` | USTC | 6,076,506,826,157,021 | **≈ 6.08 billion USTC** |
| `ueur` | **EUTC** | **62,485,015,359** | **≈ 62,485 EUTC (~€62k)** |
| `ukrw` | KRTC | 38,524,124,989,705,181 | ≈ 38.5 billion KRTC |
| `ujpy` | JPTC | 4,297,695,868,819 | ≈ 4.30 million JPTC |
| `usdr` | SDTC | 700,143,664,767,918 | ≈ 700 million SDR-units |
| `umnt` | MNTC | 223,567,483,750,691 | ≈ 223.6 million MNTC |
| `ucny` | CNTC | 494,308,959,040 | ≈ 494 thousand |
| `uthb` | THTC | 705,436,996,647 | ≈ 705 thousand |
| `uinr` | INTC | 2,216,922,294,703 | ≈ 2.22 million |
| `usek` | SETC | 958,732,029,807 | ≈ 959 thousand |
| `ugbp` | GBTC | 148,258,973,109 | ≈ 148 thousand |
| `umyr`/`uphp`/`uaud`/`uhkd`/`unok`/`ucad`/`uchf`/`udkk`/`usgd` | regional | 6.3B–207B µ each | ≈ 6k–207k units each |
| `uils`, `umxn`, `ubrl`, `uzar` | — | **NOT PRESENT** in bank supply (registered in oracle/market code, zero supply — Gemini's list overcounts) |

For scale: LUNC (`uluna`) = 6.45 trillion LUNC. **Key insight: EUTC's dead supply is tiny (~62.5k ≈ €62k) vs USTC's ~6.08 billion — roughly 97,000× smaller — which goes a long way toward explaining the repeg candidate selection.** Economic note: at this size, a repeg bridge for EUTC is *cheap to capitalize* (€62k-scale problem, not a €6B problem) — but the same native-denom reality means a universal in-core migration could remap these balances directly (see §8.2), and any "TC" re-issue decision interacts with these frozen balances.

Also note: the market module's **allowed swap denoms include all of the above** (they appear in the `x/tax` gas-price list) — the frozen swap infrastructure is still wired for the whole family, one flag away from reactivation. That is the backdrop against which a CSM must justify itself as the *safe* revival path.

**How the freeze is enforced (verified live, 2026-09-12):** the market module is disabled *economically*, not removed — `terra/market/v1beta1/params` shows `min_stability_spread: 1.0` (a 100% spread: any oracle mint/burn swap forfeits the entire principal, so nothing swaps). The denoms can only re-enter circulation via a governance decision. Meanwhile **staking mints nothing via LUNC inflation** (`cosmos/mint` params all zero) — **but staking rewards DO still pay out in the forex denoms** (initially misstated here; corrected 2026-09-12 after live verification). Verified live: a validator's outstanding rewards contain **20 forex denoms** (206,433 USTC, 3.02 EUTC, 305,584 KRTC, 6.34M JPTC, …), and the **community pool** holds 577.5 EUTC, 62.4M USTC, 18.1M KRTC, 23.5M JPTC. Mechanism: the burn tax is collected **in-kind** on forex-denom transfers, then split three ways (`ProcessTaxSplits`: burn / **oracle** / community); the oracle share funds ballot rewards, so validators voting accurate EUR/KRW/JPY rates earn those denoms and pass them to delegators. Crucially this is **redistribution + burning of existing units — not issuance**: the market-module mint path stays dead (100% spread), so supplies can only churn or shrink. Two CSM-relevant consequences, now expanded below: (a) the community pool's forex balances (see §10d) are candidate seed funding for the liquidity prop — a governance decision, not new supply; (b) **the chain's `x/oracle` price votes are live for all 20 denoms, and the wasmbinding exposes them to contracts** (verified: `core/wasmbinding/query_plugin.go` `ExchangeRates` handler + `stargate_whitelist.go` whitelists `/terra.oracle.v1beta1.Query/ExchangeRate`) — see §10e.

## 10d. Community-pool forex balances — uses and non-uses for the CSM

Verified live (2026-09-12, publicnode): the community pool holds **577.5 EUTC (`ueur`), 62.37M USTC (`uusd`), 18.1M KRTC (`ukrw`), 23.5M JPTC (`ujpy`), ~1.04M MNTC, ~1.14M CNTC, 92.4k GBP, 150k SDR** — across 40 denoms total. What this treasury can and cannot legitimately do for the project:

**Legitimate uses (governance-approved):**
1. **Audit + deployment funding** — a community-pool spend prop (LUNC slice) covers the audit and testnet deployment; this is the conventional use and uncontroversial.
2. **Seed collateral for EUTC-C** — a small slice of pool assets swapped *on DEX* to EURC/USDC (size it ~€5–10k equivalent to avoid market impact) becomes the initial collateral and LP seed. This directly executes what 12209 deferred ("new prop will be put up for liquidity investment") without new taxes or asks.
3. **Symbolic first conversion** — the pool's own 577.5 EUTC could be the first legacy unit converted through the bridge (governance-approved), proving the program end-to-end. Economically trivial; optically useful.

**Non-uses (hard limits):**
- **Never treat pool forex balances as backing/reserves** for EUTC-C — they are unbacked legacy units; using them as "reserves" reproduces the exact collapse pattern.
- **Never dump large slices** (e.g., 62M USTC) into thin markets to raise funds — crashes USTC, poisons community goodwill, and the amounts involved would be self-defeating.
- **Don't entangle the CSM with community-pool politics**: pool spends need their own proposals and coalitions; the CSM's technical viability must not depend on winning them. Sequence: ship testnet first; pool props later, as funding *options*.

**Verdict: keep the idea, but as a Phase-2 funding *option*, not a dependency.** The pool's real value to the project is political (a ready-made, governance-controlled funding path that 12209 already anticipated) — not the modest euros involved.

## 10e. Oracle strategy resolved — consume the chain's own consensus rates via wasmbinding

Verified in `core/wasmbinding/`: the **`ExchangeRates` custom query** is registered and whitelisted (`/terra.oracle.v1beta1.Query/ExchangeRate` in `stargate_whitelist.go`; cross-rate handler in `query_plugin.go:140-163` computing `quote/base` from `GetLunaExchangeRate`). Our CSM contracts can therefore query **validator-consensus exchange rates** (slashing-backed, updated every block, covering all 20 forex denoms) directly — e.g. EUR/USD from `ueur`/`uusd` votes.

**Consequences for the design:**
- No proprietary feeder set needed at deployment → kills the single-feeder trust risk (ARCHITECTURE §5 E4) for free.
- Freshness is structural (ballot rates update per block); fail-closed handling remains for failed ballots (rate query errors) — keep `oracle_stale_secs` semantics as an error-handling policy.
- **Role of StrathCole's `oracle-go`:** it is *feeder software that validators run to post prices INTO* `x/oracle` — we consume the pipeline's output, we don't operate it. (Tie-in: our earlier audit's F1 finding — the USD-per-USTC feeder fix, oracle-go PR #1 still open upstream — improves exactly this pipeline. Worth nudging again.)
- **Our `oracle-adapter` stays**: re-purposed as (a) the interface boundary (so tests stay hermetic), (b) an override/fallback source if a denom ballot fails or a quote is needed outside the 20, (c) an admin-switchable source selection (consensus-rate primary, feeder fallback). Phase 1 implementation task: a `ConsensusRate` query path via wasmbinding in csm-core with the adapter as backup.

## 10a. Attack-model review (peg defense)

**The Soros/GBP or LUNC-short analog for EUTC is a spot dump + redeem drain** (no short/perp market exists for EUTC, and no borrow-lend market to build one from — so the classic "short below the peg" attack has no venue; this is a structural defense in itself).

What the proposal already contains: the **10% daily redemption cap is its Soros defense** ("even if someone mints 10M EUTC, they can't redeem more than 1M/day" — bounds a run to ~6.6 days at par while each mint-redeem loop bleeds the attacker 1.5%×2 + premiums), the 0.5% USDC premium, fail-closed oracle freshness, and mint-gated-by-collateral (an attacker cannot dump EUTC they did not first back).

**Final recommendation (layered anti-dump stack):** L0 structural — collateral-gated issuance + no short venue (keep it that way: no leveraged EUTC listings while unseasoned). L1 rate limits — 10% daily redeem cap (in proposal) **plus per-address redeem cap (~1% of the daily window) and a daily mint-velocity cap per collateral** (both additions). L2 dynamic friction — redeem spread widening with daily-window utilization and vault shortfall, plus a flash-fee on EUTC younger than N hours (kills wash mint-dump loops). L3 solvency governor (E2) — halt mints while primary ratio < 100% so every capped window settles at par. L4 oracle integrity — sanity min/max + max rate-of-change + median-of-N feeders. L5 repeg containment — global and per-address legacy-conversion caps with vesting (with a €62k legacy overhang this is fairness optics more than systemic risk). L6 operations — deep seeded pools + protocol-owned backstop liquidity from the reserve. Under this stack an attacker pays escalating fees, can never drain more than the capped window at par, cannot short, and the backing remains intact and publicly verifiable — the attack becomes a donation to the reserve.

What it lacks (must be built regardless of MM2, which is USTC-only and irrelevant here):
1. **Ratio-governed mints** (our E2): halt new issuance when vault primary ratio falls below threshold, so attacks can't compound via freshly minted units.
2. **Oracle bounds**: min/max sane EUR rate and max rate-of-change per window — a manipulated feeder can't open a mint/redeem mispricing window.
3. **Dynamic spread**: widen mint/redeem fees as reserve imbalance or redeem-window pressure grows (Curve-style), instead of a flat 1.5% that is cheap relative to peg risk during stress.
4. **Legacy-supply containment**: the pre-existing dead EUTC is the real dump fuel; the repeg bridge (open question #1) must include per-address and global conversion caps, so old bags can't be converted en masse and dumped on fresh liquidity.

## 10b. Cross-TC-family swaps & the native-denom bridge (design)

**Today there is nothing to swap:** all legacy TC denoms are frozen ledger entries — no mint/burn path (market module disabled post-collapse), no AMM pools, no price discovery. "Frozen" applies to *all* of them equally, EUTC included; EUTC's difference is only that it has a live repeg effort (12209) targeting it.

**What NOT to do:** re-enable the old market-module oracle swaps between TC denoms (`ukrw`→`ueur` would natively burn one and mint the other at oracle rate with zero collateral changing hands). That is the exact unbacked-mint mechanic that killed the chain. The denoms remain one governance flag from reactivation — the CSM must be positioned as the *safe* alternative before anyone reaches for that flag.

**Safe design — collateral-hub routing:** each TC asset gets its own CSM instance (KRTC-C backed by USDC at the USDKRW rate, EUTC-C by EURC/USDC at EURUSD — the oracle interface already supports multiple quotes). Cross-asset swaps then always route **through collateral**: KRTC-C → redeem → USDC → mint → EUTC-C. Value never materializes from thin air; every spoke is independently solvent. DEX pools (seeded per the liquidity prop) handle the retail-facing pairs; the two-hop CSM route is the arbitrage backstop that keeps pool prices honest.

**Wasm-compatible repeg bridge for the frozen native denoms (the enabling mechanism):** a contract cannot burn bank balances, but it *can* receive them as **attached funds on `MsgExecuteContract`** (`msg.funds`). Legacy holder sends `ukrw` attached to a `Convert{}` call; the bridge records the claim (per-address + global caps, vesting per §10a L5), forwards the coins to an unrecoverable sink address, and mints/allocates the corresponding KRTC-C claim. Works identically for `ueur`, `ujpy`, etc. No core changes needed — the whole TC-family revival fits in the standalone wasm repo.

**Why EUTC's supply is so small (62,485 vs USTC's 6.08B):** (a) pre-collapse euro demand on Terra was always niche — issuance was demand-driven (burn LUNC → mint stable), and the ecosystem was USD/KRW-centric (KRTC's 38.5B ≈ Chai payment-app footprint; uusd was the DeFi default); (b) through the collapse, oracle swaps were live, so ueur holders could and did route ueur→uusd→LUNC to exit — the residual is what never left; (c) post-collapse governance froze mint/burn, stranding the remainder. So the tiny supply is partly *selection effect* (the asset few people used) and partly *exit* (the people who used it left). Both make the repeg overhang a ~€62k problem.

## 10c. Why EUTC first (assessed)

The sources never state the selection rationale explicitly. Best-supported reading: (a) EUTC is **untainted by the pre-collapse mint machinery** — unlike USTC, whose supply/mint history is the albatross MM2 must drag; a clean euro unit is the cheapest credible repeg story; (b) it already had a community, ticker, and holder base ("Repeg" framing); (c) euro is the largest fiat market after USD and no USD stables were proposed (USDC plays that role as collateral). Treat as inference, not documented fact.

## 10f. Collateral asset selection — EURC/USDC vs BTC/ETH (policy, decided 2026-09-13)

**Decision: fiat-backed stables stay the collateral base; BTC/ETH are explicitly out of scope for genesis.** 12209 already specifies this (EURC primary, USDC secondary at the EUR/USD rate + 0.5% premium), and the rationale holds on the merits:

1. **Unit-of-account match.** EUTC references the euro; EURC backing makes the vault ratio *identically* the solvency measure (what I1-I3 and the E2 governor check). With BTC backing, solvency becomes a function of the crypto market - a 40% drawdown turns "1:1 backed" into 0.6:1 backed overnight, and the E2 governor would spend most of its life halted. Compensating requires ~150-300% over-collateralization plus a full liquidation stack (oracles, auctions, keeper incentives) - the entire Maker/DAI machinery - plus native-BTC bridge custody, historically the single largest hack category (Ronin, Wormhole, Nomad).
2. **Capital efficiency is existential here.** Every collateral unit must be raised through community governance (12209 deferred the liquidity prop). A 1:1 fiat-stable design needs ~1.0-1.02x per unit; a conservative BTC design needs ~1.5x+ of community-raised value for the same credibility. Tripling the fundraising ask to gain volatility exposure we do not want is backwards.
3. **The regulatory objection does not track the collateral asset.** What makes EUTC-C an ART-shaped question under MiCA is that it *purports to reference the euro* - true regardless of what backs it. The KYC reality is also unchanged: minters must acquire EURC/USDC on regulated venues first, so the "censorship-resistant backing" argument dissolves at the entry point. The genuinely debatable point is Circle-issuance concentration (blacklisting, custody risk) - but that is a Phase-2 diversification problem, not a genesis blocker.
4. **Precedented path.** DAI launched with a centralized USDC PSM (~$10B) and diversified only after survival. Our equivalent: keep `CollateralType`/oracle extensible (already is), and via a future governance vote add a *capped* crypto tranche (e.g. <=30% of backing at a conservative haircut) for issuer-failure resilience - a parameter change, not an architecture change. Existing defenses (E2 ratio governor, reserve diversification, multisig pause) contain an issuer event in the meantime.

**Recorded consequences:** none for the current implementation (already EURC/USDC per spec); the diversification tranche is registered as open question #8.

## 10g. Breaking: Circle sunset of Noble USDC (2026-09-13) — collateral rail risk is live

Verified 2026-09-13 (Forkast/News, CryptoSlate, Cosmos Hub): Circle announced 2026-09-10 the discontinuation of USDC + CCTP V1 on **Noble**. Timeline: new minting to Noble via Circle Mint stops **2026-10-13**; CCTP V1 burn limits ramp to zero 2026-10-31; Noble USDC contract + all CCTP routes **fully paused 2027-01-12** (manual redemption portal after). Noble gets **no CCTP V2**; Circle consolidates on its own L1 (Arc mainnet 2026-09-16; BlackRock/DTCC/Visa/ICE validator set) ahead of the GENIUS Act yield ban (2027-01-18). Circle promises an unspecified Cosmos "intermediate solution" - no date, no architecture.

**Consequences for the CSM:**
1. "USDC arrives natively via IBC (Noble)" (10f) is now a **dying rail, not a plan**. Any USDC the ecosystem wants moved to Terra Classic must be IBC-hopped (e.g. via Osmosis balances) **before 2026-10-13** for new issuance, or use third-party bridges indefinitely.
2. **EURC-first becomes the default posture**: the spec already models both collaterals as cw20s on Terra Classic; EURC acquisition runs through a vetted EVM bridge and is unaffected by the Noble sunset. Treat USDC as the *optional* secondary leg, gated on a verified live rail (Circle's "intermediate solution" when/if it ships).
3. This is a live demonstration of **rail/issuer concentration risk** (the exact concern in 10f 3) happening *to the ecosystem we build on* - strengthens the case for open question #8 (diversified tranche) and for an insurance reserve (open question #10) rather than relying on any single issuer or bridge.
4. Governance-relevant: any proposal text should not promise USDC support before a rail is verified; leading with EURC-only genesis is the defensible scope.

## 11. Open questions registry

1. **Repeg bridge for legacy EUTC** — opt-in conversion (contract-based claims, haircut policy) vs universal migration (in-core upgrade handler). Blocker for the "Repeg" promise; biggest open design item.
2. **Vault vs burn disposition** — needs an explicit governance decision before funds accumulate (§3).
3. **Buyback automation** — proposal demands "no human involvement"; our PoC is admin-triggered. Needs epoch/crank design.
4. **Kill switch governance** — proposal's 5-of-9 multisig vs PoC single admin.
5. **Liquidity prop** — deferred by 12209, still unfiled; without it, thin-market risks (ARCHITECTURE §5 E5) dominate at bootstrap.
6. **Oracle decentralization** — single-feeder trust; median-of-N + rate-of-change bounds needed.
7. **Rebalancing enforcement** — `rebalance_threshold_bps` reserved, not implemented.
8. **Collateral diversification (issuer-concentration hedge)** — add a capped crypto tranche (BTC/ETH, e.g. <=30% of backing at a conservative haircut) via governance once the fiat-stable core is seasoned. Deliberately out of scope for genesis (see 10f).
9. **USDC acquisition rail post-Noble-sunset** — verify a live, canonical path for USDC onto Terra Classic (Circle's Cosmos "intermediate solution", or a third-party bridge; pre-deadline pre-positioning is rejected as a plan-of-record). Until verified, genesis collateral = EURC only (see 10g). Bridge vetting must weigh the 2026-06-10 Axelar<->Secret exploit (~$4.67M of unbacked tokens minted through the custom ics20-for-axelar receiving contract): the weak link was bridge contract code, not light-client verification.
10. **Insurance reserve against issuer failure** — a fee-funded second-layer pool (separate from the 1:1 vault) in uncorrelated assets with objective depeg triggers; design after genesis (see 10g). Candidate assets surveyed 2026-09-13: USDS (governance-issued, T-bill/USDC backed; not on Cosmos - bridge needed), GLDT (ICP gold token, 1 = 0.01g, Loomis-Zurich custody, Metalor bars, ~4-monthly third-party audits, GOLDAO-governed - backing is *attestation-based*, not cryptographic, and shares the ICP-corridor delivery problem), capped BTC/ETH tranche (bridge custody risk; both named BTC routes into Cosmos broke within 3 months of each other - Axelar->Secret 2026-06-10, Nomic nBTC double-spend 2026-09, ~39.8 nBTC unbacked, 22.65 BTC frozen on Osmosis, ~36% of alloyed-BTC backing hit, detection lagged ~74 days per Protos). By contrast the ICP chain-key family (ckBTC/ckETH/ckUSDC/ckUSDT/ckDOGE) has **zero exploits** since ckBTC's 2023 launch (Trail-of-Bits assessed): no key committee exists to steal, but ICP-side assets still have no corridor to Cosmos, so none are usable as vault collateral today.
11. **LUNC→EVM corridor / "inverted ck" (ICP→Cosmos twin)** — no active EVM-compatibility track found on Terra Classic as of 2026-09-15 (v4.0.0 was SDK modernization; no EVM-module CIP on the tracker). Watch item: if an EVM route ships, ICP↔EVM corridors (ckERC-20 pipeline + EIP-2537 BLS precompile) become the cleanest ICP path to LUNC. Parallel track: a native "inverted ck" design — ICP canister custodies the asset (ckERC-20), burns ck-twins, and mints a cw20 on LUNC against chain-key certificates — blocked only by LUNC-side BLS12-381 verification (candidates: blst in a wasmbinding custom query or a small native module; wasmvm v3.0.3 confirms full CosmWasm support for the cw20 twin). Reverse-leg trust could shortcut IBC-style via threshold-Schnorr ed25519 verification of validator-signed supply attestations (open: validator-set tracking in the canister). **Legal gate (2026-09-15):** asset-dependent. Wrapping *fiat-referenced* stables (USDC/EURC twins minted by a bridge we operate) lands in the GENIUS Act issuance prohibition (S.1582: unlawful for anyone but a permitted issuer to "issue a payment stablecoin in the United States"; a wrapped representation redeemable at fixed value arguably fits the definition) and the EU MiCA ART regime - do not build fiat-wrapping legs; use canonical rails only (Circle's own CCTP-style burn-mint exists precisely because Circle wants no third-party wrappers). Non-fiat assets (ICP, ckBTC, GLDT, game tokens) carry no fiat reference: GENIUS/MiCA issuance rules do not attach, so the inverted-ck should target those. Note IBC on Terra Classic is already live (re-enabled via proposals 2022-2024; channels to Osmosis/Crescent confirmed operating as of 2026-09); what remains absent is any EVM track, so LUNC->EVM->ICP stays a watch item.

## Sources (retrieved 2026-09-12)

- Proposal record: `terra-classic-lcd.publicnode.com/cosmos/gov/v1beta1/proposals/12209`
- Proposal text + discussion: https://discourse.luncgoblins.com/t/lunc-forex-genesis-eutc-repeg/290
- Docs (status + developer reference): https://docs.terra-classic.money/forex-protocol/ · source: `Terra-Classic-money-Website/Terra-Classic-Docs` (`governance-and-status.md`, `developer-reference.md`)
- Burn tax: `classic-terra/core` v4 `x/tax`, `x/treasury` (local clone); live params via publicnode/hexxagon LCD
- MM2: classic-terra/core#664; `Market-Module-2-0/core` PRs #2/#3; StrathCole/Boulay GitHub profiles
- Press (context only, low trust): Binance Square 34061216870737/33982721854289, KuCoin LUNC news — both repeat community wishes (burn) not proposal text
