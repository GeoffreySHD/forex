//! Secondary reserve interface: fee sink, LUNC buyback state machine, and
//! the non-spendable LUNC vault.

use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;

use crate::CollateralType;

/// Buyback state machine states (public diagnostics requirement).
#[cw_serde]
pub enum BuybackStatus {
    /// Reserve holds fees but no buyback is in flight.
    Idle,
    /// A buyback message was accepted and routed to the DEX router.
    Pending,
    /// Route failed (slippage or execution); retry allowed.
    Failed { reason: String },
    /// Swapped; proceeds sit in the LUNC vault.
    Completed,
}

/// Cross-contract hook sent into the reserve via cw20 Send.
#[cw_serde]
pub enum ReserveExecuteMsg {
    /// Fee deposit from csm-core (mint fees) or csm-vault (redemption fees).
    Deposit {},
}

#[cw_serde]
pub enum ReserveQueryMsg {
    /// Per-collateral secondary reserve balances.
    Balances {},
    /// Current buyback state machine position.
    BuybackStatus {},
    /// LUNC held in the non-spendable vault (public diagnostics).
    Vault {},
}

#[cw_serde]
pub struct ReserveBalancesResponse {
    pub balances: Vec<(CollateralType, Uint128)>,
}

#[cw_serde]
pub struct BuybackStatusResponse {
    pub status: BuybackStatus,
    pub buybacks_executed: u64,
    pub lunc_purchased_total: Uint128,
    pub last_route: Option<String>,
}

#[cw_serde]
pub struct VaultResponse {
    /// LUNC in the vault, stored as a native denom balance held by the
    /// reserve contract. The vault is non-circulating by construction: the
    /// reserve exposes no spend path for it.
    pub lunc: Uint128,
}

/// DEX router surface the reserve calls into. Implementations: mock-router
/// (tests), later a GDEX/Terraswap/Terraport route selector contract.
#[cw_serde]
pub enum RouterExecuteMsg {
    /// Swap `amount` of `offer` for LUNC; fails when the executed price
    /// breaches `max_slippage_bps` against the router's quoted price.
    /// Returns purchased LUNC (native) to the caller (the reserve).
    SwapToLunc {
        offer: CollateralType,
        amount: Uint128,
        max_slippage_bps: u64,
    },
}

/// Outcome callback routers send to the reserve. JSON-shape compatible with
/// `csm-reserve::msg::ExecuteMsg::BuybackResult`, so any router can report
/// without depending on the reserve crate.
#[cw_serde]
pub enum ReserveCallbackMsg {
    BuybackResult {
        success: bool,
        purchased: Uint128,
        reason: Option<String>,
    },
}
