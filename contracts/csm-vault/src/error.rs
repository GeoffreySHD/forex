use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("caller is not the admin")]
    Unauthorized,

    #[error("withdrawals are restricted to csm-core")]
    NotCore,

    #[error("unsupported cw20 hook")]
    UnsupportedHook,
}
