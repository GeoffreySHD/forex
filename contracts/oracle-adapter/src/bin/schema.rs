use cosmwasm_schema::write_api;
use csm_oracle_adapter::msg::InstantiateMsg;
use csm_std::oracle::{OracleExecuteMsg, OracleQueryMsg};

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: OracleExecuteMsg,
        query: OracleQueryMsg,
    }
}
