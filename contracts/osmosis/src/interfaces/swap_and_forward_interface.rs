use crate::contract::{execute, instantiate, migrate, query};
use andromeda_std::ado_base::MigrateMsg;
use andromeda_swap_and_forward::osmosis::{ExecuteMsg, InstantiateMsg, QueryMsg};
use cw_orch::{interface, prelude::*};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
pub struct SwapAndForwardContract;

impl<Chain> Uploadable for SwapAndForwardContract<Chain> {
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query).with_migrate(migrate))
    }

    fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
        artifacts_dir_from_workspace!()
            .find_wasm_path("swap_and_forward_osmosis")
            .unwrap()
    }
}
