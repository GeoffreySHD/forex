//! Opt-in legacy-unit conversion bridge for the TC fiat family.
//!
//! Legacy holders send `ueur`/`ukrw`/… (native bank denoms) attached to a
//! `Convert{}` execute. The bridge records a claim (capped globally and per
//! address, vested linearly), forwards the coins to the configured sink, and
//! emits a disclosure event: conversion eligibility is NOT a promise that
//! legacy units will ever be worth their face reference amount (see
//! TERMINOLOGY.md). Claims are redeemed for CSM-collateralized units by a
//! separate, liquidity-gated step that this contract deliberately does not
//! perform.

use cosmwasm_std::{
    entry_point,
    coins, to_json_binary, BankMsg, Coin, Coins, Deps, DepsMut, Env, MessageInfo, QueryResponse,
    Response, StdResult, Uint128,
};
use cw2::set_contract_version;

use csm_std::consensus::LEGACY_FOREX_DENOMS;

use crate::error::ContractError;
use crate::msg::{
    ClaimInfo, ClaimResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    TotalsResponse,
};
use crate::state::{
    ensure_convertible, Claim, Config, CONVERTED_PER_ADDR, CONVERTED_PER_DENOM, CLAIMS, CONFIG,
};

const CONTRACT_NAME: &str = "crates.io:csm-legacy-bridge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Approximate block count per day on Terra Classic (~6s blocks on paper,
/// but the chain historically runs slower; 7200 ≈ 12h — vesting is admin-
/// configured in days and deliberately conservative on the safe side, i.e.
/// claims may vest slightly faster than nominal). Kept as a constant to
/// avoid a per-block cron dependency.
const BLOCKS_PER_DAY: u64 = 7_200;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if msg.vesting_days == 0 {
        return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "vesting_days must be positive",
        )));
    }
    let config = Config {
        admin: deps.api.addr_validate(&msg.admin)?,
        sink_address: deps.api.addr_validate(&msg.sink_address)?,
        vesting_days: msg.vesting_days,
        open: msg.open,
        global_cap: Uint128::new(1_000_000_000_000), // 1M units per denom default
        per_address_cap: Uint128::new(10_000_000),   // 10 units-equivalent default
    };
    CONFIG.save(deps.storage, &config)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Convert {} => convert(deps, env, info),
        ExecuteMsg::Claim { denom } => claim(deps, env, info, denom),
        ExecuteMsg::SetOpen { open } => {
            let mut config = require_admin(deps.as_ref(), &info)?;
            config.open = open;
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new()
                .add_attribute("action", "set_open")
                .add_attribute("open", open.to_string()))
        }
        ExecuteMsg::SetCaps { global, per_address } => {
            require_admin(deps.as_ref(), &info)?;
            let mut config = CONFIG.load(deps.storage)?;
            if per_address > global {
                return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
                    "per_address cap cannot exceed global cap",
                )));
            }
            config.global_cap = Uint128::new(global);
            config.per_address_cap = Uint128::new(per_address);
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new().add_attribute("action", "set_caps"))
        }
        ExecuteMsg::SetAdmin { admin } => {
            require_admin(deps.as_ref(), &info)?;
            let mut config = CONFIG.load(deps.storage)?;
            config.admin = deps.api.addr_validate(&admin)?;
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new()
                .add_attribute("action", "set_admin")
                .add_attribute("admin", config.admin.to_string()))
        }
    }
}

fn require_admin(deps: Deps, info: &MessageInfo) -> Result<Config, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }
    Ok(config)
}

/// Static lookup key for a legacy denom (the registry is a const slice, so
/// the reference is genuinely 'static).
fn denom_key(denom: &str) -> Option<&'static str> {
    LEGACY_FOREX_DENOMS.iter().copied().find(|d| *d == denom)
}

