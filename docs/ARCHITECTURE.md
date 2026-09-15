# EUTC / CSM — Architecture

**Status:** PoC reference implementation (compiled, 14/14 integration tests green). Not audited; not deployed.
**Companion docs:** [SPEC.md](./SPEC.md) (normative) · [USER-STORIES.md](./USER-STORIES.md) · [DIAGRAMS.md](./DIAGRAMS.md) (Mermaid UML) · [RESEARCH-CONTEXT.md](./RESEARCH-CONTEXT.md) (all verified research: proposal record/text, burn tax, workstream split, standalone-vs-in-core) · [TERMINOLOGY.md](./TERMINOLOGY.md) (regulatory wording policy — applies to all public-facing materials).

---

## 1. Design goals (from Proposal 12209 / the forex docs)

> Full verified context — proposal record, text, team, controversies, burn tax, deployment analysis — lives in [RESEARCH-CONTEXT.md](./RESEARCH-CONTEXT.md).

1. A **collateralized** euro stablecoin (EUTC) — 1:1 fiat-backed, no algorithmic expansion. The failure of UST is the design anti-model.
2. **Two collateral assets**: EURC (primary, euro-native) and USDC (secondary, via EUR/USD oracle rate + premium).
3. **Value accrual to Terra Classic**: mint/redeem fees flow to a secondary reserve that periodically **buys back LUNC** into a non-spendable vault — structural demand for the chain's asset.
4. **Full transparency**: every backing number independently recomputable from public state; layered per-collateral accounting.
5. **Safety-first failure modes**: fail-closed on stale oracle, independent mint/redeem kill switches, daily redemption cap, no admin path to user funds.

### 1.1 Proposal fidelity notes (verified against primary sources, 2026-09-12)

