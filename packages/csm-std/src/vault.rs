//! Vault interface types, shared between csm-vault and its callers (csm-core
//! routes deposits and withdrawals through these messages). Kept in csm-std
//! so contract crates can depend on the message types without linking a
//! contract crate (linking two `cdylib` contract crates together duplicates
//! their `extern "C"` entry-point symbols at wasm link time).

use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;

#[cw_serde]
pub struct VaultInstantiateMsg {
    pub admin: String,
}

#[cw_serde]
pub enum VaultExecuteMsg {
    Receive(Cw20ReceiveMsg),
    /// csm-core only: release collateral to a redeemer. When `hook` is set,
    /// the token is delivered via cw20 Send so the receiver can run its own
    /// receive path (used to route redemption fees into the reserve).
    Withdraw {
        to: String,
        token: String,
        amount: Uint128,
        hook: Option<cosmwasm_std::Binary>,
    },
    /// Admin: bind the core contract allowed to withdraw.
    RegisterCore { core: String },
}

/// Hook delivered via cw20 Send from csm-core when custodying collateral.
#[cw_serde]
pub enum VaultHook {
    Deposit {},
}

#[cw_serde]
#[derive(cosmwasm_schema::QueryResponses)]
pub enum VaultQueryMsg {
    #[returns(VaultConfigResponse)]
    Config {},
    #[returns(VaultBalanceResponse)]
    Balance { token: String },
}

#[cw_serde]
pub struct VaultConfigResponse {
    pub admin: String,
    pub core: Option<String>,
}

#[cw_serde]
pub struct VaultBalanceResponse {
    pub token: String,
    pub balance: Uint128,
}
