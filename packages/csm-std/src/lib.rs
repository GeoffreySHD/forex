//! Shared types for the Collateralized Stablecoin Module (CSM).
//!
//! Spec source: Terra Classic Forex Protocol (Proposal 12209) developer
//! reference. EUTC is the first stable asset; EURC and USDC are the initial
//! collateral types.

pub mod consensus;
pub mod oracle;
pub mod reserve;
pub mod vault;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;

/// Collateral types accepted for EUTC minting (spec: EURC primary at 1:1,
/// USDC with a 0.5% premium).
#[cw_serde]
#[derive(Copy, Eq, PartialOrd, Ord)]
pub enum CollateralType {
    Eurc,
    Usdc,
}

impl CollateralType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CollateralType::Eurc => "eurc",
            CollateralType::Usdc => "usdc",
        }
    }

    /// Storage-key inverse of `as_str()` (cw-storage-plus maps use string keys).
    pub fn from_str_key(s: &str) -> Option<CollateralType> {
        match s {
            "eurc" => Some(CollateralType::Eurc),
            "usdc" => Some(CollateralType::Usdc),
            _ => None,
        }
    }
}

impl std::fmt::Display for CollateralType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Kill-switch granularity. Mint and redemption pause independently so a
/// collateral or oracle problem can halt issuance without trapping holders.
#[cw_serde]
#[derive(Copy, Eq, PartialOrd, Ord)]
pub struct ProtocolStatus {
    pub mint_paused: bool,
    pub redeem_paused: bool,
}

impl ProtocolStatus {
    pub const ACTIVE: ProtocolStatus = ProtocolStatus {
        mint_paused: false,
        redeem_paused: false,
    };
}

/// Price-source selection. `ConsensusPrimary` consumes the chain oracle's
/// slashing-backed ballot rates via the terra wasmbinding (`ExchangeRates`
/// custom query) and falls back to the adapter contract on failure;
/// `AdapterOnly` uses only the feeder adapter (used by tests and for quotes
/// outside the ballot denom set).
#[cw_serde]
#[derive(Copy, Eq, PartialOrd, Ord)]
pub enum OracleMode {
    ConsensusPrimary,
    AdapterOnly,
}

/// Protocol parameters. Fee/cap values are in basis points (1.5% = 150).
#[cw_serde]
pub struct Params {
    /// Fee taken in the collateral used, on mint.
    pub mint_fee_bps: u64,
    /// Fee taken in the collateral withdrawn, on redemption.
    pub redeem_fee_bps: u64,
    /// USDC mint premium protecting against EUR/USD moves and DEX spread.
    pub usdc_premium_bps: u64,
    /// Maximum share of total EUTC supply redeemable per day (10% = 1000).
    pub daily_redeem_cap_bps: u64,
    /// USDC/EURC value differential triggering reserve rebalancing (5% = 500).
    /// RESERVED, not yet enforced (see ARCHITECTURE §5).
    #[deprecated]
    pub rebalance_threshold_bps: u64,
    /// Minimum seconds between oracle price updates (spec: 30s refresh).
    pub oracle_min_interval_secs: u64,
    /// Price older than this is treated as stale; mints fail closed.
    pub oracle_stale_secs: u64,
    /// Per-address share of supply redeemable per day (1% = 100). Bounds one
    /// whale's share of the daily window.
    pub per_addr_redeem_cap_bps: u64,
    /// Maximum new supply mintable per day, per collateral (20% = 2000).
    /// Bounds same-day mint→dump velocity.
    pub daily_mint_cap_bps: u64,
    /// Absolute floor on the daily mint allowance per collateral, in
    /// collateral micro-units. The percentage cap is relative to circulating
    /// supply, which is zero at protocol genesis — without this floor the
    /// first mints would be impossible (cap = supply × 0). Bootstrap-only in
    /// practice: the percentage cap overtakes it once supply grows.
    pub min_daily_mint_floor: u64,
    /// Dynamic redeem-spread headroom: up to this many bps are added to the
    /// redeem fee linearly as the daily window fills (0..=10_000 bps of
    /// utilization → 0..=max_dynamic_spread_bps added).
    pub max_dynamic_spread_bps: u64,
    /// Extra fee applied when redeeming EUTC minted to the sender within
    /// `flash_fee_window_blocks` (anti wash mint→dump loop).
    pub flash_fee_bps: u64,
    pub flash_fee_window_blocks: u64,
    /// Mints revert while aggregate primary collateral ratio is below this
    /// (95% = 9_500). The solvency governor (anti-dump L3). The floor must
    /// sit BELOW the steady-state ratio: the 1.5% mint fee skims the EURC
    /// ratio to ~98.5% by design (the 0.5% USDC premium lifts that leg to
    /// ~104%), so a 100% floor would halt all normal operation. 95% only
    /// trips on abnormal drain — extraction beyond fees approaching 4% of
    /// backing.
    pub min_primary_ratio_bps: u64,
}

