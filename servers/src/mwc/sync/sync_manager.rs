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

// sync_utils contain banch of shared between mutiple sync modules routines
// Normally we would put that into the base class, but rust doesn't support that.

use crate::mwc::sync::body_sync::BodySync;
use crate::mwc::sync::header_hashes_sync::HeadersHashSync;
use crate::mwc::sync::header_sync::HeaderSync;
use crate::mwc::sync::state_sync::StateSync;
use crate::mwc::sync::sync_peers::SyncPeers;
use crate::mwc::sync::sync_utils::{CachedResponse, SyncRequestResponses, SyncResponse};
use chrono::Duration;
use mwc_chain::txhashset::BitmapChunk;
use mwc_chain::{Chain, SyncState};
use mwc_core::core::hash::Hash;
use mwc_core::core::{OutputIdentifier, Segment, TxKernel};
use mwc_p2p::{Capabilities, PeerAddr, Peers};
use mwc_util::secp::pedersen::RangeProof;
use mwc_util::StopState;
use std::sync::Arc;

/// Sync Manager is reponsible for coordination of all syncing process
pub struct SyncManager {
	headers_hashes: HeadersHashSync,
	headers: HeaderSync,
	state: StateSync,
	body: BodySync,

	// Headers has complications with banning. In case of bad hashes, we will found that much later
	// when we ban many peers. That is why we need to track that separately and unban it in such case.
	headers_sync_peers: SyncPeers,
	// state & body sync
	state_sync_peers: SyncPeers,
	sync_state: Arc<SyncState>,
	stop_state: Arc<StopState>,

	cached_response: Option<CachedResponse<SyncResponse>>,
}

impl SyncManager {
	pub fn new(chain: Arc<Chain>, sync_state: Arc<SyncState>, stop_state: Arc<StopState>) -> Self {
		SyncManager {
			headers_hashes: HeadersHashSync::new(chain.clone()),
			headers: HeaderSync::new(chain.clone()),
			state: StateSync::new(chain.clone()),
			body: BodySync::new(chain),

			headers_sync_peers: SyncPeers::new(),
			state_sync_peers: SyncPeers::new(),
			sync_state,
			stop_state,
			cached_response: None,
		}
	}

	pub fn request(&mut self, peers: &Arc<Peers>) -> SyncResponse {
		if let Some(cached_response) = &self.cached_response {
			if !cached_response.is_expired() {
				return cached_response.get_response().clone();
			} else {
				self.cached_response = None;
			}
		}

		// Apply peers status (ban if needed)
		self.headers_sync_peers.apply_peers_status(peers);
		self.state_sync_peers.apply_peers_status(peers);

		let mut best_height = peers
			.iter()
			.outbound()
			.connected()
			.into_iter()
			.max_by_key(|p| {
				// Height is updated later, we better to handle that
				let live_info = p.info.live_info.read();
				if live_info.height > 0 {
					live_info.total_difficulty.to_num()
				} else {
					0
				}
			});
		if best_height.is_none() {
			// both inbound/outbound
			best_height = peers.iter().connected().into_iter().max_by_key(|p| {
				// Height is updated later, we better to handle that
				let live_info = p.info.live_info.read();
				if live_info.height > 0 {
					live_info.total_difficulty.to_num()
				} else {
					0
				}
			});
		}

		let best_height = match best_height {
			Some(best_peer) => best_peer.info.live_info.read().height,
			None => 0,
		};

		if best_height == 0 {
			return SyncResponse::new(
				SyncRequestResponses::WaitingForPeers,
				Capabilities::UNKNOWN,
				"Peers height is 0".into(),
			);
		}

		let headers_hash_resp = self.headers_hashes.request(
			peers,
			&self.sync_state,
			&mut self.headers_sync_peers,
			best_height,
		);
		debug!("headers_hash_resp: {:?}", headers_hash_resp);
		match headers_hash_resp.response {
			SyncRequestResponses::WaitingForPeers => return headers_hash_resp,
			SyncRequestResponses::Syncing => return headers_hash_resp,
			SyncRequestResponses::HeadersPibdReady | SyncRequestResponses::HeadersHashReady => {}
			_ => {
				debug_assert!(false);
			}
		}

		let mut headers_ready = false;

		let headers_resp = self.headers.request(
			peers,
			&self.sync_state,
			&mut self.headers_sync_peers,
			&self.headers_hashes,
			best_height,
		);
		debug!("headers_resp: {:?}", headers_resp);
		match headers_resp.response {
			SyncRequestResponses::WaitingForPeers => {
				self.headers_hashes
					.reset_ban_commited_to_hash(peers, &mut self.headers_sync_peers);
				self.headers_sync_peers.reset();
				return headers_resp;
			}
			SyncRequestResponses::Syncing => return headers_resp,
			SyncRequestResponses::WaitingForHeadersHash => {
				debug_assert!(false); // should never happen, headers_hashes above must be in sync or wait for peers
				return headers_resp;
			}
			SyncRequestResponses::HeadersPibdReady => self.headers_hashes.reset_hash_data(),
			SyncRequestResponses::HeadersReady => headers_ready = true,
			_ => {
				debug_assert!(false);
			}
		}

		let state_resp = self.state.request(
			peers,
			self.sync_state.clone(),
			&mut self.state_sync_peers,
			self.stop_state.clone(),
			best_height,
		);
		debug!("state_resp: {:?}", state_resp);
		match state_resp.response {
			SyncRequestResponses::Syncing => return state_resp,
			SyncRequestResponses::WaitingForPeers => return state_resp,
			SyncRequestResponses::WaitingForHeaders => return state_resp,
			SyncRequestResponses::StatePibdReady => {}
			_ => {
				debug_assert!(false);
			}
		}

		match self.body.request(
			peers,
			&self.sync_state,
			&mut self.state_sync_peers,
			best_height,
		) {
			Ok(body_resp) => {
				debug!("body_resp: {:?}", body_resp);
				match body_resp.response {
					SyncRequestResponses::Syncing => return body_resp,
					SyncRequestResponses::BodyReady => {
						if headers_ready {
							let resp = SyncResponse::new(
								SyncRequestResponses::SyncDone,
								Capabilities::UNKNOWN,
								"DONE!".into(),
							);
							self.cached_response =
								Some(CachedResponse::new(resp.clone(), Duration::seconds(180)));
							return resp;
						} else {
							return SyncResponse::new(
								SyncRequestResponses::Syncing,
								self.body.get_peer_capabilities(),
								"Waiting for headers, even body is done, more is expected".into(),
							);
						}
					}
					SyncRequestResponses::WaitingForPeers => return body_resp,
					SyncRequestResponses::BadState => {
						self.state.reset_desegmenter_data();
						return body_resp;
					}
					_ => debug_assert!(false),
				}
			}
			Err(e) => error!("Body request is failed, {}", e),
		}

		debug_assert!(false);
		SyncResponse::new(
			SyncRequestResponses::Syncing,
			Capabilities::UNKNOWN,
			"Invalid state, internal error".into(),
		)
	}

