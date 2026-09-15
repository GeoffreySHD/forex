# EUTC / CSM — User Stories & Acceptance Criteria

Personas: **User** (EUTC holder/minter), **Redeemer**, **Governance/Admin** (proposal-driven admin of all modules), **Feeder** (oracle), **DEX/LP** (router operator), **LUNC holder** (the public good being funded).

Every story references the acceptance criteria (AC) it can be verified against. Behavior-level rules live in `SPEC.md`; this file is the product-facing view.

---

## A. Minting EUTC

### US-1 — Mint EUTC with EURC (primary path)
**As a** User, **I want** to deposit EURC and receive EUTC 1:1 (minus fees) **so that** I can hold a euro stablecoin backed by a verifiable collateral vault.

- **AC1** Sending EURC to csm-core via CW20 `Send` with `HookMsg::Mint` credits me EUTC = `deposited − 1.5% mint fee`, converted at the oracle rate (EURC: 1:1).
- **AC2** The 1.5% fee is not retained by the vault — I can verify on the reserve contract that the fee balance increased by exactly the fee.
- **AC3** If I send a token type that is not registered, my tx fails and my funds are unchanged.

### US-2 — Mint EUTC with USDC (secondary, premium path)
**As a** User holding only USDC, **I want** to mint EUTC at the EUR/USD rate minus a 0.5% premium **so that** I can still enter the ecosystem without first swapping on a DEX.

- **AC1** EUTC received = `USDC / (EUR/USD rate) × (1 − 0.005) − mint fee` per §3.1 of SPEC.md.
- **AC2** When EUR/USD moves, the minted amount moves with it (test: `mint_uses_current_oracle_rate`).

### US-3 — Protected from a broken price feed
**As a** User, **I want** mints to fail when the oracle is stale **so that** nobody mints EUTC against mispriced collateral.

- **AC1** If no price was ever posted for the collateral, or the latest price is older than `oracle_stale_secs`, the mint reverts and funds stay with me (test: `mint_fails_when_oracle_stale`).
- **AC2** There is no fallback price path — fail closed.

## B. Redeeming EUTC

### US-4 — Redeem EUTC for EURC
**As a** Redeemer, **I want** to burn EUTC and receive EURC 1:1 minus the 1.5% redeem fee **so that** I can exit at any time at par, verifiably.

