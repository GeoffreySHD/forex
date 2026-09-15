use cosmwasm_std::{
    entry_point,
    from_json, to_json_binary, Coin, Deps, DepsMut, Env, MessageInfo, QueryResponse, Response,
    StdResult, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use csm_std::reserve::{BuybackStatus, ReserveExecuteMsg, RouterExecuteMsg};
use csm_std::CollateralType;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    Config, BALANCES, BUYBACKS_EXECUTED, BUYBACK_STATUS, CONFIG, LAST_OFFER, LAST_ROUTE,
    LUNC_PURCHASED_TOTAL, TOKENS,
};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        admin: deps.api.addr_validate(&msg.admin)?,
        core: deps.api.addr_validate(&msg.core)?,
        vault: deps.api.addr_validate(&msg.vault)?,
        router: None,
        lunc_denom: msg.lunc_denom,
    };
    CONFIG.save(deps.storage, &config)?;
    BUYBACK_STATUS.save(deps.storage, &BuybackStatus::Idle)?;
    BUYBACKS_EXECUTED.save(deps.storage, &0)?;
    LUNC_PURCHASED_TOTAL.save(deps.storage, &Uint128::zero())?;
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
        ExecuteMsg::Receive(receive) => handle_receive(deps, info, receive),
        ExecuteMsg::RegisterToken {
            collateral_type,
            token,
        } => {
            require_admin(deps.as_ref(), &info)?;
            let token = deps.api.addr_validate(&token)?;
            TOKENS.save(deps.storage, collateral_type.as_str().to_string(), &token)?;
            Ok(Response::new()
                .add_attribute("action", "register_token")
                .add_attribute("collateral", collateral_type.as_str())
                .add_attribute("token", token.to_string()))
        }
        ExecuteMsg::SetCore { core } => {
            require_admin(deps.as_ref(), &info)?;
            let core = deps.api.addr_validate(&core)?;
            CONFIG.update(deps.storage, |mut c| {
                c.core = core.clone();
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new()
                .add_attribute("action", "set_core")
                .add_attribute("core", core.to_string()))
        }
        ExecuteMsg::SetVault { vault } => {
            require_admin(deps.as_ref(), &info)?;
            let vault = deps.api.addr_validate(&vault)?;
            CONFIG.update(deps.storage, |mut c| {
                c.vault = vault.clone();
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new()
                .add_attribute("action", "set_vault")
                .add_attribute("vault", vault.to_string()))
        }
        ExecuteMsg::Buyback {
            offer,
            max_slippage_bps,
        } => buyback(deps, info, offer, max_slippage_bps),
        ExecuteMsg::BuybackResult {
            success,
            purchased,
            reason,
        } => buyback_result(deps, info, success, purchased, reason),
        ExecuteMsg::SetRouter { router } => {
            require_admin(deps.as_ref(), &info)?;
            let router = deps.api.addr_validate(&router)?;
            CONFIG.update(deps.storage, |mut c| {
                c.router = Some(router.clone());
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new()
                .add_attribute("action", "set_router")
                .add_attribute("router", router.to_string()))
        }
        ExecuteMsg::SetAdmin { admin } => {
            require_admin(deps.as_ref(), &info)?;
            let admin = deps.api.addr_validate(&admin)?;
            CONFIG.update(deps.storage, |mut c| {
                c.admin = admin.clone();
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new()
                .add_attribute("action", "set_admin")
                .add_attribute("admin", admin.to_string()))
        }
    }
}

fn require_admin(deps: Deps, info: &MessageInfo) -> Result<(), ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}

fn collateral_of_token(
    deps: &DepsMut,
    token: &cosmwasm_std::Addr,
) -> Result<CollateralType, ContractError> {
    for entry in TOKENS.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let (key, addr) = entry?;
        if addr == *token {
            if let Some(ct) = CollateralType::from_str_key(&key) {
                return Ok(ct);
            }
        }
    }
    Err(ContractError::UnknownToken)
}

fn handle_receive(
    deps: DepsMut,
    info: MessageInfo,
    receive: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let hook: ReserveExecuteMsg =
        from_json(&receive.msg).map_err(|_| ContractError::UnsupportedHook)?;
    match hook {
        ReserveExecuteMsg::Deposit {} => {
            // Fees flow only from the core module (mint fees) or the vault
            // (redemption fees routed through custody); direct user sends of
            // stablecoins cannot inflate the reserve (spec: fees are the
            // secondary collateral source).
            let config = CONFIG.load(deps.storage)?;
            let sender = deps.api.addr_validate(&receive.sender)?;
            if sender != config.core && sender != config.vault {
                return Err(ContractError::NotCore);
            }
            let collateral = collateral_of_token(&deps, &info.sender)?;
            BALANCES.update(deps.storage, collateral.as_str().to_string(), |bal| {
                Ok::<_, ContractError>(bal.unwrap_or_default() + receive.amount)
            })?;
            Ok(Response::new()
                .add_attribute("action", "deposit")
                .add_attribute("collateral", collateral.as_str())
                .add_attribute("amount", receive.amount.to_string()))
        }
    }
}

