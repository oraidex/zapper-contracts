use std::num::ParseIntError;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Binary, Uint128};

use osmosis_std::types::osmosis::poolmanager::v1beta1::SwapAmountInRoute as OsmosisSwapAmountInRoute;

use crate::error::ZapperError;

#[cw_serde]
pub struct Route {
    pub token_in: String,
    pub offer_amount: Uint128,
    pub operations: Vec<SwapOperation>,
    pub minimum_receive: Option<Uint128>,
}

impl Route {
    pub fn ask_denom(&self) -> Result<String, ZapperError> {
        match self.operations.last() {
            Some(op) => Ok(op.denom_out.clone()),
            None => Err(ZapperError::SwapOperationsEmpty),
        }
    }
}

pub fn get_ask_denom_for_routes(routes: &[Route]) -> Result<String, ZapperError> {
    match routes.last() {
        Some(route) => route.ask_denom(),
        None => Err(ZapperError::RoutesEmpty),
    }
}

// Standard swap operation type that contains the pool, denom in, and denom out
// for the swap operation. The type is converted into the respective swap venues
// expected format in each adapter contract.
#[cw_serde]
pub struct SwapOperation {
    pub pool: String,
    pub denom_in: String,
    pub denom_out: String,
    pub interface: Option<Binary>,
}

// OSMOSIS CONVERSIONS

// Converts a swap operation to an osmosis swap amount in route
// Error if the given String for pool in the swap operation is not a valid u64.
impl TryFrom<SwapOperation> for OsmosisSwapAmountInRoute {
    type Error = ParseIntError;

    fn try_from(swap_operation: SwapOperation) -> Result<Self, Self::Error> {
        Ok(OsmosisSwapAmountInRoute {
            pool_id: swap_operation.pool.parse()?,
            token_out_denom: swap_operation.denom_out,
        })
    }
}

// Converts a vector of  swap operation to vector of osmosis swap
// amount in/out routes, returning an error if any of the swap operations
// fail to convert. This only happens if the given String for pool in the
// swap operation is not a valid u64, which is the pool_id type for Osmosis.
pub fn convert_swap_operations<T>(
    swap_operations: Vec<SwapOperation>,
) -> Result<Vec<T>, ParseIntError>
where
    T: TryFrom<SwapOperation, Error = ParseIntError>,
{
    swap_operations.into_iter().map(T::try_from).collect()
}
