pub(crate) mod generated;

pub use crate::constants::NEXUS_PROGRAM_ID as ID;
pub use generated::{accounts, errors, events, instructions, types};

pub mod pda;

use crate::math::fee_decay::{FeeDecayArgs, calculate_fee_decay_premium};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectiveFeeArgs {
	/// `BondingCurve.creator_fee_bps` or `Pool.creator_fee_bps`.
	pub creator_fee_bps: u16,
	/// The market's creation time, in Unix seconds.
	pub created_at: i64,
	/// The time of the trade, in Unix seconds.
	pub now: i64,
}

impl types::LaunchpadFees {
	/// Returns `protocol_fee_bps` plus `creator_fee_bps`. Pass
	/// `BondingCurve.creator_fee_bps`, not `max_creator_fee_bps`. Returns
	/// `None` on overflow.
	#[must_use]
	pub const fn total_fee_bps(&self, creator_fee_bps: u16) -> Option<u16> {
		self.protocol_fee_bps.checked_add(creator_fee_bps)
	}

	/// Returns `total_fee_bps` plus the fee decay premium at `args.now`.
	/// Returns `None` on overflow or on a negative time.
	#[must_use]
	pub fn effective_fee_bps(&self, args: EffectiveFeeArgs) -> Option<u16> {
		let standard_fee_bps = self.total_fee_bps(args.creator_fee_bps)?;
		add_decay_premium(
			standard_fee_bps,
			FeeDecayArgs {
				current_timestamp: u64::try_from(args.now).ok()?,
				created_at_timestamp: u64::try_from(args.created_at).ok()?,
				decay_seconds: self.fee_decay_seconds,
				decay_start_bps: self.fee_decay_start_bps,
				standard_fee_bps: u64::from(standard_fee_bps),
			},
		)
	}
}

impl types::DexFees {
	/// Returns `protocol_fee_bps` plus `lp_fee_bps` plus `creator_fee_bps`.
	/// Pass `Pool.creator_fee_bps`, not `max_creator_fee_bps`. Returns `None`
	/// on overflow.
	#[must_use]
	pub const fn total_fee_bps(&self, creator_fee_bps: u16) -> Option<u16> {
		match self.protocol_fee_bps.checked_add(self.lp_fee_bps) {
			Some(v) => v.checked_add(creator_fee_bps),
			None => None,
		}
	}

