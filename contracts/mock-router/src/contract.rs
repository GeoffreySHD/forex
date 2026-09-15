//! Deterministic DEX router stub for the CSM test suite. Implements the
//! `RouterExecuteMsg` surface: swaps at a fixed configurable rate, fails on
//! demand, and reports the outcome back to the reserve via the shared
//! `ReserveCallbackMsg`.

use cosmwasm_std::{
    from_json, to_json_binary, BankMsg, Coin, Deps, DepsMut, Env, Fraction, MessageInfo,
    QueryResponse, Response, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ReceiveMsg;

use csm_std::reserve::{ReserveCallbackMsg, RouterExecuteMsg};

use crate::msg::{ExecuteMsg, InstantiateMsg, RouterQuery};
use crate::state::{Config, CONFIG, OFFERED_TOTAL, SWAPS_EXECUTED};

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, cosmwasm_std::StdError> {
    let config = Config {
        rate: msg.rate.parse()?,
        fail_swaps: msg.fail_swaps,
    };
    CONFIG.save(deps.storage, &config)?;
    SWAPS_EXECUTED.save(deps.storage, &0)?;
    OFFERED_TOTAL.save(deps.storage, &Uint128::zero())?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, cosmwasm_std::StdError> {
    match msg {
        ExecuteMsg::Receive(recv) => receive(deps, env, info, recv),
        ExecuteMsg::SetFailSwaps { fail_swaps } => {
            CONFIG.update(deps.storage, |mut c| {
                c.fail_swaps = fail_swaps;
                Ok::<_, cosmwasm_std::StdError>(c)
            })?;
            Ok(Response::new().add_attribute("action", "set_fail_swaps"))
        }
    }
}

/// cw20 receive hook entrypoint.
pub fn receive(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    receive: Cw20ReceiveMsg,
) -> Result<Response, cosmwasm_std::StdError> {
    let swap: RouterExecuteMsg = from_json(&receive.msg)?;
    let RouterExecuteMsg::SwapToLunc { .. } = swap;
    let config = CONFIG.load(deps.storage)?;
    SWAPS_EXECUTED.update(deps.storage, |n| Ok::<_, cosmwasm_std::StdError>(n + 1))?;
    OFFERED_TOTAL.update(deps.storage, |n| Ok::<_, cosmwasm_std::StdError>(n + receive.amount))?;

    if config.fail_swaps {
        // Return the offered stablecoins, then report failure to the reserve.
        return Ok(Response::new()
            .add_message(WasmMsg::Execute {
                contract_addr: info.sender.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Transfer {
                    recipient: receive.sender.to_string(),
                    amount: receive.amount,
                })?,
                funds: vec![],
            })
            .add_message(WasmMsg::Execute {
                contract_addr: receive.sender.to_string(),
                msg: to_json_binary(&ReserveCallbackMsg::BuybackResult {
                    success: false,
                    purchased: Uint128::zero(),
                    reason: Some("mock: failing router".to_string()),
                })?,
                funds: vec![],
            })
            .add_attribute("action", "swap_failed"));
    }

    let purchased = receive
        .amount
        .checked_multiply_ratio(config.rate.numerator(), config.rate.denominator())
        .map_err(|_| cosmwasm_std::StdError::generic_err("mock rate overflow"))?;

    // Deliver LUNC to the reserve (the cw20 sender), then report success.
    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: receive.sender.to_string(),
            amount: vec![Coin {
                denom: "ulunc".to_string(),
                amount: purchased,
            }],
        })
        .add_message(WasmMsg::Execute {
            contract_addr: receive.sender.to_string(),
            msg: to_json_binary(&ReserveCallbackMsg::BuybackResult {
                success: true,
                purchased,
                reason: None,
            })?,
            funds: vec![],
        })
        .add_attribute("action", "swap")
        .add_attribute("purchased", purchased.to_string()))
}

pub fn query(deps: Deps, _env: Env, msg: RouterQuery) -> StdResult<QueryResponse> {
    match msg {
        RouterQuery::Swaps {} => {
            to_json_binary(&SWAPS_EXECUTED.may_load(deps.storage)?.unwrap_or(0))
        }
        RouterQuery::Offered {} => {
            to_json_binary(&OFFERED_TOTAL.may_load(deps.storage)?.unwrap_or_default())
        }
    }
}