#[allow(deprecated)]
impl Default for Params {
    fn default() -> Self {
        Params {
            mint_fee_bps: 150,
            redeem_fee_bps: 150,
            usdc_premium_bps: 50,
            daily_redeem_cap_bps: 1000,
            rebalance_threshold_bps: 500,
            oracle_min_interval_secs: 30,
            oracle_stale_secs: 300,
            per_addr_redeem_cap_bps: 100,
            daily_mint_cap_bps: 2000,
            min_daily_mint_floor: 100_000_000,
            max_dynamic_spread_bps: 1000,
            flash_fee_bps: 300,
            flash_fee_window_blocks: 30,
            min_primary_ratio_bps: 9_500,
        }
    }
}

impl Params {
    pub fn validate(&self) -> Result<(), String> {
        if self.mint_fee_bps > 5_000 || self.redeem_fee_bps > 5_000 {
            return Err("fees above 50% are rejected".to_string());
        }
        if self.usdc_premium_bps > 5_000 {
            return Err("USDC premium above 50% is rejected".to_string());
        }
        if self.daily_redeem_cap_bps == 0 || self.daily_redeem_cap_bps > 10_000 {
            return Err("daily redeem cap must be between 0.01% and 100%".to_string());
        }
        if self.oracle_min_interval_secs == 0 || self.oracle_stale_secs < self.oracle_min_interval_secs {
            return Err("stale threshold must be >= min feed interval".to_string());
        }
        if self.per_addr_redeem_cap_bps == 0 || self.per_addr_redeem_cap_bps > self.daily_redeem_cap_bps {
            return Err("per-address redeem cap must be in (0, daily_redeem_cap]".to_string());
        }
        if self.daily_mint_cap_bps == 0 || self.daily_mint_cap_bps > 10_000 {
            return Err("daily mint cap must be between 0.01% and 100%".to_string());
        }
        if self.min_daily_mint_floor == 0 {
            return Err("min daily mint floor must be positive".to_string());
        }
        if self.max_dynamic_spread_bps > 5_000 {
            return Err("dynamic spread headroom above 50% is rejected".to_string());
        }
        if self.flash_fee_bps > 5_000 || self.flash_fee_window_blocks > 500 {
            return Err("flash fee above 50% or window above 500 blocks is rejected".to_string());
        }
        if self.min_primary_ratio_bps > 30_000 {
            return Err("min primary ratio above 300% is rejected".to_string());
        }
        Ok(())
    }
}

/// Per-collateral accounting layer. The spec requires these to stay
/// independently verifiable, never collapsed into one balance.
#[cw_serde]
#[derive(Default)]
pub struct CollateralState {
    /// EUTC minted against this collateral type, gross of premiums.
    pub eutc_minted: Uint128,
    /// Collateral currently custodied by the module (primary collateral).
    pub primary_balance: Uint128,
    /// Fees routed to the secondary reserve from this collateral type.
    pub fees_collected: Uint128,
    /// Lifetime collateral returned to redeemers.
    pub collateral_returned: Uint128,
}

/// Public diagnostics answer. Maps 1:1 to the spec's required diagnostics
/// list (total supply comes from the bank module).
#[cw_serde]
pub struct ProtocolInfo {
    pub status: ProtocolStatus,
    pub params: Params,
    pub eutc_denom: String,
    pub eutc_total_supply: Uint128,
    pub eutc_minted_total: Uint128,
    pub collateral: Vec<(CollateralType, CollateralState)>,
    /// primary_balance / eutc_minted, in percent (x100), 0 when nothing minted.
    pub collateral_ratio_pct: Uint128,
    pub daily_redeem_remaining: Uint128,
    pub redeem_window_day: u64,
    pub secondary_reserve: Vec<(CollateralType, Uint128)>,
    pub reserve_lunc_vault: Uint128,
    pub oracle_healthy: bool,
}

/// Messages csm-core accepts. Mint/Redeem arrive as cw20 `Send` hooks from a
/// registered collateral token (see `csm-core::msg::HookMsg`); admin messages
/// are direct and gated on the module admin (governance or multisig).
#[cw_serde]
#[derive(cosmwasm_schema::QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(StateResponse)]
    State {},
    #[returns(PriceResponse)]
    Price { quote: CollateralType },
    #[returns(ProtocolInfo)]
    ProtocolInfo {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub eutc_token: String,
    pub vault: String,
    pub reserve: String,
    pub oracle: Option<String>,
    pub oracle_mode: OracleMode,
    pub collateral_tokens: Vec<(CollateralType, String)>,
}

#[cw_serde]
pub struct StateResponse {
    pub status: ProtocolStatus,
    pub params: Params,
    pub collateral: Vec<(CollateralType, CollateralState)>,
    pub redeem_window_day: u64,
    pub redeemed_today: Uint128,
}

#[cw_serde]
pub struct PriceResponse {
    pub quote: CollateralType,
    /// Collateral units per 1 EUTC (1.0 for EURC; EUR/USD for USDC).
    /// Mint: EUTC = deposit / rate x (1 - premium); Redeem: payout = EUTC x rate x (1 - fee).
    pub rate: String,
    pub fresh: bool,
}
