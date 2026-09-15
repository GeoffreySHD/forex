//! End-to-end tests for the CSM contract suite, mapped to the Forex Protocol
//! developer reference "Test requirements" list.

use cosmwasm_std::{to_json_binary, Addr, Coin, Empty, Uint128};
use cw20::MinterResponse;
use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor};

use csm_core::msg::{ExecuteMsg as CoreExec, HookMsg, InstantiateMsg as CoreInit};
use csm_oracle_adapter::msg::InstantiateMsg as OracleInit;
use csm_reserve::msg::InstantiateMsg as ReserveInit;
use csm_std::oracle::OracleExecuteMsg;
use csm_std::reserve::ReserveExecuteMsg;
use csm_std::{CollateralType, Params, ProtocolInfo};
use csm_vault::msg::ExecuteMsg as VaultExec;
use mock_router::msg::InstantiateMsg as RouterInit;

type Cw20Init = cw20_base::msg::InstantiateMsg;
type Cw20Exec = cw20_base::msg::ExecuteMsg;
type Cw20Query = cw20_base::msg::QueryMsg;

const LUNC_DENOM: &str = "ulunc";

fn contract_cw20() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ))
}

fn contract_vault() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        csm_vault::contract::execute,
        csm_vault::contract::instantiate,
        csm_vault::contract::query,
    ))
}

fn contract_oracle() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        csm_oracle_adapter::contract::execute,
        csm_oracle_adapter::contract::instantiate,
        csm_oracle_adapter::contract::query,
    ))
}

fn contract_reserve() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        csm_reserve::contract::execute,
        csm_reserve::contract::instantiate,
        csm_reserve::contract::query,
    ))
}

fn contract_core() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        csm_core::contract::execute,
        csm_core::contract::instantiate,
        csm_core::contract::query,
    ))
}

fn contract_router() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        mock_router::contract::execute,
        mock_router::contract::instantiate,
        mock_router::contract::query,
    ))
}

struct Chain {
    app: App,
    owner: Addr,
    user: Addr,
    feeder: Addr,
    eutc: Addr,
    eurc: Addr,
    usdc: Addr,
    vault: Addr,
    oracle: Addr,
    reserve: Addr,
    core: Addr,
}

fn mint_cw20(app: &mut App, token: &Addr, minter: &Addr, to: &Addr, amount: u128) {
    app.execute_contract(
        minter.clone(),
        token.clone(),
        &Cw20Exec::Mint {
            recipient: to.to_string(),
            amount: Uint128::new(amount),
        },
        &[],
    )
    .unwrap();
}

