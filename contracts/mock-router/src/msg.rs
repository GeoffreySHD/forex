use cosmwasm_schema::cw_serde;
use cw20::Cw20ReceiveMsg;

#[cw_serde]
pub struct InstantiateMsg {
    /// LUNC (micro) bought per offered stablecoin (micro), kept constant so
    /// tests can assert exact amounts.
    pub rate: String,
    /// When true, swaps fail with a slippage-breach result regardless of price.
    pub fail_swaps: bool,
}

#[cw_serde]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    /// Test hook: flip the fail_swaps switch at runtime.
    SetFailSwaps { fail_swaps: bool },
}

#[cosmwasm_schema::cw_serde]
pub enum RouterQuery {
    Swaps {},
    Offered {},
}
