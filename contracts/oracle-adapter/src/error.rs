use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("caller is not an authorized feeder")]
    UnauthorizedFeeder,

    #[error("caller is not the admin")]
    Unauthorized,

    #[error("no price has been posted for this quote yet")]
    NoPrice,

    #[error("invalid instantiation parameters")]
    InvalidConfig,
}