fn instantiate_chain(fail_swaps: bool) -> Chain {
    let mut app = AppBuilder::new().build(|_, _, _| {});
    let owner = app.api().addr_make("owner");
    let user = app.api().addr_make("user");
    let feeder = app.api().addr_make("feeder");

    let cw20_id = app.store_code(contract_cw20());
    let vault_id = app.store_code(contract_vault());
    let oracle_id = app.store_code(contract_oracle());
    let reserve_id = app.store_code(contract_reserve());
    let core_id = app.store_code(contract_core());
    let router_id = app.store_code(contract_router());

    let vault = app
        .instantiate_contract(
            vault_id,
            owner.clone(),
            &csm_vault::msg::InstantiateMsg { admin: owner.to_string() },
            &[],
            "vault",
            None,
        )
        .unwrap();

    let oracle = app
        .instantiate_contract(
            oracle_id,
            owner.clone(),
            &OracleInit { admin: owner.to_string(), stale_secs: 300 },
            &[],
            "oracle",
            None,
        )
        .unwrap();

    let reserve = app
        .instantiate_contract(
            reserve_id,
            owner.clone(),
            &ReserveInit {
                admin: owner.to_string(),
                core: owner.to_string(),
                vault: vault.to_string(),
                lunc_denom: LUNC_DENOM.to_string(),
            },
            &[],
            "reserve",
            None,
        )
        .unwrap();

    let core = app
        .instantiate_contract(
            core_id,
            owner.clone(),
            &CoreInit {
                admin: owner.to_string(),
                // Placeholder until the EUTC token exists; rebind via SetEutc.
                eutc_token: owner.to_string(),
                vault: vault.to_string(),
                reserve: reserve.to_string(),
                oracle: Some(oracle.to_string()),
                params: None,
                // None → ConsensusPrimary. The Empty-custom test app has no
                // terra wasmbinding, so every consensus lookup fails and the
                // contract falls back to the feeder adapter (the designed
                // failure path — and the consensus path itself is covered by
                // consensus_fallback_and_custom_module.rs).
                oracle_mode: None,
            },
            &[],
            "core",
            None,
        )
        .unwrap();

    let eutc = app
        .instantiate_contract(
            cw20_id,
            owner.clone(),
            &Cw20Init {
                name: "Euro Terra Classic".into(),
                symbol: "EUTC".into(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: core.to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            &[],
            "eutc",
            None,
        )
        .unwrap();

    let eurc = app
        .instantiate_contract(
            cw20_id,
            owner.clone(),
            &Cw20Init {
                name: "Euro Coin".into(),
                symbol: "EURC".into(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse { minter: owner.to_string(), cap: None }),
                marketing: None,
            },
            &[],
            "eurc",
            None,
        )
        .unwrap();

    let usdc = app
        .instantiate_contract(
            cw20_id,
            owner.clone(),
            &Cw20Init {
                name: "USD Coin".into(),
                symbol: "USDC".into(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse { minter: owner.to_string(), cap: None }),
                marketing: None,
            },
            &[],
            "usdc",
            None,
        )
        .unwrap();

    let router = app
        .instantiate_contract(
            router_id,
            owner.clone(),
            &RouterInit {
                rate: "1000".to_string(),
                fail_swaps,
            },
            &[],
            "router",
            None,
        )
        .unwrap();

    app.execute_contract(
        owner.clone(),
        core.clone(),
        &CoreExec::SetEutc { eutc: eutc.to_string() },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        core.clone(),
        &CoreExec::RegisterCollateral {
            collateral_type: CollateralType::Eurc,
            token: eurc.to_string(),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        core.clone(),
        &CoreExec::RegisterCollateral {
            collateral_type: CollateralType::Usdc,
            token: usdc.to_string(),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        vault.clone(),
        &VaultExec::RegisterCore { core: core.to_string() },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        reserve.clone(),
        &csm_reserve::msg::ExecuteMsg::RegisterToken {
            collateral_type: CollateralType::Eurc,
            token: eurc.to_string(),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        reserve.clone(),
        &csm_reserve::msg::ExecuteMsg::RegisterToken {
            collateral_type: CollateralType::Usdc,
            token: usdc.to_string(),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        oracle.clone(),
        &OracleExecuteMsg::SetFeeder { feeder: feeder.clone(), authorized: true },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        reserve.clone(),
        &csm_reserve::msg::ExecuteMsg::SetRouter { router: router.to_string() },
        &[],
    )
    .unwrap();
    app.execute_contract(
        owner.clone(),
        reserve.clone(),
        &csm_reserve::msg::ExecuteMsg::SetCore { core: core.to_string() },
        &[],
    )
    .unwrap();
    // The mock router pays buybacks from its own bank balance.
    app.sudo(cw_multi_test::SudoMsg::Bank(cw_multi_test::BankSudo::Mint {
        to_address: router.to_string(),
        amount: cosmwasm_std::coins(1_000_000_000_000, LUNC_DENOM),
    }))
    .unwrap();

    Chain {
        app,
        owner,
        user,
        feeder,
        eutc,
        eurc,
        usdc,
        vault,
        oracle,
        reserve,
        core,
    }
}

fn post_price(chain: &mut Chain, quote: CollateralType, price: &str) {
    chain
        .app
        .execute_contract(
            chain.feeder.clone(),
            chain.oracle.clone(),
            &OracleExecuteMsg::PostPrice {
                quote,
                units_per_eutc: price.parse().unwrap(),
            },
            &[],
        )
        .unwrap();
}

fn mint_via_send(chain: &mut Chain, collateral: CollateralType, amount: u128) {
    mint_via_send_res(chain, collateral, amount).unwrap();
}

fn mint_via_send_res(
    chain: &mut Chain,
    collateral: CollateralType,
    amount: u128,
) -> anyhow::Result<cw_multi_test::AppResponse> {
    let token = match collateral {
        CollateralType::Eurc => &chain.eurc,
        CollateralType::Usdc => &chain.usdc,
    };
    chain.app.execute_contract(
        chain.user.clone(),
        token.clone(),
        &Cw20Exec::Send {
            contract: chain.core.to_string(),
            amount: Uint128::new(amount),
            msg: to_json_binary(&HookMsg::Mint { collateral_type: collateral }).unwrap(),
        },
        &[],
    )
}

fn eutc_balance(chain: &Chain, addr: &Addr) -> Uint128 {
    chain
        .app
        .wrap()
        .query_wasm_smart::<cw20::BalanceResponse>(
            &chain.eutc,
            &Cw20Query::Balance { address: addr.to_string() },
        )
        .unwrap()
        .balance
}

fn protocol_info(chain: &Chain) -> ProtocolInfo {
    chain
        .app
        .wrap()
        .query_wasm_smart(&chain.core, &csm_std::QueryMsg::ProtocolInfo {})
        .unwrap()
}

// [spec test] EURC minting at 1:1 with the 1.5% fee routed to the reserve.
#[test]
fn mint_eurc_one_to_one_with_fee() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 10_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 10_000_000);

    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(10_000_000));

    let info = protocol_info(&c);
    let (eurc_state,) = info
        .collateral
        .iter()
        .find(|(ct, _)| *ct == CollateralType::Eurc)
        .map(|(_, st)| (st,))
        .unwrap();
    assert_eq!(eurc_state.primary_balance, Uint128::new(9_850_000));
    assert_eq!(eurc_state.fees_collected, Uint128::new(150_000));

    let balances = c
        .app
        .wrap()
        .query_wasm_smart::<csm_std::reserve::ReserveBalancesResponse>(
            &c.reserve,
            &csm_std::reserve::ReserveQueryMsg::Balances {},
        )
        .unwrap();
    let eurc_fee = balances
        .balances
        .iter()
        .find(|(ct, _)| *ct == CollateralType::Eurc)
        .unwrap();
    assert_eq!(eurc_fee.1, Uint128::new(150_000));
}

// [spec test] USDC minting applies the 0.5% premium: EUTC = USDC x 0.995.
#[test]
fn mint_usdc_applies_premium() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Usdc, "1.0");
    mint_cw20(&mut c.app, &c.usdc, &c.owner, &c.user, 1_000_000);
    mint_via_send(&mut c, CollateralType::Usdc, 1_000_000);

    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(995_000));
}

