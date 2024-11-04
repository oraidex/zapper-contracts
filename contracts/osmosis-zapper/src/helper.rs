use cosmwasm_std::{Coin, CosmosMsg, Deps, Env, Uint128};
use osmosis_std::types::osmosis::{
    gamm::v1beta1::MsgSwapExactAmountIn, poolmanager::v1beta1::SwapAmountInRoute,
};
use zapper::{
    asset::{get_current_asset_available, Asset},
    proto_coin::ProtoCoin,
    swap::{convert_swap_operations, SwapOperation},
};

use crate::{
    error::{ContractError, ContractResult},
    state::SNAP_BALANCES,
};

pub fn create_osmosis_swap_msg(
    sender: String,
    coin_in: Coin,
    swap_operations: Vec<SwapOperation>,
    minimum_receive: Option<Uint128>,
) -> ContractResult<CosmosMsg> {
    // Convert the swap operations to osmosis swap amount in routes
    // Return an error if there was an error converting the swap
    // operations to osmosis swap amount in routes.
    let osmosis_swap_amount_in_routes: Vec<SwapAmountInRoute> =
        convert_swap_operations(swap_operations).map_err(ContractError::ParseIntPoolID)?;

    // Create the osmosis poolmanager swap exact amount in message
    // The token out min amount is set to 1 because we are not concerned
    // with the minimum amount in this contract, that gets verified in the
    // entry point contract.
    let swap_msg: CosmosMsg = MsgSwapExactAmountIn {
        sender,
        routes: osmosis_swap_amount_in_routes,
        token_in: Some(ProtoCoin(coin_in).into()),
        token_out_min_amount: minimum_receive.unwrap_or_default().to_string(),
    }
    .into();

    Ok(swap_msg)
}

pub fn create_refund_msg(
    deps: &Deps,
    env: &Env,
    denom: &str,
    receiver: &str,
) -> ContractResult<Option<CosmosMsg>> {
    // query snapshot balances
    let balance_before = SNAP_BALANCES.load(deps.storage, denom)?;

    // query balance after
    let balance_after =
        get_current_asset_available(deps.api, &deps.querier, &env.contract.address, denom)?
            .amount();

    if balance_after > balance_before {
        let refund_amount = balance_after - balance_before;
        return Ok(Some(
            Asset::new(deps.api, denom, refund_amount).transfer(receiver),
        ));
    }
    Ok(None)
}
