use crate::contract::{execute, instantiate, query};
use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR,
};
use cosmwasm_std::{
    from_binary, to_binary, BankMsg, Coin, CosmosMsg, Decimal, OwnedDeps, Response, StdError,
    SubMsg, Uint128, WasmMsg,
};
use cw20::{BalanceResponse, MinterResponse, TokenInfoResponse};
use cw20_legacy::{
    msg::{ExecuteMsg, QueryMsg},
    ContractError,
};
use gohm_staking::reward_token::InstantiateMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        name: "gOHM reward token".to_string(),
        symbol: "rgOHM".to_string(),
        decimals: 6u8,
        minter: "minter".to_string(),
        gohm_token: "gohm_token".to_string(),
        denom: "uluna".to_string(),
        gohm_rate: Decimal::percent(1000),
        denom_rate: Decimal::percent(10),
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the token_info
    let res = query(deps.as_ref(), mock_env(), QueryMsg::TokenInfo {}).unwrap();
    let token_info: TokenInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "gOHM reward token".to_string(),
            symbol: "rgOHM".to_string(),
            decimals: 6u8,
            total_supply: Uint128::zero()
        }
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Minter {}).unwrap();
    let minter_res: MinterResponse = from_binary(&res).unwrap();
    assert_eq!(
        minter_res,
        MinterResponse {
            minter: "minter".to_string(),
            cap: None,
        }
    );
}

#[test]
fn test_mint_tokens_fails_if_unauthorized() {
    let mut deps = mock_dependencies(&[]);

    let (_, denom_rate) = initialize_reward_token(&mut deps, None, None);

    let amount = Uint128::from(1000000u128);
    let msg = ExecuteMsg::Mint {
        recipient: "recipient".to_string(),
        amount,
    };

    let info = mock_info(
        "addr0001",
        &[Coin {
            denom: "uluna".to_string(),
            amount: amount * denom_rate,
        }],
    );

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    assert_eq!(res, ContractError::Unauthorized {});
}

#[test]
fn test_mint_tokens_fails_if_several_denoms_received() {
    let mut deps = mock_dependencies(&[]);

    let (_, denom_rate) = initialize_reward_token(&mut deps, None, None);

    let amount = Uint128::from(1000000u128);
    let msg = ExecuteMsg::Mint {
        recipient: "recipient".to_string(),
        amount,
    };

    let info = mock_info(
        "minter",
        &[
            Coin {
                denom: "uluna".to_string(),
                amount: amount * denom_rate,
            },
            Coin {
                denom: "uust".to_string(),
                amount: amount * denom_rate,
            },
        ],
    );

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    assert_eq!(
        res,
        ContractError::Std(StdError::generic_err("Cannot receive several denoms",))
    );
}

#[test]
fn test_mint_tokens_fails_if_received_denom_is_invalid() {
    let mut deps = mock_dependencies(&[]);

    let (_, _) = initialize_reward_token(&mut deps, None, None);

    let amount = Uint128::from(1000000u128);
    let msg = ExecuteMsg::Mint {
        recipient: "recipient".to_string(),
        amount,
    };

    let info = mock_info(
        "mint",
        &[Coin {
            denom: "uluna".to_string(),
            amount: Uint128::from(1000u128),
        }],
    );

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    assert_eq!(
        res,
        ContractError::Std(StdError::generic_err("Invalid denom amount",))
    );
}

#[test]
fn test_mint_tokens_when_all_rates_are_not_zero() {
    let mut deps = mock_dependencies(&[]);

    let (gohm_rate, denom_rate) = initialize_reward_token(&mut deps, None, None);

    let amount = Uint128::from(1000000u128);

    let (res, gohm_amount, _) = mint_token(
        &mut deps,
        gohm_rate,
        denom_rate,
        amount,
        "recipient".to_string(),
    );

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "gohm_token".to_string(),
            msg: to_binary(&ExecuteMsg::TransferFrom {
                owner: "minter".to_string(),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
                amount: gohm_amount,
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::TokenInfo {}).unwrap();
    let token_info: TokenInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "gOHM reward token".to_string(),
            symbol: "rgOHM".to_string(),
            decimals: 6u8,
            total_supply: amount
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Balance {
            address: "recipient".to_string(),
        },
    )
    .unwrap();
    let balance: BalanceResponse = from_binary(&res).unwrap();
    assert_eq!(balance, BalanceResponse { balance: amount });
}

