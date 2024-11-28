// Copyright 2021 The Grin Developers
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

use crate::chain::{self, SyncState, SyncStatus};
use crate::core::core::hash::{Hash, Hashed};
use crate::grin::sync::sync_peers::SyncPeers;
use crate::grin::sync::sync_utils;
use crate::grin::sync::sync_utils::{RequestTracker, SyncRequestResponses};
use crate::p2p;
use grin_chain::{pibd_params, Chain};
use grin_p2p::{Peer, PeerAddr};
use p2p::Capabilities;
use rand::prelude::*;
use std::cmp;
use std::sync::Arc;

pub struct BodySync {
	chain: Arc<Chain>,
	required_capabilities: Capabilities,
	request_tracker: RequestTracker<Hash>,
	request_series: Vec<(Hash, u64)>, // Hash, height
}

impl BodySync {
	pub fn new(chain: Arc<Chain>) -> BodySync {
		BodySync {
			chain,
			required_capabilities: Capabilities::UNKNOWN,
			request_tracker: RequestTracker::new(),
			request_series: Vec::new(),
		}
	}

	pub fn get_peer_capabilities(&self) -> Capabilities {
		self.required_capabilities
	}

	// Expected that it is called ONLY when state_sync is done
	pub fn request(
		&mut self,
		peers: &Arc<p2p::Peers>,
		sync_state: &SyncState,
		sync_peers: &mut SyncPeers,
		best_height: u64,
	) -> Result<SyncRequestResponses, chain::Error> {
		// check if we need something
		let head = self.chain.head()?;
		let header_head = self.chain.header_head()?;

		if head.last_block_h == header_head.last_block_h {
			// sync is done, we are ready.
			return Ok(SyncRequestResponses::BodyReady);
		}

		let archive_height = Chain::height_2_archive_height(best_height);

		let head = self.chain.head()?;
		let header_head = self.chain.header_head()?;
		let fork_point = self.chain.fork_point()?;

		if !self.chain.archive_mode() {
			if fork_point.height < archive_height {
				warn!("body_sync: cannot sync full blocks earlier than horizon. will request txhashset");
				return Ok(SyncRequestResponses::BadState);
			}
		}

		let (peer_capabilities, required_capabilities) =
			if self.chain.archive_mode() && head.height <= archive_height {
				(
					Capabilities::BLOCK_HIST,
					Capabilities::BLOCK_HIST | Capabilities::HEADER_HIST,
				)
			} else {
				(Capabilities::UNKNOWN, Capabilities::HEADER_HIST) // needed for headers sync, that can go in parallel
			};
		self.required_capabilities = required_capabilities;

		let (peers, excluded_requests) = sync_utils::get_sync_peers(
			peers,
			pibd_params::BLOCKS_REQUEST_PER_PEER,
			peer_capabilities,
			head.height,
			self.request_tracker.get_requests_num(),
			&self.request_tracker.get_peers_queue_size(),
		);
		if peers.is_empty() {
			if excluded_requests == 0 {
				return Ok(SyncRequestResponses::WaitingForPeers);
			} else {
				return Ok(SyncRequestResponses::Syncing);
			}
		}

		// requested_blocks, check for expiration
		self.request_tracker
			.retain_expired(pibd_params::SEGMENT_REQUEST_TIMEOUT_SECS, sync_peers);

		sync_state.update(SyncStatus::BodySync {
			archive_height: if self.chain.archive_mode() {
				0
			} else {
				archive_height
			},
			current_height: fork_point.height,
			highest_height: best_height,
		});

		// if we have 5 peers to sync from then ask for 50 blocks total (peer_count *
		// 10) max will be 80 if all 8 peers are advertising more work
		// also if the chain is already saturated with orphans, throttle

		let mut need_request = self.request_tracker.calculate_needed_requests(
			peers.len(),
			excluded_requests as usize,
			pibd_params::BLOCKS_REQUEST_PER_PEER,
			pibd_params::BLOCKS_REQUEST_LIMIT,
		);

		if need_request > 0 {
			let mut rng = rand::thread_rng();

			self.send_requests(&mut need_request, &peers, &mut rng, sync_peers)?;

			// We can send more requests, let's check if we need to update request_series
			if need_request > 0 {
				let mut need_refresh_request_series = false;

				// If request_series first if processed, need to update
				if let Some((hash, height)) = self.request_series.last() {
					debug!("Updating body request series for {} / {}", hash, height);
					if !self.is_need_request_block(hash)? {
						// The tail is updated, so we can request more
						need_refresh_request_series = true;
					}
				} else {
					need_refresh_request_series = true;
				}

				// Check for stuck orphan
				if let Ok(next_block) = self.chain.get_header_by_height(fork_point.height + 1) {
					let next_block_hash = next_block.hash();
					if self.chain.block_exists(&next_block_hash)? {
						let fork_point2 = self.chain.fork_point()?;
						error!(
							"FORK POINT changed? Was {} / {}, now {} / {}.  Next block: {:?}",
							fork_point.height,
							fork_point.hash(),
							fork_point2.height,
							fork_point2.hash(),
							next_block
						);
					}
					// Kick the stuck orphan
					match self.chain.get_orphan(&next_block_hash) {
						Some(orph) => {
							info!("There is stuck orphan is found, let's kick it...");
							if self.chain.process_block(orph.block, orph.opts).is_ok() {
								info!("push stuck orphan was successful. Should be able continue to go forward now");
								need_refresh_request_series = true;
							}
						}
						None => {}
					}
				}

				if need_refresh_request_series {
					self.request_series.clear();
					// Don't collect more than 500 blocks in the cache. The block size limit is 1.5MB, so total cache mem can be up to 750 Mb which is ok
					let max_height = cmp::min(fork_point.height + 500 as u64, header_head.height);
					let mut current = self.chain.get_header_by_height(max_height)?;

					while current.height > fork_point.height {
						let hash = current.hash();
						if !self.chain.is_orphan(&hash) {
							self.request_series.push((hash, current.height));
						}
						current = self.chain.get_previous_header(&current)?;
					}

					if let Some((hash, height)) = self.request_series.last() {
						debug!("New body request series tail is {} / {}", hash, height);
					}
				}

				// Now we can try to submit more requests...
				self.send_requests(&mut need_request, &peers, &mut rng, sync_peers)?;
			}
		}

		return Ok(SyncRequestResponses::Syncing);
	}

