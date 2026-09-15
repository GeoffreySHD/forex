use cosmwasm_std::{Decimal, Uint128};
use cw_storage_plus::Item;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Config {
    /// LUNC (micro) bought per offered stablecoin (micro).
    pub rate: Decimal,
    pub fail_swaps: bool,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const SWAPS_EXECUTED: Item<u64> = Item::new("swaps_executed");
pub const OFFERED_TOTAL: Item<Uint128> = Item::new("offered_total");
