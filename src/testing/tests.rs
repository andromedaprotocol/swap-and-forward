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
        swap_router: None,
    };

    let info = mock_info(OWNER, &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use andromeda_app::app::AppComponent;
    use andromeda_finance::splitter::AddressPercent;
    use andromeda_std::{
        amp::{AndrAddr, Recipient},
        common::{denom::Asset, encode_binary},
        error::ContractError,
    };
    use astroport::router::{
        Cw20HookMsg as AstroCw20HookMsg, ExecuteMsg as AstroExecuteMsg, SwapOperation,
    };
    use cosmrs::AccountId;
    use cosmwasm_std::{
        coin,
        testing::{mock_dependencies, mock_env, mock_info},
        to_json_binary, wasm_execute, Addr, Binary, Decimal, DepsMut, Empty, Response, StdError,
        SubMsg, Uint128, WasmMsg,
    };
    use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
    use cw_orch::prelude::*;

    use crate::{
        astroport::{generate_asset_info_from_asset, ASTROPORT_MSG_SWAP_ID},
        contract::execute,
        interfaces::{
            app_interface::AppContract, swap_and_forward_interface::SwapAndForwardContract,
        },
        msg::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, SwapOperation as SocketSwapOperation},
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

        let router_address = AndrAddr::from_string("/lib/astroport/router")
            .get_raw_address(&deps.as_ref())
            .unwrap();
        let msg = WasmMsg::Execute {
            contract_addr: router_address.to_string(),
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

        let router_address = AndrAddr::from_string("/lib/astroport/router")
            .get_raw_address(&deps.as_ref())
            .unwrap();
        let send_msg = Cw20ExecuteMsg::Send {
            contract: router_address.to_string(),
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

    #[test]
    fn test_execute_swap() {
        let mut deps = mock_dependencies();
        init(deps.as_mut());
        let denom = "uluna";
        let env = mock_env();

        let exec_msg = ExecuteMsg::SwapAndForward {
            dex: "astroport".to_string(),
            to_asset: Asset::Cw20Token(AndrAddr::from_string(
                "terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv",
            )),
            forward_addr: None,
            forward_msg: None,
            max_spread: None,
            minimum_receive: None,
        };
        let info = mock_info(SENDER, &[coin(1000000, denom)]);

        let res = execute(deps.as_mut(), env.clone(), info, exec_msg);

        let expected_operations = vec![SwapOperation::AstroSwap {
            offer_asset_info: astroport::asset::AssetInfo::NativeToken {
                denom: denom.to_string(),
            },
            ask_asset_info: astroport::asset::AssetInfo::Token {
                contract_addr: Addr::unchecked(
                    "terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv",
                ),
            },
        }];
        let expected_astro_swap_msg = AstroExecuteMsg::ExecuteSwapOperations {
            operations: expected_operations,
            to: None,
            max_spread: None,
            minimum_receive: None,
        };

        let exec_msg = WasmMsg::Execute {
            contract_addr: AccountId::from_str(
                "terra1j8hayvehh3yy02c2vtw5fdhz9f4drhtee8p5n5rguvg3nyd6m83qd2y90a",
            )
            .unwrap()
            .to_string(),
            msg: cosmwasm_std::Binary(serde_json::to_vec(&expected_astro_swap_msg).unwrap()),
            funds: vec![coin(1000000, denom)],
        };
        let sub_msg: SubMsg<Empty> = SubMsg::reply_always(exec_msg, ASTROPORT_MSG_SWAP_ID);
        assert_eq!(res, Ok(Response::default().add_submessage(sub_msg)));
    }

    use cw_orch_daemon::{networks::PHOENIX_1, Daemon, TxSender};
    use dotenv::dotenv;
    #[ignore]
    #[test]
    fn test_onchain_native() {
        // 1. prepare environment and variables
        dotenv().ok();
        env_logger::init();
        let mnemonic = std::env::var("MNEMONIC").expect("MNEMONIC must be set.");
        let daemon = Daemon::builder(PHOENIX_1)
            .mnemonic(mnemonic)
            .build()
            .unwrap();
        let denom = PHOENIX_1.gas_denom;
        // ==================================================================================== //

        // 2. prepare swap-and-forward ado
        let swap_and_forward_contract =
            SwapAndForwardContract::new("swap-and-forward", daemon.clone());

        // upload contract (not needed if contract is already uploaded)
        // swap_and_forward_contract.upload().unwrap();
        // println!("swap_and_forward_contract code_id: {:?}", swap_and_forward_contract.code_id().unwrap());
        // use code_id if contract is already uploaded
        swap_and_forward_contract.set_code_id(3354);
        // ==================================================================================== //

        // 3. prepare app ado
        let app_contract = AppContract::new("app-contract", daemon.clone());
        app_contract.set_code_id(2834);

        let swap_and_forward_init_msg = InstantiateMsg {
            kernel_address: "terra1g0vzxc6a0layhxdwc24kwwam4v93pjmam5a77wtvfhzpdltp82estk3kpc"
                .to_string(),
            owner: None,
            swap_router: None,
        };
        let swap_and_forward_component = AppComponent::new(
            "swap-and-forward",
            "swap-and-forward",
            to_json_binary(&swap_and_forward_init_msg).unwrap(),
        );

        // 3.1 use initialized app contract if needed
        // let app_components = vec![swap_and_forward_component.clone()];
        // app_contract.init("swap and forward without splitter", app_components, None);
        // println!("=======================app contract addresss: {:?}", app_contract.addr_str());
        app_contract.set_address(&Addr::unchecked(
            "terra1eyw5nmej8n0hq6vav8e3ts395as4hme0la6mjgzs9kszkurm8aps6knkl6",
        ));

        // // 3.2 migrate app component if needed
        let swap_and_forward_addr =
            app_contract.query_address_by_component_name(swap_and_forward_component.name);
        swap_and_forward_contract.set_address(&Addr::unchecked(swap_and_forward_addr.clone()));
        // let res = swap_and_forward_contract
        //     .migrate(&MigrateMsg {}, swap_and_forward_contract.code_id().unwrap()).unwrap();
        // println!("=========================={:?}==========================", res);
        // ==================================================================================== //

        // 4. execute swap operation
        swap_and_forward_contract.execute_swap_from_native(
            "astroport".to_string(),
            Asset::Cw20Token(AndrAddr::from_string(
                "terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv",
            )),
            None,
            None,
            None,
            None,
            &[coin(2000000, denom)],
        );
        // ==================================================================================== //

        // 5. manual astroswap operation via astroport router
        // let operations = vec![SwapOperation::AstroSwap {
        //     offer_asset_info: astroport::asset::AssetInfo::NativeToken { denom: denom.to_string() },
        //     ask_asset_info: astroport::asset::AssetInfo::Token {contract_addr: Addr::unchecked("terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv")}
        // }];
        // let astro_swap_msg = AstroExecuteMsg::ExecuteSwapOperations {
        //     operations,
        //     to: None,
        //     max_spread: None,
        //     minimum_receive: None,
        // };
        // let exec_msg: MsgExecuteContract = MsgExecuteContract {
        //     sender: daemon.sender().account_id(),
        //     contract: AccountId::from_str("terra1j8hayvehh3yy02c2vtw5fdhz9f4drhtee8p5n5rguvg3nyd6m83qd2y90a").unwrap(),
        //     msg: serde_json::to_vec(&astro_swap_msg).unwrap(),
        //     funds: vec![
        //         cosmrs::Coin {
        //             amount: 1000000,
        //             denom: Denom::from_str(denom).unwrap()
        //         }
        //     ],
        // };
        // let result = daemon.rt_handle.block_on(
        //     async {
        //         daemon.sender().commit_tx(vec![exec_msg], None).await
        //     }
        // );
        // ==================================================================================== //
    }
    #[ignore]
    #[test]
    fn test_onchain_cw20() {
        // 1. prepare environment and variables
        dotenv().ok();
        env_logger::init();
        let mnemonic = std::env::var("MNEMONIC").expect("MNEMONIC must be set.");

        let daemon = Daemon::builder(PHOENIX_1)
            .mnemonic(mnemonic)
            .build()
            .unwrap();
        let denom = PHOENIX_1.gas_denom;
        // ==================================================================================== //

        // 2. prepare swap-and-forward ado
        let swap_and_forward_contract =
            SwapAndForwardContract::new("swap-and-forward", daemon.clone());

        // upload contract (not needed if contract is already uploaded)
        // swap_and_forward_contract.upload().unwrap();
        // println!("swap_and_forward_contract code_id: {:?}", swap_and_forward_contract.code_id().unwrap());
        // use code_id if contract is already uploaded
        swap_and_forward_contract.set_code_id(3354);
        // ==================================================================================== //

        // 3. prepare app ado
        let app_contract = AppContract::new("app-contract", daemon.clone());
        app_contract.set_code_id(2834);

        // swap and forward ado
        let swap_and_forward_init_msg = InstantiateMsg {
            kernel_address: "terra1g0vzxc6a0layhxdwc24kwwam4v93pjmam5a77wtvfhzpdltp82estk3kpc"
                .to_string(),
            owner: None,
            swap_router: None,
        };
        let swap_and_forward_component = AppComponent::new(
            "swap-and-forward",
            "swap-and-forward",
            to_json_binary(&swap_and_forward_init_msg).unwrap(),
        );

        // prepare splitter ado
        let recipients = vec![
            AddressPercent {
                recipient: Recipient::from_string("terra1rrygg76zspacdmqf7elgleehjmsvvz726ysnng"),
                percent: Decimal::from_str("0.5").unwrap(),
            },
            AddressPercent {
                recipient: Recipient::from_string("terra1nk4lzshp0kkg9xw40vrjqd8ty4vh7gehztr2nv"),
                percent: Decimal::from_str("0.5").unwrap(),
            },
        ];

        let splitter_init_msg = andromeda_finance::splitter::InstantiateMsg {
            recipients,
            lock_time: None,
            kernel_address: "terra1g0vzxc6a0layhxdwc24kwwam4v93pjmam5a77wtvfhzpdltp82estk3kpc"
                .to_string(),
            owner: None,
        };

        let splitter_component = AppComponent::new(
            "splitter",
            "splitter",
            to_json_binary(&splitter_init_msg).unwrap(),
        );

        // 3.1 Initialize app contract if needed
        // let app_components = vec![splitter_component.clone(), swap_and_forward_component.clone()];
        // app_contract.init("swap and forward with splitter", app_components, None);
        // println!("====================app address: {:?}", app_contract.addr_str());
        // 3.2 use initialized app contract if already initialized
        app_contract.set_address(&Addr::unchecked(
            "terra1eyw5nmej8n0hq6vav8e3ts395as4hme0la6mjgzs9kszkurm8aps6knkl6",
        ));

        // 3.3 migrate app component if needed
        let swap_and_forward_addr =
            app_contract.query_address_by_component_name(swap_and_forward_component.name);
        swap_and_forward_contract.set_address(&Addr::unchecked(swap_and_forward_addr.clone()));

        // swap_and_forward_contract.migrate(&MigrateMsg {}, swap_and_forward_contract.code_id().unwrap());
        // ==================================================================================== //

        // swap_and_forward_contract.clone().query_astrport_simulate_swap_operation(
        //     Uint128::new(381492581367), SocketSwapOperation {
        //         offer_asset_info: Asset::Cw20Token(AndrAddr::from_string("terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv")),
        //         ask_asset_info: Asset::NativeToken("uluna".to_owned()),
        //     }
        // );
        // // 4. execute swap operation
        // let forward_addr = AndrAddr::from_string(format!("./{}", splitter_component.clone().name));
        let forward_addr = AndrAddr::from_string(format!(
            "/home/{}/swap_and_forward_with_splitter/{}",
            daemon.sender().address(),
            splitter_component.name
        ));
        let forward_msg =
            to_json_binary(&andromeda_finance::splitter::ExecuteMsg::Send {}).unwrap();

        swap_and_forward_contract.execute_swap_from_cw20(
            &daemon,
            "astroport".to_string(),
            "terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv",
            Uint128::new(381492581367),
            Asset::NativeToken(denom.to_string()),
            // None,
            // None,
            Some(forward_addr),
            Some(forward_msg),
            None,
            None,
        );

        // 5. Manual swap operation
        // let operations = vec![SwapOperation::AstroSwap {
        //     ask_asset_info: astroport::asset::AssetInfo::NativeToken { denom: denom.to_string() },
        //     offer_asset_info: astroport::asset::AssetInfo::Token {contract_addr: Addr::unchecked("terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv")}
        // }];

        // let astro_swap_msg = AstroCw20HookMsg::ExecuteSwapOperations {
        //     operations,
        //     to: None,
        //     max_spread: None,
        //     minimum_receive: None,
        // };
        // let cw_20_transfer_msg = cw20::Cw20ExecuteMsg::Send {
        //     contract: "terra1j8hayvehh3yy02c2vtw5fdhz9f4drhtee8p5n5rguvg3nyd6m83qd2y90a".to_string(),
        //     amount: Uint128::new(375096832972),
        //     msg: to_json_binary(&astro_swap_msg).unwrap(),
        // };
        // let exec_msg: MsgExecuteContract = MsgExecuteContract {
        //     sender:  daemon.sender().account_id(),
        //     contract: AccountId::from_str("terra1lxx40s29qvkrcj8fsa3yzyehy7w50umdvvnls2r830rys6lu2zns63eelv").unwrap(),
        //     msg: serde_json::to_vec(&cw_20_transfer_msg).unwrap(),
        //     funds: vec![],
        // };
        //  let result = daemon.rt_handle.block_on(
        //     async {
        //         daemon.sender().commit_tx(vec![exec_msg], None).await
        //     }
        // );
        // println!("==========================result: {:?}", result);
    }
    #[ignore]
    #[test]
    fn test_onchain_native_to_native() {
        // 1. prepare environment and variables
        dotenv().ok();
        env_logger::init();
        let mnemonic = std::env::var("MNEMONIC").expect("MNEMONIC must be set.");

        let daemon = Daemon::builder(PHOENIX_1)
            .mnemonic(mnemonic)
            .build()
            .unwrap();
        let denom = PHOENIX_1.gas_denom;
        // ==================================================================================== //

        // 2. prepare swap-and-forward ado
        let swap_and_forward_contract =
            SwapAndForwardContract::new("swap-and-forward", daemon.clone());

        swap_and_forward_contract.set_code_id(3354);
        // ==================================================================================== //

        // 3. prepare app ado
        let app_contract = AppContract::new("app-contract", daemon.clone());
        app_contract.set_code_id(2834);

        // swap and forward ado
        let swap_and_forward_init_msg = InstantiateMsg {
            kernel_address: "terra1g0vzxc6a0layhxdwc24kwwam4v93pjmam5a77wtvfhzpdltp82estk3kpc"
                .to_string(),
            owner: None,
            swap_router: None,
        };
        let swap_and_forward_component = AppComponent::new(
            "swap-and-forward",
            "swap-and-forward",
            to_json_binary(&swap_and_forward_init_msg).unwrap(),
        );

        // prepare splitter ado
        let recipients = vec![
            AddressPercent {
                recipient: Recipient::from_string("terra1rrygg76zspacdmqf7elgleehjmsvvz726ysnng"),
                percent: Decimal::from_str("0.5").unwrap(),
            },
            AddressPercent {
                recipient: Recipient::from_string("terra1nk4lzshp0kkg9xw40vrjqd8ty4vh7gehztr2nv"),
                percent: Decimal::from_str("0.5").unwrap(),
            },
        ];

        let splitter_init_msg = andromeda_finance::splitter::InstantiateMsg {
            recipients,
            lock_time: None,
            kernel_address: "terra1g0vzxc6a0layhxdwc24kwwam4v93pjmam5a77wtvfhzpdltp82estk3kpc"
                .to_string(),
            owner: None,
        };

        let splitter_component = AppComponent::new(
            "splitter",
            "splitter",
            to_json_binary(&splitter_init_msg).unwrap(),
        );

        // 3.1 Initialize app contract if needed
        // let app_components = vec![splitter_component.clone(), swap_and_forward_component.clone()];
        // app_contract.init("swap and forward with splitter", app_components, None);
        // println!("====================app address: {:?}", app_contract.addr_str());
        // 3.2 use initialized app contract if already initialized
        app_contract.set_address(&Addr::unchecked(
            "terra1eyw5nmej8n0hq6vav8e3ts395as4hme0la6mjgzs9kszkurm8aps6knkl6",
        ));

        // 3.3 migrate app component if needed
        let swap_and_forward_addr =
            app_contract.query_address_by_component_name(swap_and_forward_component.name);
        swap_and_forward_contract.set_address(&Addr::unchecked(swap_and_forward_addr.clone()));

        // simulate swap from gas denom to astro token
        swap_and_forward_contract
            .clone()
            .query_astrport_simulate_swap_operation(
                Uint128::new(2000000),
                SocketSwapOperation {
                    offer_asset_info: Asset::NativeToken("uluna".to_owned()),
                    ask_asset_info: Asset::NativeToken(
                        "ibc/8D8A7F7253615E5F76CB6252A1E1BD921D5EDB7BBAAF8913FB1C77FF125D9995"
                            .to_owned(),
                    ),
                },
            );
        // // 4. execute swap operation
        let forward_addr = AndrAddr::from_string(format!(
            "/home/{}/swap_and_forward_with_splitter/{}",
            daemon.sender().address(),
            splitter_component.name
        ));
        let forward_msg =
            to_json_binary(&andromeda_finance::splitter::ExecuteMsg::Send {}).unwrap();

        swap_and_forward_contract.execute_swap_from_native(
            "astroport".to_string(),
            Asset::NativeToken(
                "ibc/8D8A7F7253615E5F76CB6252A1E1BD921D5EDB7BBAAF8913FB1C77FF125D9995".to_owned(),
            ),
            Some(forward_addr),
            Some(forward_msg),
            None,
            None,
            &[coin(2000000, denom)],
        );
    }
}
