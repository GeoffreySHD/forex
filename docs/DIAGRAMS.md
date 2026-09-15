# EUTC / CSM — UML & Flow Diagrams (Mermaid)

All diagrams are [Mermaid](https://mermaid.js.org/) and render on GitHub. Companion docs: [SPEC.md](./SPEC.md) · [ARCHITECTURE.md](./ARCHITECTURE.md).

---

## 1. Component map (deployment view)

```mermaid
flowchart TB
    subgraph USERS[Off-chain actors]
        U[User / Holder]
        G[Governance / Admin multisig]
        F[Feeder]
    end

    subgraph CHAIN[Terra Classic chain - wasmd v0.61.8]
        subgraph CW[CW20 tokens]
            EURC[EURC token]
            USDC[USDC token]
            EUTC[EUTC token - cw20-base]
            LUNC[LUNC native denom]
        end

        subgraph CSM[Collateralized Stablecoin Module]
            CORE[csm-core<br/>mint / redeem / fees / caps / pause]
            VAULT[csm-vault<br/>collateral custody]
            RES[csm-reserve<br/>fee sink / buybacks / LUNC vault]
            ORA[oracle-adapter<br/>fiat prices]
        end

        ROUTER[DEX router<br/>swap execution]
    end

    U -- Send(collateral, Mint) --> CORE
    U -- Send(EUTC, Redeem) --> CORE
    F -- PostPrice --> ORA
    G -- admin msgs --> CSM

    CORE -- TransferFrom user->vault --> EURC
    CORE -- TransferFrom user->vault --> USDC
    CORE -- Mint / Burn --> EUTC
    CORE -- query Price/Status --> ORA
    CORE -- Withdraw(payout, fee) --> VAULT
    VAULT -- Send fee, Deposit --> RES

    RES -- Swap(offer, slippage) --> ROUTER
    ROUTER -- BuybackResult(success, purchased) --> RES
    RES -- buys & holds (non-spendable) --> LUNC
    ROUTER -. pools .-> EURC
    ROUTER -. pools .-> LUNC
```

## 2. Class diagram (contracts, state, key operations)

```mermaid
classDiagram
    direction LR

    class CsmStd {
        <<package csm-std>>
        +CollateralType Eurc|Usdc
        +Params params
        +CollateralState per-collateral ledgers
        +ProtocolStatus mint_paused|redeem_paused
        +Fraction rate
    }

    class CsmCore {
        <<contract csm-core>>
        +Config admin, eutc, vault, reserve, oracle
        +Map collateral_state
        +Item params, status
        +redeemed_today, window_day
        +mint(deposit)
        +redeem(eutc_amount, collateral_type)
        +pause(mint, redeem)
        +set_params(params)
        +register_collateral(type, token)
        +set_eutc_oracle_vault_reserve(...)
        +query_protocol_info() ProtocolInfo
    }

    class CsmVault {
        <<contract csm-vault>>
        +Map balances[token]
        +receive(cw20 deposit)
        +withdraw(core-only, amount, fee_split)
        +query_balances()
    }

    class OracleAdapter {
        <<contract oracle-adapter>>
        +Map prices[quote]
        +Set feeders
        +post_price(quote, units_per_eutc)
        +register_remove_feeder(addr)
        +query_price(quote) fresh
        +query_status()
    }

    class CsmReserve {
        <<contract csm-reserve>>
        +Map reserve_collateral[offer]
        +lunc_vault Uint128
        +BuybackState state
        +deposit(core/vault-only)
        +register_token(type, token)
        +buyback(offer, max_slippage_bps)
        +buyback_result(success, purchased, reason)  router-only
        +set_router_or_core_or_vault(...)
        +query_balances() / query_buyback_status() / query_vault()
    }

    class Router {
        <<contract mock-router / real DEX>>
        +swap(offer, amount, min_out, to)
    }

    class EutcToken {
        <<cw20-base>>
        +minter = csm-core
        +mint / burn / send / transfer
    }

    CsmCore --> CsmStd : uses
    CsmReserve --> CsmStd : uses
    OracleAdapter --> CsmStd : uses
    CsmCore --> EutcToken : mint / burn
    CsmCore --> CsmVault : withdraw (core-only)
    CsmCore --> OracleAdapter : rate query (fail-closed)
    CsmVault --> CsmReserve : fee forwarding (Deposit hook)
    CsmReserve --> Router : Swap
    Router --> CsmReserve : BuybackResult callback
```

## 3. Sequence — Mint (EURC example)

```mermaid
sequenceDiagram
    autonumber
    actor U as User
    participant T as EURC (cw20)
    participant C as csm-core
    participant O as oracle-adapter
    participant V as csm-vault
    participant R as csm-reserve
    participant E as EUTC (cw20)

    U->>T: Send(core, 1_000_000, msg=Mint)
    T->>C: Receive(sender=User, amount, Mint)
    C->>O: Price { quote: eurc }
    O-->>C: OraclePrice { units_per_eutc: 1.0, fresh: true }
    Note over C: eutc_gross = 1_000_000<br/>fee = 1.5% = 15_000<br/>net = 985_000
    C->>T: TransferFrom(User -> Vault, 985_000)
    T->>V: Receive(sender=Core, 985_000)  [mirror: eurc += 985_000]
    C->>V: instruction: forward fee
    V->>R: Send(15_000, msg=Deposit)
    R-->>C: ok  [reserve_collateral.eurc += 15_000]
    C->>E: Mint(User, 1_000_000)
    E-->>C: ok
    C-->>T: ok (hook success) → funds move settle
```

## 4. Sequence — Redeem (cap check → burn → payout + fee)

```mermaid
sequenceDiagram
    autonumber
    actor U as User
    participant E as EUTC (cw20)
    participant C as csm-core
    participant O as oracle-adapter
    participant V as csm-vault
    participant R as csm-reserve

    U->>E: Send(core, 500_000, msg=Redeem{collateral: eurc})
    E->>C: Receive(sender=User, amount, Redeem)
    C->>O: Price { quote: eurc }
    O-->>C: fresh: true
    Note over C: cap check:<br/>redeemed_today + 500_000 <= supply × 10%<br/>else revert DailyRedeemCapExceeded
    C->>E: Burn(User, 500_000)
    Note over C: collateral_units = 500_000 × rate<br/>payout = units × 98.5%<br/>fee = units × 1.5%
    C->>V: Withdraw(payout -> User, fee -> Reserve)
    V->>R: Send(fee, msg=Deposit)
    C-->>E: ok → User receives payout EURC
    Note over C: ledgers: primary_balance −= units<br/>collateral_returned += payout<br/>redeemed_today += amount
```

## 5. Sequence — LUNC buyback (escrow → router → settle/refund)

```mermaid
sequenceDiagram
    autonumber
    actor G as Governance/Admin
    participant R as csm-reserve
    participant RT as DEX router

    G->>R: Buyback { offer: eurc, max_slippage_bps }
    Note over R: amount from accumulated fees<br/>(escrowed out of reserve_collateral)
    R->>RT: Swap { offer tokens, amount, min_out, to: reserve }
    alt swap succeeds
        RT-->>R: (execution completes)
        RT->>R: BuybackResult { success: true, purchased: LUNC_amount }
        Note over R: lunc_vault += purchased (non-spendable)<br/>swaps_executed++<br/>lunc_purchased_total += purchased<br/>LAST_OFFER settled
    else swap fails
        RT->>R: BuybackResult { success: false, reason }
        Note over R: escrow refunded to reserve_collateral<br/>failure reason recorded<br/>retry allowed later
    end
```

## 6. State machine — protocol gates (orthogonal)

Mint and redeem pause **independently** (`Pause { mint, redeem }`); the four combinations are legal states.

```mermaid
stateDiagram-v2
    direction LR
    state "mint_paused / redeem_paused" as gates {
        [*] --> M0
        M0: mint ✚ redeem ✚ (ACTIVE)
        M0 --> M1: Pause(mint=true)
        M1: mint ✖ redeem ✚
        M0 --> M2: Pause(redeem=true)
        M2: mint ✚ redeem ✖
        M1 --> M3: Pause(redeem=true)
        M2 --> M3: Pause(mint=true)
        M3: mint ✖ redeem ✖ (full halt)
        M1 --> M0: Pause(mint=false)
        M2 --> M0: Pause(redeem=false)
        M3 --> M1: Pause(redeem=false)
        M3 --> M2: Pause(mint=false)
    }
```

## 7. State machine — buyback lifecycle

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Escrowed: Buyback(offer, slippage) [admin, fees available]
    Escrowed --> AwaitingResult: Swap sent to router
    AwaitingResult --> Settled: BuybackResult(success=true)
    Settled --> Idle: vault credited, ledger updated
    AwaitingResult --> Refunded: BuybackResult(success=false, reason)
    Refunded --> Idle: escrow returned, retry allowed
    note right of AwaitingResult: only the configured router<br/>may deliver BuybackResult
```

## 8. Data model (accounting layer per collateral)

```mermaid
erDiagram
    PROTOCOL ||--o{ COLLATERAL_STATE : "per type"
    PROTOCOL {
        ProtocolStatus status
        Params params
        Uint128 eutc_total_supply
        Uint128 redeemed_today
        uint64 redeem_window_day
        bool oracle_healthy
    }
    COLLATERAL_STATE {
        CollateralType type "eurc | usdc"
        Uint128 eutc_minted "gross minted vs this collateral"
        Uint128 primary_balance "custodied in vault"
        Uint128 fees_collected "sent to reserve"
        Uint128 collateral_returned "lifetime redemptions"
    }
    RESERVE ||--o{ RESERVE_BALANCE : "per offer token"
    RESERVE {
        Uint128 lunc_vault "non-spendable"
        BuybackState state
        uint64 swaps_executed
        Uint128 lunc_purchased_total
    }
    RESERVE_BALANCE {
        CollateralType token
        Uint128 amount "fee sink"
    }
```

> Reading guide: solvency (SPEC I1) is `Σ primary_balance ≥ Σ eutc_minted` at last-used rates; fee conservation (I3) is `Σ fees_collected == Σ reserve_balance` per token.
