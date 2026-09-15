# Regulatory Notes — terminology policy & jurisdictional landscape

**Not legal advice.** Written by engineers for a community project; before any mainnet deployment or public marketing, obtain qualified legal counsel in the relevant jurisdictions. This is the project's working policy to keep our materials defensible.

## 1. The landscape (as of 2026-09; verify with counsel)

- **EU — MiCA** (in force): "asset-referenced tokens" (ARTs) — crypto-assets that *purport to maintain a stable value* by referencing fiat — are **regulated instruments**: white-paper requirements, authorized issuers, reserve and redemption rules. Calling our product a "stablecoin"/"pegged euro" in public materials is how a token gets characterized as an ART offered to EU persons.
- **US — GENIUS Act** (signed 2025): federal regime for "payment stablecoins" — permitted issuers (banks/qualified nonbanks), 1:1 high-quality liquid reserves, redemption rights. A community-deployed token marketing itself as a dollar/euro stablecoin invites exactly this characterization; securities exposure (Howey — expectation of profit from others' efforts) is the secondary risk, mainly via "repeg profit" narratives.
- **US — CLARITY Act** (progressing through Congress): attempts to divide Commodity (decentralized) vs Securities (control/effort) jurisdiction — improves predictability but does not bless unlicensed stablecoin issuance.

**Consequence:** the words we choose in repos, docs, proposals, and marketing materially affect how regulators characterize the thing. We are not a licensed e-money/ART issuer and must not sound like one — nor mislead users about what the system is.

## 2. Banned wording and approved replacements (project policy)

| Do not use | Use instead | Why |
|---|---|---|
| **repeg** | *restitution of legacy-unit holders*, *legacy-unit conversion*, *collateral-backed relaunch* | "Repeg" promises to restore a price on a legacy instrument — reads as an investment promise on a dead asset (the exact liability trap), and attaches the old token's debt narrative to us. The proposal title contains it; our materials must not. |
| **stablecoin** | *collateralized unit*, *collateral-backed unit of account*, *EUTC-C / "collateral unit" (working name)* | The ART/payment-stablecoin trigger word. Describe mechanics, not the regulated category. |
| **peg / pegged (to the euro)** | *maintained at a 1:1 reference against deposited collateral at oracle reference rates* | "Peg" asserts a value promise to the market. State the mechanism: units are minted **against** deposited collateral and redeemable **from** it. |
| **fully backed / guaranteed** | *collateral is held 1:1 on-chain and publicly verifiable* | "Guarantee" implies an issuer standing behind value. We have no issuer; we have custody + verifiability. |
| **redeemable for euros / fiat redemption** | *withdrawal of the deposited collateral asset (EURC/USDC)* | We never promise fiat. Users get back the collateral asset they (or prior minters) deposited. |
| **e-money / digital euro / currency** | *collateralized crypto-asset unit* | e-money characterization (EMD2/MiCA EMT) is worse than ART. |
| **yield / returns (any promise)** | never | Any profit expectation strengthens a securities characterization; buyback mechanics are described as protocol operations, never as "earnings for holders". |

Permitted and encouraged: **collateralized**, **overcollateralized (at issuance)**, **verifiable reserves**, **conversion**, **withdrawal**, **oracle reference rate**, **user-deposited collateral**. Describe what the contracts *do*; avoid promising what the system *is*.

## 3. Rename: the "Repeg" framing is out of our docs

- Proposal 12209's title contains "EUTC Repeg" — we cite it accurately **as a citation** (it is the governance record), but our own materials (README, SPEC, USER-STORIES, proposals, marketing) must use the new framing:
  - **EUTC-C** ("Euro Collateral Unit", working name): a *new, cleanly-issued* collateral-backed unit. Nothing "restored", nothing "re-pegged".
  - The legacy `ueur` balance family: **"legacy units"** — eligible for a voluntary, capped, opt-in **conversion program** (§10b bridge), described as restitution mechanics, never as a promise of future value.
- The repeg-bridge spec must carry a prominent "no promise of value" statement: conversion eligibility ≠ guarantee that legacy units will ever be worth their face reference amount; the program exists to let holders *exit into* collateral-backed units if and when collateral exists.

## 4. Operational hygiene (compliance-adjacent engineering)

- **No fiat on/off-ramps in our contracts.** Users obtain EURC/USDC from regulated venues on their own; we touch crypto-assets only. This keeps us away from money-transmission licensing narratives.
- **Geo-fencing is a frontend concern**: the website/app layer should restrict EU/US marketing language and consider jurisdiction warnings; the contracts themselves are permissionless (document this honestly — permissionless contracts do not make the *marketing* exempt).
- **Reserves transparency**: publish on-chain verifiability (we already do via `ProtocolInfo`) — verifiability without "guarantee" language.
- **Trademark/branding**: avoid "Terra", "UST", "Terraform" in the product name; reference the chain factually (a Terra Classic community project) without implying endorsement.
- **"FOREX" in docs is fine (2026-09-13 decision)**: the word is a generic term for the FX market, and generic terms cannot be exclusively appropriated; existing registrations (e.g. FOREX.com/StoneX) protect composite marks in retail FX services, not the bare dictionary word. Engineering/repo/docs usage stays. A distinct public *product* name may still be adopted later; that choice is branding, not legal necessity.
- **Every public proposal gets a legal-risk section** once counsel is engaged; until then, keep public texts mechanical and promise-free.

## 5. What this changes in our docs

- SPEC/USER-STORIES/ARCHITECTURE keep internal engineering terms (they describe code behavior honestly); public-facing wrappers (README intro, proposals, website copy) use the §2 table.
- The word "stablecoin" in code comments is harmless (not marketing), but new public docs should not lead with it.
- The liquidity/governance proposals (future) must be drafted against this policy from the start.
