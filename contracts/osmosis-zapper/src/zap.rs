use cosmwasm_std::{
    coin, to_json_binary, wasm_execute, CosmosMsg, DepsMut, Env, MessageInfo, Response, SubMsg,
    Uint128,
};
use cw_utils::one_coin;
use osmosis_std::types::{
    cosmos::base::v1beta1::Coin as OsmosisCoin,
    osmosis::concentratedliquidity::v1beta1::{
        ConcentratedliquidityQuerier, MsgCreatePosition, MsgWithdrawPosition,
    },
};
use zapper::{
    asset::{get_current_asset_available, Asset},
    error::ZapperError,
    swap::Route,
};

use crate::{
    contract::{CREATE_POSITION_ID, WITHDRAW_POSITION_ID},
    error::{ContractError, ContractResult},
    helper::create_osmosis_swap_msg,
    msg::ExecuteMsg,
    state::{
        snapshot_balances, PendingPosition, PendingZapOut, PENDING_POSITION, PENDING_ZAP_OUT,
        PROTOCOL_FEE, SNAP_BALANCES,
    },
};

pub fn zap_in_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_id: u64,
    token_0: String,
    token_1: String,
    lower_tick: u64,
    upper_tick: u64,
    token_min_amount_0: Option<Uint128>,
    token_min_amount_1: Option<Uint128>,
    asset_in: Option<Asset>,
    routes: Vec<Route>,
) -> ContractResult<Response> {
    // Validate and unwrap the sent asset
    let asset_in = match asset_in {
        Some(sent_asset) => {
            sent_asset.validate(&deps, &env, &info)?;
            sent_asset
        }
        None => one_coin(&info)?.into(),
    };

    // init messages and submessages
    let mut msgs: Vec<CosmosMsg> = vec![];

    let mut amount_after_fee = asset_in.amount();
    // handle deduct zap in fee
    if let Some(protocol_fee) = PROTOCOL_FEE.may_load(deps.storage)? {
        if !protocol_fee.percent.is_zero() {
            let fee_amount = asset_in.amount() * protocol_fee.percent;
            amount_after_fee -= fee_amount;
            // transfer fee to fee_receiver
            msgs.push(asset_in.transfer_amount(fee_amount, protocol_fee.fee_receiver.as_str()));
        }
    }

    // validate asset_in and routes
    let total_swap_amount: Uint128 = routes
        .iter()
        .map(|route| route.offer_amount)
        .collect::<Vec<Uint128>>()
        .iter()
        .sum();
    if total_swap_amount.gt(&amount_after_fee) {
        return Err(ContractError::Zapper(ZapperError::InvalidFund {}));
    }

    let mut balance_0 =
        get_current_asset_available(deps.api, &deps.querier, &env.contract.address, &token_0)?;
    let mut balance_1 =
        get_current_asset_available(deps.api, &deps.querier, &env.contract.address, &token_1)?;

    if asset_in.denom() == token_0 {
        balance_0.sub(asset_in.amount())?;
    }
    if asset_in.denom() == token_1 {
        balance_1.sub(asset_in.amount())?;
    };

    SNAP_BALANCES.save(deps.storage, &token_0, &balance_0.amount())?;
    SNAP_BALANCES.save(deps.storage, &token_1, &balance_1.amount())?;

    for i in 0..routes.len() {
        let swap_msg = create_osmosis_swap_msg(
            env.contract.address.to_string(),
            coin(routes[i].offer_amount.into(), asset_in.denom()),
            routes[i].operations.clone(),
            None,
        )?;
        msgs.push(swap_msg);
    }

    msgs.push(
        wasm_execute(
            env.contract.address.to_string(),
            &to_json_binary(&ExecuteMsg::CreatePosition {
                pool_id,
                token_0: token_0.clone(),
                token_1: token_1.clone(),
                lower_tick,
                upper_tick,
                token_min_amount_0,
                token_min_amount_1,
            })?,
            vec![],
        )?
        .into(),
    );

    // store pending position
    PENDING_POSITION.save(
        deps.storage,
        &PendingPosition {
            receiver: info.sender,
            pool_id,
            token_0,
            token_1,
        },
    )?;
    Ok(Response::new().add_messages(msgs))
}

