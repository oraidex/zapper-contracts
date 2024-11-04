use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Decimal, Env, QuerierWrapper, Storage, Uint128};
use cw_controllers::Admin;
use cw_storage_plus::{Item, Map};
use zapper::{asset::get_current_asset_available, swap::Route};

use crate::error::ContractResult;

pub const OWNER: Admin = Admin::new("owner");

pub const PROTOCOL_FEE: Item<ProtocolFee> = Item::new("protocol_fee");
pub const SNAP_BALANCES: Map<&str, Uint128> = Map::new("snap_balances");
pub const PENDING_POSITION: Item<PendingPosition> = Item::new("pending_position");
pub const PENDING_ZAP_OUT: Item<PendingZapOut> = Item::new("pending_zap_out");

#[cw_serde]
pub struct ProtocolFee {
    pub percent: Decimal,
    pub fee_receiver: Addr,
}

#[cw_serde]
pub struct PendingPosition {
    pub receiver: Addr,
    pub pool_id: u64,
    pub token_0: String,
    pub token_1: String,
}

#[cw_serde]
pub struct PendingZapOut {
    pub receiver: Addr,
    pub routes: Vec<Route>,
}

pub fn snapshot_balances(
    api: &dyn Api,
    querier: &QuerierWrapper,
    storage: &mut dyn Storage,
    env: &Env,
    denom: &str,
) -> ContractResult<()> {
    let balance = get_current_asset_available(api, querier, &env.contract.address, denom)?;
    SNAP_BALANCES.save(storage, &denom, &balance.amount())?;
    Ok(())
}
