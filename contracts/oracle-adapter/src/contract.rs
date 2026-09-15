use cosmwasm_std::{
    entry_point,
    to_json_binary, Decimal, Deps, DepsMut, Env, MessageInfo, QueryResponse, Response, StdResult,
};
use cw_storage_plus::Item;

use csm_std::oracle::{
    is_fresh, OracleExecuteMsg, OraclePrice, OracleQueryMsg, OracleStatusResponse,
};
use csm_std::CollateralType;

use crate::error::ContractError;
use crate::msg::InstantiateMsg;
use crate::state::{Config, CONFIG, FEEDERS, PRICES};

const CONTRACT_VERSION_STR: &str = env!("CARGO_PKG_VERSION");
const VERSION_ITEM: Item<String> = Item::new("contract_version_v2");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = deps.api.addr_validate(&msg.admin)?;
    if msg.stale_secs == 0 {
        return Err(ContractError::InvalidConfig);
    }
    CONFIG.save(deps.storage, &Config { admin, stale_secs: msg.stale_secs })?;
    VERSION_ITEM.save(deps.storage, &CONTRACT_VERSION_STR.to_string())?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: OracleExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        OracleExecuteMsg::PostPrice { quote, units_per_eutc } => post_price(deps, env, info, quote, units_per_eutc),
        OracleExecuteMsg::SetFeeder { feeder, authorized } => set_feeder(deps, info, feeder, authorized),
    }
}

fn post_price(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    quote: CollateralType,
    units_per_eutc: Decimal,
) -> Result<Response, ContractError> {
    let _config = CONFIG.load(deps.storage)?;
    if !FEEDERS.may_load(deps.storage, info.sender.clone())?.unwrap_or(false) {
        return Err(ContractError::UnauthorizedFeeder);
    }
    if units_per_eutc.is_zero() {
        return Err(ContractError::NoPrice);
    }
    PRICES.save(deps.storage, quote.as_str().to_string(), &OraclePrice {
        quote,
        units_per_eutc,
        updated_at: env.block.time.seconds(),
    })?;
    Ok(Response::new()
        .add_attribute("action", "post_price")
        .add_attribute("quote", quote.as_str()))
}

fn set_feeder(
    deps: DepsMut,
    info: MessageInfo,
    feeder: cosmwasm_std::Addr,
    authorized: bool,
) -> Result<Response, ContractError> {
    let feeder = deps.api.addr_validate(feeder.as_str())?;
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }
    if authorized {
        FEEDERS.save(deps.storage, feeder.clone(), &true)?;
    } else {
        FEEDERS.remove(deps.storage, feeder.clone());
    }
    Ok(Response::new()
        .add_attribute("action", "set_feeder")
        .add_attribute("feeder", feeder.to_string())
        .add_attribute("authorized", authorized.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: OracleQueryMsg) -> StdResult<QueryResponse> {
    match msg {
        OracleQueryMsg::Price { quote } => to_json_binary(&price(deps, env, quote)?),
        OracleQueryMsg::Status {} => to_json_binary(&status(deps, env)?),
    }
}

/// Latest raw observation for a quote. Freshness is reported via
/// `Status {}`; mints consult the aggregate verdict, not a single quote.
fn price(deps: Deps, env: Env, quote: CollateralType) -> StdResult<OraclePrice> {
    let stored = PRICES.load(deps.storage, quote.as_str().to_string())?;
    let _ = env;
    Ok(stored)
}

fn status(deps: Deps, env: Env) -> StdResult<OracleStatusResponse> {
    let config = CONFIG.load(deps.storage)?;
    let now = env.block.time.seconds();
    let mut prices = Vec::new();
    let mut all_fresh = true;
    for quote in [CollateralType::Eurc, CollateralType::Usdc] {
        if let Some(p) = PRICES.may_load(deps.storage, quote.as_str().to_string())? {
            if !is_fresh(p.updated_at, now, config.stale_secs) {
                all_fresh = false;
            }
            prices.push(p);
        } else {
            all_fresh = false;
        }
    }
    Ok(OracleStatusResponse {
        healthy: all_fresh && !prices.is_empty(),
        all_fresh,
        prices,
    })
}
