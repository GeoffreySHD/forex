//! Oracle adapter interface: fiat price inputs for the CSM.

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal};

use crate::CollateralType;

/// A posted fiat price: units of the quote currency per 1 EUTC.
/// For EURC this is always 1 (both are euro-denominated); for USDC it is
/// the EUR/USD market rate (USD per EUR).
#[cw_serde]
pub struct OraclePrice {
    pub quote: CollateralType,
    pub units_per_eutc: Decimal,
    pub updated_at: u64,
}

#[cw_serde]
pub enum OracleExecuteMsg {
    /// Feeder posts a fresh price observation.
    PostPrice {
        quote: CollateralType,
        units_per_eutc: Decimal,
    },
    /// Admin: authorize or revoke a feeder address.
    SetFeeder { feeder: Addr, authorized: bool },
}

#[cw_serde]
#[derive(cosmwasm_schema::QueryResponses)]
pub enum OracleQueryMsg {
    /// Current price plus freshness verdict under the configured staleness rule.
    #[returns(OraclePrice)]
    Price { quote: CollateralType },
    /// Aggregated oracle status for public diagnostics.
    #[returns(OracleStatusResponse)]
    Status {},
}

#[cw_serde]
pub struct OracleStatusResponse {
    pub prices: Vec<OraclePrice>,
    pub healthy: bool,
    /// True when any quote is within the stale window.
    pub all_fresh: bool,
}

/// Freshness helper shared by core and the adapter: a price is usable when
/// `now - updated_at <= stale_secs` (env.block.time.seconds()).
pub fn is_fresh(updated_at: u64, now: u64, stale_secs: u64) -> bool {
    now.saturating_sub(updated_at) <= stale_secs
}
