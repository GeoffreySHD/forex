use cosmwasm_std::{
    entry_point,
    from_json, to_json_binary, ContractResult, Decimal, Deps, DepsMut, Env, Fraction,
    MessageInfo, QueryRequest, QueryResponse, Response, StdResult, SystemResult, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg, TokenInfoResponse};

use csm_std::consensus::{cross_rate, ExchangeRatesQueryResponse, TerraQuery, TerraQueryWrapper};
use csm_std::oracle::{is_fresh, OraclePrice, OracleQueryMsg, OracleStatusResponse};
use csm_std::reserve::{ReserveExecuteMsg, ReserveQueryMsg, VaultResponse};
use csm_std::{
    CollateralState, CollateralType, ConfigResponse, OracleMode, PriceResponse, ProtocolInfo,
    ProtocolStatus, QueryMsg, StateResponse,
};
use csm_std::vault::{VaultExecuteMsg, VaultHook};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, HookMsg, InstantiateMsg};
use crate::state::{
    Config, DaySnapshot, COLLATERAL_STATE, COLLATERAL_TOKENS, CONFIG, DAY_SUPPLY,
    LAST_MINT_HEIGHT, MINT_WINDOWS, PARAMS, REDEEM_WINDOW, REDEEMED_TODAY_BY_ADDR, STATUS,
};

