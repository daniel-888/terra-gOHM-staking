#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128, WasmMsg,
};

use crate::state::{Config, CONFIGURATION};
use cw2::set_contract_version;
use cw20_legacy::{
    contract::{execute as cw20_execute, execute_burn, execute_mint, query as cw20_query},
    msg::{ExecuteMsg, QueryMsg},
    state::{MinterData, TokenInfo, TOKEN_INFO},
    ContractError,
};
use terraswap::asset::{Asset, AssetInfo};

use gohm_staking::reward_token::{InstantiateMsg, MigrateMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-base";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // check valid token info
    msg.validate()?;

    let mint = Some(MinterData {
        minter: deps.api.addr_canonicalize(&msg.minter)?,
        cap: None,
    });

    // store token info
    let data = TokenInfo {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply: Uint128::zero(),
        mint,
    };

    TOKEN_INFO.save(deps.storage, &data)?;

    CONFIGURATION.save(
        deps.storage,
        &Config {
            gohm_token: deps.api.addr_canonicalize(&msg.gohm_token)?,
            denom: msg.denom,
            gohm_rate: msg.gohm_rate,
            denom_rate: msg.denom_rate,
        },
    )?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Mint { recipient, amount } => try_mint(deps, env, info, recipient, amount),
        ExecuteMsg::Burn { amount } => try_burn(deps, env, info, amount),
        _ => cw20_execute(deps, env, info, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    cw20_query(deps, env, msg)
}

fn try_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIGURATION.load(deps.storage)?;

    if info.funds.len() > 1 {
        return Err(ContractError::Std(StdError::generic_err(
            "Cannot receive several denoms",
        )));
    }
    let denom_amount: Uint128 = info
        .funds
        .iter()
        .find(|c| c.denom == *config.denom)
        .map(|c| Uint128::from(c.amount))
        .unwrap_or_else(Uint128::zero);

    if amount * config.denom_rate != denom_amount {
        return Err(ContractError::Std(StdError::generic_err(
            "Invalid denom amount",
        )));
    }

    let gohm_amount = amount * config.gohm_rate;
    let gohm_token = deps.api.addr_humanize(&config.gohm_token)?.to_string();

    let execute_res = execute_mint(deps, env.clone(), info.clone(), recipient, amount);
    match execute_res {
        Ok(response) => Ok(if gohm_amount.is_zero() {
            response
        } else {
            response.add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: gohm_token,
                msg: to_binary(&ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount: gohm_amount,
                })?,
                funds: vec![],
            }))
        }),
        Err(err) => return Err(err),
    }
}

fn try_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIGURATION.load(deps.storage)?;

    let denom_amount = amount * config.denom_rate;
    let gohm_amount = amount * config.gohm_rate;
    let gohm_token = deps.api.addr_humanize(&config.gohm_token)?.to_string();

    let querier = deps.querier;
    let execute_res = execute_burn(deps, env.clone(), info.clone(), amount);
    match execute_res {
        Ok(response) => {
            let mut messages: Vec<CosmosMsg> = vec![];

            if !denom_amount.is_zero() {
                let denom_asset = Asset {
                    info: AssetInfo::NativeToken {
                        denom: config.denom,
                    },
                    amount: denom_amount,
                };
                messages.push(denom_asset.into_msg(&querier, info.sender.clone())?);
            }
            if !gohm_amount.is_zero() {
                let gohm_asset = Asset {
                    info: AssetInfo::Token {
                        contract_addr: gohm_token,
                    },
                    amount: gohm_amount,
                };
                messages.push(gohm_asset.into_msg(&querier, info.sender)?);
            }
            Ok(response.add_messages(messages))
        }
        Err(err) => return Err(err),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
