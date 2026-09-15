# Draft: DFINITY forum RFC — chain-key attestation export

> Working draft for forum.dfinity.org (developer / chain-fusion section).
> Scope locked to **non-fiat assets** (GLDT, ckBTC, ICP, third-party ICP tokens).
> Fiat-referenced twins (USDC/EURC wrappers) are deliberately out of scope:
> GENIUS Act issuance prohibition + MiCA ART make them a legal red line for a
> community project. Canonical issuer rails only.

---

**Title (proposal):** RFC: "Chain-key attestation export" — let a canister produce a signed custody/burn statement that a foreign chain can verify (use case: bringing non-fiat ICP assets to Terra Classic)

**TL;DR**
There is no trust-minimized corridor between ICP and Cosmos chains today: ck tokens can't leave the ICP ecosystem, and Cosmos assets can't get in. We'd like to change that, starting with bringing **non-fiat ICP-side assets** (GLDT gold token, ckBTC, ICP, game tokens) onto **Terra Classic** as cw20 twins. Two ways to get there; **one of them we'd rather build *with* DFINITY than around it.** This post asks for (a) feedback on whether a blessed "attestation export" primitive exists or is planned, and (b) interest in co-designing the format.

---

## Who we are

We're the team behind Terra Classic's FOREX/CSM effort (community proposal 12209 — a collateralized euro-referenced token program, fully open source: six CosmWasm contracts with a public doc set and terminology/disclosure policy), and the author of **ckLUNC** — a chain-key-styled ICRC-1 twin of LUNC on ICP, in development, which is what got us deep into your ck stack. We also maintain **lunc-skills** — the first agent skill set for any Cosmos SDK chain, in the same format as dfinity/icskills — and **lunc-mcp**, an MCP server exposing verified Terra Classic tooling; agent-facing developer experience is our home turf.

We are not here to pitch Terra Classic's size. We're here because the *pattern* we need — "prove, to a foreign chain, that a canister holds/burns X" — is generic, and ICP is the only ecosystem whose trust model makes it worth doing: **no key committee, no multisig, no bridge operator to hack.**

## The problem

- Inside ICP: ckBTC/ckETH/ckICP/ckUSDC (chain-key custody — zero exploits since 2023, Trail of Bits-assessed) and asset tokens like GLDT (gold, Loomis-Zurich custody, GOLDAO-governed).
- Outside ICP: to reach Cosmos/EVM chains, these assets currently have *no corridor* except custodial third-party bridges — which defeats the point of chain-key custody, and which the Cosmos side has recently been reminded to distrust (Axelar-to-Secret receiving-contract exploit in June 2026; Nomic nBTC double-spend in September 2026 — in both cases IBC transport held and the custom bridge code was the hole).

Terra Classic specifically: Cosmos SDK chain, CosmWasm (wasmvm v3), IBC live (re-enabled 2022-2024, channels operating today), active governance, and a community that has just funded its first collateralized-token program. What it does *not* have: any EVM track and any BLS12-381 verification in its execution path.

## Option A (preferred): a blessed "chain-key attestation export" primitive

**The ask in one sentence:** a documented, supported way for a canister to produce a signed statement — "at registry epoch R, canister C (code hash K) holds N units of asset A in escrow" or "N twin tokens were burned at height H" — that a smart contract on a *foreign* chain can verify using only ICP's public root key.

**Why we believe this is a small step, not a moonshot.** The ingredients already exist:

1. **Certified variables / canister certificates** — a canister can commit its escrow/burn state to certified data and serve (data, certificate) pairs. The BLS signature over the merkle tree is by the *subnet* key.
2. **Public root of trust** — the NNS registry publishes subnet public keys; ICP's chain-key machinery already handles **key rotation**, which is exactly the part a foreign consumer chain struggles to do right.
3. **Consumer-side crypto** — EVM chains can verify BLS12-381 via EIP-2537 (live since Prague). Cosmos chains need a BLS verifier (Go blst in a precompile/wasmbinding) — **we volunteer to build and propose that for Terra Classic**, and it is reusable by every Cosmos chain.

So Option A is less "build a feature" and more "**bless and standardize an export path that is 80% implicit today**":