// [spec test] Oracle rate changes flow through to mint math.
#[test]
fn mint_uses_current_oracle_rate() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Usdc, "1.05");
    mint_cw20(&mut c.app, &c.usdc, &c.owner, &c.user, 1_050_000);
    mint_via_send(&mut c, CollateralType::Usdc, 1_050_000);

    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(995_000));
}

// [spec test] Stale oracle data must fail minting closed. Runs in
// AdapterOnly mode: under ConsensusPrimary the EURC rate is denomination
// identity (1:1 by construction) and would bypass the feeder entirely —
// adapter freshness is only observable with the adapter as the source.
#[test]
fn mint_fails_when_oracle_stale() {
    let mut c = instantiate_chain(false);
    c.app
        .execute_contract(
            c.owner.clone(),
            c.core.clone(),
            &CoreExec::SetOracleMode { mode: csm_std::OracleMode::AdapterOnly },
            &[],
        )
        .unwrap();
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 1_000_000);

    c.app
        .update_block(|b| b.time = b.time.plus_seconds(600));

    let err = c
        .app
        .execute_contract(
            c.user.clone(),
            c.eurc.clone(),
            &Cw20Exec::Send {
                contract: c.core.to_string(),
                amount: Uint128::new(1_000_000),
                msg: to_json_binary(&HookMsg::Mint { collateral_type: CollateralType::Eurc }).unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert!(format!("{:?}", err).contains("stale"), "actual: {:?}", err);

    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_via_send(&mut c, CollateralType::Eurc, 1_000_000);
    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(1_000_000));
}

// [spec test] Redemption burns EUTC, pays the same collateral minus the fee,
// and leaves other collateral pools untouched.
#[test]
fn redeem_eurc_burns_and_returns_same_collateral() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    post_price(&mut c, CollateralType::Usdc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 10_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 10_000_000);

    // Lift the daily cap to 100% for this test (still fee-correct math),
    // drop the per-address cap so it does not bind, and disable the flash
    // fee — this test exercises pure fee accounting, not the rate limits
    // (which have dedicated tests).
    let params = csm_std::Params {
        daily_redeem_cap_bps: 10_000,
        per_addr_redeem_cap_bps: 10_000,
        flash_fee_bps: 0,
        ..Default::default()
    };
    c.app
        .execute_contract(
            c.owner.clone(),
            c.core.clone(),
            &CoreExec::SetParams { params },
            &[],
        )
        .unwrap();

    // Day 2: caps reference the day-start snapshot (10M supply → 100% caps
    // = 10M), so the rate limits never bind here — pure fee accounting.
    c.app
        .update_block(|b| {
            b.height += 1;
            b.time = b.time.plus_seconds(86_400);
        });
    post_price(&mut c, CollateralType::Eurc, "1.0");

    c.app
        .execute_contract(
            c.user.clone(),
            c.eutc.clone(),
            &Cw20Exec::Send {
                contract: c.core.to_string(),
                amount: Uint128::new(5_000_000),
                msg: to_json_binary(&HookMsg::Redeem { collateral_type: CollateralType::Eurc }).unwrap(),
            },
            &[],
        )
        .unwrap();

    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(5_000_000));

    let eurc_back = c
        .app
        .wrap()
        .query_wasm_smart::<cw20::BalanceResponse>(
            &c.eurc,
            &Cw20Query::Balance { address: c.user.to_string() },
        )
        .unwrap()
        .balance;
    assert_eq!(eurc_back, Uint128::new(4_925_000));

    let usdc_back = c
        .app
        .wrap()
        .query_wasm_smart::<cw20::BalanceResponse>(
            &c.usdc,
            &Cw20Query::Balance { address: c.user.to_string() },
        )
        .unwrap()
        .balance;
    assert_eq!(usdc_back, Uint128::zero());

    let info = protocol_info(&c);
    assert_eq!(info.eutc_total_supply, Uint128::new(5_000_000));
    assert_eq!(
        info.reserve_lunc_vault,
        Uint128::zero()
    );
    let (_, eurc_state) = info
        .collateral
        .iter()
        .find(|(ct, _)| *ct == CollateralType::Eurc)
        .unwrap();    // 10,000,000 deposited → 150,000 mint fee to reserve → 9,850,000 in
    // custody; redeeming 5,000,000 EUTC pulls 5,000,000 out of custody.
    assert_eq!(eurc_state.primary_balance, Uint128::new(4_850_000));
    let reserve_balances = c
        .app
        .wrap()
        .query_wasm_smart::<csm_std::reserve::ReserveBalancesResponse>(
            &c.reserve,
            &csm_std::reserve::ReserveQueryMsg::Balances {},
        )
        .unwrap();
    // Reserve holds the 150,000 mint fee (1.5% of the deposit) plus the
    // 75,000 redeem fee (1.5% of the payout collateral).
    assert_eq!(
        reserve_balances.balances,
        vec![(CollateralType::Eurc, Uint128::new(225_000))]
    );
}

