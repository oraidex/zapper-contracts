use cosmwasm_std::{
    coin, to_json_binary, wasm_execute, CosmosMsg, Decimal, DepsMut, Env, Order, Reply, Response,
    SubMsgResult,
};
use osmosis_std::types::osmosis::concentratedliquidity::v1beta1::{
    MsgCreatePositionResponse, MsgTransferPositions,
};
use zapper::{
    asset::{get_current_asset_available, Asset},
    error::ZapperError,
};

use crate::{
    error::{ContractError, ContractResult},
    helper::{create_osmosis_swap_msg, create_refund_msg},
    msg::ExecuteMsg,
    state::{ProtocolFee, PENDING_POSITION, PENDING_ZAP_OUT, PROTOCOL_FEE, SNAP_BALANCES},
};

pub fn reply_create_position(deps: DepsMut, env: Env, msg: Reply) -> ContractResult<Response> {
    match msg.result.clone() {
        SubMsgResult::Ok(_) => {
            let msg_create_pos_res = MsgCreatePositionResponse::try_from(msg.result)?;
            let pending_position = PENDING_POSITION.load(deps.storage)?;
            // transfer position to receiver
            let mut msgs: Vec<CosmosMsg> = vec![];
            let position_id = msg_create_pos_res.position_id;
            let receiver_address = pending_position.receiver.to_string();

            msgs.push(
                MsgTransferPositions {
                    position_ids: vec![position_id],
                    sender: env.contract.address.to_string(),
                    new_owner: receiver_address.clone(),
                }
                .into(),
            );

            // Refund tokens
            for token in [&pending_position.token_0, &pending_position.token_1].iter() {
                if let Some(msg) =
                    create_refund_msg(&deps.as_ref(), &env, token, &receiver_address)?
                {
                    msgs.push(msg);
                }
            }
            // remove pending position & snapshot balances
            PENDING_POSITION.remove(deps.storage);
            SNAP_BALANCES.remove(deps.storage, &pending_position.token_0);
            SNAP_BALANCES.remove(deps.storage, &pending_position.token_1);

            Ok(Response::new().add_messages(msgs))
        }
        SubMsgResult::Err(e) => return Err(ContractError::CreatePositionError(e)),
    }
}

pub fn reply_withdraw_position(deps: DepsMut, env: Env, msg: Reply) -> ContractResult<Response> {
    match msg.result.clone() {
        SubMsgResult::Ok(_) => {
            let pending_zap_out = PENDING_ZAP_OUT.load(deps.storage)?;
            // transfer position to receiver
            let mut msgs: Vec<CosmosMsg> = vec![];

            // no need to use hashMap because the number of tokens is very small
            let mut all_balances: Vec<Asset> = SNAP_BALANCES
                .range(deps.storage, None, None, Order::Ascending)
                .map(|item| {
                    let (denom, amount) = item?;

                    let current_balance = get_current_asset_available(
                        deps.api,
                        &deps.querier,
                        &env.contract.address,
                        &denom,
                    )?;

                    Ok(Asset::new(
                        deps.api,
                        &denom,
                        current_balance.amount().checked_sub(amount)?,
                    ))
                })
                .collect::<ContractResult<Vec<Asset>>>()?;

            let protocol_fee = PROTOCOL_FEE.may_load(deps.storage)?.unwrap_or(ProtocolFee {
                percent: Decimal::zero(),
                fee_receiver: pending_zap_out.receiver.clone(),
            });

            // try swaps
            for route in pending_zap_out.routes {
                if let Some(balance) = all_balances
                    .iter_mut()
                    .find(|b| b.denom().eq(&route.token_in))
                {
                    if balance.amount() < route.offer_amount {
                        return Err(ContractError::Zapper(
                            ZapperError::ZapOutNotEnoughBalanceToSwap {},
                        ));
                    }
                    balance.sub(route.offer_amount)?;

                    let mut amount_to_swap = route.offer_amount;
                    if !protocol_fee.percent.is_zero() {
                        let fee_amount = amount_to_swap * protocol_fee.percent;
                        amount_to_swap -= fee_amount;

                        // transfer fee to fee_receiver
                        msgs.push(
                            balance.transfer_amount(fee_amount, protocol_fee.fee_receiver.as_str()),
                        );
                    }

                    let swap_msg = create_osmosis_swap_msg(
                        env.contract.address.to_string(),
                        coin(amount_to_swap.into(), route.token_in),
                        route.operations,
                        route.minimum_receive,
                    )?;
                    msgs.push(swap_msg.into());
                }
            }

            // transfer fund back
            msgs.push(
                wasm_execute(
                    env.contract.address.to_string(),
                    &ExecuteMsg::TransferFundsBack {
                        receiver: pending_zap_out.receiver,
                    },
                    vec![],
                )?
                .into(),
            );
            // remove pending & snapshot balances
            PENDING_ZAP_OUT.remove(deps.storage);

            Ok(Response::new().add_messages(msgs))
        }
        SubMsgResult::Err(e) => return Err(ContractError::WithdrawPositionError(e)),
    }
}