	pub fn receive_headers_hash_response(
		&mut self,
		peer: &PeerAddr,
		archive_height: u64,
		headers_hash_root: Hash,
	) {
		self.headers_hashes.receive_headers_hash_response(
			peer,
			archive_height,
			headers_hash_root,
			&mut self.headers_sync_peers,
		);
	}

	pub fn receive_header_hashes_segment(
		&mut self,
		peer: &PeerAddr,
		header_hashes_root: Hash,
		segment: Segment<Hash>,
	) {
		self.headers_hashes.receive_header_hashes_segment(
			peer,
			header_hashes_root,
			segment,
			&mut self.headers_sync_peers,
		);
	}

	pub fn receive_headers(
		&mut self,
		peer: &PeerAddr,
		bhs: &[mwc_core::core::BlockHeader],
		remaining: u64,
		peers: Arc<Peers>,
	) {
		if let Err(e) = self.headers.receive_headers(
			peer,
			bhs,
			remaining,
			&mut self.headers_sync_peers,
			self.headers_hashes.get_headers_hash_desegmenter(),
			&peers,
		) {
			error!("receive_headers failed with error: {}", e);
		}
	}

	pub fn recieve_pibd_status(
		&mut self,
		peer: &PeerAddr,
		header_hash: Hash,
		header_height: u64,
		output_bitmap_root: Hash,
	) {
		self.state
			.recieve_pibd_status(peer, header_hash, header_height, output_bitmap_root);
	}

	pub fn recieve_another_archive_header(
		&mut self,
		peer: &PeerAddr,
		header_hash: Hash,
		header_height: u64,
	) {
		self.headers_hashes
			.recieve_another_archive_header(peer, &header_hash, header_height);
		self.state
			.recieve_another_archive_header(peer, &header_hash, header_height);
	}

	pub fn receive_bitmap_segment(
		&mut self,
		peer: &PeerAddr,
		archive_header_hash: &Hash,
		segment: Segment<BitmapChunk>,
		peers: &Arc<Peers>,
	) {
		self.state.receive_bitmap_segment(
			peer,
			archive_header_hash,
			segment,
			peers,
			&mut self.state_sync_peers,
		);
	}

	pub fn receive_output_segment(
		&mut self,
		peer: &PeerAddr,
		bitmap_root_hash: &Hash,
		segment: Segment<OutputIdentifier>,
		peers: &Arc<Peers>,
	) {
		self.state.receive_output_segment(
			peer,
			bitmap_root_hash,
			segment,
			peers,
			&mut self.state_sync_peers,
		);
	}

	pub fn receive_rangeproof_segment(
		&mut self,
		peer: &PeerAddr,
		bitmap_root_hash: &Hash,
		segment: Segment<RangeProof>,
		peers: &Arc<Peers>,
	) {
		self.state.receive_rangeproof_segment(
			peer,
			bitmap_root_hash,
			segment,
			peers,
			&mut self.state_sync_peers,
		);
	}

	pub fn receive_kernel_segment(
		&mut self,
		peer: &PeerAddr,
		bitmap_root_hash: &Hash,
		segment: Segment<TxKernel>,
		peers: &Arc<Peers>,
	) {
		self.state.receive_kernel_segment(
			peer,
			bitmap_root_hash,
			segment,
			peers,
			&mut self.state_sync_peers,
		);
	}

	pub fn recieve_block_reporting(
		&mut self,
		accepted: bool, // block accepted/rejected flag
		peer: &PeerAddr,
		block_hash: &Hash,
		peers: &Arc<Peers>,
	) {
		self.body.recieve_block_reporting(
			accepted,
			block_hash,
			peer,
			peers,
			&mut self.state_sync_peers,
		);
	}
}