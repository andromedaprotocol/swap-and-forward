#[cfg(test)]
mod test {
    use std::str::FromStr;

    use andromeda_app::app::AppComponent;
    use andromeda_app_contract::AppContract;
    use andromeda_finance::splitter::AddressPercent;
    use andromeda_std::{
        amp::{AndrAddr, Recipient},
        common::denom::Asset,
    };

    use cosmwasm_std::{coin, to_json_binary, Addr, Decimal, Uint128};
    use cw_orch::prelude::*;

    use crate::interfaces::swap_and_forward_interface::SwapAndForwardContract;
    use andromeda_swap_and_forward::astroport::{ExecuteMsgFns, InstantiateMsg};

    use cw_orch_daemon::{networks::PION_1, Daemon, TxSender};
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
        let app_code_id = 7966;
        let kernel_address = "neutron1zlwfu3wurn98zv3qe4cln0p4crwvfvjkn703vhhcajh6h3v00zzsdadsd8";
        let swap_ado_type = "swap-and-forward@0.1.0";
        let recipient_1 = "neutron13refwx2f8wkjt9htss6ken96ak924k794nnxkr";
        let recipient_2 = "neutron1wjnyhp5x3csl4nte8kpg0unzxn74x22nc5p0me";

        let app_contract = AppContract::new(daemon.clone());
        app_contract.set_code_id(app_code_id);
        let swap_and_forward_init_msg = InstantiateMsg {
            kernel_address: kernel_address.to_string(),
            owner: None,
            swap_router: None,
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
            lock_time: None,
            kernel_address: kernel_address.to_string(),
            owner: None,
            default_recipient: None,
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
        // let app_name = "swap and forward ado";
        let swap_and_forward_component_name = "swap-and-forward";
        // let splitter_component_name = "splitter";

        dotenv().ok();
        env_logger::init();
        let mnemonic = std::env::var("MNEMONIC").expect("MNEMONIC must be set.");
        let daemon = Daemon::builder(PION_1).mnemonic(mnemonic).build().unwrap();
        let denom = PION_1.gas_denom;

        // upload ado if not uploaded
        // upload_ado(&daemon);

        let app_contract = AppContract::new(daemon.clone());
        // instanitate app
        // let app_address = instantiate_ado_with_splitter(
        //     &daemon,
        //     app_name,
        //     swap_and_forward_component_name,
        //     splitter_component_name,
        // );
        // println!("======================app_address: {:?}", app_address);
        app_contract.set_address(&Addr::unchecked(
            // app_address,
            "neutron1x5mj0565mqrq5h7wsm66jlsscu0svlx2yj9ydkrwer5pmysj6v4shyu8mk",
        ));

        let swap_and_forward_addr: String =
            app_contract.get_address(swap_and_forward_component_name);

        let swap_and_forward_contract =
            SwapAndForwardContract::new("swap-and-forward", daemon.clone());
        swap_and_forward_contract.set_address(&Addr::unchecked(swap_and_forward_addr.clone()));

        // 4. execute swap operation
        let usdt_address = "neutron1vpsgrzedwd8fezpsu9fcfewvp6nmv4kzd7a6nutpmgeyjk3arlqsypnlhm";
        let res = swap_and_forward_contract
            .swap_and_forward(
                Asset::Cw20Token(AndrAddr::from_string(usdt_address)),
                None,
                None,
                None,
                None,
                None,
                &[coin(100, denom)],
            )
            .unwrap();
        println!(
            "=========================swap_and_forward_response: {:?}=========================",
            res
        );
    }

