use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;

use csm_std::CollateralType;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    /// The core module; its fee sends are accepted as deposits.
    pub core: String,
    /// The primary collateral vault; redemption fees arrive through it.
    pub vault: String,
    /// Native LUNC denom bought by buybacks and held in the vault.
    pub lunc_denom: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    Receive(cw20::Cw20ReceiveMsg),
    /// Admin: accept a collateral stablecoin for fee deposits.
    RegisterToken {
        collateral_type: CollateralType,
        token: String,
    },
    /// Admin: rebind the core module (deployment wiring).
    SetCore { core: String },
    /// Admin: rebind the primary vault (deployment wiring).
    SetVault { vault: String },
    /// Admin: execute one buyback of LUNC from the selected reserve.
    Buyback {
        offer: CollateralType,
        max_slippage_bps: u64,
    },
    /// Router callback: report the buyback outcome. Only the configured
    /// router may call this.
    BuybackResult {
        success: bool,
        purchased: Uint128,
        reason: Option<String>,
    },
    /// Admin: wire the swap router (DEX) contract.
    SetRouter { router: String },
    /// Admin: transfer admin authority (governance handover).
    SetAdmin { admin: String },
}

#[cw_serde]
#[derive(cosmwasm_schema::QueryResponses)]
pub enum QueryMsg {
    #[returns(csm_std::reserve::ReserveBalancesResponse)]
    Balances {},
    #[returns(csm_std::reserve::BuybackStatus)]
    BuybackStatus {},
    #[returns(csm_std::reserve::VaultResponse)]
    Vault {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub core: String,
    pub router: Option<String>,
    pub lunc_denom: String,
}