// [spec test] No more than 10% of supply can be redeemed per day; the window
// resets after a day. Caps reference the day-start supply snapshot, so the
// minting must happen on day 1 and the redemptions on day 2 (on day 1 the
// cap equals the bootstrap floor, which would never bind).
#[test]
fn daily_redemption_cap_enforced_and_resets() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 10_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 10_000_000);

    let params = csm_std::Params {
        per_addr_redeem_cap_bps: 1_000,
        ..Default::default()
    };
    c.app
        .execute_contract(
            c.owner.clone(),
            c.core.clone(),
            &CoreExec::SetParams { params },
            &[],
        )
        .unwrap();

    // Day 2: the day-start snapshot is now 10M → global cap 1M/day and
    // per-address cap 1M/day (per-addr raised to the same 1000 bps so only
    // the GLOBAL window's consumption pattern is exercised here; validation
    // rejects per-addr > daily).

    let redeem = |c: &mut Chain, amount: u128| {
        c.app
            .execute_contract(
                c.user.clone(),
                c.eutc.clone(),
                &Cw20Exec::Send {
                    contract: c.core.to_string(),
                    amount: Uint128::new(amount),
                    msg: to_json_binary(&HookMsg::Redeem { collateral_type: CollateralType::Eurc }).unwrap(),
                },
                &[],
            )
    };

    // Advance to day 2 first: the day-1 snapshot (0 supply) would pin both
    // caps to the bootstrap floor; day-2's snapshot of 10M makes the 10%
    // daily cap actually bind at 1M.
    c.app
        .update_block(|b| {
            b.height += 1;
            b.time = b.time.plus_seconds(86_400);
        });
    post_price(&mut c, CollateralType::Eurc, "1.0");

    redeem(&mut c, 1_000_000).unwrap();
    let err = redeem(&mut c, 500_000).unwrap_err();
    assert!(format!("{:?}", err).contains("daily redemption cap"), "actual: {:?}", err);

    c.app
        .update_block(|b| {
            b.height += 1;
            b.time = b.time.plus_seconds(86_400);
        });
    // A new day also means the old price is stale again.
    post_price(&mut c, CollateralType::Eurc, "1.0");
    // Day-3 snapshot: 9M supply → cap 900k; redeeming 500k fits.
    redeem(&mut c, 500_000).unwrap();
    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(8_500_000));
}

// [spec test] Emergency pause/resume on both knobs independently.
#[test]
fn pause_blocks_mint_and_redeem_then_resume() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 1_000_000);

    c.app
        .execute_contract(
            c.owner.clone(),
            c.core.clone(),
            &CoreExec::Pause { mint: true, redeem: false },
            &[],
        )
        .unwrap();
    let err = c
        .app
        .execute_contract(
            c.user.clone(),
            c.eurc.clone(),
            &Cw20Exec::Send {
                contract: c.core.to_string(),
                amount: Uint128::new(500_000),
                msg: to_json_binary(&HookMsg::Mint { collateral_type: CollateralType::Eurc }).unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert!(format!("{:?}", err).contains("minting is paused"), "actual: {:?}", err);

    c.app
        .execute_contract(
            c.owner.clone(),
            c.core.clone(),
            &CoreExec::Pause { mint: false, redeem: false },
            &[],
        )
        .unwrap();
    mint_via_send(&mut c, CollateralType::Eurc, 500_000);
    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(500_000));
}

