use crate::math::amm::{
	self, AmmError, AmmInput, BuyArgs, QuoteError, SellArgs,
};
use crate::nexus::types::LaunchpadFees;
use crate::utils::{
	Landing, MarketQuote, MarketQuoteError, QuoteRequest, TradeDirection,
	TradeMode,
};

#[derive(Clone, Copy, Debug)]
pub struct CurveMarket<'a> {
	pub virtual_base_reserves: u64,
	pub virtual_quote_reserves: u64,
	pub real_base_reserves: u64,
	pub real_quote_reserves: u64,
	pub created_at: i64,
	/// `PartnerConfig.launchpad` of the trade's partner on the curve's
	/// platform.
	pub fees: &'a LaunchpadFees,
	pub landing: Landing,
}

/// Calculates a trade on a bonding curve. The price comes from the virtual
/// reserves. The status is not checked: pass a `Funding` curve.
pub fn quote(
	market: &CurveMarket<'_>,
	request: QuoteRequest,
) -> Result<MarketQuote, MarketQuoteError> {
	// The program refuses every trade here with `ThresholdReached`.
	if market.real_base_reserves == 0 {
		return Err(MarketQuoteError::SupplyExhausted);
	}
	let fee_bps = market
		.fees
		.effective_fee_bps(market.created_at, market.landing.unix_timestamp)
		.ok_or(MarketQuoteError::FeeOutOfRange)?;
	let amm = AmmInput {
		quote_reserves: market.virtual_quote_reserves,
		base_reserves: market.virtual_base_reserves,
		amount: request.amount,
		fee_bps,
	};
	let Landing {
		base_fee,
		quote_fee,
		..
	} = market.landing;

	match request.direction {
		TradeDirection::Buy => {
			let args = BuyArgs {
				amm,
				quote_fee,
				base_fee,
				base_reserve_cap: Some(market.real_base_reserves),
			};
			let bought = match request.mode {
				TradeMode::ExactIn => amm::buy_exact_in_with_fees(args),
				TradeMode::ExactOut => amm::buy_exact_out_with_fees(args),
			}?;
			Ok(MarketQuote::bought(&bought, request, fee_bps))
		}
		TradeDirection::Sell => {
			let args = SellArgs {
				amm,
				quote_fee,
				base_fee,
			};
			let sold = match request.mode {
				TradeMode::ExactIn => amm::sell_exact_in_with_fees(args),
				TradeMode::ExactOut => amm::sell_exact_out_with_fees(args),
			}?;
			// The virtual reserves can price more quote than the curve holds.
			// The debit includes the fee.
			let debit = sold
				.quote_amount
				.checked_add(sold.fee)
				.ok_or(QuoteError::Amm(AmmError::Overflow))?;
			if debit > market.real_quote_reserves {
				return Err(MarketQuoteError::ExceedsQuoteHeld);
			}
			Ok(MarketQuote::sold(&sold, fee_bps))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::math::amm::MintFee;

	const NOW: i64 = 1_000;
	const ONE_SOL: u64 = 1_000_000_000;

	const FEES: LaunchpadFees = LaunchpadFees {
		creation_fee_cents: 0,
		protocol_fee_bps: 100,
		creator_fee_bps: 0,
		fee_decay_seconds: 0,
		fee_decay_start_bps: 0,
	};

	const LANDING: Landing = Landing {
		base_fee: None,
		quote_fee: None,
		unix_timestamp: NOW,
	};

	const MARKET: CurveMarket<'static> = CurveMarket {
		virtual_base_reserves: 1_000_000_000_000_000,
		virtual_quote_reserves: 30_000_000_000,
		real_base_reserves: 500_000_000_000_000,
		real_quote_reserves: 0,
		created_at: 0,
		fees: &FEES,
		landing: LANDING,
	};

	const BASE_100_BPS: MintFee = MintFee {
		bps: 100,
		maximum_fee: u64::MAX,
	};

	const fn request(
		direction: TradeDirection,
		mode: TradeMode,
		amount: u64,
	) -> QuoteRequest {
		QuoteRequest {
			direction,
			mode,
			amount,
		}
	}

	#[test]
	fn a_buy_prices_off_the_virtual_reserves() {
		let quote = quote(
			&MARKET,
			request(TradeDirection::Buy, TradeMode::ExactIn, ONE_SOL),
		)
		.unwrap();
		assert_eq!(quote.in_amount, ONE_SOL);
		assert_eq!(quote.out_amount, 31_945_788_964_181);
		assert_eq!(quote.fee, 10_000_000);
		assert_eq!(quote.fee_bps, 100);
		assert!(!quote.supply_capped);
	}

	#[test]
	fn the_fee_decays_from_the_curves_creation() {
		let decaying = LaunchpadFees {
			fee_decay_seconds: 12,
			fee_decay_start_bps: 5_000,
			..FEES
		};
		let fresh = quote(
			&CurveMarket {
				created_at: NOW,
				fees: &decaying,
				..MARKET
			},
			request(TradeDirection::Buy, TradeMode::ExactIn, ONE_SOL),
		)
		.unwrap();
		assert_eq!(fresh.fee_bps, 5_000);
		assert_eq!(fresh.fee, 500_000_000);
		assert_eq!(fresh.out_amount, 16_393_442_622_950);

		assert_eq!(
			quote(
				&CurveMarket {
					created_at: -1,
					..MARKET
				},
				request(TradeDirection::Buy, TradeMode::ExactIn, ONE_SOL)
			),
			Err(MarketQuoteError::FeeOutOfRange)
		);
	}

	#[test]
	fn an_exact_in_buy_past_the_supply_left_fills_the_cap() {
		let quote = quote(
			&CurveMarket {
				real_base_reserves: 1_000_000_000,
				..MARKET
			},
			request(TradeDirection::Buy, TradeMode::ExactIn, ONE_SOL),
		)
		.unwrap();
		assert_eq!(quote.out_amount, 1_000_000_000);
		assert_eq!(quote.in_amount, 30_305);
		assert!(quote.supply_capped);
	}

	/// With a base transfer fee, the cap applies to the base that leaves the
	/// vault, not to the base that the buyer receives.
	#[test]
	fn an_exact_out_buy_past_the_supply_left_is_flagged() {
		let wanted = 10_000_000_000_000;
		let landing = Landing {
			base_fee: Some(BASE_100_BPS),
			..LANDING
		};
		let holding = |real_base_reserves| CurveMarket {
			real_base_reserves,
			landing,
			..MARKET
		};
		let just_enough = quote(
			&holding(10_101_010_101_011),
			request(TradeDirection::Buy, TradeMode::ExactOut, wanted),
		)
		.unwrap();
		assert_eq!(just_enough.out_amount, wanted);
		assert!(!just_enough.supply_capped);

		let one_short = quote(
			&holding(10_101_010_101_010),
			request(TradeDirection::Buy, TradeMode::ExactOut, wanted),
		)
		.unwrap();
		assert_eq!(one_short.out_amount, 9_999_999_999_999);
		assert!(one_short.supply_capped);
	}

	#[test]
	fn a_sell_is_bounded_by_the_quote_the_curve_holds() {
		let sell = request(
			TradeDirection::Sell,
			TradeMode::ExactIn,
			10_000_000_000_000,
		);
		let sold = amm::sell_exact_in_with_fees(SellArgs {
			amm: AmmInput {
				quote_reserves: 30_000_000_000,
				base_reserves: 1_000_000_000_000_000,
				amount: 10_000_000_000_000,
				fee_bps: 100,
			},
			quote_fee: None,
			base_fee: None,
		})
		.unwrap();
		let debit = sold.quote_amount + sold.fee;
		assert_eq!(debit, 297_029_702);

		let holding = |real_quote_reserves| CurveMarket {
			real_quote_reserves,
			..MARKET
		};
		let covered = quote(&holding(debit), sell).unwrap();
		assert_eq!(covered.in_amount, 10_000_000_000_000);
		assert_eq!(covered.out_amount, sold.quote_to_user);
		assert!(!covered.supply_capped);
		assert_eq!(
			quote(&holding(debit - 1), sell),
			Err(MarketQuoteError::ExceedsQuoteHeld)
		);
	}

	#[test]
	fn a_curve_with_no_supply_left_quotes_nothing() {
		let complete = CurveMarket {
			real_base_reserves: 0,
			real_quote_reserves: 85_000_000_000,
			..MARKET
		};
		for direction in [TradeDirection::Buy, TradeDirection::Sell] {
			for mode in [TradeMode::ExactIn, TradeMode::ExactOut] {
				assert_eq!(
					quote(&complete, request(direction, mode, 1_000)),
					Err(MarketQuoteError::SupplyExhausted)
				);
			}
		}
	}
}
