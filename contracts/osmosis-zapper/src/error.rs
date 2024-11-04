use cosmwasm_std::StdError;
use thiserror::Error;
use zapper::error::ZapperError;

pub type ContractResult<T> = core::result::Result<T, ContractError>;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error(transparent)]
    Zapper(#[from] ZapperError),

    #[error(transparent)]
    Overflow(#[from] cosmwasm_std::OverflowError),

    #[error(transparent)]
    Admin(#[from] cw_controllers::AdminError),

    #[error(transparent)]
    Payment(#[from] cw_utils::PaymentError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Parse Int error raised: invalid pool String to pool id u64 conversion")]
    ParseIntPoolID(#[from] std::num::ParseIntError),

    #[error("swap_operations cannot be empty")]
    SwapOperationsEmpty,

    #[error("coin_in denom must match the first swap operation's denom in")]
    CoinInDenomMismatch,

    #[error("coin_out denom must match the last swap operation's denom out")]
    CoinOutDenomMismatch,

    #[error("Asset Must Be Native, Osmosis Does Not Support CW20 Tokens")]
    AssetNotNative,

    #[error("Create position error {0}")]
    CreatePositionError(String),

    #[error("Create position error {0}")]
    WithdrawPositionError(String),
}

impl From<ContractError> for StdError {
    fn from(source: ContractError) -> Self {
        Self::generic_err(source.to_string())
    }
}
