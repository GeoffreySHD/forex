use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use csm_std::{CollateralState, OracleMode, Params, ProtocolStatus};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Config {
    pub admin: Addr,
    pub eutc_token: Addr,
    pub vault: Addr,
    pub reserve: Addr,
    pub oracle: Option<Addr>,
    /// Price-source selection (see `OracleMode`); tests typically use
    /// `AdapterOnly`, mainnet defaults to `ConsensusPrimary`.
    pub oracle_mode: OracleMode,
}

pub const CONFIG: Item<Config> = Item::new("config");

/// Total supply at the start of the current UTC day, lazily refreshed on
/// the first state-changing operation of the day. Daily caps reference this
/// snapshot: burning supply during the day must not shrink the day's own
/// windows (a cap against live supply would make the window self-consuming
/// — every redemption reduces the allowance for all subsequent ones).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct DaySnapshot {
    pub day: u64,
    pub supply: Uint128,
}

pub const DAY_SUPPLY: Item<DaySnapshot> = Item::new("day_supply");
pub const PARAMS: Item<Params> = Item::new("params");
pub const STATUS: Item<ProtocolStatus> = Item::new("status");
/// Registered cw20 per collateral type, keyed by `CollateralType::as_str()`.
pub const COLLATERAL_TOKENS: Map<String, Addr> = Map::new("collateral_tokens");
/// Independent per-collateral accounting (spec: never collapse these).
pub const COLLATERAL_STATE: Map<String, CollateralState> = Map::new("collateral_state");
/// (day number, EUTC redeemed in that window) for the daily redemption cap.
pub const REDEEM_WINDOW: Item<(u64, Uint128)> = Item::new("redeem_window");
/// Per-address share of the daily redemption window (anti-whale).
pub const REDEEMED_TODAY_BY_ADDR: Map<(String, Addr), Uint128> =
    Map::new("redeemed_today_by_addr");
/// (day number, EUTC minted in that window) per collateral — mint-velocity cap.
pub const MINT_WINDOWS: Map<String, (u64, Uint128)> = Map::new("mint_windows");
/// Last mint to an address: (height, amount) — drives the flash fee.
pub const LAST_MINT_HEIGHT: Map<Addr, (u64, Uint128)> = Map::new("last_mint_height");
