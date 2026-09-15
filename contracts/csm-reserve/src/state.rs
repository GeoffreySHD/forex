use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use csm_std::reserve::BuybackStatus;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Config {
    pub admin: Addr,
    pub core: Addr,
    pub vault: Addr,
    pub router: Option<Addr>,
    pub lunc_denom: String,
}

pub const CONFIG: Item<Config> = Item::new("config");
/// Accepted stablecoin deposit tokens, keyed by `CollateralType::as_str()`.
pub const TOKENS: Map<String, Addr> = Map::new("tokens");
/// Secondary reserve balances per collateral type.
pub const BALANCES: Map<String, Uint128> = Map::new("balances");
/// Current buyback state machine position.
pub const BUYBACK_STATUS: Item<BuybackStatus> = Item::new("buyback_status");
/// Lifetime counters for public diagnostics.
pub const BUYBACKS_EXECUTED: Item<u64> = Item::new("buybacks_executed");
pub const LUNC_PURCHASED_TOTAL: Item<Uint128> = Item::new("lunc_purchased_total");
pub const LAST_ROUTE: Item<String> = Item::new("last_route");
/// (collateral key, amount) routed to the router for the buyback in flight.
pub const LAST_OFFER: Item<(String, Uint128)> = Item::new("last_offer");
