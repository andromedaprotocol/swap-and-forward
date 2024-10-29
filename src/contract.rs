use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    amp::AndrAddr,
    common::{context::ExecuteContext, denom::Asset},
    error::ContractError,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_json, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
    Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
use cw_utils::one_coin;

use crate::{
    astroport::{
        execute_swap_astroport_msg, handle_astroport_swap, ASTROPORT_MSG_FORWARD_ID,
        ASTROPORT_MSG_SWAP_ID,
    },
    msg::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{ForwardReplyState, FORWARD_REPLY_STATE},
};

const CONTRACT_NAME: &str = "crates.io:swap-and-forward";
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
        ExecuteMsg::Receive(msg) => handle_receive_cw20(ctx, msg),
        ExecuteMsg::SwapAndForward {
            dex,
            to_asset,
            forward_addr,
            forward_msg,
            max_spread,
            minimum_receive,
            ..
        } => execute_swap_and_forward(
            ctx,
            dex,
            to_asset,
            forward_addr,
            forward_msg,
            max_spread,
            minimum_receive,
        ),
        _ => ADOContract::default().execute(ctx, msg),
    }
}

fn handle_receive_cw20(
    ctx: ExecuteContext,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let ExecuteContext { ref info, .. } = ctx;

    let amount = cw20_msg.amount;
    let sender = cw20_msg.sender;
    let from_addr = AndrAddr::from_string(info.sender.clone());
    let from_asset = Asset::Cw20Token(from_addr);

    match from_json(&cw20_msg.msg)? {
        Cw20HookMsg::SwapAndForward {
            dex,
            to_asset,
            forward_addr,
            forward_msg,
            max_spread,
            minimum_receive,
            ..
        } => {
            let forward_addr = match forward_addr {
                None => AndrAddr::from_string(&sender),
                Some(andr_addr) => andr_addr,
            };
            swap_and_forward_cw20(
                ctx,
                dex,
                from_asset,
                Uint128::new(amount.u128()),
                to_asset,
                forward_addr,
                AndrAddr::from_string(sender),
                forward_msg,
                max_spread,
                minimum_receive,
            )
        }
    }
}

fn execute_swap_and_forward(
    ctx: ExecuteContext,
    dex: String,
    to_asset: Asset,
    forward_addr: Option<AndrAddr>,
    forward_msg: Option<Binary>,
    max_spread: Option<Decimal>,
    minimum_receive: Option<Uint128>,
) -> Result<Response, ContractError> {
    let fund = one_coin(&ctx.info).map_err(|_| ContractError::InvalidAsset {
        asset: "Invalid or missing coin".to_string(),
    })?;

    let from_asset = Asset::NativeToken(fund.denom);
    let sender = AndrAddr::from_string(&ctx.info.sender);
    let forward_addr = match forward_addr {
        None => sender.clone(),
        Some(andr_addr) => andr_addr,
    };

    let swap_msg = match dex.as_str() {
        "astroport" => execute_swap_astroport_msg(
            ctx,
            from_asset,
            fund.amount,
            to_asset,
            forward_addr.clone(),
            sender,
            forward_msg,
            max_spread,
            minimum_receive,
        )?,
        _ => return Err(ContractError::Std(StdError::generic_err("Unsupported Dex"))),
    };

    Ok(Response::default().add_submessage(swap_msg))
}

#[allow(clippy::too_many_arguments)]
fn swap_and_forward_cw20(
    ctx: ExecuteContext,
    dex: String,
    from_asset: Asset,
    from_amount: Uint128,
    to_asset: Asset,
    forward_addr: AndrAddr,
    refund_addr: AndrAddr,
    forward_msg: Option<Binary>,
    max_spread: Option<Decimal>,
    minimum_receive: Option<Uint128>,
) -> Result<Response, ContractError> {
    let swap_msg = match dex.as_str() {
        "astroport" => execute_swap_astroport_msg(
            ctx,
            from_asset,
            from_amount,
            to_asset,
            forward_addr.clone(),
            refund_addr,
            forward_msg,
            max_spread,
            minimum_receive,
        )?,
        _ => return Err(ContractError::Std(StdError::generic_err("Unsupported Dex"))),
    };

    Ok(Response::default().add_submessage(swap_msg))
}

pub fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    ADOContract::default().execute(ctx, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    ADOContract::default().query(deps, env, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        ASTROPORT_MSG_SWAP_ID => {
            let state: ForwardReplyState = FORWARD_REPLY_STATE.load(deps.storage)?;
            FORWARD_REPLY_STATE.remove(deps.storage);

            if msg.result.is_err() {
                Err(ContractError::Std(StdError::generic_err(
                    msg.result.unwrap_err(),
                )))
            } else {
                match state.dex.as_str() {
                    "astroport" => handle_astroport_swap(deps, env, msg, state),
                    _ => Err(ContractError::Std(StdError::generic_err("Unsupported dex"))),
                }
            }
        }
        ASTROPORT_MSG_FORWARD_ID => {
            if msg.result.is_err() {
                return Err(ContractError::Std(StdError::generic_err(
                    msg.result.unwrap_err(),
                )));
            }
            Ok(Response::default()
                .add_attributes(vec![attr("action", "message_forwarded_success")]))
        }
        _ => Err(ContractError::Std(StdError::GenericErr {
            msg: "Invalid Reply ID".to_string(),
        })),
    }
}
