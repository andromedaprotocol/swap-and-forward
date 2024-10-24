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
    use cosmwasm_std::{
        coin,
        testing::{mock_dependencies, mock_env, mock_info},
        Binary, Response, StdError, Uint128,
    };
    use cw20::Cw20ReceiveMsg;

    use crate::{
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
    #[test]
    fn test_swap_and_forward() {
        let env = mock_env();

        let test_cases: Vec<SwapAndForwardTestCase> = vec![
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
        ];
        for test_case in test_cases {
            let mut deps = mock_dependencies();
            init(deps.as_mut());
            if test_case.reply_state.is_some() {
                FORWARD_REPLY_STATE
                    .save(deps.as_mut().storage, &test_case.reply_state.unwrap())
                    .unwrap();
            }
            let (info, msg) = match test_case.from_asset {
                Asset::Cw20Token(cw20_addr) => {
                    let info = mock_info(cw20_addr.as_ref(), &vec![]);
                    let hook_msg = Cw20HookMsg::SwapAndForward {
                        dex: test_case.dex,
                        to_asset: test_case.to_asset,
                        forward_addr: test_case.forward_addr,
                        forward_msg: test_case.forward_msg,
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
                    let info = mock_info(SENDER, &vec![coin(test_case.from_amount.u128(), &denom)]);
                    let msg = ExecuteMsg::SwapAndForward {
                        dex: test_case.dex,
                        to_asset: test_case.to_asset,
                        forward_addr: test_case.forward_addr,
                        forward_msg: test_case.forward_msg,
                        max_spread: None,
                        minimum_receive: None,
                    };
                    (info, msg)
                }
            };
            let res = execute(deps.as_mut(), env.clone(), info, msg);

            assert_eq!(res, test_case.expected_res, "Test case: {}", test_case.name);
        }
    }
}