Sources: on-chain record of Proposal 12209 (TextProposal, submitted 2025-12-13, voting ended 2025-12-20, PASSED: ~490B LUNC yes / 81B no / 43B veto) and the [Discourse thread](https://discourse.luncgoblins.com/t/lunc-forex-genesis-eutc-repeg/290) it links.

**Faithful to the proposal:** 1.5% mint/redeem fees in-kind, 0.5% USDC premium, EURC 1:1 / USDC via EUR-USD rate, 10% daily redeem cap, fees → secondary reserve → LUNC buyback → vault, 30s oracle refresh, 5% USDC/EURC rebalance trigger, non-algorithmic 1:1 collateral design.

**Three deviations to close before any deployment:**
1. **Kill switch governance:** proposal requires a **5-of-9 multisig** for the pause; our PoC uses a single admin. Deployment MUST set admin to a multisig (or add one).
2. **Buyback automation:** proposal states buybacks run "automatically… no governance needed, no human involvement"; our `Buyback` is admin-triggered. Needs an epoch/anyone-can-crank mechanism with parameterized sizing.
3. **Vault vs burn disposition:** openly contested in the thread (community demanded buyback LUNC go to the **burn wallet**; proposer argued the vault is rebalancing collateral). Our I4 (non-spendable vault) follows the docs, but final disposition is an unresolved governance question — treat as a deployment blocker until settled by vote.

Context: EUTC pre-existed this proposal as a community token that lost its peg (hence "Repeg" in the title); the prop green-lights development only — no code, no funds — with liquidity seeding deferred to a future proposal (still not filed as of the docs' 2026-06-01 verification).

## 2. Why CosmWasm (decision record)

| Option | Verdict | Reason |
|---|---|---|
| **CosmWasm contracts** (chosen) | ✅ | Terra Classic runs wasmd v0.61.8 on cosmos-sdk v0.53.x (verified in `core/go.mod`). Fast iteration, CW20 ecosystem compatibility (EURC/USDC deployments exist as cw20s), `cw-multi-test` gives a full local chain harness, no hard fork required. |
| Native SDK module (fork `core/`) | ❌ for now | Hard-fork politics, slow coordination, and the wasm path reaches production sooner. A future promotion path exists (see §7). |
| CW20-only with off-chain reserve | ❌ | Buyback custody and fee conservation require on-chain enforcement. |

Deliberate consequence: collateral, EUTC, and the reserve all speak **CW20**, so one protocol (cw20 `Send` + hooks) covers user flows, fee forwarding, and vault custody.

**Standalone vs in-core (verified 2026-09-12):** this CSM does **not** require touching `classic-terra/core`. The chain already ships wasmd v0.61.8 + wasmvm 3.0.3 (`core/go.mod`), so these contracts deploy to mainnet/testnet as an independent repo via a governance upload prop — no hard fork, no validator coordination beyond that prop, upgradable via contract admin/migrations. Context only, out of scope going forward: a separate team is rebuilding the *old* Market Module in-core as "MM2" (the USTC↔LUNC repeg — different asset, different proposal, tracked in classic-terra/core#664 and the `Market-Module-2-0/core` org). **This project has no dependency on MM2, does not track its progress, and shares no code with it.** The Forex/CSM (this workspace) is the collateralized repeg of the fiat-pegged TC family (EUTC first, WONTC/YENTC later) with external collateral and no LUNC in the peg. As of 2026-09-12, **no CSM application-layer code exists anywhere** except this workspace.

**Decision criterion — in-core vs standalone:** go native only when consensus-level privileges are unavoidable (fee-flow rewiring, native denom minting, protocol-parameterized burns). The CSM needs none of these: it is custody + accounting over assets that are themselves **cw20 tokens** (the legacy EUTC/WONTC/YENTC family), so native code would spend its time reading wasm state anyway. Precedent agrees — Astroport, Terraswap, and every prior TC stable live as contracts, not modules. **Decision: standalone wasm repo, shipped independently (one governance upload prop, admin-migratable); revisit in-core only if wasm gas economics (650k/op floor) ever prove material.**

## 3. System decomposition

Five contracts, one shared types crate:

| Contract | Responsibility | Trust boundary |
|---|---|---|
| **`csm-std`** (package) | Shared types: `CollateralType`, `Params`, `CollateralState`, `ProtocolStatus`, oracle & reserve interfaces. | — |
| **`csm-core`** | Mint/redeem state machine, fees, caps, pause, layered accounting, protocol diagnostics. Emits `TokenMint`/`TokenBurn` + vault transfers. | Admin-only config; user flows via CW20 hooks only. |
| **`csm-vault`** | Dumb custody: holds collateral cw20s, accepts deposits from anyone (cw20 Receive), releases only on csm-core's `Withdraw`, mirrors per-token balances. | No policy — every spend is co-signed by csm-core's instruction. |
| **`oracle-adapter`** | Feeder-posted fiat prices (`units_per_eutc` per quote currency), freshness (`Price`/`Status`), feeder registry, min-interval anti-spam. | Feeders trusted for values; staleness bounds damage. |
| **`csm-reserve`** | Fee sink (`Deposit` hooks from core/vault only), buyback state machine (`Buyback` → router → `BuybackResult`), non-spendable LUNC vault, buyback ledger. | Router only supplies execution; slippage caps and refund-on-failure bound it. |
| **`mock-router`** | Test double implementing the router interface (fixed-rate swaps, configurable failure). Not deployed. | — |

EUTC itself is a **plain `cw20-base` instance** with `csm-core` registered as the sole minter — no custom token contract needed.

### 3.1 Message surface (summary)

- **csm-core execute:** `Receive` (hooks `Mint`/`Redeem`), `Pause`, `SetParams`, `RegisterCollateral`, `SetEutc`, `SetVault`, `SetReserve`, `SetOracle`.
- **csm-reserve execute:** `Receive` (`Deposit` hook), `RegisterToken`, `SetCore`, `SetVault`, `Buyback { offer, max_slippage_bps }`, `BuybackResult { success, purchased, reason }`, `SetRouter`, `SetAdmin`.
- **oracle-adapter execute:** `PostPrice`, `RegisterFeeder`, `RemoveFeeder`, `SetAdmin`. Queries: `Price`, `Status`, `Feeders`.
- **csm-vault execute:** `Receive`, `Withdraw` (core-only, with fee-forwarding hook). Queries: balances, per-token mirror.
- **csm-core queries:** `Config`, `State`, `Price { quote }`, `ProtocolInfo` (the full diagnostics answer).

### 3.2 Key accounting flows (see DIAGRAMS.md for sequences)

**Mint:** user → cw20 `Send(collateral → core, Mint)` → core computes EUTC & fee → `TransferFrom(user→vault, net)` + `Send(vault→reserve, fee)` + `Mint(eutc, user)` → update 4 ledgers.

**Redeem:** user → cw20 `Send(eutc → core, Redeem{C})` → cap check → burn → `Withdraw` from vault: `payout → user`, `fee → reserve` → decrement ledgers.

**Buyback:** admin → reserve `Buyback{offer, slippage}` → escrow from `reserve_collateral` → `Swap` to router → router `BuybackResult{success?}` → success: LUNC → vault, ledger++; failure: refund escrow, record reason.

## 4. Deployment sequence (mainnet-shaped)

1. Instantiate **oracle-adapter** (admin, staleness params) → register feeders.
2. Instantiate **csm-vault** (admin = governance; core = placeholder).
3. Instantiate **csm-reserve** (admin, core placeholder, vault).
4. Instantiate **cw20-base EUTC** with minter = csm-core placeholder... *cycle*: csm-core needs EUTC's address and vice versa. Broken by **rewiring messages**: instantiate EUTC with a throwaway minter or instantiate core first, then `SetEutc`/`SetCore`/`SetVault`/`SetReserve`/`SetOracle` to complete the graph (FR-A2; exercised in the test harness).
5. `RegisterCollateral` for EURC/USDC (core) + `RegisterToken` (reserve); register the vault with the reserve (fee path).
6. Feeders post initial prices; governance verifies `ProtocolInfo`.
7. Admin hands over to governance: `SetAdmin` (reserve) / migrate-style admin updates.
8. Enable: mints flow automatically once wiring + prices are live.

## 5. Security posture

**Trust minimization**
- No admin path to user collateral: the vault releases only to csm-core instructions, and csm-core only moves funds in mint/redeem (I7, tested).
- Reserve accepts deposits only from core/vault (I6, tested); router callbacks only from the router (tested).
- LUNC vault has **no spend path at all** (I4).

**Failure containment**
- Fail-closed oracle (no fallback price), independent pause gates, daily redemption cap (run damping), validated params (no fee > 50%, no cap > 100%).
- Buyback atomicity: escrow + explicit result callback; failure refunds and re-enables retry (I8, tested both paths).

**Known PoC gaps (honest list)**
- `rebalance_threshold_bps` reserved but **not enforced** — no automated USDC/EURC rebalancing yet.
- Rate math truncates (floor) on integer division; dust stays with the protocol rather than the user. No rounding-direction audit yet.
- Feeder key compromise is a live risk until the planned **consensus-rate integration** lands: the chain's `x/oracle` ballot rates (slashing-backed, all 20 forex denoms) are already exposed to contracts via the wasmbinding `ExchangeRates` query (verified in `core/wasmbinding/`, see RESEARCH-CONTEXT §10e). Phase 1: csm-core reads consensus rates as primary source, `oracle-adapter` demoted to fallback/override (also keeps multi-test hermetic).
- No cw20-blacklist/malicious-token hardening: only whitelisted tokens are accepted, but upstream token behavior (pause-able USDС etc.) is an external dependency risk.
- Buyback sizing policy is deliberately simple (per-route parameters); it is not an optimal-execution engine.

## 6. Interaction with Terra Classic chain taxes (verified against `core/` v4 source)

This matters for mainnet economics and is **not** modeled in multi-test:

| Path | Burn tax applied? | Evidence in `core/` |
|---|---|---|
| Bank `MsgSend` / `MsgMultiSend` | **Yes** (unless exempted) | `x/tax/handlers/bank_msg_server.go` → `taxKeeper.DeductTax` on send/multisend (input side; output side on multisend), gated by `taxexemptionKeeper.IsExemptedFromTax`. |
| **Wasm `MsgExecuteContract`** | **No (currently)** | `x/tax/handlers/wasm_msg_server.go` `ExecuteContract`: the `DeductTax` block is **commented out**; it is a pass-through to the underlying message server. Same for Instantiate/Instantiate2. |
| Settlement | Post-handler collects `ContextKeyTaxDue`, `ProcessTaxSplits` (default split 90% burn / 10% community pool), treasury epoch proceeds. Rate: code genesis default 0.1% (`DefaultTaxRate`), **live chain rate 1.5%** (governance-set; verified via `/terra/tax/v1beta1/params` on public LCD nodes, 2026-09-12). | `x/tax/post/post.go`, `x/treasury` defaults, `x/tax/keeper/params.go` |

**Which assets the burn tax can touch (from `x/tax/types/compute.go` `ComputeTaxes`):**

- `coin.Denom == sdk.DefaultBondDenom` → **skipped**. `DefaultBondDenom` = `uluna` (`app/params/params.go`) — **LUNC itself is never burn-taxed** on its transfers.
- `IsIBCDenom(coin.Denom)` → **skipped** — bridged (IBC) assets are exempt.
- Everything else that is a **bank denom** (classic terra currencies `uusd`, `ueur`, `ukrw`, … and factory denoms) is taxed at `rate × amount`, clamped by the **per-denom `TaxCap`** — on small transfers the cap, not the rate, is what applies.
- **CW20 tokens are not bank denoms**: EUTC, WON, or any other cw20 cannot be taxed by the bank path at all — they exist only as contract state moved via `MsgExecuteContract`, whose tax logic is currently disabled.

**Implications for the CSM:**
1. User→core cw20 sends are `MsgExecuteContract`s → **not taxed today**. If governance re-enables the wasm tax path (it is visibly under active design), the tax attaches to `msg.funds` — which is **empty** in cw20 `Send` hook flows (funds move inside the token contract, not as attached bank coins) — so hook-based mint/redeem would still be untouched. EUTC itself, being cw20, can never appear as a bank denom and is structurally out of reach of the bank tax.
2. The reserve's buyback `Swap` to the DEX router is also a wasm execute → no tax; but if the router's pools or delivery legs use **native bank denoms** (e.g., classic terra currencies), those legs are taxed at the live 1.5% rate subject to per-denom caps. **LUNC received by the vault is never taxed** (`uluna` skip).
3. Tax exemption: `x/taxexemption` allows address-pair exemptions via governance — the CSM contracts (core/vault/reserve) are natural exemption candidates if bank-path tax ever intersects the flows.
4. Requirement derived: **integration tests on a live wasmd chain (not multi-test) must assert fee conservation with the chain tax active**, and the reserve's buyback sizing must account for tax on any native-denom router legs. The live 1.5% rate (coincidentally equal to the protocol fee) makes this material — but only for bank-denom legs, which the current CW20-only design avoids.

## 7. Roadmap

**Phase 0 — this PoC (done)**
- [x] 6 contracts + shared types, cosmwasm-std 2.1 / cw-plus 2.0 generation
- [x] 28 tests green (multi-test integration + unit), clippy-clean, incl. the six-test anti-dump suite and the mint→dump→redeem attack scenario (each wash loop costs ~5.9%)
- [x] Architecture + UML + formal spec + user stories (this doc set)

**Phase 1 — hardening (partially done)**
- [x] **Consensus-rate oracle integration**: csm-core `ConsensusPrimary` path via the wasmbinding `ExchangeRates` query (types mirrored in `csm-std::consensus`); oracle-adapter is the fallback; the adapter path is used in hermetic tests
- [x] **Anti-dump stack** (per-address redeem cap, mint-velocity cap with bootstrap floor, dynamic spread, flash fee, ratio governor — RESEARCH-CONTEXT §10a), with caps referencing a day-start supply snapshot
- [x] **Legacy-unit conversion bridge** (`legacy-bridge`: `Convert{}` attached-funds, global + per-address caps, linear vesting, permanent sink, no-promise-of-value disclosure — §10b/§10a L5)
- [x] wasm32-unknown-unknown release builds (all six contracts, entry points verified) incl. the `--import-undefined` linker flag pinned for rustc ≥1.96; JSON schemas generated per contract
- [ ] Property tests for the invariants (I1–I8) via stateful fuzzing (`proptest` / `cw-fuzz`-style state machine)
- [ ] `cw-check-contract` metadata + reproducible build (Docker/Rust version pinned)
- [ ] Consensus-oracle degradation tests against a live RPC (failed-ballot behavior)
- [ ] Live `wasmd` testnet integration suite incl. chain-tax interaction (§6.4)
- [ ] Rebalancing policy implementation (enforce `rebalance_threshold_bps`) or its removal from `Params`

**Phase 2 — economics & governance**
- [ ] Buyback sizing/ frequency policy + simulation against historical LUNC liquidity
- [ ] Migration from admin-multisig to on-chain governance (proposal-driven `SetParams`/`Buyback`)
- [ ] Collateral onboarding framework (third stablecoin path), incl. oracle + premium policy
- [ ] External audit (state machine + cw20 edge cases) before any mainnet funds

**Phase 3 — chain integration (optional promotion)**
- [ ] Evaluating promotion of csm-core accounting into a native module if gas/perf requires; otherwise stay wasm
- [ ] Tax-exemption proposals for CSM contract addresses
- [ ] Frontend + indexer (ProtocolInfo feed), stableswap integration for EUTC<->EURC/USDC liquidity bootstrapping

**Phase 4 — cross-chain corridor: "inverted ck" for non-fiat ICP assets (RESEARCH-CONTEXT open question #11)**

Goal: bring non-fiat ICP-side assets onto Terra Classic as cw20 twins — GLDT (gold), ckBTC, ICP, and third-party ICP tokens (e.g. game tokens) — minted/burned against ICP chain-key attestations. **Scope excludes fiat-referenced twins (USDC/EURC wrappers): GENIUS Act issuance prohibition + MiCA ART make wrapping fiat stables a legal red line; canonical issuer rails only.** Preferred outcome is a joint design with DFINITY (chain-key attestation format for external chains is a natural extension of the ckERC-20 pipeline); failing that, ship the threshold-Schnorr (ed25519) attestation variant, which requires no LUNC hard fork (pure cw20 + tSchnorr verification in CosmWasm) and mirrors the ckLUNC pattern in reverse.

- [ ] Design doc: attestation format (canister-signed burn/lock statements), subnet public-key rotation handling, mint/burn accounting invariants (twins in circulation <= attested custody)
- [ ] DFINITY outreach: forum RFC proposing chain-key attestation export for non-ICP chains; explore Axelar ITS as alternative transport if it ever gains ICP GMP support
- [ ] `twins-minter` cw20 contract: tSchnorr ed25519 attestation verification in CosmWasm, per-asset registry with supply ceilings, pausable mint, twinned with an ICP-side escrow/burn canister spec
- [ ] GLDT pilot (with Gold DAO / GOLDAO cooperation): 1 GLDT-on-LUNC == 1 ICP-side GLDT, custody attested, ceilings set by GOLDAO governance
- [ ] ckBTC pilot (iff DFINITY cooperation lands; ckBTC custody is chain-key cryptographic — the strongest custody tier available)
- [ ] Reverse leg (LUNC -> ICP): requires ICP-side verification of LUNC statements — either a Motoko light client (research item) or tSchnorr-signed LUNC attestations the canister verifies; shares the design with the ckLUNC B2b work
- [ ] Legal review of the finalized attestation model (non-fiat scope re-confirmed; no yield, no offer, disclosures per TERMINOLOGY.md)