// [spec test] Permission checks: only the admin (governance/multisig) can
// change parameters; the vault only releases funds to core.
#[test]
fn admin_and_core_permissions_enforced() {
    let mut c = instantiate_chain(false);

    let err = c
        .app
        .execute_contract(
            c.user.clone(),
            c.core.clone(),
            &CoreExec::SetParams { params: Params::default() },
            &[],
        )
        .unwrap_err();
    assert!(format!("{:?}", err).contains("not the admin"));

    let err = c
        .app
        .execute_contract(
            c.owner.clone(),
            c.vault.clone(),
            &VaultExec::Withdraw {
                to: c.owner.to_string(),
                token: c.eurc.to_string(),
                amount: Uint128::new(1),
                hook: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(format!("{:?}", err).contains("restricted to csm-core"));
}

// [spec test] Vault balance immutability: mirror matches actual custody.
#[test]
fn vault_mirror_matches_custody() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 2_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 2_000_000);

    let mirror = c
        .app
        .wrap()
        .query_wasm_smart::<csm_vault::msg::BalanceResponse>(
            &c.vault,
            &csm_vault::msg::QueryMsg::Balance { token: c.eurc.to_string() },
        )
        .unwrap();
    assert_eq!(mirror.balance, Uint128::new(1_970_000));

    let custodied = c
        .app
        .wrap()
        .query_wasm_smart::<cw20::BalanceResponse>(
            &c.eurc,
            &Cw20Query::Balance { address: c.vault.to_string() },
        )
        .unwrap()
        .balance;
    assert_eq!(custodied, Uint128::new(1_970_000));
}

// [spec test] Buyback execution: reserve swaps fees for LUNC into the vault.
#[test]
fn buyback_success_updates_vault_and_ledger() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 1_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 1_000_000);

    c.app
        .execute_contract(
            c.owner.clone(),
            c.reserve.clone(),
            &csm_reserve::msg::ExecuteMsg::Buyback {
                offer: CollateralType::Eurc,
                max_slippage_bps: 500,
            },
            &[],
        )
        .unwrap();

    let vault = c
        .app
        .wrap()
        .query_wasm_smart::<csm_std::reserve::VaultResponse>(
            &c.reserve,
            &csm_std::reserve::ReserveQueryMsg::Vault {},
        )
        .unwrap();
    assert_eq!(vault.lunc, Uint128::new(15_000_000));

    let status = c
        .app
        .wrap()
        .query_wasm_smart::<csm_std::reserve::BuybackStatusResponse>(
            &c.reserve,
            &csm_std::reserve::ReserveQueryMsg::BuybackStatus {},
        )
        .unwrap();
    assert_eq!(status.buybacks_executed, 1);
    assert_eq!(status.lunc_purchased_total, Uint128::new(15_000_000));

    let balances = c
        .app
        .wrap()
        .query_wasm_smart::<csm_std::reserve::ReserveBalancesResponse>(
            &c.reserve,
            &csm_std::reserve::ReserveQueryMsg::Balances {},
        )
        .unwrap();
    let eurc_left = balances
        .balances
        .iter()
        .find(|(ct, _)| *ct == CollateralType::Eurc)
        .map(|(_, b)| *b)
        .unwrap_or_default();
    assert_eq!(eurc_left, Uint128::zero());
}

// [spec test] Buyback route failure: funds return, state records the failure,
// and a retry is allowed.
#[test]
fn buyback_failure_returns_funds_and_allows_retry() {
    let mut c = instantiate_chain(true);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 1_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 1_000_000);

    c.app
        .execute_contract(
            c.owner.clone(),
            c.reserve.clone(),
            &csm_reserve::msg::ExecuteMsg::Buyback {
                offer: CollateralType::Eurc,
                max_slippage_bps: 500,
            },
            &[],
        )
        .unwrap();

    let status = c
        .app
        .wrap()
        .query_wasm_smart::<csm_std::reserve::BuybackStatusResponse>(
            &c.reserve,
            &csm_std::reserve::ReserveQueryMsg::BuybackStatus {},
        )
        .unwrap();
    assert!(matches!(status.status, csm_std::reserve::BuybackStatus::Failed { .. }));
    assert_eq!(status.buybacks_executed, 0);

    let balances = c
        .app
        .wrap()
        .query_wasm_smart::<csm_std::reserve::ReserveBalancesResponse>(
            &c.reserve,
            &csm_std::reserve::ReserveQueryMsg::Balances {},
        )
        .unwrap();
    let eurc_left = balances
        .balances
        .iter()
        .find(|(ct, _)| *ct == CollateralType::Eurc)
        .map(|(_, b)| *b)
        .unwrap_or_default();
    assert_eq!(eurc_left, Uint128::new(15_000));
}