const DAY_SECS: u64 = 86_400;
/// Chain denoms used for consensus cross-rates: the euro ballot denom (base)
/// and the dollar ballot denom (quote). USDC collateral is USD-denominated;
/// its EUTC rate is USD-per-EUR. EURC is euro-denominated: 1:1 by identity.
const CONSENSUS_BASE_DENOM: &str = "ueur";
const CONSENSUS_USD_DENOM: &str = "uusd";

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let params = msg.params.unwrap_or_default();
    params.validate().map_err(ContractError::InvalidParams)?;

    let config = Config {
        admin: deps.api.addr_validate(&msg.admin)?,
        eutc_token: deps.api.addr_validate(&msg.eutc_token)?,
        vault: deps.api.addr_validate(&msg.vault)?,
        reserve: deps.api.addr_validate(&msg.reserve)?,
        oracle: msg.oracle.map(|o| deps.api.addr_validate(&o)).transpose()?,
        oracle_mode: msg.oracle_mode.unwrap_or(OracleMode::ConsensusPrimary),
    };
    CONFIG.save(deps.storage, &config)?;
    PARAMS.save(deps.storage, &params)?;
    STATUS.save(deps.storage, &ProtocolStatus::ACTIVE)?;
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
        ExecuteMsg::Receive(receive) => handle_receive(deps, env, info, receive),
        ExecuteMsg::Pause { mint, redeem } => {
            require_admin(deps.as_ref(), &info)?;
            STATUS.save(deps.storage, &ProtocolStatus { mint_paused: mint, redeem_paused: redeem })?;
            Ok(Response::new()
                .add_attribute("action", "pause")
                .add_attribute("mint_paused", mint.to_string())
                .add_attribute("redeem_paused", redeem.to_string()))
        }
        ExecuteMsg::SetParams { params } => {
            require_admin(deps.as_ref(), &info)?;
            params.validate().map_err(ContractError::InvalidParams)?;
            PARAMS.save(deps.storage, &params)?;
            Ok(Response::new().add_attribute("action", "set_params"))
        }
        ExecuteMsg::RegisterCollateral {
            collateral_type,
            token,
        } => {
            require_admin(deps.as_ref(), &info)?;
            let token = deps.api.addr_validate(&token)?;
            COLLATERAL_TOKENS.save(deps.storage, collateral_type.as_str().to_string(), &token)?;
            Ok(Response::new()
                .add_attribute("action", "register_collateral")
                .add_attribute("collateral", collateral_type.as_str())
                .add_attribute("token", token.to_string()))
        }
        ExecuteMsg::SetEutc { eutc } => {
            require_admin(deps.as_ref(), &info)?;
            let eutc = deps.api.addr_validate(&eutc)?;
            CONFIG.update(deps.storage, |mut c| {
                c.eutc_token = eutc.clone();
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new().add_attribute("action", "set_eutc").add_attribute("eutc", eutc.to_string()))
        }
        ExecuteMsg::SetVault { vault } => {
            require_admin(deps.as_ref(), &info)?;
            let vault = deps.api.addr_validate(&vault)?;
            CONFIG.update(deps.storage, |mut c| {
                c.vault = vault.clone();
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new().add_attribute("action", "set_vault").add_attribute("vault", vault.to_string()))
        }
        ExecuteMsg::SetReserve { reserve } => {
            require_admin(deps.as_ref(), &info)?;
            let reserve = deps.api.addr_validate(&reserve)?;
            CONFIG.update(deps.storage, |mut c| {
                c.reserve = reserve.clone();
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new().add_attribute("action", "set_reserve").add_attribute("reserve", reserve.to_string()))
        }
        ExecuteMsg::SetOracle { oracle } => {
            require_admin(deps.as_ref(), &info)?;
            let oracle = oracle.map(|o| deps.api.addr_validate(&o)).transpose()?;
            CONFIG.update(deps.storage, |mut c| {
                c.oracle = oracle;
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new().add_attribute("action", "set_oracle"))
        }
        ExecuteMsg::SetOracleMode { mode } => {
            require_admin(deps.as_ref(), &info)?;
            CONFIG.update(deps.storage, |mut c| {
                c.oracle_mode = mode;
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new().add_attribute("action", "set_oracle_mode"))
        }
        ExecuteMsg::SetAdmin { admin } => {
            require_admin(deps.as_ref(), &info)?;
            let admin = deps.api.addr_validate(&admin)?;
            CONFIG.update(deps.storage, |mut c| {
                c.admin = admin.clone();
                Ok::<_, ContractError>(c)
            })?;
            Ok(Response::new().add_attribute("action", "set_admin").add_attribute("admin", admin.to_string()))
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

/// Day-start supply snapshot for cap denominators (see `DaySnapshot`):
/// refreshes lazily on the first mint or redeem of a new day. Caps computed
/// against live supply would self-consume as redemptions burn tokens.
fn refresh_day_supply(deps: DepsMut, env: &Env, config: &Config) -> StdResult<Uint128> {
    let day = env.block.time.seconds() / DAY_SECS;
    match DAY_SUPPLY.may_load(deps.storage)? {
        Some(snap) if snap.day == day => Ok(snap.supply),
        _ => {
            let supply = eutc_supply(deps.as_ref(), &config.eutc_token)?;
            DAY_SUPPLY.save(deps.storage, &DaySnapshot { day, supply })?;
            Ok(supply)
        }
    }
}

fn handle_receive(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    receive: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    if receive.amount.is_zero() {
        return Err(ContractError::ZeroAmount);
    }
    let hook: HookMsg = from_json(&receive.msg).map_err(|_| ContractError::UnsupportedHook)?;
    let user = deps.api.addr_validate(&receive.sender)?;
    match hook {
        HookMsg::Mint { collateral_type } => {
            let token = registered_token(deps.storage, collateral_type)?;
            if info.sender != token {
                return Err(ContractError::CollateralNotRegistered { collateral: collateral_type });
            }
            mint(&mut deps, env, user, collateral_type, receive.amount)
        }
        HookMsg::Redeem { collateral_type } => {
            let config = CONFIG.load(deps.storage)?;
            if info.sender != config.eutc_token {
                return Err(ContractError::NotEutc);
            }
            redeem(&mut deps, env, user, collateral_type, receive.amount)
        }
    }
}

fn registered_token(
    storage: &dyn cosmwasm_std::Storage,
    collateral: CollateralType,
) -> Result<cosmwasm_std::Addr, ContractError> {
    COLLATERAL_TOKENS
        .load(storage, collateral.as_str().to_string())
        .map_err(|_| ContractError::CollateralNotRegistered { collateral })
}

/// Collateral units per 1 EUTC.
///
/// `ConsensusPrimary`: EURC is euro-denominated, so its rate is 1:1 by
/// denomination identity; USDC is USD-denominated, so its rate is the USD/EUR
/// cross-rate computed by the chain from the slashing-backed oracle ballots
/// (custom wasmbinding query, updated per block). A failed ballot or a chain
/// without the binding falls back to the feeder adapter.
///
/// `AdapterOnly`: the feeder adapter is the sole source; its quote must be
/// fresh (fail closed) — used by tests and for quotes outside the ballot set.
fn oracle_rate(
    deps: Deps,
    env: &Env,
    collateral: CollateralType,
) -> Result<Decimal, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if matches!(config.oracle_mode, OracleMode::ConsensusPrimary) {
        if let Some(rate) = consensus_rate(deps, collateral)? {
            return Ok(rate);
        }
        // fall through to the adapter
    }

    let params = PARAMS.load(deps.storage)?;
    let oracle = config.oracle.ok_or(ContractError::OracleMissing)?;
    let price: OraclePrice =
        deps.querier.query_wasm_smart(&oracle, &OracleQueryMsg::Price { quote: collateral })?;
    if price.units_per_eutc.is_zero()
        || !is_fresh(price.updated_at, env.block.time.seconds(), params.oracle_stale_secs)
    {
        return Err(ContractError::OracleStale { collateral });
    }
    Ok(price.units_per_eutc)
}

/// Chain-consensus cross-rate (collateral units per EUTC), or None when the
/// consensus source cannot serve the quote (error, absent ballot entry, or
/// zero rate) — callers then fall back.
fn consensus_rate(deps: Deps, collateral: CollateralType) -> Result<Option<Decimal>, ContractError> {
    let rate: Option<Uint128> = match collateral {
        CollateralType::Eurc => Some(Decimal::one().atomics()),
        CollateralType::Usdc => {
            let request = QueryRequest::Custom(TerraQueryWrapper::Terra {
                terra: TerraQuery::ExchangeRates {
                    base_denom: CONSENSUS_BASE_DENOM.to_string(),
                    quote_denoms: vec![CONSENSUS_USD_DENOM.to_string()],
                },
            });
            consensus_exchange_rate(&deps.querier, &request, CONSENSUS_USD_DENOM)
        }
    };
    Ok(rate
        .and_then(|atomics| {
            // 18-decimal atomics from the wire format; zero means no usable ballot.
            Decimal::from_atomics(atomics, 18)
                .ok()
                .filter(|d| !d.is_zero())
        }))
}

/// Issue a custom (wasmbinding) query through the raw querier path.
///
/// `QuerierWrapper::query` only accepts requests of the environment's own
/// custom-query type; the raw-bytes path carries the same wire format and is
/// what the Go wasmbinding parses. Errors anywhere (missing binding, failed
/// ballot, malformed response) become `None` so callers fall back to the
/// feeder adapter.
fn consensus_exchange_rate(
    querier: &cosmwasm_std::QuerierWrapper,
    request: &QueryRequest<TerraQueryWrapper>,
    quote_denom: &str,
) -> Option<Uint128> {
    let raw = to_json_binary(request).ok()?;
    let data = match querier.raw_query(raw.as_slice()) {
        SystemResult::Ok(ContractResult::Ok(data)) => data,
        _ => return None,
    };
    let resp: ExchangeRatesQueryResponse = from_json(data).ok()?;
    cross_rate(&resp, quote_denom)
}

/// Aggregate primary ratio in bps (primary_balance × 10_000 / eutc_minted).
/// Zero minted supply counts as "above floor" (nothing to protect yet).
fn primary_ratio_bps(deps: Deps) -> Result<Uint128, ContractError> {
    let mut primary = Uint128::zero();
    let mut minted = Uint128::zero();
    for entry in COLLATERAL_STATE.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let (_, st) = entry?;
        primary += st.primary_balance;
        minted += st.eutc_minted;
    }
    if minted.is_zero() {
        return Ok(Uint128::MAX);
    }
    Ok(primary.multiply_ratio(10_000u128, minted))
}

fn mint(
    deps: &mut DepsMut,
    env: Env,
    user: cosmwasm_std::Addr,
    collateral: CollateralType,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let status = STATUS.load(deps.storage)?;
    if status.mint_paused {
        return Err(ContractError::MintPaused);
    }
    let config = CONFIG.load(deps.storage)?;
    let params = PARAMS.load(deps.storage)?;

    // Solvency governor (anti-dump L3): stop issuing while primary backing
    // has drifted below the governance floor. The floor sits below 100%
    // because the 1.5% fee skim dilutes the ratio ~1% per side by design;
    // hitting the floor means abnormal drain, not normal operation.
    let ratio = primary_ratio_bps(deps.as_ref())?;
    if ratio < Uint128::new(u128::from(params.min_primary_ratio_bps)) {
        return Err(ContractError::RatioBelowFloor {
            current_bps: ratio,
            required_bps: params.min_primary_ratio_bps.into(),
        });
    }

    let rate = oracle_rate(deps.as_ref(), &env, collateral)?;
    let eutc_gross = amount
        .checked_multiply_ratio(rate.denominator(), rate.numerator())
        .map_err(|_| ContractError::Std(cosmwasm_std::StdError::generic_err("rate math overflow")))?;
    // Premium applies to the minted amount (spec: EUTC = USDC x 0.995);
    // the mint fee is taken from the deposit in collateral units instead.
    let premium_bps = if matches!(collateral, CollateralType::Usdc) {
        params.usdc_premium_bps
    } else {
        0
    };
    let eutc_minted = eutc_gross
        .checked_multiply_ratio(10_000u128 - premium_bps as u128, 10_000u128)
        .map_err(|_| ContractError::Std(cosmwasm_std::StdError::generic_err("fee math overflow")))?;
    if eutc_minted.is_zero() {
        return Err(ContractError::ZeroAmount);
    }

    // Mint-velocity cap (anti-dump L1): new supply per day per collateral is
    // bounded relative to the day-start supply, so a same-day
    // mint→dump→redeem loop cannot outrun the redemption cap.
    let supply = refresh_day_supply(deps.branch(), &env, &config)?;
    let day = env.block.time.seconds() / DAY_SECS;
    let window = MINT_WINDOWS
        .load(deps.storage, collateral.as_str().to_string())
        .unwrap_or((day, Uint128::zero()));
    let minted_today = if window.0 == day { window.1 } else { Uint128::zero() };
    let mint_cap_pct = supply
        .checked_multiply_ratio(params.daily_mint_cap_bps as u128, 10_000u128)
        .map_err(|_| ContractError::Std(cosmwasm_std::StdError::generic_err("cap math overflow")))?;
    // Bootstrap floor: the percentage cap is relative to supply (zero at
    // genesis), so the allowance is `max(pct, floor)`.
    let mint_cap = mint_cap_pct.max(Uint128::new(u128::from(params.min_daily_mint_floor)));
    let mint_remaining = mint_cap.saturating_sub(minted_today);
    if eutc_minted > mint_remaining {
        return Err(ContractError::MintCapExceeded {
            collateral,
            remaining: mint_remaining,
        });
    }

    let fee_amount = amount
        .checked_multiply_ratio(params.mint_fee_bps as u128, 10_000u128)
        .map_err(|_| ContractError::Std(cosmwasm_std::StdError::generic_err("fee math overflow")))?;
    let to_vault = amount - fee_amount;
    let token = registered_token(deps.storage, collateral)?;

    COLLATERAL_STATE.update(
        deps.storage,
        collateral.as_str().to_string(),
        |st: Option<CollateralState>| {
            let mut st = st.unwrap_or_default();
            st.eutc_minted += eutc_minted;
            st.primary_balance += to_vault;
            st.fees_collected += fee_amount;
            Ok::<_, ContractError>(st)
        },
    )?;
    MINT_WINDOWS.save(deps.storage, collateral.as_str().to_string(), &(day, minted_today + eutc_minted))?;
    // Flash-fee anchor (anti-dump L2): remember the freshest mint to this
    // address. Known limitation (fungibility): EUTC transferred onwards after
    // minting is not tracked — this catches the direct wash loop, not every
    // passthrough.
    LAST_MINT_HEIGHT.update(deps.storage, user.clone(), |prev: Option<(u64, Uint128)>| {
        Ok::<_, ContractError>(match prev {
            Some((h, amt)) if h > env.block.height => (h, amt),
            _ => (env.block.height, eutc_minted),
        })
    })?;

    Ok(Response::new()
        .add_message(WasmMsg::Execute {
            contract_addr: token.to_string(),
            msg: to_json_binary(&Cw20ExecuteMsg::Send {
                contract: config.vault.to_string(),
                amount: to_vault,
                msg: to_json_binary(&VaultHook::Deposit {})?,
            })?,
            funds: vec![],
        })
        .add_message(WasmMsg::Execute {
            contract_addr: token.to_string(),
            msg: to_json_binary(&Cw20ExecuteMsg::Send {
                contract: config.reserve.to_string(),
                amount: fee_amount,
                msg: to_json_binary(&ReserveExecuteMsg::Deposit {})?,
            })?,
            funds: vec![],
        })
        .add_message(WasmMsg::Execute {
            contract_addr: config.eutc_token.to_string(),
            msg: to_json_binary(&Cw20ExecuteMsg::Mint {
                recipient: user.to_string(),
                amount: eutc_minted,
            })?,
            funds: vec![],
        })
        .add_attribute("action", "mint")
        .add_attribute("collateral", collateral.as_str())
        .add_attribute("collateral_in", amount.to_string())
        .add_attribute("eutc_minted", eutc_minted.to_string())
        .add_attribute("fee", fee_amount.to_string())
        .add_attribute("day", day.to_string()))
}

fn redeem(
    deps: &mut DepsMut,
    env: Env,
    user: cosmwasm_std::Addr,
    collateral: CollateralType,
    eutc_amount: Uint128,
) -> Result<Response, ContractError> {
    let status = STATUS.load(deps.storage)?;
    if status.redeem_paused {
        return Err(ContractError::RedeemPaused);
    }
    let config = CONFIG.load(deps.storage)?;
    let params = PARAMS.load(deps.storage)?;
    let token = registered_token(deps.storage, collateral)?;

    let supply = refresh_day_supply(deps.branch(), &env, &config)?;
    let day = env.block.time.seconds() / DAY_SECS;
    let window = REDEEM_WINDOW.load(deps.storage).unwrap_or((day, Uint128::zero()));
    let redeemed_today = if window.0 == day { window.1 } else { Uint128::zero() };

    // Global daily cap (anti-dump L1, proposal §1): the Soros defense.
    let daily_cap = supply
        .checked_multiply_ratio(params.daily_redeem_cap_bps as u128, 10_000u128)
        .map_err(|_| ContractError::Std(cosmwasm_std::StdError::generic_err("cap math overflow")))?;
    let remaining = daily_cap.saturating_sub(redeemed_today);

    // Per-address cap (anti-dump L1): one whale cannot consume the window.
    let addr_cap = supply
        .checked_multiply_ratio(params.per_addr_redeem_cap_bps as u128, 10_000u128)
        .map_err(|_| ContractError::Std(cosmwasm_std::StdError::generic_err("cap math overflow")))?;
    let addr_used = REDEEMED_TODAY_BY_ADDR
        .load(deps.storage, (day.to_string(), user.clone()))
        .unwrap_or_default();
    let addr_remaining = addr_cap.saturating_sub(addr_used);

    let allowed = remaining.min(addr_remaining);
    if eutc_amount > allowed {
        return Err(if remaining <= addr_remaining {
            ContractError::DailyCapExceeded { remaining }
        } else {
            ContractError::PerAddressCapExceeded { remaining: addr_remaining }
        });
    }

    // Dynamic spread (anti-dump L2): the redeem fee rises linearly with the
    // daily window utilization, making redemptions into a stressed window
    // progressively more expensive.
    let utilization_bps = if daily_cap.is_zero() {
        Uint128::zero()
    } else {
        redeemed_today.multiply_ratio(10_000u128, daily_cap).min(Uint128::new(10_000))
    };
    let spread_adder = Uint128::new(u128::from(params.max_dynamic_spread_bps))
        .multiply_ratio(utilization_bps, 10_000u128);

    // Flash fee (anti-dump L2): extra fee when the sender's freshest mint is
    // within the flash window — the wash mint→dump loop loses by construction.
    let flash_active = LAST_MINT_HEIGHT
        .may_load(deps.storage, user.clone())?
        .map(|(h, _)| env.block.height.saturating_sub(h) <= params.flash_fee_window_blocks)
        .unwrap_or(false);
    let flash_adder = if flash_active { params.flash_fee_bps } else { 0 };

    let rate = oracle_rate(deps.as_ref(), &env, collateral)?;
    let collateral_units = eutc_amount
        .checked_multiply_ratio(rate.numerator(), rate.denominator())
        .map_err(|_| ContractError::Std(cosmwasm_std::StdError::generic_err("rate math overflow")))?;
    let effective_fee_bps: u64 = (params.redeem_fee_bps + spread_adder.u128() as u64 + flash_adder)
        .min(10_000);
    let fee_amount = collateral_units
        .checked_multiply_ratio(effective_fee_bps as u128, 10_000u128)
        .map_err(|_| ContractError::Std(cosmwasm_std::StdError::generic_err("fee math overflow")))?;
    let payout = collateral_units - fee_amount;

    let mut state = COLLATERAL_STATE
        .may_load(deps.storage, collateral.as_str().to_string())?
        .unwrap_or_default();
    if state.primary_balance < collateral_units {
        return Err(ContractError::InsufficientCollateral {
            available: state.primary_balance,
            requested: collateral_units,
        });
    }
    state.eutc_minted = state.eutc_minted.saturating_sub(eutc_amount);
    state.primary_balance -= collateral_units;
    state.collateral_returned += payout;
    state.fees_collected += fee_amount;
    COLLATERAL_STATE.save(deps.storage, collateral.as_str().to_string(), &state)?;
    REDEEM_WINDOW.save(deps.storage, &(day, redeemed_today + eutc_amount))?;
    REDEEMED_TODAY_BY_ADDR.save(deps.storage, (day.to_string(), user.clone()), &(addr_used + eutc_amount))?;
    // Note: per-address entries for past days are not pruned; the map grows
    // with distinct (day, addr) activity and is bounded by real usage.

    Ok(Response::new()
        .add_message(WasmMsg::Execute {
            contract_addr: config.eutc_token.to_string(),
            msg: to_json_binary(&Cw20ExecuteMsg::Burn { amount: eutc_amount })?,
            funds: vec![],
        })
        .add_message(WasmMsg::Execute {
            contract_addr: config.vault.to_string(),
            msg: to_json_binary(&VaultExecuteMsg::Withdraw {
                to: user.to_string(),
                token: token.to_string(),
                amount: payout,
                hook: None,
            })?,
            funds: vec![],
        })
        .add_message(WasmMsg::Execute {
            contract_addr: config.vault.to_string(),
            msg: to_json_binary(&VaultExecuteMsg::Withdraw {
                to: config.reserve.to_string(),
                token: token.to_string(),
                amount: fee_amount,
                hook: Some(to_json_binary(&ReserveExecuteMsg::Deposit {})?),
            })?,
            funds: vec![],
        })
        .add_attribute("action", "redeem")
        .add_attribute("collateral", collateral.as_str())
        .add_attribute("eutc_burned", eutc_amount.to_string())
        .add_attribute("collateral_out", collateral_units.to_string())
        .add_attribute("fee", fee_amount.to_string())
        .add_attribute("effective_fee_bps", effective_fee_bps.to_string())
        .add_attribute("flash_fee", flash_active.to_string())
        .add_attribute("day", day.to_string()))
}

fn eutc_supply(deps: Deps, eutc: &cosmwasm_std::Addr) -> StdResult<Uint128> {
    let info: TokenInfoResponse = deps.querier.query_wasm_smart(eutc, &Cw20QueryMsg::TokenInfo {})?;
    Ok(info.total_supply)
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::State {} => to_json_binary(&query_state(deps)?),
        QueryMsg::Price { quote } => to_json_binary(&query_price(deps, env, quote)?),
        QueryMsg::ProtocolInfo {} => to_json_binary(&query_protocol_info(deps, env)?),
    }
}

fn collect_collateral(deps: Deps) -> StdResult<Vec<(CollateralType, CollateralState)>> {
    let mut out = Vec::new();
    for entry in COLLATERAL_STATE.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let (key, state) = entry?;
        if let Some(ct) = CollateralType::from_str_key(&key) {
            out.push((ct, state));
        }
    }
    Ok(out)
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mut collateral_tokens = Vec::new();
    for entry in COLLATERAL_TOKENS.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let (key, addr) = entry?;
        if let Some(ct) = CollateralType::from_str_key(&key) {
            collateral_tokens.push((ct, addr.to_string()));
        }
    }
    Ok(ConfigResponse {
        admin: config.admin.to_string(),
        eutc_token: config.eutc_token.to_string(),
        vault: config.vault.to_string(),
        reserve: config.reserve.to_string(),
        oracle: config.oracle.map(|o| o.to_string()),
        oracle_mode: config.oracle_mode,
        collateral_tokens,
    })
}

fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let status = STATUS.load(deps.storage)?;
    let params = PARAMS.load(deps.storage)?;
    let window = REDEEM_WINDOW.load(deps.storage).unwrap_or((0, Uint128::zero()));
    Ok(StateResponse {
        status,
        params,
        collateral: collect_collateral(deps)?,
        redeem_window_day: window.0,
        redeemed_today: window.1,
    })
}