#[test]
fn test_mint_tokens_when_gohm_rate_is_zero() {
    let mut deps = mock_dependencies(&[]);

    let (_, denom_rate) = initialize_reward_token(&mut deps, Some(Decimal::zero()), None);

    let amount = Uint128::from(1000000u128);

    let (res, _, _) = mint_token(
        &mut deps,
        Decimal::zero(),
        denom_rate,
        amount,
        "recipient".to_string(),
    );

    assert_eq!(res.messages, vec![]);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::TokenInfo {}).unwrap();
    let token_info: TokenInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "gOHM reward token".to_string(),
            symbol: "rgOHM".to_string(),
            decimals: 6u8,
            total_supply: amount
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Balance {
            address: "recipient".to_string(),
        },
    )
    .unwrap();
    let balance: BalanceResponse = from_binary(&res).unwrap();
    assert_eq!(balance, BalanceResponse { balance: amount });
}

#[test]
fn test_mint_tokens_when_denom_rate_is_zero() {
    let mut deps = mock_dependencies(&[]);

    let (gohm_rate, _) = initialize_reward_token(&mut deps, None, Some(Decimal::zero()));

    let amount = Uint128::from(1000000u128);

    let (res, gohm_amount, _) = mint_token(
        &mut deps,
        gohm_rate,
        Decimal::zero(),
        amount,
        "recipient".to_string(),
    );

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "gohm_token".to_string(),
            msg: to_binary(&ExecuteMsg::TransferFrom {
                owner: "minter".to_string(),
                recipient: MOCK_CONTRACT_ADDR.to_string(),
                amount: gohm_amount,
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::TokenInfo {}).unwrap();
    let token_info: TokenInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "gOHM reward token".to_string(),
            symbol: "rgOHM".to_string(),
            decimals: 6u8,
            total_supply: amount
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Balance {
            address: "recipient".to_string(),
        },
    )
    .unwrap();
    let balance: BalanceResponse = from_binary(&res).unwrap();
    assert_eq!(balance, BalanceResponse { balance: amount });
}

// helper
fn initialize_reward_token(
    deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier>,
    gohm_rate: Option<Decimal>,
    denom_rate: Option<Decimal>,
) -> (Decimal, Decimal) {
    let gohm_rate = if let Some(gohm_rate) = gohm_rate {
        gohm_rate
    } else {
        Decimal::percent(1000)
    };

    let denom_rate = if let Some(denom_rate) = denom_rate {
        denom_rate
    } else {
        Decimal::percent(10)
    };

    let msg = InstantiateMsg {
        name: "gOHM reward token".to_string(),
        symbol: "rgOHM".to_string(),
        decimals: 6u8,
        minter: "minter".to_string(),
        gohm_token: "gohm_token".to_string(),
        denom: "uluna".to_string(),
        gohm_rate,
        denom_rate,
    };

    let info = mock_info("addr", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    (gohm_rate, denom_rate)
}

#[test]
fn test_burn_tokens_fails_if_no_balance() {
    let mut deps = mock_dependencies(&[]);

    let (gohm_rate, denom_rate) = initialize_reward_token(&mut deps, None, None);

    let mint_amount = Uint128::from(1000000u128);
    mint_token(
        &mut deps,
        gohm_rate,
        denom_rate,
        mint_amount,
        "recipient".to_string(),
    );

    let amount = Uint128::from(1000000u128);

    let msg = ExecuteMsg::Burn { amount };

    let info = mock_info("addr0000", &[]);

    execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
}

#[test]
fn test_burn_tokens_when_all_rates_are_not_zero() {
    let mut deps = mock_dependencies(&[]);

    let (gohm_rate, denom_rate) = initialize_reward_token(&mut deps, None, None);

    let mint_amount = Uint128::from(1000000u128);
    mint_token(
        &mut deps,
        gohm_rate,
        denom_rate,
        mint_amount,
        "recipient".to_string(),
    );

    let amount = Uint128::from(1000000u128);

    let res = burn_token(&mut deps, amount, "recipient".to_string());

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "recipient".to_string(),
                amount: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: amount * denom_rate
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "gohm_token".to_string(),
                msg: to_binary(&ExecuteMsg::Transfer {
                    recipient: "recipient".to_string(),
                    amount: amount * gohm_rate,
                })
                .unwrap(),
                funds: vec![],
            })),
        ]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::TokenInfo {}).unwrap();
    let token_info: TokenInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "gOHM reward token".to_string(),
            symbol: "rgOHM".to_string(),
            decimals: 6u8,
            total_supply: mint_amount - amount
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Balance {
            address: "recipient".to_string(),
        },
    )
    .unwrap();
    let balance: BalanceResponse = from_binary(&res).unwrap();
    assert_eq!(
        balance,
        BalanceResponse {
            balance: mint_amount - amount
        }
    );
}

