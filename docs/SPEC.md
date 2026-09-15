# EUTC / Collateralized Stablecoin Module (CSM) — Formal Specification

**Status:** Reference implementation (PoC), unreviewed. Target: Terra Classic.
**Source of truth:** [Terra Classic Docs — Forex Protocol](https://github.com/Terra-Classic-money-Website/Terra-Classic-Docs) (`content/docs/forex-protocol/`), grounded by the passing of **Proposal 12209**.
**Scope of this document:** normative behavior of the EUTC stablecoin and the CSM contracts in this repository (`forex/contracts/*`, `forex/packages/csm-std`).

---

## 0. Normative language

The key words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are to be interpreted as described in RFC 2119 when they appear in ALL CAPS.

Anything marked **[PoC simplification]** deviates from, or is unspecified in, the on-chain spec and must be revisited before any mainnet deployment.

## 1. Terminology

| Term | Definition |
|---|---|
| **EUTC** | Euro-pegged stablecoin ("Euro Terra Classic"). A CW20 token minted 1:1 against euro-denominated collateral. |
| **CSM** | Collateralized Stablecoin Module — the contract system that mints/redeems EUTC against collateral. |
| **Primary collateral** | Stablecoins held 1:1 in the vault backing outstanding EUTC. |
| **EURC** | Euro-backed stablecoin (Circle). Designated **primary** collateral type. |
| **USDC** | USD stablecoin. Accepted **secondary** collateral, minted against at a premium. |
| **Secondary reserve** | Contract that accumulates protocol fees and executes periodic LUNC buybacks. |
| **LUNC vault** | Non-spendable accumulation of buyback-purchased LUNC inside the reserve (public good burn/sink). |
| **Oracle adapter** | Feeder-posted fiat price feed (EUR/USD etc.) used to value non-euro collateral. |
| **Quote** | A posted price: units of a quote currency per 1 EUTC. |

## 2. Actors

| Actor | Role | Trust assumption |
|---|---|---|
| **User / Holder** | Mints EUTC by sending collateral via CW20 `Send` hook; redeems by sending EUTC back. | Untrusted. |
| **Admin** | Governance (or a multisig during rollout): sets params, pauses/resumes, registers tokens, rewires modules, triggers buybacks. | Trusted to act within protocol rules; no key can move user funds. |
| **Feeder(s)** | Post fiat prices to the oracle adapter. | Trusted for price values (not for funds); staleness checks bound the damage. |
| **DEX router** | Executes the reserve's LUNC buyback and reports the outcome. | Adversarial-tolerant: slippage caps + failure-callback refund path. |

## 3. Core math

All arithmetic uses `Uint128`/`Decimal` with checked ops; any overflow MUST abort the message. Rates are expressed as a `Fraction { numerator, denominator }` = quote-currency units per 1 EUTC.

### 3.1 Mint

User deposits `D` units of collateral `C` (delivered via CW20 `Send` to csm-core):

```
rate      = oracle_rate(C)                          // Fraction: quote units per EUTC
eutc_gross = D × rate.denominator / rate.numerator  // = D / rate
premium   = (C == USDC) ? usdc_premium_bps : 0
EUTC      = eutc_gross × (10_000 − premium) / 10_000
fee_C     = D × mint_fee_bps / 10_000               // taken in collateral C
vault_in  = D − fee_C
```

- `fee_C` MUST be forwarded to the secondary reserve.
- `vault_in` MUST be transferred to the vault and credited to `state[C].primary_balance`.
- `EUTC` MUST be minted to the user (csm-core is the CW20 minter).
- The spec-literal formula `EUTC = USDC × 0.995` corresponds to `rate = 1/1.0925` (EUR/USD) and `premium = 50 bps`.

### 3.2 Redeem

User sends `E` units of EUTC to csm-core (CW20 `Send` hook), selecting payout collateral `C`:

```
rate      = oracle_rate(C)
collateral_units = E × rate.numerator / rate.denominator   // = E × rate
payout    = collateral_units × (10_000 − redeem_fee_bps) / 10_000
fee_C     = collateral_units × redeem_fee_bps / 10_000
```

- EUTC MUST be burned.
- `payout` MUST be transferred from the vault to the user; `fee_C` MUST be transferred from the vault to the secondary reserve.
- `state[C].primary_balance` MUST decrease by the full `collateral_units` (payout **and** fee both leave custody).

### 3.3 Daily redemption cap

Let `S` = current total EUTC supply. In a UTC-day window:

```
redeemed_today ≤ S × daily_redeem_cap_bps / 10_000
```

- If a redemption would exceed the cap, it MUST revert. The cap is evaluated against the **supply at the time of the redemption**, and `redeemed_today` resets at each window rollover (see `StateResponse.redeem_window_day`).
- Rationale: give arbitrageurs time to replenish the reserve, prevent a death-spiral run in a day.

### 3.4 Oracle freshness (fail-closed)

A quote for `C` is **fresh** iff `price.updated_at + stale_secs ≥ block_time`. Both mint and redeem MUST consult the oracle for the payout/deposit collateral and MUST revert if the quote is missing or stale. There is no fallback price.

## 4. Protocol parameters (`Params`)

| Field | Default | Meaning | Validation |
|---|---|---|---|
| `mint_fee_bps` | 150 | Fee on mint, in deposit collateral. | ≤ 5000 |
| `redeem_fee_bps` | 150 | Fee on redeem, in payout collateral. | ≤ 5000 |
| `usdc_premium_bps` | 50 | Mint premium for USDC deposits. | ≤ 5000 |
| `daily_redeem_cap_bps` | 1000 | Max share of **day-start** supply redeemable per day (10%). The cap denominator is the day-start supply snapshot (`DAY_SUPPLY`), refreshed lazily per UTC day — not live supply, which shrinks as redemptions burn tokens. | 1..10 000 |
| `per_addr_redeem_cap_bps` | 100 | Max share of day-start supply one address can redeem per day (1%). MUST be ≤ `daily_redeem_cap_bps`. | 1..daily cap |
| `daily_mint_cap_bps` | 2000 | Max new supply per day per collateral, as a share of day-start supply (20%). | 1..10 000 |
| `min_daily_mint_floor` | 100 000 000 | Absolute floor on the daily mint allowance (collateral µ-units). Bootstrap-only: the percentage cap is relative to supply (zero at genesis), so the allowance is `max(pct, floor)`. | > 0 |
| `max_dynamic_spread_bps` | 1000 | Redeem-spread headroom added linearly with daily-window utilization (0% full → +0, 100% full → +1000 bps). | ≤ 5000 |
| `flash_fee_bps` | 300 | Extra redeem fee when the sender's freshest mint is within `flash_fee_window_blocks`. | ≤ 5000 |
| `flash_fee_window_blocks` | 30 | Flash-fee window (blocks since freshest mint to the sender). | ≤ 500 |
| `min_primary_ratio_bps` | 9500 | Solvency governor: mints revert while the aggregate primary ratio is below this. Must sit BELOW the fee-diluted steady state (~98.5% on the EURC leg) or normal operation halts. | ≤ 30 000 |
| `rebalance_threshold_bps` | 500 | USDC/EURC share divergence triggering rebalancing. | — **[PoC simplification: reserved, not yet enforced]** |
| `oracle_min_interval_secs` | 30 | Min seconds between price posts. | > 0 |
| `oracle_stale_secs` | 300 | Price older than this fails closed (adapter mode; consensus rates are per-block). | ≥ min interval |

All parameter updates go through admin-only `SetParams` and MUST be validated atomically.

## 5. Invariants

- **I1 (Solvency):** For every collateral `C`: `state[C].primary_balance` ≥ total EUTC minted against `C`, at the oracle rate, at all times *between* successful operations. Mint/redeem sequences never create a state where the vault cannot pay out all outstanding EUTC at the last-used rates.
- **I2 (Layered accounting):** The four counters per collateral (`eutc_minted`, `primary_balance`, `fees_collected`, `collateral_returned`) MUST stay independently verifiable and MUST NOT be collapsed into one balance.
- **I3 (Fee conservation):** Every fee computed on a successful mint/redeem MUST arrive at the secondary reserve in the same collateral type. The vault's `balances` mirror MUST equal the CW20 balances actually held by the vault.
- **I4 (Non-spendable vault):** The reserve's LUNC vault balance MUST NOT be withdrawable by anyone, including admin; it can only increase (buybacks) — disposition (burn/send-to-community) is a future governance decision handled at the chain level.
- **I5 (Fail-closed issuance):** No mint or redeem succeeds without a fresh oracle quote for the relevant collateral (when the oracle is wired).
- **I6 (No direct reserve deposits):** Only csm-core (and the vault on the redemption-fee path) can deliver funds to the reserve; user-direct `Send` hooks MUST be rejected.
- **I7 (Permission separation):** User-callable surfaces (mint, redeem) are the CW20 hook paths only; every admin message MUST revert when called by a non-admin; the reserve's `BuybackResult` MUST revert when called by anyone but the configured router.
- **I8 (Buyback atomicity):** A failed buyback MUST return the escrowed collateral to the vault-ledger and restore availability for retry; a successful buyback MUST move the exact offered amount out of `reserve_collateral`, credit LUNC to the vault, and increment `swaps_executed`/`lunc_purchased_total`.

## 6. Functional requirements

### 6.1 Minting
- **FR-M1** The system MUST mint EUTC via a CW20 `Send` hook from a registered collateral token, per §3.1.
- **FR-M2** Mints MUST fail closed when the protocol is mint-paused, the token is unregistered, the amount is zero, or the oracle quote is stale/missing.
- **FR-M3** USDC mints MUST apply `usdc_premium_bps` to the minted amount; EURC mints MUST NOT.
- **FR-M4** The mint fee MUST be split off the deposit and forwarded to the secondary reserve (via the vault), not retained in primary custody.

### 6.2 Redemption
- **FR-R1** The system MUST redeem (burn + pay out) via a CW20 `Send` hook carrying `HookMsg::Redeem { collateral_type }`, per §3.2.
- **FR-R2** Redemptions MUST enforce the daily cap of §3.3 and reset the window per UTC day.
- **FR-R3** The redeem fee MUST be forwarded to the secondary reserve; the payout MUST come out of primary custody.
- **FR-R4** Redemptions MUST fail closed when redeem-paused, the payout collateral is unregistered, or the oracle quote is stale/missing.

### 6.3 Oracle
- **FR-O1** Registered feeders MAY post `PostPrice { quote, units_per_eutc }`; non-feeders MUST be rejected.
- **FR-O2** Posts MUST respect `oracle_min_interval_secs`; earlier posts are rejected (spam bound).
- **FR-O3** `Price { quote }` MUST return the latest price and a `fresh` flag; `Status` MUST return feeder count and per-quote staleness.
- **FR-O4** Admin MAY register/unregister feeders and rewire the admin address.

### 6.4 Secondary reserve & buyback
- **FR-B1** The reserve MUST accept fee deposits only via whitelisted senders (csm-core, the vault) with a `Deposit` hook.
- **FR-B2** Admin MUST be able to configure the DEX router route for a collateral→LUNC swap (`SetRoute`).
- **FR-B3** `Buyback { offer, max_slippage_bps }` MUST: derive the swap amount from accumulated fees per the configured policy, escrow the funds, invoke the router, and settle from the router's `BuybackResult` callback.
- **FR-B4** A successful buyback MUST credit purchased LUNC to the non-spendable vault (I4) and update the buyback ledger; a failed buyback MUST return funds (I8) and record the failure reason.
- **FR-B5** The route MUST be required to quote LUNC out; a route that does not MUST be rejected (`max_slippage_bps` sanity-checked too).

### 6.5 Administration & safety
- **FR-A1** `Pause { mint, redeem }` MUST gate mint and redeem independently (a collateral or oracle problem halts issuance without trapping holders).
- **FR-A2** Token registration (`RegisterCollateral`, reserve `RegisterToken`), wiring (`SetEutc`, `SetVault`, `SetReserve`, `SetOracle`, `SetCore`, `SetRouter`), and `SetParams` MUST be admin-only. Wiring messages exist to break deployment-time address cycles (e.g. EUTC needs csm-core as minter before csm-core can know EUTC's address).
- **FR-A3** `SetAdmin` (reserve) MUST allow governance handover.
- **FR-A4** Only the admin of csm-core MAY withdraw from the vault, and the vault MUST forward withdrawal proceeds as instructed by csm-core (fee split to reserve).

### 6.6 Diagnostics
- **FR-D1** `ProtocolInfo` MUST report: status, params, EUTC supply/minted, per-collateral layered accounting (I2), collateral ratio percent, daily redeem headroom + window day, reserve balances, LUNC vault amount, and oracle health. Every number in this answer MUST be independently recomputable from public chain state.

## 7. Test matrix (implemented in `forex/contracts/csm-core/tests/integration.rs`)

| # | Requirement | Test |
|---|---|---|
| 1 | FR-M1, FR-M4 (EURC 1:1, fee to reserve) | `mint_eurc_one_to_one_with_fee` |
| 2 | FR-M3 (USDC premium) | `mint_usdc_applies_premium` |
| 3 | FR-M1 (oracle rate honored) | `mint_uses_current_oracle_rate` |
| 4 | FR-M2 (stale price → fail closed) | `mint_fails_when_oracle_stale` |
| 5 | FR-R1, FR-R3 (burn + payout + fee split) | `redeem_eurc_burns_and_returns_same_collateral` |
| 6 | FR-R2 (10% daily cap + day rollover) | `daily_redemption_cap_enforced_and_resets` |
| 7 | FR-A1 (pause/resume, independent gates) | `pause_blocks_mint_and_redeem_then_resume` |
| 8 | FR-A2/A4, I7 (permission boundaries) | `admin_and_core_permissions_enforced` |
| 9 | I3 (vault mirror == custody) | `vault_mirror_matches_custody` |
| 10 | FR-B3/B4/I8 (buyback success path) | `buyback_success_updates_vault_and_ledger` |
| 11 | FR-B4/I8 (buyback failure → refund + retry) | `buyback_failure_returns_funds_and_allows_retry` |
| 12 | FR-B5 (slippage/route validation) | `buyback_rejects_invalid_slippage` |
| 13 | FR-D1 (diagnostics coherence) | `protocol_info_reports_diagnostics` |
| 14 | I6 (no direct reserve deposits) | `reserve_rejects_direct_user_deposits` |

**Status: 14/14 passing** against `cw-multi-test` (cosmwasm-std 2.1 / cw-plus 2.0 generation), including CW20 flows driven through real `cw20-base` instances.

## 8. Explicitly out of scope (PoC)

- On-chain IBC collateral (multi-chain EURC/USDC bridges).
- Automated rebalancing enforcement (parameter reserved, §4).
- Minter cap / algorithmic backstop — the spec intentionally excludes algorithmic components; solvency is 1:1.
- Native (Coin) collateral — CW20-only in this implementation.
- Tax integration: LUNC's chain-level burn tax is NOT modeled in multi-test; see `ARCHITECTURE.md` §6 for the interaction analysis.
