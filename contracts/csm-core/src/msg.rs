use cosmwasm_schema::cw_serde;

use csm_std::{CollateralType, OracleMode, Params};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    /// cw20 address of EUTC; the deployer registers csm-core as minter.
    pub eutc_token: String,
    /// Primary collateral custody contract.
    pub vault: String,
    /// Secondary reserve contract receiving all fees.
    pub reserve: String,
    /// Feeder oracle adapter (fallback / override). Mints fail closed until
    /// wired when `oracle_mode` is `AdapterOnly`.
    pub oracle: Option<String>,
    /// Price-source selection; defaults to `ConsensusPrimary` (chain oracle
    /// ballot rates via the terra wasmbinding).
    pub oracle_mode: Option<OracleMode>,
    /// Defaults to the spec values when omitted.
    pub params: Option<Params>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Receive(cw20::Cw20ReceiveMsg),
    /// Admin kill switch: pause or resume mint/redemption independently.
    Pause { mint: bool, redeem: bool },
    /// Admin parameter update.
    SetParams { params: Params },
    /// Admin: register or replace the cw20 contract for a collateral type.
    RegisterCollateral {
        collateral_type: CollateralType,
        token: String,
    },
    /// Admin: rebind the EUTC cw20 token (deployment wiring; breaks the
    /// EUTC-needs-core-minter / core-needs-EUTC-address cycle).
    SetEutc { eutc: String },
    /// Admin: rewire the vault.
    SetVault { vault: String },
    /// Admin: rewire the secondary reserve.
    SetReserve { reserve: String },
    /// Admin: rewire or remove the oracle adapter.
    SetOracle { oracle: Option<String> },
    /// Admin: switch price source (consensus primary vs adapter only).
    SetOracleMode { mode: OracleMode },
    /// Admin: transfer admin authority (governance/multisig handover).
    SetAdmin { admin: String },
}

/// Hook delivered via cw20 Send into csm-core.
#[cw_serde]
pub enum HookMsg {
    /// `amount` of the named collateral backs new EUTC.
    Mint { collateral_type: CollateralType },
    /// `amount` of EUTC is burned; payout is drawn from the named pool.
    Redeem { collateral_type: CollateralType },
}