// [spec test] Slippage bounds must be sane; a zero bound is rejected.
#[test]
fn buyback_rejects_invalid_slippage() {
    let mut c = instantiate_chain(false);
    let err = c
        .app
        .execute_contract(
            c.owner.clone(),
            c.reserve.clone(),
            &csm_reserve::msg::ExecuteMsg::Buyback {
                offer: CollateralType::Eurc,
                max_slippage_bps: 0,
            },
            &[],
        )
        .unwrap_err();
    assert!(err.to_string().contains("slippage"));
}

// [spec test] Public diagnostics: ratio, daily cap remaining, oracle health.
#[test]
fn protocol_info_reports_diagnostics() {
    let mut c = instantiate_chain(false);
    let mut info = protocol_info(&c);
    assert!(!info.oracle_healthy);

    post_price(&mut c, CollateralType::Eurc, "1.0");
    post_price(&mut c, CollateralType::Usdc, "1.05");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 1_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 1_000_000);

    info = protocol_info(&c);
    assert!(info.oracle_healthy);
    assert_eq!(info.eutc_total_supply, Uint128::new(1_000_000));
    // E2 semantics: the ratio counts primary custody (985k) plus the
    // secondary reserve's fee balances (15k mint fee) → 100%.
    assert_eq!(info.collateral_ratio_pct, Uint128::new(100));
    assert_eq!(info.daily_redeem_remaining, Uint128::new(100_000));
    assert_eq!(info.eutc_denom, c.eutc.to_string());
}

// Sanity: reserve rejects direct stablecoin sends from non-core senders.
#[test]
fn reserve_rejects_direct_user_deposits() {
    let mut c = instantiate_chain(false);
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 1_000);
    let err = c
        .app
        .execute_contract(
            c.user.clone(),
            c.eurc.clone(),
            &Cw20Exec::Send {
                contract: c.reserve.to_string(),
                amount: Uint128::new(1_000),
                msg: to_json_binary(&ReserveExecuteMsg::Deposit {}).unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert!(format!("{:?}", err).contains("not the core module"), "actual: {:?}", err);
}

// ---- Anti-dump stack (RESEARCH-CONTEXT §10a L1–L3) dedicated coverage ----

fn event_attr<'a>(res: &'a cw_multi_test::AppResponse, key: &str) -> Option<&'a str> {
    res.events
        .iter()
        .flat_map(|e| e.attributes.iter())
        .find(|a| a.key == key)
        .map(|a| a.value.as_str())
}

// [anti-dump L1] One whale cannot consume the whole daily window: the
// per-address cap is independent — A exhausts their own allowance while B's
// stays intact.
#[test]
fn per_address_redeem_cap_bounds_single_address() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    let user_b = c.app.api().addr_make("user-b");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 5_000_000);
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &user_b, 5_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 5_000_000); // user A
    // user B mints via their own send.
    c.app
        .execute_contract(
            user_b.clone(),
            c.eurc.clone(),
            &Cw20Exec::Send {
                contract: c.core.to_string(),
                amount: Uint128::new(5_000_000),
                msg: to_json_binary(&HookMsg::Mint { collateral_type: CollateralType::Eurc }).unwrap(),
            },
            &[],
        )
        .unwrap();
    // Day 2: the day-start snapshot is 10M → global cap 1M/day (1000 bps),
    // per-address cap 100k/day (100 bps default). Day-1 redemptions would be
    // pinned to the zero-supply snapshot, so the cap checks run on day 2.
    c.app
        .update_block(|b| {
            b.height += 1;
            b.time = b.time.plus_seconds(86_400);
        });
    post_price(&mut c, CollateralType::Eurc, "1.0");

    let redeem = |c: &mut Chain, from: &Addr, amount: u128| {
        let from = from.clone();
        c.app.execute_contract(
            from.clone(),
            c.eutc.clone(),
            &Cw20Exec::Send {
                contract: c.core.to_string(),
                amount: Uint128::new(amount),
                msg: to_json_binary(&HookMsg::Redeem { collateral_type: CollateralType::Eurc }).unwrap(),
            },
            &[],
        )
    };

    // A redeems exactly their per-address allowance.
    let user_a = c.user.clone();
    redeem(&mut c, &user_a, 100_000).unwrap();
    // One more unit trips the per-address cap (NOT the global one).
    let err = redeem(&mut c, &user_a, 1).unwrap_err();
    assert!(format!("{:?}", err).contains("per-address redemption cap"), "actual: {:?}", err);
    // B's independent allowance is untouched.
    redeem(&mut c, &user_b, 100_000).unwrap();
}

