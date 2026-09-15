use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use csm_std::consensus::is_legacy_forex_denom;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Config {
    pub admin: Addr,
    pub sink_address: Addr,
    pub vesting_days: u64,
    pub open: bool,
    /// Global per-denom conversion cap in micro-units (anti-dump L5).
    pub global_cap: Uint128,
    /// Per-address per-denom conversion cap in micro-units (anti-dump L5).
    pub per_address_cap: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");

/// Total converted per denom (micro-units), all addresses, program lifetime.
pub const CONVERTED_PER_DENOM: Map<&'static str, Uint128> = Map::new("converted_denom");

/// Converted per (denom, address) in micro-units, program lifetime.
pub const CONVERTED_PER_ADDR: Map<(&'static str, &'static str), Uint128> =
    Map::new("converted_addr");

/// Claims vest linearly from `start_block + vesting_days * blocks_per_day`
/// until fully vested. A claim with zero remaining balance is removed.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Claim {
    pub denom: String,
    pub total: Uint128,
    pub claimed: Uint128,
    pub start_block: u64,
    pub end_block: u64,
}

pub const CLAIMS: Map<(&'static str, &'static str), Claim> = Map::new("claims");

pub fn ensure_convertible(denom: &str) -> bool {
    is_legacy_forex_denom(denom)
}