	pub fn recieve_block_reporting(
		&mut self,
		accepted: bool, // block accepted/rejected flag
		block_hash: &Hash,
		peer: &PeerAddr,
		peers: &Arc<p2p::Peers>,
		sync_peers: &mut SyncPeers,
	) {
		if let Some(peer_adr) = self.request_tracker.remove_request(block_hash) {
			if accepted {
				if peer_adr == *peer {
					sync_peers.report_ok_response(peer);
				}
			}
		}

		if !accepted {
			sync_peers.report_error_response(
				peer,
				format!("Get bad block {} for peer {}", block_hash, peer),
			);
		}

		// let's request next package since we get this one...
		if self.request_tracker.get_update_requests_to_next_ask() == 0 {
			if let Ok(head) = self.chain.head() {
				let (peers, excluded_requests) = sync_utils::get_sync_peers(
					peers,
					pibd_params::BLOCKS_REQUEST_PER_PEER,
					self.required_capabilities,
					head.height,
					self.request_tracker.get_requests_num(),
					&self.request_tracker.get_peers_queue_size(),
				);
				if !peers.is_empty() {
					// requested_blocks, check for expiration
					let mut need_request = self.request_tracker.calculate_needed_requests(
						peers.len(),
						excluded_requests as usize,
						pibd_params::BLOCKS_REQUEST_PER_PEER,
						pibd_params::BLOCKS_REQUEST_LIMIT,
					);
					if need_request > 0 {
						let mut rng = rand::thread_rng();
						if let Err(e) =
							self.send_requests(&mut need_request, &peers, &mut rng, sync_peers)
						{
							error!("Unable to call send_requests, error: {}", e);
						}
					}
				}
			}
		}
	}

	fn is_need_request_block(&self, hash: &Hash) -> Result<bool, chain::Error> {
		Ok(!(self.request_tracker.has_request(&hash)
			|| self.chain.is_orphan(&hash)
			|| self.chain.block_exists(&hash)?))
	}

	fn send_requests(
		&mut self,
		need_request: &mut usize,
		peers: &Vec<Arc<Peer>>,
		rng: &mut ThreadRng,
		sync_peers: &mut SyncPeers,
	) -> Result<(), chain::Error> {
		// request_series naturally from head to tail, but requesting better to send from tail to the head....
		for (hash, height) in self.request_series.iter().rev() {
			if self.is_need_request_block(&hash)? {
				// can request a block...
				let peer = peers.choose(rng).expect("Peers can't be empty");
				if let Err(e) = peer.send_block_request(hash.clone(), chain::Options::SYNC) {
					let msg = format!(
						"Failed to send block request to peer {}, {}",
						peer.info.addr, e
					);
					warn!("{}", msg);
					sync_peers.report_error_response(&peer.info.addr, msg);
				} else {
					self.request_tracker.register_request(
						hash.clone(),
						peer.info.addr.clone(),
						format!("Block {}, {}", hash, height),
					);
					*need_request -= 1;
					if *need_request == 0 {
						break;
					}
				}
			}
		}
		Ok(())
	}
}
