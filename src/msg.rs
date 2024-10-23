use andromeda_std::{amp::AndrAddr, andr_exec, andr_instantiate, common::denom::Asset};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    /// Swap cw20 asset into another asset using dex
    Receive(Cw20ReceiveMsg),
    /// Swap native token into another asset using dex
    SwapAndForward {
        dex: String,
        to_asset: Asset,
        forward_addr: Option<AndrAddr>,
        forward_msg: Option<Binary>,
        max_spread: Option<Decimal>,
        minimum_receive: Option<Uint128>,
    },
}

#[cw_serde]
pub enum Cw20HookMsg {
    SwapAndForward {
        dex: String,
        to_asset: Asset,
        forward_addr: Option<AndrAddr>,
        forward_msg: Option<Binary>,
        max_spread: Option<Decimal>,
        minimum_receive: Option<Uint128>,
    },
}
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}