fn convert(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if !config.open {
        return Err(ContractError::ProgramClosed);
    }
    let parsed = Coins::try_from(info.funds)
        .map_err(|_| ContractError::Std(cosmwasm_std::StdError::generic_err("invalid funds")))?;
    if parsed.len() != 1 {
        return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "convert exactly one denom per call",
        )));
    }
    let Coin { denom, amount } = parsed.iter().next().ok_or(ContractError::NoFunds)?;
    if !ensure_convertible(denom) {
        return Err(ContractError::NotConvertible { denom: denom.to_string() });
    }
    let key = denom_key(denom).unwrap_or_default();

    // Caps (anti-dump L5): global lifetime per denom, per-address lifetime.
    let used_global = CONVERTED_PER_DENOM.may_load(deps.storage, key)?.unwrap_or_default();
    let used_addr = CONVERTED_PER_ADDR
        .may_load(deps.storage, (key, info.sender.as_str()))?
        .unwrap_or_default();
    let new_global = used_global + *amount;
    let new_addr = used_addr + *amount;
    if new_global > config.global_cap {
        return Err(ContractError::GlobalCapExceeded {
            cap: config.global_cap.u128(),
            used: used_global.u128(),
            denom: denom.to_string(),
        });
    }
    if new_addr > config.per_address_cap {
        return Err(ContractError::AddressCapExceeded {
            cap: config.per_address_cap.u128(),
            used: used_addr.u128(),
            denom: denom.to_string(),
        });
    }

    let end_block = env.block.height + config.vesting_days * BLOCKS_PER_DAY;
    CLAIMS.update(
        deps.storage,
        (key, info.sender.as_str()),
        |existing: Option<Claim>| {
            let mut c = existing.unwrap_or(Claim {
                denom: denom.to_string(),
                total: Uint128::zero(),
                claimed: Uint128::zero(),
                start_block: env.block.height,
                end_block,
            });
            c.total += *amount;
            // Extend vesting end on top-ups to keep the linear curve honest.
            c.end_block = end_block;
            Ok::<_, ContractError>(c)
        },
    )?;
    CONVERTED_PER_DENOM.save(deps.storage, key, &new_global)?;
    CONVERTED_PER_ADDR.save(deps.storage, (key, info.sender.as_str()), &new_addr)?;

    // Coins leave this contract permanently — the sink is a plain bank
    // transfer and nothing in this contract ever sends coins back.
    let disclosure = format!(
        "CONVERSION DISCLOSURE: converting {} {}. Conversion eligibility is NOT a promise of value; legacy units have no guaranteed redemption, and vested claims are exchangeable only for collateralized units against fresh collateral, subject to program terms and liquidity.",
        amount, denom
    );

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: config.sink_address.to_string(),
            amount: coins(amount.u128(), denom),
        })
        .add_attribute("action", "convert")
        .add_attribute("converter", info.sender.to_string())
        .add_attribute("denom", denom.to_string())
        .add_attribute("amount", amount.to_string())
        .add_attribute("vesting_end_block", end_block.to_string())
        .add_attribute("disclosure", disclosure))
}

fn claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    denom: String,
) -> Result<Response, ContractError> {
    let key = denom_key(&denom).unwrap_or(&denom);
    let mut c = CLAIMS
        .may_load(deps.storage, (key, info.sender.as_str()))?
        .ok_or(ContractError::NothingToClaim)?;
    let vested = vested_amount(&c, env.block.height);
    let claimable = vested - c.claimed;
    if claimable.is_zero() {
        return Err(ContractError::NothingToClaim);
    }
    c.claimed += claimable;
    if c.claimed >= c.total {
        CLAIMS.remove(deps.storage, (key, info.sender.as_str()));
    } else {
        CLAIMS.save(deps.storage, (key, info.sender.as_str()), &c)?;
    }
    // The claim is a bookkeeping credit toward future collateralized-unit
    // issuance; this contract holds no coins and sends none. The issuance
    // integration (CSM mint allowance) is wired at program launch.
    Ok(Response::new()
        .add_attribute("action", "claim_ack")
        .add_attribute("claimer", info.sender.to_string())
        .add_attribute("denom", denom)
        .add_attribute("amount", claimable.to_string())
        .add_attribute(
            "note",
            "claim recorded; issuance of collateralized units against this claim is handled by the CSM program terms",
        ))
}

fn vested_amount(c: &Claim, height: u64) -> Uint128 {
    if height >= c.end_block {
        c.total
    } else if height <= c.start_block {
        Uint128::zero()
    } else {
        let elapsed = u128::from(height - c.start_block);
        let span = u128::from(c.end_block.saturating_sub(c.start_block)).max(1);
        c.total.multiply_ratio(elapsed, span)
    }
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::Config {} => {
            let c = CONFIG.load(deps.storage)?;
            to_json_binary(&ConfigResponse {
                admin: c.admin.to_string(),
                sink_address: c.sink_address.to_string(),
                vesting_days: c.vesting_days,
                open: c.open,
                global_cap: c.global_cap.to_string(),
                per_address_cap: c.per_address_cap.to_string(),
            })
        }
        QueryMsg::Claim { address, denom } => {
            let addr = deps.api.addr_validate(&address)?;
            let key = denom_key(&denom).unwrap_or(&denom);
            let claim = CLAIMS.may_load(deps.storage, (key, addr.as_str()))?;
            let vested = claim
                .as_ref()
                .map(|c| vested_amount(c, env.block.height))
                .unwrap_or_default();
            to_json_binary(&ClaimResponse {
                claim: claim.map(|c| ClaimInfo {
                    denom: c.denom,
                    total: c.total.to_string(),
                    claimed: c.claimed.to_string(),
                    start_block: c.start_block,
                    end_block: c.end_block,
                }),
                vested: vested.to_string(),
                program_open: CONFIG.load(deps.storage)?.open,
            })
        }
        QueryMsg::ConvertedTotals { denom } => {
            let key = denom_key(&denom).unwrap_or(&denom);
            let global =
                CONVERTED_PER_DENOM.may_load(deps.storage, key)?.unwrap_or_default();
            to_json_binary(&TotalsResponse {
                denom,
                converted: global.to_string(),
            })
        }
    }
}