- a canonical **attestation format** (what goes in the certified tree: asset id, amount, canister principal + code hash, burn/lock event, nonce/epoch);
- a **versioned root-key distribution story** (how a foreign chain learns and tracks the current subnet/root keys — registry epochs, rotation proofs);
- a **reference verifier** (Solidity via EIP-2537; Go for Cosmos SDK);
- guidance on **code-hash pinning** so the attestation is only as strong as the canister's audited code, not just its principal.

**What it unlocks (ICP-side):** every ck token and every ICP asset token becomes exportable to any chain that can verify BLS — trust-minimized twins, no new committees, no bridge operators. It also strengthens the *inbound* leg (our ckLUNC project becomes a full corridor instead of a one-way twin). This is a primitive the whole Cosmos ecosystem could consume, not just Terra Classic.

## Option B (fallback, shippable without DFINITY): threshold-Schnorr attestations, verified in CosmWasm

If the primitive is not on the roadmap, we can still build a corridor with today's ICP:

- A custodian canister holds the ICP-side asset and derives a **threshold-Schnorr ed25519 key** (tSchnorr is live, and ed25519 is *exactly* the algorithm Cosmos validators sign with).
- The canister signs attestations: "N of asset A locked/burned by user U — mintable on LUNC by claim V."
- A twins-minter cw20 on Terra Classic verifies the ed25519 signature **with CosmWasm's standard crypto API** (ed25519_verify — no hard fork, no new host functions, no governance fight), tracks the canister's public key + code hash, enforces "circulating twins <= attested custody", pausable mint, and supply ceilings per asset (e.g. GOLDAO sets the GLDT ceiling).

**Honest trade-off:** the trust anchor moves from "subnet consensus certificate" (Option A) to "this canister's code + its tSchnorr key" (Option B). Still threshold (no single signer to corrupt), still no bridge operator — but the statement is only as good as the canister's audited code, rather than being certified at consensus level. One notch weaker; entirely buildable by us.

## Side-by-side

| | Option A — chain-key attestation export | Option B — tSchnorr fallback |
|---|---|---|
| Trust anchor | ICP subnet consensus (root key + rotation) | Canister code hash + tSchnorr key |
| Foreign-side crypto | BLS12-381 (EVM: EIP-2537 yes; Cosmos: needs verifier — we build it) | ed25519 (CosmWasm std API, zero chain changes) |
| DFINITY involvement | Format co-design + blessing + root-key export docs | None |
| LUNC-side changes | BLS verifier (wasmbinding or native module) | None |
| Reusability | Any EIP-2537 chain + any Cosmos chain | Any chain with ed25519 verification |
| Status | Requested here | We can ship it |

## Concrete questions for this forum

1. **Does something like this already exist** (certified-variable export patterns, vetKD-adjacent work, or a planned "chain-key attestations for external chains" feature)? If yes — where should we look?
2. **Is there a blessed way for a foreign chain to track ICP root/subnet public keys across rotation** (registry epoch proofs, a signed key-history, or is this still research)?
3. **Would DFINITY be open to co-designing the attestation format** with an eye to ckBTC/ckETH exports first (highest-value use case), with GLDT/ICP/game tokens following?
4. Is **Terra Classic an acceptable consumer chain** for a reference implementation? We will do the Cosmos-side BLS verifier work either way and publish it for all Cosmos chains.
5. For anyone who has built similar (e.g. bridging certified data to EVM pre-2537): what gotchas are we not seeing?

## Non-goals (so nobody has to ask)

- No fiat-referenced twins (USDC/EURC wrappers) — issuance regulation makes this a hard no for us; canonical issuer rails only.
- No yield, no "stablecoin" language, no offering — mechanics and disclosures only (we maintain a formal terminology policy).
- No plan to move *ICP consensus* or ask for Terra-specific support in protocol code — Option A is a format + export path, Option B needs nothing from DFINITY at all.

---

*Draft notes (remove before posting): author = [you]. Repos to link in the post: CSM — github.com/Semence2Porc/forex (see docs/RESEARCH-CONTEXT.md, open question #11) · ckLUNC — github.com/Semence2Porc/cklunc · skills — github.com/Semence2Porc/lunc-skills · MCP — github.com/Semence2Porc/lunc-mcp. Proposal 12209 discussion: https://discourse.luncgoblins.com/t/lunc-forex-genesis-eutc-repeg/290. Post in "Developers / Chain Fusion"; cross-link to the LUNC community forum so both sides see the same thread.*
