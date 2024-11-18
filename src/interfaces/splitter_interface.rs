use andromeda_finance::splitter::{AddressPercent, ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{ado_base::MigrateMsg, common::expiration::Expiry};
use cw_orch::{interface, prelude::*};
use cw_orch_daemon::{DaemonBase, Wallet};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
pub struct SplitterContract;

impl SplitterContract<DaemonBase<Wallet>> {
    pub fn init(
        &self,
        recipients: Vec<AddressPercent>,
        lock_time: Option<Expiry>,
        owner: Option<String>,
    ) {
        let init_msg = InstantiateMsg {
            recipients,
            lock_time,
            kernel_address: "terra1g0vzxc6a0layhxdwc24kwwam4v93pjmam5a77wtvfhzpdltp82estk3kpc"
                .to_string(),
            owner,
        };
        self.instantiate(&init_msg, None, None).unwrap();
    }
}
