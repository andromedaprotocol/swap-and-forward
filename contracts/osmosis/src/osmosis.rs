use andromeda_std::{
    ado_contract::ADOContract,
    amp::{
        messages::{AMPMsg, AMPPkt},
        AndrAddr, Recipient,
    },
    common::context::ExecuteContext,
    error::ContractError,
};
use cosmwasm_std::{
    attr, coin, ensure, to_json_binary, Coin, Deps, DepsMut, Env, Reply, Response, StdError,
    SubMsg, Uint128, WasmMsg,
};
use swaprouter::msg::{ExecuteMsg as OsmosisExecuteMsg, QueryMsg as OsmosisQueryMsg};

use crate::state::{ForwardReplyState, FORWARD_REPLY_STATE, PREV_BALANCE, SWAP_ROUTER};

use andromeda_swap_and_forward::osmosis::{GetRouteResponse, Slippage, SwapRoute};

pub const OSMOSIS_MSG_SWAP_ID: u64 = 1;
pub const OSMOSIS_MSG_FORWARD_ID: u64 = 2;

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_swap_osmosis_msg(
    ctx: ExecuteContext,
    from_denom: String,
    from_amount: Uint128,
    to_denom: String,
    recipient: Recipient,  // receiver where the swapped token goes to
    refund_addr: AndrAddr, // refund address
    slippage: Slippage,
    route: Option<Vec<SwapRoute>>,
) -> Result<SubMsg, ContractError> {
    let ExecuteContext { deps, env, .. } = ctx;

    // Prepare offer and ask asset
    ensure!(from_denom != to_denom, ContractError::DuplicateTokens {});

    // Prepare swap operations
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

    // Generate route for the `OsmosisExecuteMsg::Swap` message
    let route = route.map(|route| route.iter().map(|v| v.clone().into()).collect());

    let prev_balance = deps
        .querier
        .query_balance(env.contract.address.to_string(), &to_denom)?
        .amount;

    FORWARD_REPLY_STATE.save(
        deps.storage,
        &ForwardReplyState {
            recipient,
            refund_addr,
            amp_ctx,
            from_denom: from_denom.clone(),
            to_denom: to_denom.clone(),
        },
    )?;

    PREV_BALANCE.save(deps.storage, &prev_balance)?;

    let swap_router = SWAP_ROUTER
        .load(deps.storage)?
        .get_raw_address(&deps.as_ref())?;
    let swap_msg = OsmosisExecuteMsg::Swap {
        input_coin: coin(from_amount.u128(), from_denom.clone()),
        output_denom: to_denom,
        slippage: slippage.into(),
        route,
    };
    let msg = WasmMsg::Execute {
        contract_addr: swap_router.to_string(),
        msg: to_json_binary(&swap_msg)?,
        funds: vec![coin(from_amount.u128(), from_denom)],
    };

    Ok(SubMsg::reply_always(msg, OSMOSIS_MSG_SWAP_ID))
}

pub fn handle_osmosis_swap_reply(
    deps: DepsMut,
    env: Env,
    msg: Reply,
    state: ForwardReplyState,
) -> Result<Response, ContractError> {
    let balance = deps
        .querier
        .query_balance(env.contract.address.to_string(), &state.to_denom)?
        .amount;
    let prev_balance = PREV_BALANCE.load(deps.storage)?;
    let return_amount = balance.checked_sub(prev_balance)?;
    PREV_BALANCE.remove(deps.storage);

    if return_amount.is_zero() {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "Incomplete data in Osmosis swap response: {:?}",
            msg
        ))));
    }

    let mut resp = Response::default();

    let funds = vec![Coin {
        denom: state.to_denom.to_string(),
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

    let Recipient { address, msg, .. } = state.recipient;
    let msg = AMPMsg::new(
        address.clone(),
        msg.unwrap_or_default(),
        Some(funds.clone()),
    );

    pkt = pkt.add_message(msg);
    let kernel_address = ADOContract::default().get_kernel_address(deps.storage)?;

    let transfer_msg =
        pkt.to_sub_msg(kernel_address.clone(), Some(funds), OSMOSIS_MSG_FORWARD_ID)?;

    resp = resp.add_submessage(transfer_msg).add_attributes(vec![
        attr("action", "swap_and_forward"),
        attr("dex", "osmosis"),
        attr("to_denom", state.to_denom.to_string()),
        attr("to_amount", return_amount),
        attr("forward_addr", address.to_string()),
        attr("kernel_address", kernel_address),
    ]);
    Ok(resp)
}

pub fn query_get_route(
    deps: Deps,
    from_denom: String,
    to_denom: String,
) -> Result<GetRouteResponse, ContractError> {
    let query_msg = OsmosisQueryMsg::GetRoute {
        input_denom: from_denom,
        output_denom: to_denom,
    };

    let swap_router = SWAP_ROUTER.load(deps.storage)?.get_raw_address(&deps)?;

    let res: Result<swaprouter::msg::GetRouteResponse, ContractError> = deps
        .querier
        .query_wasm_smart(swap_router, &query_msg)
        .map_err(ContractError::Std);
    if let Err(err) = res {
        Err(err)
    } else {
        Ok(GetRouteResponse {
            pool_route: res
                .unwrap()
                .pool_route
                .iter()
                .map(|route| SwapRoute {
                    pool_id: route.pool_id,
                    token_out_denom: route.token_out_denom.clone(),
                })
                .collect(),
        })
    }
}