fn buyback(
    deps: DepsMut,
    info: MessageInfo,
    offer: CollateralType,
    max_slippage_bps: u64,
) -> Result<Response, ContractError> {
    require_admin(deps.as_ref(), &info)?;
    if max_slippage_bps == 0 || max_slippage_bps > 5_000 {
        return Err(ContractError::InvalidSlippage);
    }
    let config = CONFIG.load(deps.storage)?;
    let router = config.router.ok_or(ContractError::RouterMissing)?;

    let status = BUYBACK_STATUS.load(deps.storage)?;
    if let BuybackStatus::Pending = status {
        return Err(ContractError::BuybackInProgress("pending"));
    }

    let amount = BALANCES
        .may_load(deps.storage, offer.as_str().to_string())?
        .unwrap_or_default();
    if amount.is_zero() {
        return Err(ContractError::EmptyReserve(offer.as_str()));
    }

    let token = TOKENS.load(deps.storage, offer.as_str().to_string())?;
    BUYBACK_STATUS.save(deps.storage, &BuybackStatus::Pending)?;
    LAST_OFFER.save(deps.storage, &(offer.as_str().to_string(), amount))?;
    LAST_ROUTE.save(deps.storage, &router.to_string())?;

    // Hand the stablecoin to the router; the router swaps against DEX
    // liquidity and reports back via BuybackResult.
    Ok(Response::new()
        .add_message(WasmMsg::Execute {
            contract_addr: token.to_string(),
            msg: to_json_binary(&Cw20ExecuteMsg::Send {
                contract: router.to_string(),
                amount,
                msg: to_json_binary(&RouterExecuteMsg::SwapToLunc {
                    offer,
                    amount,
                    max_slippage_bps,
                })?,
            })?,
            funds: vec![],
        })
        .add_attribute("action", "buyback")
        .add_attribute("offer", offer.as_str())
        .add_attribute("amount", amount.to_string())
        .add_attribute("max_slippage_bps", max_slippage_bps.to_string()))
}

fn buyback_result(
    deps: DepsMut,
    info: MessageInfo,
    success: bool,
    purchased: Uint128,
    reason: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    match config.router {
        Some(router) if info.sender == router => {}
        _ => return Err(ContractError::Unauthorized),
    }

    if success {
        // The offered tokens left the reserve when the router was paid; the
        // ledger is settled here, on the authoritative result.
        if let Some((key, offered)) = LAST_OFFER.may_load(deps.storage)? {
            BALANCES.update(deps.storage, key.clone(), |bal| {
                Ok::<_, ContractError>(bal.unwrap_or_default().saturating_sub(offered))
            })?;
            LAST_OFFER.remove(deps.storage);
        }
        BUYBACKS_EXECUTED.update(deps.storage, |n| Ok::<_, ContractError>(n + 1))?;
        LUNC_PURCHASED_TOTAL.update(deps.storage, |n| {
            Ok::<_, ContractError>(n + purchased)
        })?;
        BUYBACK_STATUS.save(deps.storage, &BuybackStatus::Completed)?;
        Ok(Response::new()
            .add_attribute("action", "buyback_completed")
            .add_attribute("lunc_purchased", purchased.to_string()))
    } else {
        // Failed route: the router transferred the stablecoin back, so the
        // ledger is already accurate. Mark the failure and allow a retry.
        BUYBACK_STATUS.save(
            deps.storage,
            &BuybackStatus::Failed {
                reason: reason.unwrap_or_else(|| "router reported failure".to_string()),
            },
        )?;
        Ok(Response::new()
            .add_attribute("action", "buyback_failed")
            .add_attribute("purchased", purchased.to_string()))
    }
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::Balances {} => {
            let mut balances = Vec::new();
            for entry in BALANCES.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
                let (key, bal) = entry?;
                if let Some(ct) = CollateralType::from_str_key(&key) {
                    balances.push((ct, bal));
                }
            }
            to_json_binary(&csm_std::reserve::ReserveBalancesResponse { balances })
        }
        QueryMsg::BuybackStatus {} => {
            let status = BUYBACK_STATUS.may_load(deps.storage)?.unwrap_or(BuybackStatus::Idle);
            let buybacks_executed = BUYBACKS_EXECUTED.may_load(deps.storage)?.unwrap_or(0);
            let lunc_purchased_total = LUNC_PURCHASED_TOTAL.may_load(deps.storage)?.unwrap_or_default();
            let last_route = LAST_ROUTE.may_load(deps.storage)?;
            to_json_binary(&csm_std::reserve::BuybackStatusResponse {
                status,
                buybacks_executed,
                lunc_purchased_total,
                last_route,
            })
        }
        QueryMsg::Vault {} => {
            // Live bank balance: the LUNC vault is simply every native LUNC
            // satoshi the reserve holds and never spends.
            let config = CONFIG.load(deps.storage)?;
            let balance = deps
                .querier
                .query_balance(env.contract.address.to_string(), &config.lunc_denom)
                .map(|c: Coin| c.amount)
                .unwrap_or_default();
            to_json_binary(&csm_std::reserve::VaultResponse { lunc: balance })
        }
    }
}
