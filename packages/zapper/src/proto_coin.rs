use cosmos_sdk_proto::cosmos::base::v1beta1::Coin as CosmosSdkCoin;
use cosmwasm_schema::cw_serde;
use osmosis_std::types::cosmos::base::v1beta1::Coin as OsmosisStdCoin;

//  wrapper coin type that is used to wrap cosmwasm_std::Coin
// and be able to implement type conversions on the wrapped type.
#[cw_serde]
pub struct ProtoCoin(pub cosmwasm_std::Coin);

// Converts a coin to a cosmos_sdk_proto coin
impl From<ProtoCoin> for CosmosSdkCoin {
    fn from(coin: ProtoCoin) -> Self {
        // Convert the  coin to a cosmos_sdk_proto coin and return it
        CosmosSdkCoin {
            denom: coin.0.denom.clone(),
            amount: coin.0.amount.to_string(),
        }
    }
}

// Converts a  coin to an osmosis_std coin
impl From<ProtoCoin> for OsmosisStdCoin {
    fn from(coin: ProtoCoin) -> Self {
        // Convert the  coin to an osmosis coin and return it
        OsmosisStdCoin {
            denom: coin.0.denom,
            amount: coin.0.amount.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::Uint128;

    #[test]
    fn test_from_proto_coin_to_cosmos_sdk_proto_coin() {
        let coin = ProtoCoin(cosmwasm_std::Coin {
            denom: "uatom".to_string(),
            amount: Uint128::new(100),
        });

        let cosmos_sdk_proto_coin: CosmosSdkCoin = coin.into();

        assert_eq!(cosmos_sdk_proto_coin.denom, "uatom");
        assert_eq!(cosmos_sdk_proto_coin.amount, "100");
    }

    #[test]
    fn test_from_proto_coin_to_osmosis_std_coin() {
        let coin = ProtoCoin(cosmwasm_std::Coin {
            denom: "uatom".to_string(),
            amount: Uint128::new(100),
        });

        let osmosis_std_coin: OsmosisStdCoin = coin.into();

        assert_eq!(osmosis_std_coin.denom, "uatom");
        assert_eq!(osmosis_std_coin.amount, "100");
    }
}
