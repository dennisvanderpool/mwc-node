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

//! Basic status view definition

use cursive::direction::Orientation;
use cursive::traits::Nameable;
use cursive::view::View;
use cursive::views::{LinearLayout, ResizedView, TextView};
use cursive::Cursive;
use std::borrow::Cow;

use crate::tui::constants::VIEW_BASIC_STATUS;
use crate::tui::types::TUIStatusListener;

use crate::chain::SyncStatus;
use crate::servers::ServerStats;

pub struct TUIStatusView;

impl TUIStatusView {
	pub fn update_sync_status(sync_status: SyncStatus) -> Cow<'static, str> {
		match sync_status {
			SyncStatus::Initial => Cow::Borrowed("Initializing"),
			SyncStatus::NoSync => Cow::Borrowed("Running"),
			SyncStatus::AwaitingPeers => Cow::Borrowed("Waiting for peers"),
			SyncStatus::HeaderHashSync {
				completed_blocks,
				// total number of leaves required by archive header
				total_blocks,
			} => Cow::Owned(format!(
				"Sync step 1/7: Downloading headers hashes - {}/{}",
				completed_blocks, total_blocks
			)),
			SyncStatus::HeaderSync {
				current_height,
				archive_height,
			} => {
				let percent = if archive_height == 0 {
					0
				} else {
					current_height * 100 / archive_height
				};
				Cow::Owned(format!("Sync step 1/7: Downloading headers: {}%", percent))
			}
			SyncStatus::TxHashsetPibd {
				recieved_segments,
				total_segments,
			} => {
				if recieved_segments == 0 && total_segments == 100 {
					Cow::Owned(
						"Sync step 2/7: Selecting peers, waiting for PIBD root hash".to_string(),
					)
				} else {
					let percent = if total_segments == 0 {
						0
					} else {
						recieved_segments * 100 / total_segments
					};
					Cow::Owned(format!(
						"Sync step 2/7: Downloading Tx state (PIBD) - {} / {} segments - {}%",
						recieved_segments, total_segments, percent
					))
				}
			}
			SyncStatus::ValidatingKernelsHistory => {
				Cow::Owned("Sync step 3/7: Validating kernels history".to_string())
			}
			SyncStatus::TxHashsetHeadersValidation {
				headers,
				headers_total,
			} => {
				let percent = if headers_total == 0 {
					0
				} else {
					headers * 100 / headers_total
				};
				Cow::Owned(format!(
					"Sync step 3/7: Validating headers - {} / {} segments - {}%",
					headers, headers_total, percent
				))
			}
			SyncStatus::TxHashsetKernelsPosValidation {
				kernel_pos,
				kernel_pos_total,
			} => {
				let percent = if kernel_pos_total == 0 {
					0
				} else {
					kernel_pos * 100 / kernel_pos_total
				};
				Cow::Owned(format!(
					"Sync step 4/7: Validation kernel position - {}/{} - {}%",
					kernel_pos, kernel_pos_total, percent
				))
			}
			SyncStatus::TxHashsetRangeProofsValidation {
				rproofs,
				rproofs_total,
			} => {
				let r_percent = if rproofs_total > 0 {
					(rproofs * 100) / rproofs_total
				} else {
					0
				};
				Cow::Owned(format!(
					"Sync step 5/7: Validating chain state - range proofs: {}%",
					r_percent
				))
			}
			SyncStatus::TxHashsetKernelsValidation {
				kernels,
				kernels_total,
			} => {
				let k_percent = if kernels_total > 0 {
					(kernels * 100) / kernels_total
				} else {
					0
				};
				Cow::Owned(format!(
					"Sync step 6/7: Validating chain state - kernels: {}%",
					k_percent
				))
			}
			SyncStatus::BodySync {
				archive_height,
				current_height,
				highest_height,
			} => {
				let percent = if highest_height.saturating_sub(archive_height) == 0 {
					0
				} else {
					current_height.saturating_sub(archive_height) * 100
						/ highest_height.saturating_sub(archive_height)
				};
				Cow::Owned(format!("Sync step 7/7: Downloading blocks: {}%", percent))
			}
			SyncStatus::Shutdown => Cow::Borrowed("Shutting down, closing connections"),
		}
	}

	/// Create basic status view
	pub fn create() -> impl View {
		let basic_status_view = ResizedView::with_full_screen(
			LinearLayout::new(Orientation::Vertical)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Current Status:               "))
						.child(TextView::new("Starting").with_name("basic_current_status")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Connected Peers:              "))
						.child(TextView::new("0").with_name("connected_peers")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Disk Usage (GB):              "))
						.child(TextView::new("0").with_name("disk_usage")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal).child(TextView::new(
						"--------------------------------------------------------",
					)),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Header Tip Hash:              "))
						.child(TextView::new("  ").with_name("basic_header_tip_hash")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Header Chain Height:          "))
						.child(TextView::new("  ").with_name("basic_header_chain_height")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Header Cumulative Difficulty: "))
						.child(TextView::new("  ").with_name("basic_header_total_difficulty")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Header Tip Timestamp:         "))
						.child(TextView::new("  ").with_name("basic_header_timestamp")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal).child(TextView::new(
						"--------------------------------------------------------",
					)),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Chain Tip Hash:               "))
						.child(TextView::new("  ").with_name("tip_hash")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Chain Height:                 "))
						.child(TextView::new("  ").with_name("chain_height")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Chain Cumulative Difficulty:  "))
						.child(TextView::new("  ").with_name("basic_total_difficulty")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Chain Tip Timestamp:          "))
						.child(TextView::new("  ").with_name("chain_timestamp")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal).child(TextView::new(
						"--------------------------------------------------------",
					)),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Transaction Pool Size:        "))
						.child(TextView::new("0").with_name("tx_pool_size"))
						.child(TextView::new(" ("))
						.child(TextView::new("0").with_name("tx_pool_kernels"))
						.child(TextView::new(")")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("Stem Pool Size:               "))
						.child(TextView::new("0").with_name("stem_pool_size"))
						.child(TextView::new(" ("))
						.child(TextView::new("0").with_name("stem_pool_kernels"))
						.child(TextView::new(")")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal).child(TextView::new(
						"--------------------------------------------------------",
					)),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("  ").with_name("basic_mining_config_status")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("  ").with_name("basic_mining_status")),
				)
				.child(
					LinearLayout::new(Orientation::Horizontal)
						.child(TextView::new("  ").with_name("basic_network_info")),
				), //.child(logo_view)
		);
		basic_status_view.with_name(VIEW_BASIC_STATUS)
	}
}

impl TUIStatusListener for TUIStatusView {
	fn update(c: &mut Cursive, stats: &ServerStats) {
		let basic_status = TUIStatusView::update_sync_status(stats.sync_status);

		c.call_on_name("basic_current_status", |t: &mut TextView| {
			t.set_content(basic_status);
		});
		c.call_on_name("connected_peers", |t: &mut TextView| {
			t.set_content(stats.peer_count.to_string());
		});
		c.call_on_name("disk_usage", |t: &mut TextView| {
			t.set_content(stats.disk_usage_gb.clone());
		});
		c.call_on_name("tip_hash", |t: &mut TextView| {
			t.set_content(stats.chain_stats.last_block_h.to_string() + "...");
		});
		c.call_on_name("chain_height", |t: &mut TextView| {
			t.set_content(stats.chain_stats.height.to_string());
		});
		c.call_on_name("basic_total_difficulty", |t: &mut TextView| {
			t.set_content(stats.chain_stats.total_difficulty.to_string());
		});
		c.call_on_name("chain_timestamp", |t: &mut TextView| {
			t.set_content(stats.chain_stats.latest_timestamp.to_string());
		});
		c.call_on_name("basic_header_tip_hash", |t: &mut TextView| {
			t.set_content(stats.header_stats.last_block_h.to_string() + "...");
		});
		c.call_on_name("basic_header_chain_height", |t: &mut TextView| {
			t.set_content(stats.header_stats.height.to_string());
		});
		c.call_on_name("basic_header_total_difficulty", |t: &mut TextView| {
			t.set_content(stats.header_stats.total_difficulty.to_string());
		});
		c.call_on_name("basic_header_timestamp", |t: &mut TextView| {
			t.set_content(stats.header_stats.latest_timestamp.to_string());
		});
		if let Some(tx_stats) = &stats.tx_stats {
			c.call_on_name("tx_pool_size", |t: &mut TextView| {
				t.set_content(tx_stats.tx_pool_size.to_string());
			});
			c.call_on_name("stem_pool_size", |t: &mut TextView| {
				t.set_content(tx_stats.stem_pool_size.to_string());
			});
			c.call_on_name("tx_pool_kernels", |t: &mut TextView| {
				t.set_content(tx_stats.tx_pool_kernels.to_string());
			});
			c.call_on_name("stem_pool_kernels", |t: &mut TextView| {
				t.set_content(tx_stats.stem_pool_kernels.to_string());
			});
		}
	}
}

#[test]
fn test_status_txhashset_kernels() {
	let status = SyncStatus::TxHashsetKernelsValidation {
		kernels: 201,
		kernels_total: 5000,
	};
	let basic_status = TUIStatusView::update_sync_status(status);
	assert!(basic_status.contains("4%"), "{}", basic_status);
}

#[test]
fn test_status_txhashset_rproofs() {
	let status = SyncStatus::TxHashsetRangeProofsValidation {
		rproofs: 643,
		rproofs_total: 1000,
	};
	let basic_status = TUIStatusView::update_sync_status(status);
	assert!(basic_status.contains("64%"), "{}", basic_status);
}
