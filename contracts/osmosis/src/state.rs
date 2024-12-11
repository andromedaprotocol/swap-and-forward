use andromeda_std::amp::{messages::AMPCtx, AndrAddr};
use cosmwasm_std::Binary;
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
pub struct ForwardReplyState {
    /// Forward Address
    pub addr: AndrAddr,
    /// Refund Address
    pub refund_addr: AndrAddr,
    /// Optional binary msg forwarded to the forward address
    pub msg: Option<Binary>,
    /// Amp ctx to be used for ibc communication
    pub amp_ctx: Option<AMPCtx>,
    /// Offered denom to the osmosis
    pub from_denom: String,
    /// Asked denom returning from the osmosis
    pub to_denom: String,
}

pub const FORWARD_REPLY_STATE: Item<ForwardReplyState> = Item::new("forward_reply_state");

pub const SWAP_ROUTER: Item<AndrAddr> = Item::new("swap_router");
