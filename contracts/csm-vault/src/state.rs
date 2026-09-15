use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Config {
    pub admin: Addr,
    pub core: Option<Addr>,
}

pub const CONFIG: Item<Config> = Item::new("config");
/// Internal mirror of custodied cw20 balances (source of truth is the bank).
pub const BALANCES: Map<String, Uint128> = Map::new("balances");
