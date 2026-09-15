use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

use csm_std::oracle::OraclePrice;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Config {
    pub admin: Addr,
    /// Price older than this many seconds is reported stale; mints fail closed.
    pub stale_secs: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const FEEDERS: Map<Addr, bool> = Map::new("feeders");
/// Latest posted price per quote currency (keyed by `CollateralType::as_str()`).
pub const PRICES: Map<String, OraclePrice> = Map::new("prices");

pub const CONTRACT_VERSION: Item<String> = Item::new("contract_version");
