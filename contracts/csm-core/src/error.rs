use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

use csm_std::CollateralType;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("caller is not the admin")]
    Unauthorized,

    #[error("unsupported cw20 hook")]
    UnsupportedHook,

    #[error("cw20 sender is not the registered token for {collateral}")]
    CollateralNotRegistered { collateral: CollateralType },

    #[error("cw20 sender is not the EUTC token")]
    NotEutc,

    #[error("minting is paused")]
    MintPaused,

    #[error("redemption is paused")]
    RedeemPaused,

    #[error("amount must be positive")]
    ZeroAmount,

    #[error("no price source available; mints fail closed")]
    OracleMissing,

    #[error("price for {collateral} is stale, missing, or zero")]
    OracleStale { collateral: CollateralType },

    #[error("invalid parameters: {0}")]
    InvalidParams(String),

    #[error("daily redemption cap exceeded; {remaining} EUTC remaining today")]
    DailyCapExceeded { remaining: Uint128 },

    #[error("per-address redemption cap exceeded; {remaining} EUTC remaining today for this address")]
    PerAddressCapExceeded { remaining: Uint128 },

    #[error("daily mint cap for {collateral} exceeded; {remaining} collateral units remaining today")]
    MintCapExceeded { collateral: CollateralType, remaining: Uint128 },

    #[error(
        "primary collateral ratio below governance floor: {current_bps} bps < {required_bps} bps; mints halted"
    )]
    RatioBelowFloor { current_bps: Uint128, required_bps: Uint128 },

    #[error("insufficient custodied collateral: {available} available, {requested} requested")]
    InsufficientCollateral { available: Uint128, requested: Uint128 },
}
