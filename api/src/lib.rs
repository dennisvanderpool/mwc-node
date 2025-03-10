// Copyright 2019 The Grin Developers
// Copyright 2024 The MWC Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use mwc_chain as chain;
use mwc_core as core;
use mwc_p2p as p2p;
use mwc_pool as pool;

use mwc_util as util;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate log;

#[macro_use]
mod web;
pub mod auth;
pub mod client;
mod foreign;
pub mod foreign_rpc;
mod handlers;
pub mod json_rpc;
mod owner;
pub mod owner_rpc;
mod rest;
mod router;
mod stratum;
mod stratum_rpc;
pub mod types;

pub use crate::auth::{
	BasicAuthMiddleware, BasicAuthURIMiddleware, MWC_BASIC_REALM, MWC_FOREIGN_BASIC_REALM,
};
pub use crate::foreign::Foreign;
pub use crate::foreign_rpc::ForeignRpc;
pub use crate::handlers::node_apis;
pub use crate::owner::Owner;
pub use crate::owner::{
	get_server_onion_address, reset_server_onion_address, set_server_onion_address,
};
pub use crate::owner_rpc::OwnerRpc;
pub use crate::rest::*;
pub use crate::router::*;
pub use crate::types::*;
pub use crate::web::*;
