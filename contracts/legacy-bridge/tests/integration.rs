//! Integration tests for the legacy-unit conversion bridge: the Convert
//! flow (caps, sink delivery, disclosure), vesting, and admin controls.

use cosmwasm_std::{coins, Addr, Empty, Uint128};
use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor};

use legacy_bridge::msg::{
    ClaimResponse, ExecuteMsg, InstantiateMsg, QueryMsg, TotalsResponse,
};

fn contract_bridge() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        legacy_bridge::contract::execute,
        legacy_bridge::contract::instantiate,
        legacy_bridge::contract::query,
    ))
}

struct Chain {
    app: App,
    owner: Addr,
    user: Addr,
    sink: Addr,
    bridge: Addr,
}

fn instantiate_chain(open: bool) -> Chain {
    let mut app = AppBuilder::new().build(|_, _, _| {});
    let owner = app.api().addr_make("owner");
    let user = app.api().addr_make("user");
    let sink = app.api().addr_make("sink");

    app.init_modules(|router, _, storage| {
        // The foreign-denom test attaches ibc/ABCDEF, so fund both denoms
        // (multi-test's bank burns attached funds before the contract runs;
        // init_balance replaces the full balance, so pass one combined list).
        router
            .bank
            .init_balance(
                storage,
                &user,
                vec![
                    cosmwasm_std::Coin::new(10_000_000u128, "ueur"),
                    cosmwasm_std::Coin::new(1_000_000u128, "ibc/ABCDEF"),
                ],
            )
            .unwrap();
    });

    let code = app.store_code(contract_bridge());
    let bridge = app
        .instantiate_contract(
            code,
            owner.clone(),
            &InstantiateMsg {
                admin: owner.to_string(),
                sink_address: sink.to_string(),
                vesting_days: 1,
                open,
            },
            &[],
            "legacy-bridge",
            None,
        )
        .unwrap();

    Chain { app, owner, user, sink, bridge }
}

fn convert(
    c: &mut Chain,
    amount: u128,
    denom: &str,
) -> anyhow::Result<cw_multi_test::AppResponse> {
    c.app.execute_contract(
        c.user.clone(),
        c.bridge.clone(),
        &ExecuteMsg::Convert {},
        &coins(amount, denom),
    )
}

fn claim_response(c: &Chain) -> ClaimResponse {
    c.app
        .wrap()
        .query_wasm_smart(
            &c.bridge,
            &QueryMsg::Claim { address: c.user.to_string(), denom: "ueur".to_string() },
        )
        .unwrap()
}

// [anti-dump L5] A conversion records a vesting claim, forwards the coins to
// the sink permanently, and emits the no-promise-of-value disclosure.
#[test]
fn convert_records_claim_forwards_to_sink_and_discloses() {
    let mut c = instantiate_chain(true);
    let res = convert(&mut c, 5_000_000, "ueur").unwrap();

    // Sink received the coins; the bridge holds nothing.
    let sink_bal = c
        .app
        .wrap()
        .query_balance(&c.sink, "ueur")
        .unwrap()
        .amount;
    assert_eq!(sink_bal, Uint128::new(5_000_000));
    let bridge_bal = c
        .app
        .wrap()
        .query_balance(&c.bridge, "ueur")
        .unwrap()
        .amount;
    assert_eq!(bridge_bal, Uint128::zero());

    // Disclosure attribute present and honest.
    let mut attrs = res.events.iter().flat_map(|e| e.attributes.iter());
    assert!(attrs.any(|a| a.key == "disclosure" && a.value.contains("NOT a promise of value")));

    // Claim recorded with correct totals.
    let claim = claim_response(&c);
    let info = claim.claim.expect("claim recorded");
    assert_eq!(info.total, "5000000");
    assert_eq!(info.claimed, "0");
    assert!(claim.program_open);

    // Global totals tracked.
    let totals: TotalsResponse = c
        .app
        .wrap()
        .query_wasm_smart(
            &c.bridge,
            &QueryMsg::ConvertedTotals { denom: "ueur".to_string() },
        )
        .unwrap();
    assert_eq!(totals.converted, "5000000");
}

// Only legacy forex denoms are convertible, and the program switch gates it.
#[test]
fn convert_rejects_closed_program_and_foreign_denom() {
    let mut c = instantiate_chain(false);
    let err = convert(&mut c, 1_000_000, "ueur").unwrap_err();
    assert!(format!("{err:?}").contains("not open"), "actual: {err:?}");

    c.app
        .execute_contract(
            c.owner.clone(),
            c.bridge.clone(),
            &ExecuteMsg::SetOpen { open: true },
            &[],
        )
        .unwrap();

    let err = convert(&mut c, 1_000_000, "ibc/ABCDEF").unwrap_err();
    assert!(format!("{err:?}").contains("not a convertible legacy"), "actual: {err:?}");
}

// [anti-dump L5] The per-address cap bounds one whale's total conversion.
#[test]
fn per_address_cap_blocks_excess() {
    let mut c = instantiate_chain(true);
    c.app
        .execute_contract(
            c.owner.clone(),
            c.bridge.clone(),
            &ExecuteMsg::SetCaps { global: 100_000_000, per_address: 6_000_000 },
            &[],
        )
        .unwrap();

    convert(&mut c, 5_000_000, "ueur").unwrap();
    let err = convert(&mut c, 2_000_000, "ueur").unwrap_err();
    assert!(format!("{err:?}").contains("per-address conversion cap"), "actual: {err:?}");
}

// Claims vest linearly and are consumable exactly once.
#[test]
fn claim_vests_linearly_and_completes() {
    let mut c = instantiate_chain(true);
    convert(&mut c, 4_000_000, "ueur").unwrap();

    // Halfway through the 1-day vesting window.
    c.app.update_block(|b| b.height += 3_600);
    let mid = claim_response(&c);
    assert_eq!(mid.vested, "2000000");

    c.app
        .execute_contract(
            c.user.clone(),
            c.bridge.clone(),
            &ExecuteMsg::Claim { denom: "ueur".to_string() },
            &[],
        )
        .unwrap();

    // Fully vested after the window; the remainder is claimable once.
    c.app.update_block(|b| b.height += 7_200);
    c.app
        .execute_contract(
            c.user.clone(),
            c.bridge.clone(),
            &ExecuteMsg::Claim { denom: "ueur".to_string() },
            &[],
        )
        .unwrap();

    // The completed claim is removed: nothing further to claim.
    let err = c
        .app
        .execute_contract(
            c.user.clone(),
            c.bridge.clone(),
            &ExecuteMsg::Claim { denom: "ueur".to_string() },
            &[],
        )
        .unwrap_err();
    assert!(format!("{err:?}").contains("nothing has vested"), "actual: {err:?}");
    let done = claim_response(&c);
    assert!(done.claim.is_none());
}

// Admin-only controls.
#[test]
fn admin_ops_require_admin() {
    let mut c = instantiate_chain(true);
    let err = c
        .app
        .execute_contract(
            c.user.clone(),
            c.bridge.clone(),
            &ExecuteMsg::SetOpen { open: false },
            &[],
        )
        .unwrap_err();
    assert!(format!("{err:?}").contains("not the admin"), "actual: {err:?}");
}
