pub struct FeeSplitArgs {
	pub fee_amount: u64,
	pub protocol_bps: u64,
	pub lp_bps: u64,
	pub base_total_bps: u64,
	pub decay_premium_bps: u64,
}

/// Splits `fee_amount` into `(protocol, lp, creator, sniper)`. LP and creator
/// round down. Protocol gets the decay premium and the remainder. `sniper` is
/// the premium part of `protocol`.
#[must_use]
pub fn split_fee_amount(args: FeeSplitArgs) -> Option<(u64, u64, u64, u64)> {
	let fee_amount = args.fee_amount;
	// A zero fee splits even when the total rate is zero.
	if fee_amount == 0 {
		return Some((0, 0, 0, 0));
	}
	let effective_total_bps =
		args.base_total_bps.checked_add(args.decay_premium_bps)?;
	if effective_total_bps == 0 {
		return None;
	}

	if args.base_total_bps == 0 {
		return Some((fee_amount, 0, 0, fee_amount));
	}

	let lp_fee = u128::from(fee_amount)
		.checked_mul(u128::from(args.lp_bps))?
		.checked_div(u128::from(effective_total_bps))?;
	let lp_fee = u64::try_from(lp_fee).ok()?;

	let creator_bps = args
		.base_total_bps
		.checked_sub(args.protocol_bps)?
		.checked_sub(args.lp_bps)?;
	let creator_fee = u128::from(fee_amount)
		.checked_mul(u128::from(creator_bps))?
		.checked_div(u128::from(effective_total_bps))?;
	let creator_fee = u64::try_from(creator_fee).ok()?;

	let sniper_fee = u128::from(fee_amount)
		.checked_mul(u128::from(args.decay_premium_bps))?
		.checked_div(u128::from(effective_total_bps))?;
	let sniper_fee = u64::try_from(sniper_fee).ok()?;

	let protocol_fee =
		fee_amount.checked_sub(lp_fee)?.checked_sub(creator_fee)?;

	Some((protocol_fee, lp_fee, creator_fee, sniper_fee))
}

#[cfg(test)]
mod tests {
	use super::*;

	const STANDARD: FeeSplitArgs = FeeSplitArgs {
		fee_amount: 1000,
		protocol_bps: 200,
		lp_bps: 200,
		base_total_bps: 1000,
		decay_premium_bps: 0,
	};

	#[test]
	fn test_split_fee_amount_no_dust_lost() {
		let (protocol_fee, lp_fee, creator_fee, sniper_fee) =
			split_fee_amount(STANDARD).unwrap();

		assert_eq!(protocol_fee, 200);
		assert_eq!(lp_fee, 200);
		assert_eq!(creator_fee, 600);
		assert_eq!(sniper_fee, 0);
		assert_eq!(protocol_fee + lp_fee + creator_fee, 1000);
	}

	#[test]
	fn test_split_fee_amount_protocol_wins_rounding() {
		let (protocol_fee, lp_fee, creator_fee, sniper_fee) =
			split_fee_amount(FeeSplitArgs {
				fee_amount: 1001,
				..STANDARD
			})
			.unwrap();

		assert_eq!(lp_fee, 200);
		assert_eq!(creator_fee, 600);
		assert_eq!(protocol_fee, 201);
		assert_eq!(sniper_fee, 0);
		assert_eq!(protocol_fee + lp_fee + creator_fee, 1001);
	}

	#[test]
	fn test_split_fee_with_decay_protocol_wins_rounding() {
		let (protocol_fee, lp_fee, creator_fee, sniper_fee) =
			split_fee_amount(FeeSplitArgs {
				fee_amount: 1001,
				decay_premium_bps: 500,
				..STANDARD
			})
			.unwrap();

		assert_eq!(lp_fee, 133);
		assert_eq!(creator_fee, 400);
		assert_eq!(protocol_fee, 468);
		assert_eq!(sniper_fee, 333);
		assert_eq!(protocol_fee + lp_fee + creator_fee, 1001);
	}

	#[test]
	fn test_split_fee_max_amount() {
		let fee_amount = 1_000_000_000_000_000u64;
		let result = split_fee_amount(FeeSplitArgs {
			fee_amount,
			protocol_bps: 100,
			lp_bps: 100,
			base_total_bps: 300,
			decay_premium_bps: 0,
		});
		assert!(result.is_some());
		let (protocol, lp, creator, _decay) = result.unwrap();
		assert_eq!(protocol + lp + creator, fee_amount);
	}

	#[test]
	fn test_split_fee_zero_amount() {
		assert_eq!(
			split_fee_amount(FeeSplitArgs {
				fee_amount: 0,
				protocol_bps: 100,
				lp_bps: 100,
				base_total_bps: 300,
				decay_premium_bps: 0,
			}),
			Some((0, 0, 0, 0)),
		);
	}

	#[test]
	fn test_split_fee_invalid_bps_underflows() {
		assert_eq!(
			split_fee_amount(FeeSplitArgs {
				protocol_bps: 600,
				lp_bps: 500,
				..STANDARD
			}),
			None,
		);
	}

	#[test]
	fn test_split_fee_zero_total_bps() {
		assert_eq!(
			split_fee_amount(FeeSplitArgs {
				fee_amount: 1000,
				protocol_bps: 0,
				lp_bps: 0,
				base_total_bps: 0,
				decay_premium_bps: 0,
			}),
			None,
		);
	}

	#[test]
	fn test_split_fee_zero_amount_and_zero_total_bps() {
		assert_eq!(
			split_fee_amount(FeeSplitArgs {
				fee_amount: 0,
				protocol_bps: 0,
				lp_bps: 0,
				base_total_bps: 0,
				decay_premium_bps: 0,
			}),
			Some((0, 0, 0, 0)),
		);
	}
}