pub fn create_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_id: u64,
    token_0: String,
    token_1: String,
    lower_tick: u64,
    upper_tick: u64,
    token_min_amount_0: Option<Uint128>,
    token_min_amount_1: Option<Uint128>,
) -> ContractResult<Response> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    //  Recheck the balance of tokenX and tokenY in this contract
    let x_amount_before = SNAP_BALANCES.load(deps.storage, &token_0)?;
    let y_amount_before = SNAP_BALANCES.load(deps.storage, &token_1)?;

    //  Minus with the previous balance of tokenX and tokenY snap in state
    let x_amount_after =
        get_current_asset_available(deps.api, &deps.querier, &env.contract.address, &token_0)?;
    let y_amount_after =
        get_current_asset_available(deps.api, &deps.querier, &env.contract.address, &token_1)?;
    let x_amount = x_amount_after.amount() - x_amount_before;
    let y_amount = y_amount_after.amount() - y_amount_before;

    let mut tokens_provided: Vec<OsmosisCoin> = vec![];
    if !x_amount.is_zero() {
        tokens_provided.push(OsmosisCoin {
            denom: token_0.clone(),
            amount: x_amount.to_string(),
        });
    }
    if !y_amount.is_zero() {
        tokens_provided.push(OsmosisCoin {
            denom: token_1.clone(),
            amount: y_amount.to_string(),
        });
    }

    //Process create new position with amountX and amountY
    let msg_create_pos: CosmosMsg = MsgCreatePosition {
        pool_id,
        sender: env.contract.address.to_string(),
        lower_tick: lower_tick as i64,
        upper_tick: upper_tick as i64,
        tokens_provided,
        token_min_amount0: token_min_amount_0.unwrap_or_default().to_string(),
        token_min_amount1: token_min_amount_1.unwrap_or_default().to_string(),
    }
    .into();

    Ok(
        Response::new()
            .add_submessage(SubMsg::reply_on_success(msg_create_pos, CREATE_POSITION_ID)),
    )
}

// Ensure this position transfer to contract first
pub fn zap_out_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    position_id: u64,
    routes: Vec<Route>,
) -> ContractResult<Response> {
    // query positions
    let position_detail = ConcentratedliquidityQuerier::new(&deps.querier)
        .position_by_id(position_id)?
        .position
        .unwrap();

    // snapshot token 0 & token 1
    // TODO: dont use unwarp()
    let token_0 = position_detail.asset0.unwrap().denom;
    let token_1 = position_detail.asset1.unwrap().denom;
    let position = position_detail.position.unwrap();

    // clear snapshot balances first
    SNAP_BALANCES.clear(deps.storage);

    snapshot_balances(deps.api, &deps.querier, deps.storage, &env, &token_0)?;
    snapshot_balances(deps.api, &deps.querier, deps.storage, &env, &token_1)?;

    // snapshot incentives
    for incentive in &position_detail.claimable_incentives {
        snapshot_balances(
            deps.api,
            &deps.querier,
            deps.storage,
            &env,
            &incentive.denom,
        )?;
    }

    // snapshot token out of zap out
    for route in &routes {
        let ops = route.operations.last();
        if let Some(ops) = ops {
            snapshot_balances(deps.api, &deps.querier, deps.storage, &env, &ops.denom_out)?;
        }
    }

    PENDING_ZAP_OUT.save(
        deps.storage,
        &PendingZapOut {
            receiver: info.sender,
            routes,
        },
    )?;

    Ok(Response::new().add_submessage(SubMsg::reply_on_success(
        MsgWithdrawPosition {
            position_id,
            sender: env.contract.address.to_string(),
            liquidity_amount: position.liquidity,
        },
        WITHDRAW_POSITION_ID,
    )))
}
