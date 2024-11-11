use crate::contract::{execute, instantiate, migrate, query};
use andromeda_std::ado_base::MigrateMsg;
use andromeda_std::os::adodb::{ExecuteMsg, InstantiateMsg, QueryMsg};
use cw_orch::{interface, prelude::*};
use cw_orch_daemon::{DaemonBase, Wallet};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
pub struct AdodbContract;

impl<Chain> Uploadable for AdodbContract<Chain> {
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query).with_migrate(migrate))
    }
    // fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
    //     artifacts_dir_from_workspace!()
    //         .find_wasm_path("swap_and_forward")
    //         .unwrap()
    // }
}

impl AdodbContract<DaemonBase<Wallet>> {
    pub fn execute_publish(self, code_id: u64, ado_type: String, version: String) {
        let res = self
            .execute(
                &ExecuteMsg::Publish {
                    code_id,
                    ado_type,
                    action_fees: None,
                    version,
                    publisher: None,
                },
                None,
            )
            .unwrap();
        println!("================publish result:{:?}================", res);
    }
}
