#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use zapper::{
    asset::{get_current_asset_available, Asset},
    error::ZapperError,
};

use crate::{
    error::{ContractError, ContractResult},
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    reply::{reply_create_position, reply_withdraw_position},
    state::{ProtocolFee, OWNER, PROTOCOL_FEE, SNAP_BALANCES},
    zap::{create_position, zap_in_liquidity, zap_out_liquidity},
};

use cosmwasm_std::{
    to_json_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, Reply,
    Response, StdResult,
};
use cw2::set_contract_version;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:zapper";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const CREATE_POSITION_ID: u64 = 1;
pub const WITHDRAW_POSITION_ID: u64 = 2;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    OWNER.set(deps, Some(msg.owner.unwrap_or(info.sender)))?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::ChangeOwner { new_owner } => execute_change_owner(deps, info, new_owner),
        ExecuteMsg::ZapInLiquidity {
            pool_id,
            token_0,
            token_1,
            lower_tick,
            upper_tick,
            token_min_amount_0,
            token_min_amount_1,
            asset_in,
            routes,
        } => zap_in_liquidity(
            deps,
            env,
            info,
            pool_id,
            token_0,
            token_1,
            lower_tick,
            upper_tick,
            token_min_amount_0,
            token_min_amount_1,
            asset_in,
            routes,
        ),
        ExecuteMsg::CreatePosition {
            pool_id,
            token_0,
            token_1,
            lower_tick,
            upper_tick,
            token_min_amount_0,
            token_min_amount_1,
        } => create_position(
            deps,
            env,
            info,
            pool_id,
            token_0,
            token_1,
            lower_tick,
            upper_tick,
            token_min_amount_0,
            token_min_amount_1,
        ),
        ExecuteMsg::ZapOutLiquidity {
            position_id,
            routes,
        } => zap_out_liquidity(deps, env, info, position_id, routes),
        ExecuteMsg::TransferFundsBack { receiver } => {
            execute_transfer_funds_back(deps, env, info, receiver)
        } // ExecuteMsg::RegisterProtocolFee {
          //     percent,
          //     fee_receiver,
          // } => execute_register_protocol_fee(deps, info, percent, fee_receiver),
          // ExecuteMsg::Withdraw { assets, recipient } => withdraw(deps, info, assets, recipient),
    }
}

fn execute_change_owner(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: Addr,
) -> ContractResult<Response> {
    Ok(OWNER.execute_update_admin(deps, info, Some(new_owner))?)
}

fn execute_transfer_funds_back(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    receiver: Addr,
) -> ContractResult<Response> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    let mut msgs: Vec<CosmosMsg> = vec![];

    for item in SNAP_BALANCES.range(deps.storage, None, None, Order::Ascending) {
        let (denom, amount) = item?;
        let current_balance =
            get_current_asset_available(deps.api, &deps.querier, &env.contract.address, &denom)?;

        let refund_amount = current_balance.amount().checked_sub(amount)?;
        if !refund_amount.is_zero() {
            let asset = Asset::new(deps.api, &denom, refund_amount);
            msgs.push(asset.transfer(receiver.as_str()));
        }
    }

    // clear snap balances
    SNAP_BALANCES.clear(deps.storage);

    Ok(Response::new().add_messages(msgs))
}
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Owner {} => to_json_binary(&OWNER.get(deps)?),
        QueryMsg::ProtocolFee {} => to_json_binary(&get_protocol_fee(deps)?),
    }
}

fn get_protocol_fee(deps: Deps) -> Result<ProtocolFee, ContractError> {
    let protocol_fee = PROTOCOL_FEE.load(deps.storage)?;
    Ok(protocol_fee)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> ContractResult<Response> {
    let original_version =
        cw2::ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new().add_attribute("new_version", original_version.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> ContractResult<Response> {
    match msg.id {
        CREATE_POSITION_ID => reply_create_position(deps, env, msg),
        WITHDRAW_POSITION_ID => reply_withdraw_position(deps, env, msg),
        _ => Err(ContractError::Zapper(ZapperError::ReplyIdError(msg.id))),
    }
}
