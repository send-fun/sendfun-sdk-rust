pub(crate) mod generated;

pub use crate::constants::NEXUS_PROGRAM_ID as ID;
pub use generated::shared;
pub use generated::{accounts, errors, events, instructions, types};

pub mod pda;

use crate::math::fee_decay::{FeeDecayArgs, calculate_fee_decay_premium};

impl types::LaunchpadFees {
	#[must_use]
	pub const fn total_fee_bps(&self) -> Option<u16> {
		self.protocol_fee_bps.checked_add(self.creator_fee_bps)
	}

	/// `total_fee_bps` plus the decay premium from the market's `created_at`;
	/// times in unix seconds.
	#[must_use]
	pub fn effective_fee_bps(&self, created_at: i64, now: i64) -> Option<u16> {
		let standard_fee_bps = self.total_fee_bps()?;
		add_decay_premium(
			standard_fee_bps,
			FeeDecayArgs {
				current_timestamp: u64::try_from(now).ok()?,
				created_at_timestamp: u64::try_from(created_at).ok()?,
				decay_seconds: self.fee_decay_seconds,
				decay_start_bps: self.fee_decay_start_bps,
				standard_fee_bps: u64::from(standard_fee_bps),
			},
		)
	}
}

impl types::DexFees {
	#[must_use]
	pub const fn total_fee_bps(&self) -> Option<u16> {
		match self.protocol_fee_bps.checked_add(self.lp_fee_bps) {
			Some(v) => v.checked_add(self.creator_fee_bps),
			None => None,
		}
	}

	/// `total_fee_bps` plus the decay premium from the market's `created_at`;
	/// times in unix seconds.
	#[must_use]
	pub fn effective_fee_bps(&self, created_at: i64, now: i64) -> Option<u16> {
		let standard_fee_bps = self.total_fee_bps()?;
		add_decay_premium(
			standard_fee_bps,
			FeeDecayArgs {
				current_timestamp: u64::try_from(now).ok()?,
				created_at_timestamp: u64::try_from(created_at).ok()?,
				decay_seconds: self.fee_decay_seconds,
				decay_start_bps: self.fee_decay_start_bps,
				standard_fee_bps: u64::from(standard_fee_bps),
			},
		)
	}
}

fn add_decay_premium(standard_fee_bps: u16, args: FeeDecayArgs) -> Option<u16> {
	let premium = calculate_fee_decay_premium(args)?;
	standard_fee_bps.checked_add(u16::try_from(premium).ok()?)
}

#[cfg(test)]
mod tests {
	use super::types::{DexFees, LaunchpadFees};

	const LAUNCHPAD: LaunchpadFees = LaunchpadFees {
		creation_fee_cents: 0,
		protocol_fee_bps: 100,
		creator_fee_bps: 50,
		fee_decay_seconds: 12,
		fee_decay_start_bps: 5_000,
	};

	#[test]
	fn charges_the_decay_start_rate_at_creation() {
		assert_eq!(LAUNCHPAD.effective_fee_bps(100, 100), Some(5_000));
	}

	#[test]
	fn decays_quadratically_rounding_the_premium_up() {
		// 150 standard + ceil(6^2 * 4_850 / 12^2) = 150 + 1_213.
		assert_eq!(LAUNCHPAD.effective_fee_bps(100, 106), Some(1_363));
	}

	#[test]
	fn falls_to_the_standard_rate_when_the_window_closes() {
		assert_eq!(LAUNCHPAD.effective_fee_bps(100, 112), Some(150));
	}

	#[test]
	fn charges_the_full_premium_on_a_creation_time_in_the_future() {
		assert_eq!(LAUNCHPAD.effective_fee_bps(200, 100), Some(5_000));
	}

	#[test]
	fn rejects_a_negative_timestamp() {
		assert_eq!(LAUNCHPAD.effective_fee_bps(-1, 100), None);
	}

	#[test]
	fn sums_protocol_lp_and_creator_on_the_dex() {
		let fees = DexFees {
			creation_fee_cents: 0,
			protocol_fee_bps: 80,
			lp_fee_bps: 30,
			creator_fee_bps: 40,
			fee_decay_seconds: 0,
			fee_decay_start_bps: 0,
		};
		assert_eq!(fees.effective_fee_bps(100, 100), Some(150));
	}
}