#[test]
fn test_burn_tokens_when_gohm_rate_is_zero() {
    let mut deps = mock_dependencies(&[]);

    let (gohm_rate, denom_rate) = initialize_reward_token(&mut deps, Some(Decimal::zero()), None);

    let mint_amount = Uint128::from(1000000u128);
    mint_token(
        &mut deps,
        gohm_rate,
        denom_rate,
        mint_amount,
        "recipient".to_string(),
    );

    let amount = Uint128::from(1000000u128);

    let res = burn_token(&mut deps, amount, "recipient".to_string());

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "recipient".to_string(),
            amount: vec![Coin {
                denom: "uluna".to_string(),
                amount: amount * denom_rate
            }],
        })),]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::TokenInfo {}).unwrap();
    let token_info: TokenInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "gOHM reward token".to_string(),
            symbol: "rgOHM".to_string(),
            decimals: 6u8,
            total_supply: mint_amount - amount
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Balance {
            address: "recipient".to_string(),
        },
    )
    .unwrap();
    let balance: BalanceResponse = from_binary(&res).unwrap();
    assert_eq!(
        balance,
        BalanceResponse {
            balance: mint_amount - amount
        }
    );
}

#[test]
fn test_burn_tokens_when_denom_rate_is_zero() {
    let mut deps = mock_dependencies(&[]);

    let (gohm_rate, denom_rate) = initialize_reward_token(&mut deps, None, Some(Decimal::zero()));

    let mint_amount = Uint128::from(1000000u128);
    mint_token(
        &mut deps,
        gohm_rate,
        denom_rate,
        mint_amount,
        "recipient".to_string(),
    );

    let amount = Uint128::from(1000000u128);

    let res = burn_token(&mut deps, amount, "recipient".to_string());

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "gohm_token".to_string(),
            msg: to_binary(&ExecuteMsg::Transfer {
                recipient: "recipient".to_string(),
                amount: amount * gohm_rate,
            })
            .unwrap(),
            funds: vec![],
        })),]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::TokenInfo {}).unwrap();
    let token_info: TokenInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "gOHM reward token".to_string(),
            symbol: "rgOHM".to_string(),
            decimals: 6u8,
            total_supply: mint_amount - amount
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Balance {
            address: "recipient".to_string(),
        },
    )
    .unwrap();
    let balance: BalanceResponse = from_binary(&res).unwrap();
    assert_eq!(
        balance,
        BalanceResponse {
            balance: mint_amount - amount
        }
    );
}

fn mint_token(
    deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier>,
    gohm_rate: Decimal,
    denom_rate: Decimal,
    amount: Uint128,
    recipient: String,
) -> (Response, Uint128, Uint128) {
    let msg = ExecuteMsg::Mint { recipient, amount };

    let info = mock_info(
        "minter",
        &[Coin {
            denom: "uluna".to_string(),
            amount: amount * denom_rate,
        }],
    );

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    (res, amount * gohm_rate, amount * denom_rate)
}

fn burn_token(
    deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier>,
    amount: Uint128,
    sender: String,
) -> Response {
    let msg = ExecuteMsg::Burn { amount };

    let info = mock_info(&sender, &[]);

    execute(deps.as_mut(), mock_env(), info, msg).unwrap()
}
