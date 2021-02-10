use crate::interface::metadata::MetaData;
use crate::*;
use near_sdk::{
    near_bindgen,
    serde::{Deserialize, Serialize},
    serde_json::{self, Value},
};

const METADATA_FT: &str = "http://near.org/contract/metadata/fungible-token";

#[near_bindgen]
impl MetaData for Contract {
    fn metadata(uri: String) -> Option<Value> {
        match uri.as_str() {
            METADATA_FT => {
                let md = TokenMetadata {
                    name: "OysterPack STAKE Token",
                    symbol: "STAKE",
                    ref_url: "https://github.com/oysterpack/oysterpack-near-stake-token",
                    ref_hash: "base64-ecoded-hash".to_string(),
                    granularity: 1,
                    decimals: 24,
                };
                Some(serde_json::to_value(md).unwrap())
            }
            _ => None,
        }
    }

    fn metadata_uris() -> Vec<String> {
        vec![METADATA_FT.to_string()]
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenMetadata {
    name: &'static str,
    symbol: &'static str,
    ref_url: &'static str,
    ref_hash: String,
    granularity: u8,
    decimals: u8,
}
