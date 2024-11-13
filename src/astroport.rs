use andromeda_std::{
    amp::AndrAddr,
    common::{context::ExecuteContext, denom::Asset},
    error::ContractError,
};
use astroport::{
    asset::AssetInfo,
    router::{Cw20HookMsg as AstroCw20HookMsg, ExecuteMsg as AstroExecuteMsg, SwapOperation},
};
use cosmwasm_std::{
    coin, ensure, to_json_binary, wasm_execute, Binary, Decimal, Deps, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use crate::state::{ForwardReplyState, FORWARD_REPLY_STATE};

pub const ASTRO_ROUTER_ADDRESS: &str =
    "terra1j8hayvehh3yy02c2vtw5fdhz9f4drhtee8p5n5rguvg3nyd6m83qd2y90a";

pub const MSG_SWAP_ID: u64 = 1;

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

    Ok(SubMsg::reply_always(msg, MSG_SWAP_ID))
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
