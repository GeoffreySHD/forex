//! Message types live in `csm-std` (`csm_std::vault`) so callers can use
//! them without linking this contract crate; re-exported here for
//! backwards compatibility.

pub use csm_std::vault::{
    VaultBalanceResponse as BalanceResponse, VaultConfigResponse as VaultConfigResponse,
    VaultExecuteMsg as ExecuteMsg, VaultHook, VaultInstantiateMsg as InstantiateMsg,
    VaultQueryMsg as QueryMsg,
};
