use cosmwasm_schema::cw_serde;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    /// Where converted legacy units are delivered (e.g. the community
    /// module account, or the CSM reserve for disposition). The coins are
    /// not recoverable from the bridge afterwards — delivery is a plain
    /// bank send to this address.
    pub sink_address: String,
    /// Per-claim linear vesting period in days (default 90).
    pub vesting_days: u64,
    /// Program switch: conversions are rejected while closed.
    pub open: bool,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Admin switches the program open/closed (e.g. pending liquidity).
    SetOpen { open: bool },
    /// Admin updates caps (global per denom, per address).
    SetCaps { global: u128, per_address: u128 },
    /// Admin transfers program administration.
    SetAdmin { admin: String },
    /// Convert attached native legacy coins into vesting claims. One denom
    /// per call; the attached funds must contain exactly one denom and the
    /// full amount converts.
    Convert {},
    /// Acknowledge vested progress on a recorded claim. The claim is a
    /// bookkeeping credit toward future collateralized-unit issuance; coin
    /// movement happens only in the CSM issuance step, never here.
    Claim { denom: String },
}

#[cw_serde]
pub enum MigrateMsg {}

#[cw_serde]
#[derive(cosmwasm_schema::QueryResponses)]
pub enum QueryMsg {
    /// Program configuration.
    #[returns(ConfigResponse)]
    Config {},
    /// A holder's claim (if any), vested amount, and program state.
    #[returns(ClaimResponse)]
    Claim { address: String, denom: String },
    /// Lifetime converted totals for a denom.
    #[returns(TotalsResponse)]
    ConvertedTotals { denom: String },
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub sink_address: String,
    pub vesting_days: u64,
    pub open: bool,
    pub global_cap: String,
    pub per_address_cap: String,
}

#[cw_serde]
pub struct ClaimResponse {
    pub claim: Option<ClaimInfo>,
    pub vested: String,
    pub program_open: bool,
}

#[cw_serde]
pub struct ClaimInfo {
    pub denom: String,
    pub total: String,
    pub claimed: String,
    pub start_block: u64,
    pub end_block: u64,
}

#[cw_serde]
pub struct TotalsResponse {
    pub denom: String,
    pub converted: String,
}
