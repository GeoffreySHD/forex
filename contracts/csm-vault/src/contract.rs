use cosmwasm_std::{
    entry_point,
    to_json_binary, Addr, Deps, DepsMut, Env, MessageInfo, QueryResponse, Response, StdResult,
    Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    BalanceResponse, ExecuteMsg, InstantiateMsg, QueryMsg, VaultConfigResponse, VaultHook,
};
use crate::state::{BALANCES, CONFIG, Config};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = deps.api.addr_validate(&msg.admin)?;
    CONFIG.save(deps.storage, &Config { admin, core: None })?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(receive) => {
            let hook: VaultHook = cosmwasm_std::from_json(&receive.msg)
                .map_err(|_| ContractError::UnsupportedHook)?;
            // In the cw20 Send flow the executing contract (`info.sender`) is
            // the token; `receive.sender` is the holder (csm-core).
            match hook {
                VaultHook::Deposit {} => deposit(deps, info.sender, receive.amount),
            }
        }
        ExecuteMsg::Withdraw { to, token, amount, hook } => withdraw(deps, info, to, token, amount, hook),
        ExecuteMsg::RegisterCore { core } => register_core(deps, info, core),
    }
}

fn deposit(
    deps: DepsMut,
    token: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    BALANCES.update(deps.storage, token.to_string(), |bal: Option<Uint128>| {
        Ok::<_, ContractError>(bal.unwrap_or_default() + amount)
    })?;
    Ok(Response::new()
        .add_attribute("action", "deposit")
        .add_attribute("token", token)
        .add_attribute("amount", amount.to_string()))
}

fn withdraw(
    deps: DepsMut,
    info: MessageInfo,
    to: String,
    token: String,
    amount: Uint128,
    hook: Option<cosmwasm_std::Binary>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    match &config.core {
        Some(core) if info.sender == *core => {}
        _ => return Err(ContractError::NotCore),
    }
    let recipient = deps.api.addr_validate(&to)?;
    BALANCES.update(deps.storage, token.clone(), |bal: Option<Uint128>| {
        let bal = bal.unwrap_or_default();
        if bal < amount {
            return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
                "insufficient custodied balance",
            )));
        }
        Ok::<_, ContractError>(bal - amount)
    })?;
    let transfer = match hook {
        Some(hook) => WasmMsg::Execute {
            contract_addr: token.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Send {
                contract: recipient.to_string(),
                amount,
                msg: hook,
            })?,
            funds: vec![],
        },
        None => WasmMsg::Execute {
            contract_addr: token.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient.to_string(),
                amount,
            })?,
            funds: vec![],
        },
    };
    Ok(Response::new()
        .add_message(transfer)
        .add_attribute("action", "withdraw")
        .add_attribute("to", recipient.to_string())
        .add_attribute("token", token)
        .add_attribute("amount", amount.to_string()))
}

fn register_core(deps: DepsMut, info: MessageInfo, core: String) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }
    config.core = Some(deps.api.addr_validate(&core)?);
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "register_core"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::Config {} => {
            let config = CONFIG.load(deps.storage)?;
            to_json_binary(&VaultConfigResponse {
                admin: config.admin.to_string(),
                core: config.core.map(|c| c.to_string()),
            })
        }
        QueryMsg::Balance { token } => {
            let balance = BALANCES.may_load(deps.storage, token.clone())?.unwrap_or_default();
            to_json_binary(&BalanceResponse { token, balance })
        }
    }
}
