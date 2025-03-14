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

//! Library module for the key holder functionalities provided by Mwc.

use blake2_rfc as blake2;

#[macro_use]
extern crate mwc_util as util;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

pub mod base58;
pub mod extkey_bip32;
pub mod mnemonic;
mod types;
pub mod view_key;

pub mod keychain;
pub use crate::extkey_bip32::ChildNumber;
pub use crate::keychain::ExtKeychain;
pub use crate::types::{
	BlindSum, BlindingFactor, Error, ExtKeychainPath, Identifier, Keychain, SwitchCommitmentType,
	IDENTIFIER_SIZE,
};
pub use crate::view_key::ViewKey;