    #[ignore]
    #[test]
    fn test_onchain_cw20() {
        let app_name = "swap and forward ado";
        let app_name_parsed = app_name.replace(' ', "_");
        let swap_and_forward_component_name = "swap-and-forward";
        let splitter_component_name = "splitter";

        dotenv().ok();
        env_logger::init();
        let mnemonic = std::env::var("MNEMONIC").expect("MNEMONIC must be set.");

        let daemon = Daemon::builder(PION_1).mnemonic(mnemonic).build().unwrap();
        let denom = PION_1.gas_denom;

        // upload ado if not uploaded
        // upload_ado(&daemon);

        let app_contract = AppContract::new(daemon.clone());
        // instanitate app
        // let app_address = instantiate_ado_with_splitter(
        //     &daemon,
        //     app_name,
        //     swap_and_forward_component_name,
        //     splitter_component_name,
        // );
        app_contract.set_address(&Addr::unchecked(
            // app_address,
            "neutron1x5mj0565mqrq5h7wsm66jlsscu0svlx2yj9ydkrwer5pmysj6v4shyu8mk",
        ));

        let swap_and_forward_addr: String =
            app_contract.get_address(swap_and_forward_component_name);

        let swap_and_forward_contract =
            SwapAndForwardContract::new("swap-and-forward", daemon.clone());
        swap_and_forward_contract.set_address(&Addr::unchecked(swap_and_forward_addr.clone()));

        let forward_addr = AndrAddr::from_string(format!(
            "/home/{}/{}/{}",
            daemon.sender().address(),
            app_name_parsed,
            splitter_component_name
        ));
        let forward_msg =
            to_json_binary(&andromeda_finance::splitter::ExecuteMsg::Send { config: None })
                .unwrap();

        let usdt_address = "neutron1vpsgrzedwd8fezpsu9fcfewvp6nmv4kzd7a6nutpmgeyjk3arlqsypnlhm";
        swap_and_forward_contract.execute_swap_from_cw20(
            &daemon,
            usdt_address,
            Uint128::new(36),
            Asset::NativeToken(denom.to_string()),
            Some(forward_addr),
            Some(forward_msg),
            None,
            None,
            None,
        );

        // ==================================================================================== //
    }
    #[ignore]
    #[test]
    fn test_onchain_native_to_native() {
        let app_name = "swap and forward ado with updated vfs";
        let app_name_parsed = app_name.replace(' ', "_");
        let swap_and_forward_component_name = "swap-and-forward";
        let splitter_component_name = "splitter";

        dotenv().ok();
        env_logger::init();
        let mnemonic = std::env::var("MNEMONIC").expect("MNEMONIC must be set.");

        let daemon = Daemon::builder(PION_1).mnemonic(mnemonic).build().unwrap();

        // upload ado if not uploaded
        // upload_ado(&daemon);
        // return;

        let app_contract = AppContract::new(daemon.clone());
        // instanitate app
        // let app_address = instantiate_ado_with_splitter(
        //     &daemon,
        //     app_name,
        //     swap_and_forward_component_name,
        //     splitter_component_name,
        // );
        app_contract.set_address(&Addr::unchecked(
            // app_address, //
            "neutron1x5mj0565mqrq5h7wsm66jlsscu0svlx2yj9ydkrwer5pmysj6v4shyu8mk",
        ));

        let swap_and_forward_addr: String =
            app_contract.get_address(swap_and_forward_component_name);

        let swap_and_forward_contract =
            SwapAndForwardContract::new("swap-and-forward", daemon.clone());
        swap_and_forward_contract.set_address(&Addr::unchecked(swap_and_forward_addr.clone()));

        let forward_addr = AndrAddr::from_string(format!(
            "/home/{}/{}/{}",
            daemon.sender().address(),
            app_name_parsed,
            splitter_component_name
        ));
        let forward_msg =
            to_json_binary(&andromeda_finance::splitter::ExecuteMsg::Send { config: None })
                .unwrap();

        let osmos_denom = "ibc/0471F1C4E7AFD3F07702BEF6DC365268D64570F7C1FDC98EA6098DD6DE59817B";
        let astro_denom = "ibc/8D8A7F7253615E5F76CB6252A1E1BD921D5EDB7BBAAF8913FB1C77FF125D9995";
        let res = swap_and_forward_contract
            .swap_and_forward(
                Asset::NativeToken(osmos_denom.to_owned()),
                Some(forward_addr),
                Some(forward_msg),
                None,
                None,
                None,
                &[coin(100000000, astro_denom)],
            )
            .unwrap();
        println!(
            "=========================swap_and_forward_response: {:?}=========================",
            res
        );
    }
}
