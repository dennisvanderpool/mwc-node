// Copyright 2019 The Grin Developers
// Copyright 2024 The MWC Developers
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

use chrono::Utc;
use mwc_core::consensus::{
	next_difficulty, HeaderDifficultyInfo, AR_SCALE_DAMP_FACTOR, BLOCK_TIME_SEC,
	DIFFICULTY_ADJUST_WINDOW, MIN_DIFFICULTY,
};
use mwc_core::global;
use mwc_core::pow::Difficulty;
use std::collections::VecDeque;

/// Checks different next_target adjustments and difficulty boundaries
#[test]
fn next_target_adjustment() {
	global::set_local_chain_type(global::ChainTypes::AutomatedTesting);
	let cur_time = Utc::now().timestamp() as u64;
	let diff_min = Difficulty::min();

	// Check we don't get stuck on difficulty <= MIN_DIFFICULTY (at 4x faster blocks at least)
	let mut hi = HeaderDifficultyInfo::from_diff_scaling(diff_min, AR_SCALE_DAMP_FACTOR as u32);
	hi.is_secondary = false;
	let mut cache_values = VecDeque::new();
	let hinext = next_difficulty(
		1,
		repeat(
			BLOCK_TIME_SEC / 4,
			hi.clone(),
			DIFFICULTY_ADJUST_WINDOW,
			None,
		),
		&mut cache_values,
	);

	assert_ne!(hinext.difficulty, diff_min);

	// Check we don't get stuck on scale MIN_DIFFICULTY, when primary frequency is too high
	assert_ne!(hinext.secondary_scaling, MIN_DIFFICULTY as u32);

	// just enough data, right interval, should stay constant
	let just_enough = DIFFICULTY_ADJUST_WINDOW + 1;
	hi.difficulty = Difficulty::from_num(10000);
	assert_eq!(
		next_difficulty(
			1,
			repeat(BLOCK_TIME_SEC, hi.clone(), just_enough, None),
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(10000)
	);

	// check pre difficulty_data_to_vector effect on retargetting
	assert_eq!(
		next_difficulty(
			1,
			vec![HeaderDifficultyInfo::from_ts_diff(42, hi.difficulty)],
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(14913)
	);

	// checking averaging works
	hi.difficulty = Difficulty::from_num(500);
	let sec = DIFFICULTY_ADJUST_WINDOW / 2;
	let mut s1 = repeat(BLOCK_TIME_SEC, hi.clone(), sec, Some(cur_time));
	let mut s2 = repeat_offs(
		BLOCK_TIME_SEC,
		1500,
		sec,
		cur_time + (sec * BLOCK_TIME_SEC) as u64,
	);
	s2.append(&mut s1);
	assert_eq!(
		next_difficulty(1, s2, &mut cache_values).difficulty,
		Difficulty::from_num(1000)
	);

	// too slow, diff goes down
	hi.difficulty = Difficulty::from_num(1000);
	assert_eq!(
		next_difficulty(
			1,
			repeat(90, hi.clone(), just_enough, None),
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(857)
	);
	assert_eq!(
		next_difficulty(
			1,
			repeat(120, hi.clone(), just_enough, None),
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(750)
	);

	// too fast, diff goes up
	assert_eq!(
		next_difficulty(
			1,
			repeat(55, hi.clone(), just_enough, None),
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(1028)
	);
	assert_eq!(
		next_difficulty(
			1,
			repeat(45, hi.clone(), just_enough, None),
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(1090)
	);
	assert_eq!(
		next_difficulty(
			1,
			repeat(30, hi.clone(), just_enough, None),
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(1200)
	);

	// hitting lower time bound, should always get the same result below
	assert_eq!(
		next_difficulty(
			1,
			repeat(0, hi.clone(), just_enough, None),
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(1500)
	);

	// hitting higher time bound, should always get the same result above
	assert_eq!(
		next_difficulty(
			1,
			repeat(300, hi.clone(), just_enough, None),
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(500)
	);
	assert_eq!(
		next_difficulty(
			1,
			repeat(400, hi.clone(), just_enough, None),
			&mut cache_values
		)
		.difficulty,
		Difficulty::from_num(500)
	);

	// We should never drop below minimum
	hi.difficulty = Difficulty::zero();
	assert_eq!(
		next_difficulty(1, repeat(90, hi, just_enough, None), &mut cache_values).difficulty,
		Difficulty::min()
	);
}

// Builds an iterator for next difficulty calculation with the provided
// constant time interval, difficulty and total length.
fn repeat(
	interval: u64,
	diff: HeaderDifficultyInfo,
	len: u64,
	cur_time: Option<u64>,
) -> Vec<HeaderDifficultyInfo> {
	let cur_time = match cur_time {
		Some(t) => t,
		None => Utc::now().timestamp() as u64,
	};
	// watch overflow here, length shouldn't be ridiculous anyhow
	assert!(len < std::usize::MAX as u64);
	let diffs = vec![diff.difficulty; len as usize];
	let times = (0..(len as usize)).map(|n| n * interval as usize).rev();
	let pairs = times.zip(diffs.iter());
	pairs
		.enumerate()
		.map(|(index, (t, d))| {
			HeaderDifficultyInfo::new(
				index as u64,
				None,
				cur_time + t as u64,
				*d,
				diff.secondary_scaling,
				diff.is_secondary,
			)
		})
		.collect::<Vec<_>>()
}

fn repeat_offs(interval: u64, diff: u64, len: u64, from: u64) -> Vec<HeaderDifficultyInfo> {
	repeat(
		interval,
		HeaderDifficultyInfo::from_ts_diff(1, Difficulty::from_num(diff)),
		len,
		Some(from),
	)
}
