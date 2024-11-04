use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use zapper::{asset::Asset, swap::Route};

use crate::state::ProtocolFee;

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Option<Addr>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    ChangeOwner {
        new_owner: Addr,
    },
    ZapInLiquidity {
        pool_id: u64,
        token_0: String,
        token_1: String,
        lower_tick: u64,
        upper_tick: u64,
        token_min_amount_0: Option<Uint128>,
        token_min_amount_1: Option<Uint128>,
        routes: Vec<Route>,
    },
    CreatePosition {
        pool_id: u64,
        token_0: String,
        token_1: String,
        lower_tick: u64,
        upper_tick: u64,
        token_min_amount_0: Option<Uint128>,
        token_min_amount_1: Option<Uint128>,
    },
    ZapOutLiquidity {
        position_id: u64,
        routes: Vec<Route>,
    },
    TransferFundsBack {
        receiver: Addr,
    },
    RegisterProtocolFee {
        percent: Decimal,
        fee_receiver: Addr,
    },
    Withdraw {
        assets: Vec<Asset>,
        recipient: Option<Addr>,
    },
}
/// This structure describes a CW20 hook message.
#[cw_serde]
pub enum Cw20HookMsg {
    ZapInLiquidity {
        pool_id: u64,
        token_0: String,
        token_1: String,
        lower_tick: u64,
        upper_tick: u64,
        token_min_amount_0: Option<Uint128>,
        token_min_amount_1: Option<Uint128>,
        routes: Vec<Route>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Addr)]
    Owner {},
    #[returns(ProtocolFee)]
    ProtocolFee {},
}

#[cw_serde]
pub struct MigrateMsg {}
