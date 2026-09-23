pub struct FeeDecayArgs {
	pub current_timestamp: u64,
	pub created_at_timestamp: u64,
	pub decay_seconds: u16,
	pub decay_start_bps: u16,
	pub standard_fee_bps: u64,
}

/// Fee decay premium, in bps. It falls quadratically from
/// `decay_start_bps - standard_fee_bps` at creation to 0 at `decay_seconds`.
#[must_use]
pub fn calculate_fee_decay_premium(args: FeeDecayArgs) -> Option<u64> {
	if args.decay_seconds == 0 {
		return Some(0);
	}

	let start_bps = u64::from(args.decay_start_bps);

	// A creation time in the future pays the full premium.
	let Some(elapsed) = args
		.current_timestamp
		.checked_sub(args.created_at_timestamp)
	else {
		return start_bps.checked_sub(args.standard_fee_bps).or(Some(0));
	};
	let decay_seconds_u64 = u64::from(args.decay_seconds);

	if elapsed >= decay_seconds_u64 {
		return Some(0);
	}
	if start_bps <= args.standard_fee_bps {
		return Some(0);
	}

	let range = start_bps.checked_sub(args.standard_fee_bps)?;
	let remaining = decay_seconds_u64.checked_sub(elapsed)?;

	let numerator = u128::from(remaining)
		.checked_mul(u128::from(remaining))?
		.checked_mul(u128::from(range))?;
	let denominator = u128::from(decay_seconds_u64)
		.checked_mul(u128::from(decay_seconds_u64))?;

	// Rounds up. The payer pays the remainder.
	let mut result = numerator.checked_div(denominator)?;
	let remainder = numerator.checked_rem(denominator)?;
	if remainder > 0 {
		result = result.checked_add(1)?;
	}
	u64::try_from(result).ok()
}

#[cfg(test)]
mod tests {
	use super::*;

	const STANDARD: FeeDecayArgs = FeeDecayArgs {
		current_timestamp: 100,
		created_at_timestamp: 100,
		decay_seconds: 12,
		decay_start_bps: 5000,
		standard_fee_bps: 100,
	};

	#[test]
	fn test_decay_disabled() {
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				created_at_timestamp: 50,
				decay_seconds: 0,
				..STANDARD
			}),
			Some(0),
		);
	}

	#[test]
	fn test_decay_at_start() {
		assert_eq!(calculate_fee_decay_premium(STANDARD), Some(4900));
	}

	#[test]
	fn test_decay_past_window() {
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				current_timestamp: 200,
				..STANDARD
			}),
			Some(0),
		);
	}

	#[test]
	fn test_decay_exactly_at_window_end() {
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				current_timestamp: 112,
				..STANDARD
			}),
			Some(0),
		);
	}

	#[test]
	fn test_decay_midpoint() {
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				current_timestamp: 106,
				..STANDARD
			}),
			Some(1225),
		);
	}

	#[test]
	fn test_decay_one_second_remaining() {
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				current_timestamp: 111,
				..STANDARD
			}),
			Some(35),
		);
	}

	#[test]
	fn test_decay_start_less_than_standard() {
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				decay_start_bps: 100,
				..STANDARD
			}),
			Some(0),
		);
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				decay_start_bps: 50,
				..STANDARD
			}),
			Some(0),
		);
	}

	#[test]
	fn test_decay_created_in_future() {
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				created_at_timestamp: 200,
				..STANDARD
			}),
			Some(4900),
		);
	}

	#[test]
	fn test_overflow_protection() {
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				current_timestamp: 0,
				created_at_timestamp: 0,
				decay_seconds: u16::MAX,
				decay_start_bps: 10000,
				standard_fee_bps: 100,
			}),
			Some(9900),
		);
	}

	#[test]
	fn test_max_bps_u64_downcast_safe() {
		assert_eq!(
			calculate_fee_decay_premium(FeeDecayArgs {
				current_timestamp: 0,
				created_at_timestamp: 0,
				decay_seconds: u16::MAX,
				decay_start_bps: u16::MAX,
				standard_fee_bps: 0,
			}),
			Some(u64::from(u16::MAX)),
		);
	}
}