// [anti-dump L1] Daily mint velocity cap: same-day issuance is bounded
// (bootstrap floor here), accumulates across mints, and resets next day.
#[test]
fn daily_mint_velocity_cap_blocks_and_resets() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    let params = csm_std::Params { min_daily_mint_floor: 1_000_000, ..Default::default() };
    c.app
        .execute_contract(c.owner.clone(), c.core.clone(), &CoreExec::SetParams { params }, &[])
        .unwrap();
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 2_000_000);

    mint_via_send(&mut c, CollateralType::Eurc, 600_000);
    let err = mint_via_send_res(&mut c, CollateralType::Eurc, 600_000).unwrap_err();
    assert!(format!("{:?}", err).contains("daily mint cap"), "actual: {:?}", err);

    // Next day the window resets.
    c.app.update_block(|b| {
        b.height += 1;
        b.time = b.time.plus_seconds(86_400);
    });
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_via_send(&mut c, CollateralType::Eurc, 600_000);
    // 600k (day 1) + 600k (day 2).
    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(1_200_000));
}

// [anti-dump L2] The redeem spread rises with daily-window utilization:
// the second redemption into the same window pays a higher fee.
#[test]
fn dynamic_spread_rises_with_window_utilization() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 10_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 10_000_000);
    let params = csm_std::Params {
        daily_redeem_cap_bps: 10_000,
        per_addr_redeem_cap_bps: 10_000,
        flash_fee_bps: 0,
        ..Default::default()
    };
    c.app
        .execute_contract(c.owner.clone(), c.core.clone(), &CoreExec::SetParams { params }, &[])
        .unwrap();

    // Day 2: caps reference the 10M day-start snapshot (100% caps → 10M),
    // so both redemptions clear the rate limits and only the spread math
    // differs between them.
    c.app
        .update_block(|b| {
            b.height += 1;
            b.time = b.time.plus_seconds(86_400);
        });
    post_price(&mut c, CollateralType::Eurc, "1.0");

    let redeem = |c: &mut Chain, amount: u128| {
        c.app.execute_contract(
            c.user.clone(),
            c.eutc.clone(),
            &Cw20Exec::Send {
                contract: c.core.to_string(),
                amount: Uint128::new(amount),
                msg: to_json_binary(&HookMsg::Redeem { collateral_type: CollateralType::Eurc }).unwrap(),
            },
            &[],
        )
    };

    // First redeem: empty window → base fee only (150 bps).
    let r1 = redeem(&mut c, 4_000_000).unwrap();
    let fee1: u64 = event_attr(&r1, "effective_fee_bps").unwrap().parse().unwrap();
    assert_eq!(fee1, 150);
    // Second redeem: window at 40% → +400 bps of headroom → 550 bps.
    let r2 = redeem(&mut c, 4_000_000).unwrap();
    let fee2: u64 = event_attr(&r2, "effective_fee_bps").unwrap().parse().unwrap();
    assert_eq!(fee2, 550);
    assert!(fee2 > fee1);
}

// [anti-dump L2] Flash fee: redeeming freshly minted units pays an extra
// fee; after the window passes, the fee is base-only.
#[test]
fn flash_fee_punishes_immediate_redeem_loop() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 2_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 2_000_000);
    let params = csm_std::Params {
        daily_redeem_cap_bps: 10_000,
        per_addr_redeem_cap_bps: 10_000,
        ..Default::default()
    };
    c.app
        .execute_contract(c.owner.clone(), c.core.clone(), &CoreExec::SetParams { params }, &[])
        .unwrap();

    // Day 2: the redeem cap references the 2M day-start snapshot (100% →
    // 2M allowance), so the cap never binds and only the flash fee shows.
    c.app
        .update_block(|b| {
            b.height += 1;
            b.time = b.time.plus_seconds(86_400);
        });
    post_price(&mut c, CollateralType::Eurc, "1.0");

    let redeem = |c: &mut Chain, amount: u128| {
        c.app.execute_contract(
            c.user.clone(),
            c.eutc.clone(),
            &Cw20Exec::Send {
                contract: c.core.to_string(),
                amount: Uint128::new(amount),
                msg: to_json_binary(&HookMsg::Redeem { collateral_type: CollateralType::Eurc }).unwrap(),
            },
            &[],
        )
    };

    // First redeem (fresh day-2 blocks): flash window active →
    // 150 base + 300 flash = 450 bps, utilization 0.
    let r1 = redeem(&mut c, 500_000).unwrap();
    assert_eq!(event_attr(&r1, "flash_fee"), Some("true"));
    let fee1: u64 = event_attr(&r1, "effective_fee_bps").unwrap().parse().unwrap();
    assert_eq!(fee1, 450);

    // Beyond the 30-block window: flash expired, but the window now sits at
    // 25% utilization (500k/2M) → +250 bps spread → 400 bps total.
    c.app.update_block(|b| b.height += 31);
    let r2 = redeem(&mut c, 500_000).unwrap();
    assert_eq!(event_attr(&r2, "flash_fee"), Some("false"));
    let fee2: u64 = event_attr(&r2, "effective_fee_bps").unwrap().parse().unwrap();
    assert_eq!(fee2, 400);
}

