use andromeda_std::{andr_exec, andr_instantiate};
use cosmwasm_schema::{cw_serde, QueryResponses};

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}
