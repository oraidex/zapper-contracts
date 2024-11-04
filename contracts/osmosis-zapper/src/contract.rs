#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw20::{Cw20Coin, Cw20ReceiveMsg};
use zapper::{
    asset::{get_current_asset_available, Asset},
    error::ZapperError,
};

use crate::{
    error::{ContractError, ContractResult},
    msg::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    reply::{reply_create_position, reply_withdraw_position},
    state::{ProtocolFee, OWNER, PROTOCOL_FEE, SNAP_BALANCES},
    zap::{create_position, zap_in_liquidity, zap_out_liquidity},
};

use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Order, Reply, Response, StdResult,
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
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::ChangeOwner { new_owner } => execute_change_owner(deps, info, new_owner),
        ExecuteMsg::ZapInLiquidity {
            pool_id,
            token_0,
            token_1,
            lower_tick,
            upper_tick,
            token_min_amount_0,
            token_min_amount_1,
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
            None,
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
        }
        ExecuteMsg::RegisterProtocolFee {
            percent,
            fee_receiver,
        } => execute_register_protocol_fee(deps, info, percent, fee_receiver),
        ExecuteMsg::Withdraw { assets, recipient } => {
            execute_withdraw(deps, info, assets, recipient)
        }
    }
}

//////////////////////////
/// RECEIVE ENTRYPOINT ///
//////////////////////////

// Receive is the main entry point for the contract to
// receive cw20 tokens and execute the swap and action message
fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> ContractResult<Response> {
    let sent_asset = Asset::Cw20(Cw20Coin {
        address: info.sender.to_string(),
        amount: cw20_msg.amount,
    });

    match from_json(&cw20_msg.msg)? {
        Cw20HookMsg::ZapInLiquidity {
            pool_id,
            token_0,
            token_1,
            lower_tick,
            upper_tick,
            token_min_amount_0,
            token_min_amount_1,
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
            Some(sent_asset),
            routes,
        ),
    }
}

fn execute_change_owner(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: Addr,
) -> ContractResult<Response> {
    Ok(OWNER.execute_update_admin(deps, info, Some(new_owner))?)
}

pub fn execute_register_protocol_fee(
    deps: DepsMut,
    info: MessageInfo,
    percent: Decimal,
    fee_receiver: Addr,
) -> Result<Response, ContractError> {
    OWNER.assert_admin(deps.as_ref(), &info.sender)?;

    // validate percent must be < 1
    if percent.gt(&Decimal::one()) {
        return Err(ContractError::Zapper(ZapperError::InvalidFee {}));
    }

    PROTOCOL_FEE.save(
        deps.storage,
        &ProtocolFee {
            percent,
            fee_receiver: fee_receiver.clone(),
        },
    )?;

    Ok(Response::new().add_attributes(vec![
        ("action", "register_protocol_fee"),
        ("percent", &percent.to_string()),
        ("fee_receiver", fee_receiver.as_str()),
    ]))
}

pub fn execute_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    assets: Vec<Asset>,
    recipient: Option<Addr>,
) -> Result<Response, ContractError> {
    OWNER.assert_admin(deps.as_ref(), &info.sender)?;
    let receiver = recipient.unwrap_or_else(|| info.sender.clone());

    let mut msgs: Vec<CosmosMsg> = vec![];
    for asset in assets {
        msgs.push(asset.transfer(receiver.as_str()))
    }

    Ok(Response::new()
        .add_attribute("action", "withdraw")
        .add_messages(msgs))
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
