use andromeda_std::testing::mock_querier::MOCK_KERNEL_CONTRACT;
use cosmwasm_std::{
    testing::{mock_env, mock_info},
    DepsMut, Response,
};

use crate::{contract::instantiate, msg::InstantiateMsg};

pub const OWNER: &str = "owner";
pub const SENDER: &str = "sender";

fn init(deps: DepsMut) -> Response {
    let msg = InstantiateMsg {
        owner: Some(OWNER.to_owned()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
    };

    let info = mock_info(OWNER, &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

#[cfg(test)]
mod test {
    use andromeda_std::{
        amp::AndrAddr,
        common::{denom::Asset, encode_binary},
        error::ContractError,
    };
    use astroport::router::{
        Cw20HookMsg as AstroCw20HookMsg, ExecuteMsg as AstroExecuteMsg, SwapOperation,
    };
    use cosmwasm_std::{
        coin,
        testing::{mock_dependencies, mock_env, mock_info},
        to_json_binary, wasm_execute, Binary, Decimal, DepsMut, Response, StdError, SubMsg,
        Uint128, WasmMsg,
    };
    use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

    use crate::{
        astroport::{generate_asset_info_from_asset, ASTROPORT_MSG_SWAP_ID, ASTRO_ROUTER_ADDRESS},
        contract::execute,
        msg::{Cw20HookMsg, ExecuteMsg},
        state::{ForwardReplyState, FORWARD_REPLY_STATE},
    };

    use super::{init, SENDER};

    struct SwapAndForwardTestCase {
        name: String,
        from_asset: Asset,
        from_amount: Uint128,
        to_asset: Asset,
        dex: String,
        forward_addr: Option<AndrAddr>,
        forward_msg: Option<Binary>,
        reply_state: Option<ForwardReplyState>,
        expected_res: Result<Response, ContractError>,
    }
    fn swap_and_forward_native_response(
        deps: DepsMut,
        from_asset: Asset,
        to_asset: Asset,
        from_amount: Uint128,
        minimum_receive: Option<Uint128>,
        max_spread: Option<Decimal>,
    ) -> Response {
        let operations = vec![SwapOperation::AstroSwap {
            offer_asset_info: generate_asset_info_from_asset(&deps.as_ref(), from_asset.clone())
                .unwrap(),
            ask_asset_info: generate_asset_info_from_asset(&deps.as_ref(), to_asset.clone())
                .unwrap(),
        }];

        let from_denom = match from_asset.clone() {
            Asset::NativeToken(denom) => denom,
            _ => panic!("from_asset should be native token"),
        };
        let msg = AstroExecuteMsg::ExecuteSwapOperations {
            operations,
            to: None,
            max_spread,
            minimum_receive,
        };
        let msg = WasmMsg::Execute {
            contract_addr: ASTRO_ROUTER_ADDRESS.to_string(),
            msg: to_json_binary(&msg).unwrap(),
            funds: vec![coin(from_amount.u128(), from_denom)],
        };
        let sub_msg = SubMsg::reply_always(msg, ASTROPORT_MSG_SWAP_ID);
        Response::default().add_submessage(sub_msg)
    }

    fn swap_and_forward_cw20_response(
        deps: DepsMut,
        from_asset: Asset,
        to_asset: Asset,
        from_amount: Uint128,
        minimum_receive: Option<Uint128>,
        max_spread: Option<Decimal>,
    ) -> Response {
        let operations = vec![SwapOperation::AstroSwap {
            offer_asset_info: generate_asset_info_from_asset(&deps.as_ref(), from_asset.clone())
                .unwrap(),
            ask_asset_info: generate_asset_info_from_asset(&deps.as_ref(), to_asset.clone())
                .unwrap(),
        }];

        let cw20_contract = match from_asset.clone() {
            Asset::Cw20Token(andr_addr) => andr_addr
                .get_raw_address(&deps.as_ref())
                .unwrap()
                .to_string(),
            _ => panic!("from_asset should be native token"),
        };
        let astro_swap_hook_msg = AstroCw20HookMsg::ExecuteSwapOperations {
            operations,
            to: None,
            max_spread,
            minimum_receive,
        };
        let send_msg = Cw20ExecuteMsg::Send {
            contract: ASTRO_ROUTER_ADDRESS.to_string(),
            amount: from_amount,
            msg: to_json_binary(&astro_swap_hook_msg).unwrap(),
        };
        let msg = wasm_execute(cw20_contract, &send_msg, vec![]).unwrap();
        let sub_msg = SubMsg::reply_always(msg, ASTROPORT_MSG_SWAP_ID);
        Response::default().add_submessage(sub_msg)
    }

    #[test]
    fn test_swap_and_forward() {
        let env = mock_env();
        let test_cases: Vec<SwapAndForwardTestCase> = vec![
            SwapAndForwardTestCase {
                name: "Invalid or Missing coin".to_string(),
                from_asset: Asset::NativeToken("uosmo".to_string()),
                from_amount: Uint128::new(0),
                to_asset: Asset::Cw20Token(AndrAddr::from_string("to_asset")),
                dex: "astroport".to_string(),
                forward_addr: None,
                forward_msg: None,
                reply_state: None,
                expected_res: Err(ContractError::InvalidAsset {
                    asset: "Invalid or missing coin".to_string(),
                }),
            },
            SwapAndForwardTestCase {
                name: "invalid dex with native asset".to_string(),
                from_asset: Asset::NativeToken("uosmo".to_string()),
                from_amount: Uint128::new(100),
                to_asset: Asset::Cw20Token(AndrAddr::from_string("to_asset")),
                dex: "dummy dex".to_string(),
                forward_addr: None,
                forward_msg: None,
                reply_state: None,
                expected_res: Err(ContractError::Std(StdError::generic_err("Unsupported Dex"))),
            },
            SwapAndForwardTestCase {
                name: "invalid dex with cw20 asset".to_string(),
                from_asset: Asset::Cw20Token(AndrAddr::from_string("from_asset")),
                from_amount: Uint128::new(100),
                to_asset: Asset::NativeToken("uosmo".to_string()),
                dex: "dummy dex".to_string(),
                forward_addr: None,
                forward_msg: None,
                reply_state: None,
                expected_res: Err(ContractError::Std(StdError::generic_err("Unsupported Dex"))),
            },
            SwapAndForwardTestCase {
                name: "invalid reply state with native from_asset".to_string(),
                from_asset: Asset::NativeToken("uosmo".to_string()),
                from_amount: Uint128::new(100),
                to_asset: Asset::Cw20Token(AndrAddr::from_string("from_asset")),
                dex: "astroport".to_string(),
                forward_addr: None,
                forward_msg: None,
                reply_state: Some(ForwardReplyState {
                    addr: AndrAddr::from_string("forward_addr"),
                    refund_addr: AndrAddr::from_string("refund_addr"),
                    msg: None,
                    dex: "astroport".to_string(),
                    amp_ctx: None,
                    from_asset: Asset::Cw20Token(AndrAddr::from_string("from_asset")),
                    to_asset: Asset::NativeToken("uosmo".to_string()),
                }),
                expected_res: Err(ContractError::Unauthorized {}),
            },
            SwapAndForwardTestCase {
                name: "invalid reply state with cw20 from_asset".to_string(),
                from_asset: Asset::Cw20Token(AndrAddr::from_string("from_asset")),
                from_amount: Uint128::new(100),
                to_asset: Asset::NativeToken("uosmo".to_string()),
                dex: "astroport".to_string(),
                forward_addr: None,
                forward_msg: None,
                reply_state: Some(ForwardReplyState {
                    addr: AndrAddr::from_string("forward_addr"),
                    refund_addr: AndrAddr::from_string("refund_addr"),
                    msg: None,
                    dex: "astroport".to_string(),
                    amp_ctx: None,
                    from_asset: Asset::Cw20Token(AndrAddr::from_string("from_asset")),
                    to_asset: Asset::NativeToken("uosmo".to_string()),
                }),
                expected_res: Err(ContractError::Unauthorized {}),
            },
            SwapAndForwardTestCase {
                name: "Duplicated Tokens".to_string(),
                from_asset: Asset::NativeToken("uosmo".to_string()),
                from_amount: Uint128::new(100),
                to_asset: Asset::NativeToken("uosmo".to_string()),
                dex: "astroport".to_string(),
                forward_addr: None,
                forward_msg: None,
                reply_state: None,
                expected_res: Err(ContractError::DuplicateTokens {}),
            },
            SwapAndForwardTestCase {
                name: "Successful swap and forward with native token".to_string(),
                from_asset: Asset::NativeToken("uosmo".to_string()),
                from_amount: Uint128::new(100),
                to_asset: Asset::Cw20Token(AndrAddr::from_string("to_asset")),
                dex: "astroport".to_string(),
                forward_addr: None,
                forward_msg: None,
                reply_state: None,
                expected_res: Ok(swap_and_forward_native_response(
                    mock_dependencies().as_mut(),
                    Asset::NativeToken("uosmo".to_string()),
                    Asset::Cw20Token(AndrAddr::from_string("to_asset")),
                    Uint128::new(100),
                    None,
                    None,
                )),
            },
            SwapAndForwardTestCase {
                name: "Successful swap and forward with cw20 token".to_string(),
                from_asset: Asset::Cw20Token(AndrAddr::from_string("from_asset")),
                from_amount: Uint128::new(100),
                to_asset: Asset::NativeToken("uosmo".to_string()),
                dex: "astroport".to_string(),
                forward_addr: None,
                forward_msg: None,
                reply_state: None,
                expected_res: Ok(swap_and_forward_cw20_response(
                    mock_dependencies().as_mut(),
                    Asset::Cw20Token(AndrAddr::from_string("from_asset")),
                    Asset::NativeToken("uosmo".to_string()),
                    Uint128::new(100),
                    None,
                    None,
                )),
            },
        ];
        for test_case in test_cases {
            let mut deps = mock_dependencies();
            init(deps.as_mut());
            if test_case.reply_state.is_some() {
                FORWARD_REPLY_STATE
                    .save(deps.as_mut().storage, &test_case.reply_state.unwrap())
                    .unwrap();
            }
            let (info, msg) = match test_case.from_asset.clone() {
                Asset::Cw20Token(cw20_addr) => {
                    let info = mock_info(cw20_addr.as_ref(), &[]);
                    let hook_msg = Cw20HookMsg::SwapAndForward {
                        dex: test_case.dex.clone(),
                        to_asset: test_case.to_asset.clone(),
                        forward_addr: test_case.forward_addr.clone(),
                        forward_msg: test_case.forward_msg.clone(),
                        max_spread: None,
                        minimum_receive: None,
                    };
                    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
                        sender: SENDER.to_owned(),
                        amount: test_case.from_amount,
                        msg: encode_binary(&hook_msg).unwrap(),
                    });
                    (info, msg)
                }
                Asset::NativeToken(denom) => {
                    let info = mock_info(SENDER, &[coin(test_case.from_amount.u128(), &denom)]);
                    let msg = ExecuteMsg::SwapAndForward {
                        dex: test_case.dex.clone(),
                        to_asset: test_case.to_asset.clone(),
                        forward_addr: test_case.forward_addr.clone(),
                        forward_msg: test_case.forward_msg.clone(),
                        max_spread: None,
                        minimum_receive: None,
                    };
                    (info, msg)
                }
            };
            let res = execute(deps.as_mut(), env.clone(), info, msg);

            assert_eq!(res, test_case.expected_res, "Test case: {}", test_case.name);

            if res.is_ok() {
                let state = FORWARD_REPLY_STATE.load(deps.as_ref().storage).unwrap();
                let expected_state = ForwardReplyState {
                    addr: test_case
                        .forward_addr
                        .unwrap_or(AndrAddr::from_string(SENDER.to_string())),
                    refund_addr: AndrAddr::from_string(SENDER.to_string()),
                    msg: test_case.forward_msg,
                    dex: test_case.dex,
                    amp_ctx: None,
                    from_asset: test_case.from_asset,
                    to_asset: test_case.to_asset,
                };
                assert_eq!(state, expected_state, "Test case: {}", test_case.name);
            }
        }
    }
}
