use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("caller is not the admin")]
    Unauthorized,

    #[error("caller is not the core module")]
    NotCore,

    #[error("unsupported cw20 hook")]
    UnsupportedHook,

    #[error("deposits of this token are not accepted")]
    UnknownToken,

    #[error("no router is configured")]
    RouterMissing,

    #[error("a buyback is already {0}")]
    BuybackInProgress(&'static str),

    #[error("no funds available for buyback of {0}")]
    EmptyReserve(&'static str),

    #[error("slippage bound must be between 1 and 5000 bps")]
    InvalidSlippage,

    #[error("router returned less than the slippage bound allowed")]
    SlippageExceeded,
}
