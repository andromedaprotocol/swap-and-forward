use std::str::FromStr;

use crate::contract::{execute, instantiate, migrate, query};
use andromeda_std::ado_base::MigrateMsg;
use andromeda_std::amp::AndrAddr;
use andromeda_std::common::denom::Asset;
use andromeda_swap_and_forward::astroport::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, SwapOperation,
};
use cosmrs::cosmwasm::MsgExecuteContract;
use cosmrs::AccountId;
use cosmwasm_std::{to_json_binary, Binary, Decimal, Uint128};
use cw_orch::{interface, prelude::*};
use cw_orch_daemon::{Daemon, DaemonBase, TxSender, Wallet};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
pub struct SwapAndForwardContract;

impl<Chain> Uploadable for SwapAndForwardContract<Chain> {
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query).with_migrate(migrate))
    }

    fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
        artifacts_dir_from_workspace!()
            .find_wasm_path("swap_and_forward_astroport")
            .unwrap()
    }
}

impl SwapAndForwardContract<DaemonBase<Wallet>> {
    #[allow(clippy::too_many_arguments)]
    pub fn execute_swap_from_cw20(
        self,
        daemon: &Daemon,
        from_asset_addr: &str,
        from_amount: Uint128,
        to_asset: Asset,
        forward_addr: Option<AndrAddr>,
        forward_msg: Option<Binary>,
        max_spread: Option<Decimal>,
        minimum_receive: Option<Uint128>,
        operations: Option<Vec<SwapOperation>>,
    ) {
        let hook_msg = Cw20HookMsg::SwapAndForward {
            to_asset,
            forward_addr,
            forward_msg,
            max_spread,
            minimum_receive,
            operations,
        };
        let cw_20_transfer_msg = cw20::Cw20ExecuteMsg::Send {
            contract: self.addr_str().unwrap(),
            amount: from_amount,
            msg: to_json_binary(&hook_msg).unwrap(),
        };
        let exec_msg: MsgExecuteContract = MsgExecuteContract {
            sender: daemon.sender().account_id(),
            contract: AccountId::from_str(from_asset_addr).unwrap(),
            msg: serde_json::to_vec(&cw_20_transfer_msg).unwrap(),
            funds: vec![],
        };

        daemon
            .rt_handle
            .block_on(async { daemon.sender().commit_tx(vec![exec_msg], None).await })
            .unwrap();
    }
}
