use std::str::FromStr;

use andromeda_std::{
    ado_contract::ADOContract,
    amp::{
        messages::{AMPMsg, AMPPkt},
        AndrAddr,
    },
    common::{context::ExecuteContext, denom::Asset},
    error::ContractError,
};
use astroport::{
    asset::AssetInfo,
    router::{Cw20HookMsg as AstroCw20HookMsg, ExecuteMsg as AstroExecuteMsg, SwapOperation},
};
use cosmwasm_std::{
    attr, coin, ensure, to_json_binary, wasm_execute, Binary, Coin, Decimal, Deps, DepsMut, Env,
    Reply, Response, StdError, SubMsg, SubMsgResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use crate::state::{ForwardReplyState, FORWARD_REPLY_STATE};

pub const ASTRO_ROUTER_ADDRESS: &str =
    "terra1j8hayvehh3yy02c2vtw5fdhz9f4drhtee8p5n5rguvg3nyd6m83qd2y90a";

pub const ASTROPORT_MSG_SWAP_ID: u64 = 1;
pub const ASTROPORT_MSG_FORWARD_ID: u64 = 2;

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_swap_astroport_msg(
    ctx: ExecuteContext,
    from_asset: Asset,
    from_amount: Uint128,
    to_asset: Asset,
    forward_addr: AndrAddr, // receiver where the swapped token goes to
    refund_addr: AndrAddr,  // refund address
    forward_msg: Option<Binary>,
    max_spread: Option<Decimal>,
    minimum_receive: Option<Uint128>,
) -> Result<SubMsg, ContractError> {
    let ExecuteContext { deps, .. } = ctx;

    // Prepare offer and ask asset
    ensure!(from_asset != to_asset, ContractError::DuplicateTokens {});
    let from_denom = match from_asset.clone() {
        Asset::NativeToken(denom) => denom,
        Asset::Cw20Token(andr_addr) => andr_addr.get_raw_address(&deps.as_ref())?.to_string(),
    };

    // Prepare swap operations
    let operations = vec![SwapOperation::AstroSwap {
        offer_asset_info: generate_asset_info_from_asset(&deps.as_ref(), from_asset.clone())?,
        ask_asset_info: generate_asset_info_from_asset(&deps.as_ref(), to_asset.clone())?,
    }];

    ensure!(
        FORWARD_REPLY_STATE
            .may_load(deps.as_ref().storage)?
            .is_none(),
        ContractError::Unauthorized {}
    );

    let amp_ctx = if let Some(pkt) = ctx.amp_ctx.clone() {
        Some(pkt.ctx)
    } else {
        None
    };

    FORWARD_REPLY_STATE.save(
        deps.storage,
        &ForwardReplyState {
            addr: forward_addr,
            refund_addr,
            msg: forward_msg,
            dex: "astroport".to_string(),
            amp_ctx,
            from_asset: from_asset.clone(),
            to_asset: to_asset.clone(),
        },
    )?;

    // Build swap msg
    let msg = match from_asset {
        Asset::NativeToken(_) => {
            let astro_swap_msg = AstroExecuteMsg::ExecuteSwapOperations {
                operations,
                to: None,
                max_spread,
                minimum_receive,
            };
            WasmMsg::Execute {
                contract_addr: ASTRO_ROUTER_ADDRESS.to_string(),
                msg: to_json_binary(&astro_swap_msg)?,
                funds: vec![coin(from_amount.u128(), from_denom)],
            }
        }
        Asset::Cw20Token(cw20_contract) => {
            let astro_swap_hook_msg = AstroCw20HookMsg::ExecuteSwapOperations {
                operations,
                to: None,
                max_spread,
                minimum_receive,
            };

            let send_msg = Cw20ExecuteMsg::Send {
                contract: ASTRO_ROUTER_ADDRESS.to_string(),
                amount: from_amount,
                msg: to_json_binary(&astro_swap_hook_msg)?,
            };

            wasm_execute(
                cw20_contract.get_raw_address(&deps.as_ref())?,
                &send_msg,
                vec![],
            )?
        }
    };

    Ok(SubMsg::reply_always(msg, ASTROPORT_MSG_SWAP_ID))
}

#[derive(Clone, Debug, PartialEq)]
pub struct AstroportSwapResponse {
    pub spread_amount: Uint128, // remaining Asset that is not consumed by the swap operation
    pub return_amount: Uint128, // amount of token_out swapped from astroport
}

pub fn generate_asset_info_from_asset(
    deps: &Deps,
    asset: Asset,
) -> Result<AssetInfo, ContractError> {
    match asset {
        Asset::Cw20Token(andr_addr) => {
            let contract_addr = andr_addr.get_raw_address(deps)?;
            Ok(AssetInfo::Token { contract_addr })
        }
        Asset::NativeToken(denom) => Ok(AssetInfo::NativeToken { denom }),
    }
}

pub fn handle_astroport_swap(
    deps: DepsMut,
    env: Env,
    msg: Reply,
    state: ForwardReplyState,
) -> Result<Response, ContractError> {
    let AstroportSwapResponse {
        spread_amount,
        return_amount,
    } = match parse_astroport_swap_reply(msg) {
        Ok(resp) => resp,
        Err(e) => return Err(e),
    };

    let mut resp = Response::default();

    let transfer_msg = match &state.to_asset {
        Asset::NativeToken(denom) => {
            let funds = vec![Coin {
                denom: denom.to_string(),
                amount: return_amount,
            }];

            let mut pkt = if let Some(amp_ctx) = state.amp_ctx.clone() {
                AMPPkt::new(amp_ctx.get_origin(), amp_ctx.get_previous_sender(), vec![])
            } else {
                AMPPkt::new(
                    env.contract.address.clone(),
                    env.contract.address.clone(),
                    vec![],
                )
            };

            let msg = AMPMsg::new(
                state.addr.clone(),
                state.msg.clone().unwrap_or_default(),
                Some(funds.clone()),
            );

            pkt = pkt.add_message(msg);
            let kernel_address = ADOContract::default().get_kernel_address(deps.storage)?;
            pkt.to_sub_msg(kernel_address, Some(funds), ASTROPORT_MSG_FORWARD_ID)?
        }
        Asset::Cw20Token(andr_addr) => {
            let transfer_msg = Cw20ExecuteMsg::Transfer {
                recipient: state.addr.get_raw_address(&deps.as_ref())?.to_string(),
                amount: return_amount,
            };
            let wasm_msg = wasm_execute(
                andr_addr.get_raw_address(&deps.as_ref())?,
                &transfer_msg,
                vec![],
            )?;
            SubMsg::new(wasm_msg)
        }
    };
    let kernel_address = ADOContract::default().get_kernel_address(deps.storage)?;
    resp = resp.add_submessage(transfer_msg).add_attributes(vec![
        attr("action", "swap_and_forward"),
        attr("dex", state.dex),
        attr("to_denom", state.to_asset.to_string()),
        attr("to_amount", return_amount),
        attr("spread_amount", spread_amount),
        attr("forward_addr", state.addr),
        attr("kernel_address", kernel_address),
    ]);
    Ok(resp)
}

pub(crate) fn parse_astroport_swap_reply(
    msg: Reply,
) -> Result<AstroportSwapResponse, ContractError> {
    match msg.result {
        SubMsgResult::Ok(response) => {
            // Extract relevant information from events
            let mut spread_amount = Uint128::zero();
            let mut return_amount = Uint128::zero();

            for event in response.events.iter() {
                if event.ty == "wasm" {
                    for attr in event.attributes.iter() {
                        match attr.key.as_str() {
                            "return_amount" => {
                                return_amount = Uint128::from_str(&attr.value)?;
                            }
                            "spread_amount" => {
                                spread_amount = Uint128::from_str(&attr.value)?;
                            }
                            _ => {}
                        }
                    }
                }
            }

            if return_amount.is_zero() {
                return Err(ContractError::Std(StdError::generic_err(format!(
                    "Incomplete data in Astroport swap response: {:?}",
                    response.events
                ))));
            }

            Ok(AstroportSwapResponse {
                return_amount,
                spread_amount,
            })
        }
        SubMsgResult::Err(err) => Err(ContractError::Std(StdError::generic_err(format!(
            "Astroport swap failed with error: {:?}",
            err
        )))),
    }
}
