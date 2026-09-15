use cosmwasm_schema::write_api;
use csm_core::msg::{ExecuteMsg, InstantiateMsg};
use csm_std::QueryMsg;

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}