- **AC1** Sending EUTC to csm-core with `HookMsg::Redeem { collateral_type: eurc }` burns the EUTC and returns `payout` = `E × rate × (1 − 0.015)` from the vault to my address.
- **AC2** The redeem fee is forwarded to the secondary reserve in the same tx (verifiable in the reserve's balances).
- **AC3** Total EUTC supply decreases by exactly the burned amount.

### US-5 — Predictable availability of redemptions
**As a** Redeemer, **I want** to know how much can be redeemed today **so that** I can plan my exit.

- **AC1** `ProtocolInfo` exposes `daily_redeem_remaining` and the window day.
- **AC2** If a redemption would exceed 10% of total supply per day (default), it reverts; the window resets after a UTC day (test: `daily_redemption_cap_enforced_and_resets`).

### US-6 — Exiting during a crisis
**As a** Redeemer, **I want** redemption to stay available when minting is paused **so that** a collateral or oracle problem can't trap my funds.

- **AC1** `Pause { mint: true, redeem: false }` blocks mints but allows redemptions (test: `pause_blocks_mint_and_redeem_then_resume`).
- **AC2** Only governance can pause; no role can pause both gates simultaneously via a single unauthenticated path.

## C. Collateral safety & transparency

### US-7 — Verifiable 1:1 backing
**As a** User, **I want** to recompute the collateral ratio myself **so that** I don't have to trust a dashboard.

- **AC1** `ProtocolInfo` exposes per-collateral `eutc_minted`, `primary_balance`, `fees_collected`, `collateral_returned` — the four ledgers stay separate (SPEC I2).
- **AC2** The vault's own balance query equals the module's `primary_balance` (test: `vault_mirror_matches_custody`).
- **AC3** The collateral ratio (`primary_balance / eutc_minted`, in percent ×100) is ≥ 100 in normal operation; deviations are attributable to specific flows (fees out).

### US-8 — No hidden admin power over funds
**As a** User, **I want** assurance that the admin cannot spend user collateral **so that** the system has no unilateral confiscation path.

- **AC1** The vault only accepts withdrawals initiated by csm-core, and csm-core only moves funds as part of mint/redeem flows (test: `admin_and_core_permissions_enforced`).
- **AC2** Users cannot deposit fees directly into the reserve to pollute the buyback accounting — only core/vault can (test: `reserve_rejects_direct_user_deposits`).

## D. Oracle operations

### US-9 — Feeder posts prices
**As a** Feeder, **I want** to post EUR/USD (and other) prices **so that** the protocol can value USDC deposits.

- **AC1** Posting `units_per_eutc` for a quote updates the stored price with `updated_at = block time`; posts closer together than `oracle_min_interval_secs` are rejected (FR-O2).
- **AC2** Only registered feeders can post; others are rejected.

### US-10 — Oracle health is observable
**As a** User, **I want** to see oracle health in the diagnostics **so that** I can anticipate fail-closed behavior before it hits me.

- **AC1** `ProtocolInfo.oracle_healthy` reflects per-quote freshness; `Price { quote }` exposes `fresh` per quote and `Status` the feeder count.

## E. Secondary reserve, buybacks & LUNC

### US-11 — Fees fund LUNC buybacks
**As a** LUNC holder, **I want** protocol fees to periodically buy LUNC **so that** the stablecoin creates structural demand for the chain's asset.

- **AC1** Buybacks are admin-triggered (`Buyback { offer, max_slippage_bps }`) and only from accumulated fee balances; primary collateral is untouched.
- **AC2** Purchased LUNC goes into the non-spendable vault (SPEC I4) — nobody, not even admin, can withdraw it (test: `buyback_success_updates_vault_and_ledger`).
- **AC3** If the router fails (slippage, liquidity), the escrowed collateral is returned and the reserve can retry later (test: `buyback_failure_returns_funds_and_allows_retry`).

### US-12 — Router cannot steal escrow
**As a** DEX/router operator, **I want** a clean failure-callback contract **so that** honest routing is easy and dishonest routing is bounded.

- **AC1** `BuybackResult` is only accepted from the configured router address; forged callbacks revert.
- **AC2** Slippage below the configured minimum sanity bound is rejected up-front (test: `buyback_rejects_invalid_slippage`).

## F. Governance operations

### US-13 — Deploy the system without a chicken-and-egg problem
**As** Governance, **I want** to wire contracts after instantiation **so that** address cycles (EUTC↔core↔vault↔reserve) don't block deployment.

- **AC1** `SetEutc`, `SetVault`, `SetReserve`, `SetOracle` (core), `SetCore`/`SetVault`/`SetRouter` (reserve) let me complete wiring in any order, admin-only.
- **AC2** Until wiring completes, the protocol fails closed (e.g., mints require the oracle; redeem requires registered tokens).

### US-14 — Tune protocol economics
**As** Governance, **I want** to update fees/premium/caps via `SetParams` **so that** the system can adapt to market conditions.

- **AC1** Params are validated atomically (fee ≤ 50%, cap in (0,100%], stale ≥ min interval) — invalid sets revert.
- **AC2** All parameter values are publicly readable via `State`/`ProtocolInfo`.

### US-15 — Emergency response
**As** Governance, **I want** an independent mint/redeem kill switch **so that** I can contain an incident without freezing redemptions.

- **AC1** `Pause { mint, redeem }` gates each path independently; resuming is the same message.

## G. Non-functional stories

### US-16 — Deterministic, verifiable arithmetic
**AC1** All token math uses checked integer ops (no floats); overflow aborts the tx rather than wrapping (SPEC §3).

### US-17 — Diagnostics are complete
**As** any observer, **I want** one query that summarizes protocol health **so that** explorers/dashboards can index it.
- **AC1** `ProtocolInfo` returns every diagnostic the forex spec's diagnostics list requires (FR-D1), each independently recomputable from public state.

### US-18 — Fee flows are conservation-checked
**AC1** For any sequence of operations, fees computed == fees received by the reserve (SPEC I3); the integration suite asserts this on mint, redeem, and buyback paths.

---

*Traceability: US-1/2/3 → FR-M1..M4 · US-4/5/6 → FR-R1..R4 · US-9/10 → FR-O1..O4 · US-11/12 → FR-B1..B5 · US-13..15 → FR-A1..A4 · US-17 → FR-D1 · US-7/8/16/18 → Invariants I1..I8.*