fn query_price(deps: Deps, env: Env, quote: CollateralType) -> StdResult<PriceResponse> {
    let (rate, fresh) = match oracle_rate(deps, &env, quote) {
        Ok(rate) => (rate, true),
        Err(_) => (Decimal::zero(), false),
    };
    Ok(PriceResponse { quote, rate: rate.to_string(), fresh })
}

fn query_protocol_info(deps: Deps, env: Env) -> StdResult<ProtocolInfo> {
    let config = CONFIG.load(deps.storage)?;
    let status = STATUS.load(deps.storage)?;
    let params = PARAMS.load(deps.storage)?;
    let supply = eutc_supply(deps, &config.eutc_token)?;

    let window = REDEEM_WINDOW.load(deps.storage).unwrap_or((0, Uint128::zero()));
    let day = env.block.time.seconds() / DAY_SECS;
    let redeemed_today = if window.0 == day { window.1 } else { Uint128::zero() };
    let daily_cap = supply
        .checked_multiply_ratio(params.daily_redeem_cap_bps as u128, 10_000u128)
        .unwrap_or_default();

    let collateral = collect_collateral(deps)?;
    let primary_total: Uint128 = collateral.iter().map(|(_, st)| st.primary_balance).sum();
    let minted_total: Uint128 = collateral.iter().map(|(_, st)| st.eutc_minted).sum();

    let secondary_reserve = match deps.querier.query_wasm_smart::<csm_std::reserve::ReserveBalancesResponse>(
        &config.reserve,
        &ReserveQueryMsg::Balances {},
    ) {
        Ok(r) => r.balances,
        Err(_) => Vec::new(),
    };
    let reserve_lunc_vault = deps
        .querier
        .query_wasm_smart::<VaultResponse>(&config.reserve, &ReserveQueryMsg::Vault {})
        .map(|v| v.lunc)
        .unwrap_or_default();

    // Collateral ratio counts primary custody PLUS the secondary reserve's
    // stablecoin balances (E2 diagnostics fix): fees are protocol assets
    // backing the unit until spent on buybacks.
    let reserve_stable_total: Uint128 = secondary_reserve.iter().map(|(_, amt)| *amt).sum();
    let collateral_ratio_pct = if minted_total.is_zero() {
        Uint128::zero()
    } else {
        (primary_total + reserve_stable_total).multiply_ratio(100u128, minted_total)
    };

    let oracle_healthy = match &config.oracle {
        Some(oracle) => deps
            .querier
            .query_wasm_smart::<OracleStatusResponse>(oracle, &OracleQueryMsg::Status {})
            .map(|s| s.healthy)
            .unwrap_or(false),
        None => false,
    };

    Ok(ProtocolInfo {
        status,
        params,
        eutc_denom: config.eutc_token.to_string(),
        eutc_total_supply: supply,
        eutc_minted_total: minted_total,
        collateral,
        collateral_ratio_pct,
        daily_redeem_remaining: daily_cap.saturating_sub(redeemed_today),
        redeem_window_day: window.0,
        secondary_reserve,
        reserve_lunc_vault,
        oracle_healthy,
    })
}
