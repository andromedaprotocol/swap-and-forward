use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    amp::{AndrAddr, Recipient},
    common::{context::ExecuteContext, encode_binary},
    error::ContractError,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, ensure, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
};
use cw2::set_contract_version;
use cw_utils::one_coin;

use crate::{
    osmosis::{
        execute_swap_osmosis_msg, handle_osmosis_swap_reply, query_get_route,
        OSMOSIS_MSG_FORWARD_ID, OSMOSIS_MSG_SWAP_ID,
    },
    state::{ForwardReplyState, FORWARD_REPLY_STATE, SWAP_ROUTER},
};

use andromeda_swap_and_forward::osmosis::{
    ExecuteMsg, InstantiateMsg, QueryMsg, Slippage, SwapRoute,
};

const CONTRACT_NAME: &str = "crates.io:swap-and-forward-osmosis";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let inst_resp = ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address.clone(),
            owner: msg.owner,
        },
    )?;

    let swap_router = msg
        .swap_router
        .unwrap_or(AndrAddr::from_string("/lib/osmosis/router"));
    swap_router.get_raw_address(&deps.as_ref())?;
    SWAP_ROUTER.save(deps.storage, &swap_router)?;

    Ok(inst_resp
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);

    match msg {
        ExecuteMsg::AMPReceive(pkt) => {
            ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
        }
        _ => handle_execute(ctx, msg),
    }
}

pub fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SwapAndForward {
            to_denom,
            recipient,
            slippage,
            route,
        } => execute_swap_and_forward(ctx, to_denom, recipient, slippage, route),
        ExecuteMsg::UpdateSwapRouter { swap_router } => {
            execute_update_swap_router(ctx, swap_router)
        }
        _ => ADOContract::default().execute(ctx, msg),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_swap_and_forward(
    ctx: ExecuteContext,
    to_denom: String,
    recipient: Option<Recipient>,
    slippage: Slippage,
    route: Option<Vec<SwapRoute>>,
) -> Result<Response, ContractError> {
    let fund = one_coin(&ctx.info).map_err(|_| ContractError::InvalidAsset {
        asset: "Invalid or missing coin".to_string(),
    })?;

    let from_denom = fund.denom;
    let sender = AndrAddr::from_string(&ctx.info.sender);
    let recipient = match recipient {
        None => Recipient::new(sender.clone(), None),
        Some(recipient) => recipient,
    };

    let swap_msg = execute_swap_osmosis_msg(
        ctx,
        from_denom.clone(),
        fund.amount,
        to_denom.clone(),
        recipient.clone(),
        sender,
        slippage,
        route,
    )?;

    Ok(Response::default()
        .add_submessage(swap_msg)
        .add_attributes(vec![
            attr("from_denom", from_denom),
            attr("from_amount", fund.amount),
            attr("to_denom", to_denom),
            attr("recipient", recipient.get_addr()),
        ]))
}

fn execute_update_swap_router(
    ctx: ExecuteContext,
    swap_router: AndrAddr,
) -> Result<Response, ContractError> {
    let sender = ctx.info.sender;
    ensure!(
        ADOContract::default().is_owner_or_operator(ctx.deps.storage, sender.as_ref())?,
        ContractError::Unauthorized {}
    );
    let ExecuteContext { deps, .. } = ctx;

    swap_router.get_raw_address(&deps.as_ref())?;
    let previous_swap_router = SWAP_ROUTER.load(deps.storage)?;

    SWAP_ROUTER.save(deps.storage, &swap_router)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "update-swap-router"),
        attr("previous_swap_router", previous_swap_router),
        attr("swap_router", swap_router),
    ]))
}
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetRoute {
            from_denom,
            to_denom,
        } => encode_binary(&query_get_route(deps, from_denom, to_denom)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        OSMOSIS_MSG_SWAP_ID => {
            let state: ForwardReplyState = FORWARD_REPLY_STATE.load(deps.storage)?;
            FORWARD_REPLY_STATE.remove(deps.storage);

            if msg.result.is_err() {
                Err(ContractError::Std(StdError::generic_err(format!(
                    "Osmosis swap failed with error: {:?}",
                    msg.result.unwrap_err()
                ))))
            } else {
                handle_osmosis_swap_reply(deps, env, msg, state)
            }
        }
        OSMOSIS_MSG_FORWARD_ID => {
            if msg.result.is_err() {
                return Err(ContractError::Std(StdError::generic_err(format!(
                    "Osmosis msg forwarding failed with error: {:?}",
                    msg.result.unwrap_err()
                ))));
            }
            Ok(Response::default()
                .add_attributes(vec![attr("action", "message_forwarded_success")]))
        }
        _ => Err(ContractError::Std(StdError::GenericErr {
            msg: "Invalid Reply ID".to_string(),
        })),
    }
}
