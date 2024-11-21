use andromeda_std::{amp::AndrAddr, andr_exec, andr_instantiate, common::denom::Asset};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub swap_router: Option<AndrAddr>,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    /// Swap cw20 asset into another asset using dex
    Receive(Cw20ReceiveMsg),
    /// Swap native token into another asset using dex
    SwapAndForward {
        /// The name of the dex that is to be used for the swap operation
        dex: String,
        /// The asset swap to be swapped to
        to_asset: Asset,
        /// The address where the swapped token is supposed to be sent
        forward_addr: Option<AndrAddr>,
        /// The binary message that is to be sent with swapped token transfer
        forward_msg: Option<Binary>,
        /// The max spread. Equals to slippage tolerance / 100
        max_spread: Option<Decimal>,
        /// The minimum amount of tokens to receive from swap operation
        minimum_receive: Option<Uint128>,
    },
}

#[cw_serde]
pub enum Cw20HookMsg {
    SwapAndForward {
        /// The name of the dex that is to be used for the swap operation
        dex: String,
        /// The asset swap to be swapped to
        to_asset: Asset,
        /// The address where the swapped token is supposed to be sent
        forward_addr: Option<AndrAddr>,
        /// The binary message that is to be sent with swapped token transfer
        forward_msg: Option<Binary>,
        /// The max spread. Equals to slippage tolerance / 100
        max_spread: Option<Decimal>,
        /// The minimum amount of tokens to receive from swap operation
        minimum_receive: Option<Uint128>,
    },
}
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(SimulateSwapOperationResponse)]
    SimulateSwapOperation {
        /// The name of the dex that is to be used for the swap operation
        dex: String,
        /// The amount of tokens to swap
        offer_amount: Uint128,
        /// The swap operation to perform
        operation: SwapOperation,
    },
}

#[cw_serde]
pub struct SwapOperation {
    /// The asset being swapped
    pub offer_asset_info: Asset,
    /// The asset swap to be swapped to
    pub ask_asset_info: Asset,
}

#[cw_serde]
pub struct SimulateSwapOperationResponse {
    /// The expected amount of tokens being received from swap operation
    pub amount: Uint128,
}
