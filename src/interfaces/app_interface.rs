use andromeda_app::app::{AppComponent, ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::ado_base::MigrateMsg;
use cw_orch::{interface, prelude::*};
use cw_orch_daemon::{DaemonBase, Wallet};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
pub struct AppContract;

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
