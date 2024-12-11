#[cfg(test)]
mod test {
    use std::str::FromStr;

    use andromeda_app::app::AppComponent;
    use andromeda_finance::splitter::AddressPercent;
    use andromeda_std::amp::{AndrAddr, Recipient};

    use cosmwasm_std::{coin, to_json_binary, Addr, Decimal, Uint128};
    use cw_orch::prelude::*;

    use crate::interfaces::swap_and_forward_interface::SwapAndForwardContract;
    use andromeda_app_contract::AppContract;

    use andromeda_swap_and_forward::osmosis::{
        ExecuteMsgFns, InstantiateMsg, QueryMsgFns, Slippage, SwapRoute,
    };

    use cw_orch_daemon::{networks::OSMO_5, Daemon, TxSender};
    use dotenv::dotenv;

    #[allow(dead_code)]
    fn upload_ado(daemon: &Daemon) {
        let swap_and_forward_contract =
            SwapAndForwardContract::new("swap-and-forward", daemon.clone());
        swap_and_forward_contract.upload().unwrap();
        println!(
            "swap_and_forward_contract code_id: {:?}",
            swap_and_forward_contract.code_id().unwrap()
        );
    }
    #[allow(dead_code)]
    fn instantiate_ado_with_splitter(
        daemon: &Daemon,
        app_name: &str,
        swap_and_forward_component_name: &str,
        splitter_component_name: &str,
    ) -> String {
        let app_code_id = 11766;
        let kernel_address = "osmo1kjzha97wvwhpxc83dwcxad8w4cfau4k9vul2vcezuteh0n4jaf3sg9csr4";
        let swap_ado_type = "swap-and-forward@0.1.2";
        let recipient_1 = "osmo18epw87zc64a6m63323l6je0nlwdhnjpghtsyq8";
        let recipient_2 = "osmo13refwx2f8wkjt9htss6ken96ak924k79ehf56k";

        let app_contract = AppContract::new(daemon.clone());
        app_contract.set_code_id(app_code_id);
        let swap_and_forward_init_msg = InstantiateMsg {
            kernel_address: kernel_address.to_string(),
            owner: None,
            swap_router: Some(AndrAddr::from_string(
                "osmo19upgmw22nyg9qc8prw6s8ncljkfjdq62xylxhtretm9uzzvj45sq46ggv9".to_string(),
            )),
        };
        let swap_and_forward_component = AppComponent::new(
            swap_and_forward_component_name,
            swap_ado_type,
            to_json_binary(&swap_and_forward_init_msg).unwrap(),
        );

        // prepare splitter ado
        let recipients = vec![
            AddressPercent {
                recipient: Recipient::from_string(recipient_1),
                percent: Decimal::from_str("0.5").unwrap(),
            },
            AddressPercent {
                recipient: Recipient::from_string(recipient_2),
                percent: Decimal::from_str("0.5").unwrap(),
            },
        ];

        let splitter_init_msg = andromeda_finance::splitter::InstantiateMsg {
            recipients,
            default_recipient: None,
            lock_time: None,
            kernel_address: kernel_address.to_string(),
            owner: None,
        };

        let splitter_component = AppComponent::new(
            splitter_component_name,
            "splitter",
            to_json_binary(&splitter_init_msg).unwrap(),
        );
        let app_components = vec![
            splitter_component.clone(),
            swap_and_forward_component.clone(),
        ];

        app_contract
            .instantiate(
                &andromeda_app::app::InstantiateMsg {
                    app_components,
                    name: app_name.to_string(),
                    chain_info: None,
                    kernel_address: kernel_address.to_string(),
                    owner: None,
                },
                None,
                None,
            )
            .unwrap();
        app_contract.addr_str().unwrap()
    }

    #[ignore]
    #[test]
    fn test_onchain_native() {
        let app_name = "swap and forward ado-0.1.2";
        let app_name_parsed = app_name.replace(' ', "_");

        let swap_and_forward_component_name = "swap-and-forward";
        let splitter_component_name = "splitter";

        dotenv().ok();
        env_logger::init();
        let mnemonic = std::env::var("TEST_MNEMONIC").expect("MNEMONIC must be set.");
        let daemon = Daemon::builder(OSMO_5).mnemonic(mnemonic).build().unwrap();
        let denom = OSMO_5.gas_denom;

        // upload ado if not uploaded
        // upload_ado(&daemon);
        // return;

        let app_contract = AppContract::new(daemon.clone());
        // instanitate app
        let app_address = instantiate_ado_with_splitter(
            &daemon,
            app_name,
            swap_and_forward_component_name,
            splitter_component_name,
        );
        app_contract.set_address(&Addr::unchecked(
            app_address,
            // "osmo1xwjy2t6ugdr8nxlz67kwj93j5kqaqnl25rl9jhacl9z43r8pf2dq22svf5",
        ));

        let swap_and_forward_addr: String =
            app_contract.get_address(swap_and_forward_component_name);

        let swap_and_forward_contract =
            SwapAndForwardContract::new("swap-and-forward", daemon.clone());
        swap_and_forward_contract.set_address(&Addr::unchecked(swap_and_forward_addr.clone()));

        // 4. execute swap operation
        let slippage = Slippage::MinOutputAmount(Uint128::one());
        let atom_denom =
            "ibc/A8C2D23A1E6F95DA4E48BA349667E322BD7A6C996D8A4AAE8BA72E190F3D1477".to_string();
        let _res = swap_and_forward_contract.get_route("uosmo", atom_denom.clone());
        let forward_msg =
            to_json_binary(&andromeda_finance::splitter::ExecuteMsg::Send { config: None })
                .unwrap();
        let forward_addr = Recipient::new(
            format!(
                "/home/{}/{}/{}",
                daemon.sender().address(),
                app_name_parsed,
                splitter_component_name
            ),
            Some(forward_msg),
        );
        swap_and_forward_contract
            .swap_and_forward(
                slippage,
                atom_denom.clone(),
                Some(forward_addr),
                Some(vec![SwapRoute {
                    pool_id: 94,
                    token_out_denom: atom_denom.to_string(),
                }]),
                &[coin(1000000, denom)],
            )
            .unwrap();
    }
}
