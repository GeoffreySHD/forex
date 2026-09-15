use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("caller is not the admin")]
    Unauthorized,

    #[error("denom {denom} is not a convertible legacy forex unit")]
    NotConvertible { denom: String },

    #[error("sent {sent} {denom} but declared amount {declared}")]
    FundsMismatch { sent: u128, declared: u128, denom: String },

    #[error("no legacy coins attached")]
    NoFunds,

    #[error("conversion program is not open")]
    ProgramClosed,

    #[error("global conversion cap of {cap} {denom} would be exceeded (used {used})")]
    GlobalCapExceeded { cap: u128, used: u128, denom: String },

    #[error("per-address conversion cap of {cap} {denom} would be exceeded (used {used})")]
    AddressCapExceeded { cap: u128, used: u128, denom: String },

    #[error("nothing has vested yet")]
    NothingToClaim,
}
