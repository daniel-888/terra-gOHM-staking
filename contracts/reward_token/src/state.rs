use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal};
use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub gohm_token: CanonicalAddr,
    pub denom: String,
    pub gohm_rate: Decimal,
    pub denom_rate: Decimal,
}

pub const CONFIGURATION: Item<Config> = Item::new("config");
