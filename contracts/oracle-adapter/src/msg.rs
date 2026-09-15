use cosmwasm_schema::cw_serde;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    /// Price older than this many seconds is reported stale; mints fail closed.
    pub stale_secs: u64,
}
