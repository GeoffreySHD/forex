# forex — EUTC / Collateralized Unit Module (reference implementation)

[![CI](https://github.com/Semence2Porc/forex/actions/workflows/ci.yml/badge.svg)](https://github.com/Semence2Porc/forex/actions/workflows/ci.yml)

A working CosmWasm implementation of the **Euro Collateral Unit (working name EUTC-C)** and its **Collateralized Unit Module (CSM)** — units minted *against* user-deposited collateral (EURC/USDC) and withdrawable *from* it at oracle reference rates — following the [Terra Classic Forex Protocol](https://github.com/Terra-Classic-money-Website/Terra-Classic-Docs) direction accepted via Proposal 12209. **Greenfield by design** — as of 2026-09 there is no upstream CSM/EUTC code anywhere; this workspace is the first implementation. Public-facing wording follows [docs/TERMINOLOGY.md](./docs/TERMINOLOGY.md) (mechanism descriptions, no value promises).

> ⚠️ **PoC, not production.** Unaudited, not deployed, no mainnet funds. See [docs/ARCHITECTURE.md §5](./docs/ARCHITECTURE.md) for the known-gaps list.

## What's here

| Contract | Role |
|---|---|
| `contracts/csm-core` | Mint/redeem state machine, 1.5% fees, layered rate limits (10% daily redeem cap, per-address cap, daily mint-velocity cap, dynamic spread, flash fee, solvency governor), USDC premium, independent pause gates, `ProtocolInfo` diagnostics. |
| `contracts/csm-vault` | Dumb collateral custody — releases funds only on csm-core instruction; mirrors balances. |
| `contracts/csm-reserve` | Fee sink; LUNC buyback state machine (escrow → router → settle/refund); **non-spendable LUNC vault**. |
| `contracts/oracle-adapter` | Feeder-posted fiat prices with freshness (fail-closed issuance) and anti-spam min interval — fallback source behind chain consensus rates. |
| `contracts/legacy-bridge` | Opt-in conversion of legacy `ueur`/`ukrw`/… balances: attached-funds Convert, global + per-address caps, linear vesting, permanent sink, explicit no-promise-of-value disclosure. |
| `contracts/mock-router` | Test double DEX router (not deployed). |
| `packages/csm-std` | Shared types: `CollateralType`, `Params` (spec defaults), oracle/reserve interfaces, **chain-consensus rate types** mirroring the terra wasmbinding. |
| `contracts/*/tests/` | 28 multi-test integration tests mapped to the spec's test matrix, incl. the six-test anti-dump suite and the attack-scenario test. EUTC is a real `cw20-base` instance. |

EUTC itself needs no custom token: it's a plain `cw20-base` deployment with `csm-core` registered as minter.

## Documentation

| Doc | Contents |
|---|---|
| [`docs/SPEC.md`](./docs/SPEC.md) | Formal spec: math, parameters, invariants I1–I8, functional requirements, test matrix. |
| [`docs/USER-STORIES.md`](./docs/USER-STORIES.md) | Personas + user stories with acceptance criteria, traceable to the tests. |
| [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) | Decision record, decomposition, deployment sequence, security posture, **chain burn-tax interaction analysis** (verified against `core/` v4 source), roadmap. |
| [`docs/RESEARCH-CONTEXT.md`](./docs/RESEARCH-CONTEXT.md) | **Everything researched from primary sources**: Proposal 12209 record & text, vault-vs-burn controversy, EUTC history, MM2-vs-FOREX workstream split, burn-tax mechanics, standalone-vs-in-core analysis (incl. the in-core advantages), status & open questions. |
| [`docs/DIAGRAMS.md`](./docs/DIAGRAMS.md) | Mermaid UML: component map, class diagram, mint/redeem/buyback sequences, state machines, ER model. |
| [`docs/TERMINOLOGY.md`](./docs/TERMINOLOGY.md) | **Terminology policy (regulatory)**: banned vs approved wording for public materials (MiCA/GENIUS awareness; not legal advice). |

## Quick start

```bash
cargo test --workspace          # 28 tests (cw-multi-test + unit)
cargo clippy --all-targets      # zero warnings
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown --lib -p csm-core -p csm-vault -p csm-reserve -p csm-oracle-adapter -p legacy-bridge
cargo run --bin schema -p csm-core    # per contract; writes contracts/<name>/schema/
```

> Note: `--lib` matters — the workspace also contains per-contract schema generator binaries which refuse to compile for wasm32 (by design). The `--import-undefined` linker flag required by rustc ≥1.96 on this target is pinned in `.cargo/config.toml`.

Stack: cosmwasm-std **2.1** / cw-plus **2.0** / cw-multi-test **2.2**, Rust stable (1.98 used; GNU toolchain on Windows works — no MSVC Build Tools needed).

## Implementation status

| Area | Status |
|---|---|
| Mint/redeem math (spec §3.1–3.2) | ✅ implemented + tested |
| Daily redeem cap + window reset (day-start snapshot) | ✅ implemented + tested |
| Per-address redeem cap | ✅ implemented + tested |
| Daily mint-velocity cap (bootstrap floor) | ✅ implemented + tested |
| Dynamic redeem spread (utilization-based) | ✅ implemented + tested |
| Flash fee on fresh mints | ✅ implemented + tested |
| Solvency governor (ratio floor) | ✅ implemented + tested |
| Attack scenario: mint→dump→redeem loses money | ✅ tested (−5.9% per loop) |
| Chain-consensus rates via wasmbinding (primary oracle) | ✅ implemented (adapter fallback tested) |
| Legacy-unit conversion bridge (caps, vesting, disclosure) | ✅ implemented + tested |
| USDC premium, fees to reserve | ✅ implemented + tested |
| Fail-closed oracle gating (mint & redeem) | ✅ implemented + tested |
| Independent pause gates + `SetAdmin` | ✅ implemented + tested |
| Vault custody + mirror | ✅ implemented + tested |
| Reserve fee sink + direct-deposit rejection | ✅ implemented + tested |
| Trustless buyback settlement (balance-delta verified) | ✅ implemented + tested |
| Diagnostics (`ProtocolInfo`) | ✅ implemented + tested |
| Automated rebalancing (`rebalance_threshold_bps`) | ⛔ param reserved, not enforced |
| Live wasmd testnet suite (incl. chain tax) | ⛔ pending (Phase 1) |
| External audit | ⛔ pending (Phase 2) |

## License

MIT — see [LICENSE](LICENSE). Not affiliated with or endorsed by Terra Classic governance (yet) — this is a community implementation offered as a starting point for the protocol work Proposal 12209 called for.