// [anti-dump L3 / E2] The solvency governor halts mints when the primary
// ratio falls below the governance floor, and releases them when raised
// parameters accommodate the (fee-diluted) steady state.
#[test]
fn ratio_governor_halts_mints_below_floor() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    // 10M for the initial mint + 1M still to spend in the relaxed step.
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, 11_000_000);
    mint_via_send(&mut c, CollateralType::Eurc, 10_000_000);
    // Steady state: custody 9.85M / minted 10M = 9850 bps (fee skim).

    // Floor above steady state → mints halted.
    let strict = csm_std::Params { min_primary_ratio_bps: 9_900, ..Default::default() };
    c.app
        .execute_contract(c.owner.clone(), c.core.clone(), &CoreExec::SetParams { params: strict }, &[])
        .unwrap();
    let err = mint_via_send_res(&mut c, CollateralType::Eurc, 1_000_000).unwrap_err();
    assert!(format!("{:?}", err).contains("ratio below governance floor"), "actual: {:?}", err);

    // Floor below steady state → mints flow again.
    let relaxed = csm_std::Params { min_primary_ratio_bps: 9_800, ..Default::default() };
    c.app
        .execute_contract(c.owner.clone(), c.core.clone(), &CoreExec::SetParams { params: relaxed }, &[])
        .unwrap();
    mint_via_send(&mut c, CollateralType::Eurc, 1_000_000);
    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(11_000_000));
}

// [combined attack scenario] The full mint→dump→redeem wash loop, executed
// honestly against the contract, LOSES money even with zero price movement:
// flash fee + base fee + the mint fee make the round trip cost 4.5%.
#[test]
fn attack_scenario_mint_dump_redeem_loses_money() {
    let mut c = instantiate_chain(false);
    post_price(&mut c, CollateralType::Eurc, "1.0");
    let params = csm_std::Params {
        daily_redeem_cap_bps: 10_000,
        per_addr_redeem_cap_bps: 10_000,
        ..Default::default()
    };
    c.app
        .execute_contract(c.owner.clone(), c.core.clone(), &CoreExec::SetParams { params }, &[])
        .unwrap();

    let attacker_start = 1_000_000u128;
    mint_cw20(&mut c.app, &c.eurc, &c.owner, &c.user, attacker_start);
    // Step 1 (day 1): mint 1M EUTC against 1M EURC (mint fee 15k stays in
    // protocol).
    mint_via_send(&mut c, CollateralType::Eurc, attacker_start);
    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(attacker_start));
    // Step 2 (day 2): dump the whole position back through redemption. The
    // redeem cap references the day-start snapshot, so day-1 redemptions
    // (zero supply) are impossible by construction.
    c.app
        .update_block(|b| {
            b.height += 1;
            b.time = b.time.plus_seconds(86_400);
        });
    post_price(&mut c, CollateralType::Eurc, "1.0");
    let r = c
        .app
        .execute_contract(
            c.user.clone(),
            c.eutc.clone(),
            &Cw20Exec::Send {
                contract: c.core.to_string(),
                amount: Uint128::new(985_000),
                msg: to_json_binary(&HookMsg::Redeem { collateral_type: CollateralType::Eurc }).unwrap(),
            },
            &[],
        )
        .unwrap();
    // Flash fee (fresh mint) + base fee on a cold window.
    assert_eq!(event_attr(&r, "flash_fee"), Some("true"));
    let fee_bps: u64 = event_attr(&r, "effective_fee_bps").unwrap().parse().unwrap();
    assert_eq!(fee_bps, 450);

    // Net position: spent 1,000,000 EURC; got back 985,000 × (1 − 0.045) =
    // 940,675 → a 5.9% loss. The residual 15,000 EUTC is unbacked residue
    // (the mint fee haircut) that cannot be redeemed until fresh collateral
    // arrives — the wash loop burns money twice over.
    let eurc_left = c
        .app
        .wrap()
        .query_wasm_smart::<cw20::BalanceResponse>(
            &c.eurc,
            &Cw20Query::Balance { address: c.user.to_string() },
        )
        .unwrap()
        .balance;
    assert_eq!(eurc_left, Uint128::new(940_675));
    assert_eq!(eutc_balance(&c, &c.user), Uint128::new(15_000));
    // Every loop is a donation to the protocol's reserve and fee sinks.
}

// Silence unused warnings for helpers kept for readability.
#[allow(dead_code)]
fn unused(_: Coin) {}