	/// Returns `total_fee_bps` plus the fee decay premium at `args.now`.
	/// Returns `None` on overflow or on a negative time.
	#[must_use]
	pub fn effective_fee_bps(&self, args: EffectiveFeeArgs) -> Option<u16> {
		let standard_fee_bps = self.total_fee_bps(args.creator_fee_bps)?;
		add_decay_premium(
			standard_fee_bps,
			FeeDecayArgs {
				current_timestamp: u64::try_from(args.now).ok()?,
				created_at_timestamp: u64::try_from(args.created_at).ok()?,
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
	use super::EffectiveFeeArgs;
	use super::types::{DexFees, LaunchpadFees};

	/// The partner max differs from each market rate in the tests.
	const LAUNCHPAD: LaunchpadFees = LaunchpadFees {
		creation_fee_cents: 0,
		protocol_fee_bps: 100,
		max_creator_fee_bps: 50,
		fee_decay_seconds: 12,
		fee_decay_start_bps: 5_000,
	};

	const DEX: DexFees = DexFees {
		creation_fee_cents: 0,
		protocol_fee_bps: 80,
		lp_fee_bps: 30,
		max_creator_fee_bps: 40,
		fee_decay_seconds: 12,
		fee_decay_start_bps: 5_000,
	};

	const fn at(now: i64, creator_fee_bps: u16) -> EffectiveFeeArgs {
		EffectiveFeeArgs {
			creator_fee_bps,
			created_at: 100,
			now,
		}
	}

	#[test]
	fn charges_the_decay_start_rate_at_creation() {
		assert_eq!(LAUNCHPAD.effective_fee_bps(at(100, 30)), Some(5_000));
	}

	#[test]
	fn decays_quadratically_rounding_the_premium_up() {
		// 130 standard + ceil(6^2 * 4_870 / 12^2) = 130 + 1_218.
		assert_eq!(LAUNCHPAD.effective_fee_bps(at(106, 30)), Some(1_348));
	}

	#[test]
	fn falls_to_the_standard_rate_when_the_window_closes() {
		assert_eq!(LAUNCHPAD.effective_fee_bps(at(112, 30)), Some(130));
	}

	#[test]
	fn charges_the_full_premium_on_a_creation_time_in_the_future() {
		let args = EffectiveFeeArgs {
			created_at: 200,
			..at(100, 30)
		};
		assert_eq!(LAUNCHPAD.effective_fee_bps(args), Some(5_000));
	}

	#[test]
	fn rejects_a_negative_timestamp() {
		let args = EffectiveFeeArgs {
			created_at: -1,
			..at(100, 30)
		};
		assert_eq!(LAUNCHPAD.effective_fee_bps(args), None);
		assert_eq!(LAUNCHPAD.effective_fee_bps(at(-1, 30)), None);
	}

	/// A market's rate can be above the partner max. A partner can lower its
	/// max after the market's creation.
	#[test]
	fn charges_the_curves_creator_rate_not_the_partner_max() {
		// Below the max of 50.
		assert_eq!(LAUNCHPAD.total_fee_bps(30), Some(130));
		// Above: 180 + 6^2 * 4_820 / 12^2 = 180 + 1_205, exact.
		assert_eq!(LAUNCHPAD.total_fee_bps(80), Some(180));
		assert_eq!(LAUNCHPAD.effective_fee_bps(at(106, 80)), Some(1_385));
		assert_eq!(LAUNCHPAD.effective_fee_bps(at(112, 80)), Some(180));
		// Rate 0: 100 + 6^2 * 4_900 / 12^2 = 100 + 1_225, exact.
		assert_eq!(LAUNCHPAD.total_fee_bps(0), Some(100));
		assert_eq!(LAUNCHPAD.effective_fee_bps(at(106, 0)), Some(1_325));
		assert_eq!(LAUNCHPAD.effective_fee_bps(at(112, 0)), Some(100));
	}

	#[test]
	fn charges_the_pools_creator_rate_not_the_partner_max() {
		// Below the max of 40: 135 + ceil(6^2 * 4_865 / 12^2) = 135 + 1_217.
		assert_eq!(DEX.total_fee_bps(25), Some(135));
		assert_eq!(DEX.effective_fee_bps(at(106, 25)), Some(1_352));
		assert_eq!(DEX.effective_fee_bps(at(112, 25)), Some(135));
		// Above: 170 + ceil(6^2 * 4_830 / 12^2) = 170 + 1_208.
		assert_eq!(DEX.total_fee_bps(60), Some(170));
		assert_eq!(DEX.effective_fee_bps(at(106, 60)), Some(1_378));
		assert_eq!(DEX.effective_fee_bps(at(112, 60)), Some(170));
		// Rate 0: 110 + ceil(6^2 * 4_890 / 12^2) = 110 + 1_223.
		assert_eq!(DEX.total_fee_bps(0), Some(110));
		assert_eq!(DEX.effective_fee_bps(at(106, 0)), Some(1_333));
		assert_eq!(DEX.effective_fee_bps(at(112, 0)), Some(110));
	}

	#[test]
	fn the_partner_max_never_moves_the_fee() {
		for max_creator_fee_bps in [0, u16::MAX] {
			let launchpad = LaunchpadFees {
				max_creator_fee_bps,
				..LAUNCHPAD
			};
			assert_eq!(launchpad.total_fee_bps(30), Some(130));
			assert_eq!(launchpad.effective_fee_bps(at(106, 30)), Some(1_348));

			let dex = DexFees {
				max_creator_fee_bps,
				..DEX
			};
			assert_eq!(dex.total_fee_bps(25), Some(135));
			assert_eq!(dex.effective_fee_bps(at(106, 25)), Some(1_352));
		}
	}

	#[test]
	fn a_total_past_u16_is_none() {
		assert_eq!(LAUNCHPAD.total_fee_bps(65_435), Some(u16::MAX));
		assert_eq!(LAUNCHPAD.total_fee_bps(65_436), None);
		assert_eq!(LAUNCHPAD.effective_fee_bps(at(112, 65_436)), None);
		assert_eq!(DEX.total_fee_bps(65_425), Some(u16::MAX));
		assert_eq!(DEX.total_fee_bps(65_426), None);
		assert_eq!(DEX.effective_fee_bps(at(112, 65_426)), None);
	}

	#[test]
	fn sums_protocol_lp_and_creator_on_the_dex() {
		let fees = DexFees {
			fee_decay_seconds: 0,
			fee_decay_start_bps: 0,
			..DEX
		};
		assert_eq!(fees.effective_fee_bps(at(100, 25)), Some(135));
	}
}
