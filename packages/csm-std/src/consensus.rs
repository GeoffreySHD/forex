//! Terra chain consensus rates via the terra wasmbinding.
//!
//! The chain exposes its oracle ballot rates (slashing-backed validator
//! votes, updated per block) to contracts through the `terra` custom query
//! `exchange_rates`. Types mirror `core/wasmbinding/bindings/query.go`
//! exactly — field names are part of the wire contract.

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, Decimal, Uint128};

/// Legacy fiat-pegged native denoms of the Terra Classic chain (the frozen
/// pre-collapse family). USDC and EURC are bridged cw20s, not on this list —
/// consensus rates for those come from their issuer feeds, hence the
/// adapter fallback.
///
/// Source: live bank supply query + `x/tax` gas-price denom list (2026-09).
pub const LEGACY_FOREX_DENOMS: &[&str] = &[
    "uusd", "ueur", "ukrw", "ugbp", "ujpy", "ucny", "umnt", "uchf", "ucad", "uaud", "usdr",
    "udkk", "unok", "usek", "usgd", "uhkd", "umyr", "uphp", "uinr", "uthb",
];

/// Chain denom for the native LUNC token (buyback target, `uluna`).
pub const LUNC_DENOM: &str = "uluna";

/// Micro-units per unit for all legacy terra denoms.
pub const MICRO: u128 = 1_000_000;

/// Whether a denom is part of the legacy fiat family (convertible through
/// the legacy-bridge).
pub fn is_legacy_forex_denom(denom: &str) -> bool {
    LEGACY_FOREX_DENOMS.contains(&denom)
}

/// Terra custom query envelope. Mirrors `bindings.TerraQuery`.
#[cw_serde]
pub enum TerraQuery {
    /// Cross exchange rates computed by the chain from oracle ballots:
    /// `quote/base` for each requested quote denom (e.g. base `ueur`,
    /// quotes `["uusd"]` → EUR/USD).
    ExchangeRates {
        base_denom: String,
        quote_denoms: Vec<String>,
    },
}

/// Custom-query wrapper used by contracts on terra-classic chains.
#[cw_serde]
pub enum TerraQueryWrapper {
    Terra { terra: TerraQuery },
}

impl cosmwasm_std::CustomQuery for TerraQueryWrapper {}

/// Mirrors `bindings.ExchangeRateItem`.
#[cw_serde]
pub struct ExchangeRateItem {
    /// Decimal string, e.g. "1.092500000000000000".
    pub exchange_rate: String,
    pub quote_denom: String,
}

/// Mirrors `bindings.ExchangeRatesQueryResponse`.
#[cw_serde]
pub struct ExchangeRatesQueryResponse {
    pub exchange_rates: Vec<ExchangeRateItem>,
    pub base_denom: String,
}

/// Cross-rate between two legacy denoms from a consensus response.
/// Returns None when the requested quote is absent (failed ballot for that
/// denom) — callers treat that as a stale/missing price and fall back.
/// The rate is returned in 18-decimal `Decimal` atomics (precision-preserving).
pub fn cross_rate(resp: &ExchangeRatesQueryResponse, quote_denom: &str) -> Option<Uint128> {
    resp.exchange_rates
        .iter()
        .find(|item| item.quote_denom == quote_denom)
        .and_then(|item| item.exchange_rate.parse::<Decimal>().ok())
        .map(|d| d.atomics())
}

/// Helper for contracts: build the custom query into a `QueryRequest`.
pub fn consensus_cross_rate_query(
    base_denom: &str,
    quote_denom: &str,
) -> cosmwasm_std::QueryRequest<TerraQueryWrapper> {
    cosmwasm_std::QueryRequest::Custom(TerraQueryWrapper::Terra {
        terra: TerraQuery::ExchangeRates {
            base_denom: base_denom.to_string(),
            quote_denoms: vec![quote_denom.to_string()],
        },
    })
}

/// Convenience: native Coin for a legacy denom amount (µ-units).
pub fn legacy_coin(denom: &str, amount_micro: Uint128) -> Coin {
    Coin {
        denom: denom.to_string(),
        amount: amount_micro,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cross_rate() {
        let resp = ExchangeRatesQueryResponse {
            base_denom: "ueur".to_string(),
            exchange_rates: vec![ExchangeRateItem {
                quote_denom: "uusd".to_string(),
                exchange_rate: "1.0925".to_string(),
            }],
        };
        let atomics = cross_rate(&resp, "uusd").expect("rate present");
        let d = Uint128::new(atomics.u128());
        // 18-decimal atomics for 1.0925
        assert_eq!(d.to_string(), "1092500000000000000");
    }

    #[test]
    fn missing_quote_is_none() {
        let resp = ExchangeRatesQueryResponse {
            base_denom: "ueur".to_string(),
            exchange_rates: vec![],
        };
        assert!(cross_rate(&resp, "uusd").is_none());
    }

    #[test]
    fn legacy_denom_registry() {
        assert!(is_legacy_forex_denom("ueur"));
        assert!(!is_legacy_forex_denom("uluna"));
        assert!(!is_legacy_forex_denom("ibc/ABC"));
    }
}
