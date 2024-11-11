use crate::contract::{execute, instantiate, migrate, query};
use andromeda_app::app::{AppComponent, ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::ado_base::MigrateMsg;
use cw_orch::{interface, prelude::*};
use cw_orch_daemon::{DaemonBase, Wallet};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
pub struct AppContract;

impl<Chain> Uploadable for AppContract<Chain> {
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query).with_migrate(migrate))
    }
    // fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
    //     artifacts_dir_from_workspace!()
    //         .find_wasm_path("swap_and_forward")
    //         .unwrap()
    // }
}

impl AppContract<DaemonBase<Wallet>> {
    pub fn init(
        &self,
        name: impl Into<String>,
        app_components: Vec<AppComponent>,
        owner: Option<String>,
    ) {
        let init_msg = InstantiateMsg {
            app_components,
            name: name.into(),
            chain_info: None,
            kernel_address: "terra1g0vzxc6a0layhxdwc24kwwam4v93pjmam5a77wtvfhzpdltp82estk3kpc"
                .to_string(),
            owner,
        };
        self.instantiate(&init_msg, None, None).unwrap();
    }

    pub fn query_address_by_component_name(&self, name: impl Into<String>) -> String {
        let query_msg = QueryMsg::GetAddress { name: name.into() };
        self.query(&query_msg).unwrap()
    }
}
