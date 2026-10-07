use crate::math::amm::{self, AmmInput, BuyArgs, SellArgs};
use crate::nexus::EffectiveFeeArgs;
use crate::nexus::types::DexFees;
use crate::utils::{
	Landing, MarketQuote, MarketQuoteError, QuoteRequest, TradeDirection,
	TradeMode,
};

/// Read `quote_reserves`, `created_at` and `creator_fee_bps` from the `Pool`
/// account, not from a vault.
#[derive(Clone, Copy, Debug)]
pub struct PoolMarket<'a> {
	/// The base vault's token amount. The program prices from it, not from
	/// `pool.base_reserves`.
	pub base_vault_amount: u64,
	pub quote_reserves: u64,
	pub created_at: i64,
	/// `PartnerConfig.dex` of the trade's partner on the pool's platform. It
	/// sets the protocol rate, the LP rate and the decay.
	pub fees: &'a DexFees,
	pub landing: Landing,
	/// `Pool.creator_fee_bps`, not the partner's `max_creator_fee_bps`.
	pub creator_fee_bps: u16,
}

/// Calculates a trade on a DEX pool. The status is not checked: pass an
/// `Active` pool.
pub fn quote(
	market: &PoolMarket<'_>,
	request: QuoteRequest,
) -> Result<MarketQuote, MarketQuoteError> {
	let fee_bps = market
		.fees
		.effective_fee_bps(EffectiveFeeArgs {
			creator_fee_bps: market.creator_fee_bps,
			created_at: market.created_at,
			now: market.landing.unix_timestamp,
		})
		.ok_or(MarketQuoteError::FeeOutOfRange)?;
	let amm = AmmInput {
		quote_reserves: market.quote_reserves,
		base_reserves: market.base_vault_amount,
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
				base_reserve_cap: None,
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
			Ok(MarketQuote::sold(&sold, fee_bps))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::math::amm::{BuyQuote, SellQuote};

	const NOW: i64 = 1_000;
	const BASE_VAULT: u64 = 400_000_000_000;
	const QUOTE_RESERVES: u64 = 25_000_000_000;

	/// The partner max differs from each pool's rate in the tests.
	const FEES: DexFees = DexFees {
		creation_fee_cents: 0,
		protocol_fee_bps: 80,
		lp_fee_bps: 30,
		max_creator_fee_bps: 100,
		fee_decay_seconds: 0,
		fee_decay_start_bps: 0,
	};

	const LANDING: Landing = Landing {
		base_fee: None,
		quote_fee: None,
		unix_timestamp: NOW,
	};

	const MARKET: PoolMarket<'static> = PoolMarket {
		base_vault_amount: BASE_VAULT,
		quote_reserves: QUOTE_RESERVES,
		created_at: 0,
		fees: &FEES,
		landing: LANDING,
		creator_fee_bps: 40,
	};

	const fn priced(amount: u64) -> AmmInput {
		AmmInput {
			quote_reserves: QUOTE_RESERVES,
			base_reserves: BASE_VAULT,
			amount,
			fee_bps: 150,
		}
	}

	fn bought(args: BuyArgs, mode: TradeMode) -> BuyQuote {
		match mode {
			TradeMode::ExactIn => amm::buy_exact_in_with_fees(args),
			TradeMode::ExactOut => amm::buy_exact_out_with_fees(args),
		}
		.unwrap()
	}

	fn sold(args: SellArgs, mode: TradeMode) -> SellQuote {
		match mode {
			TradeMode::ExactIn => amm::sell_exact_in_with_fees(args),
			TradeMode::ExactOut => amm::sell_exact_out_with_fees(args),
		}
		.unwrap()
	}

	#[test]
	fn a_buy_prices_off_the_base_vault() {
		let quote = quote(
			&MARKET,
			QuoteRequest {
				direction: TradeDirection::Buy,
				mode: TradeMode::ExactIn,
				amount: 1_000_000_000,
			},
		)
		.unwrap();
		assert_eq!(quote.out_amount, 15_162_593_804);
		assert_eq!(quote.fee, 15_000_000);
		assert_eq!(quote.fee_bps, 150);
		assert!(!quote.supply_capped);
	}

	#[test]
	fn every_trade_matches_the_math_over_the_programs_inputs() {
		let amount = 1_000_000_000;
		for mode in [TradeMode::ExactIn, TradeMode::ExactOut] {
			let buy = bought(
				BuyArgs {
					amm: priced(amount),
					quote_fee: None,
					base_fee: None,
					base_reserve_cap: None,
				},
				mode,
			);
			assert_eq!(
				quote(
					&MARKET,
					QuoteRequest {
						direction: TradeDirection::Buy,
						mode,
						amount,
					}
				),
				Ok(MarketQuote::bought(
					&buy,
					QuoteRequest {
						direction: TradeDirection::Buy,
						mode,
						amount,
					},
					150
				))
			);

			let sell = sold(
				SellArgs {
					amm: priced(amount),
					quote_fee: None,
					base_fee: None,
				},
				mode,
			);
			let quote = quote(
				&MARKET,
				QuoteRequest {
					direction: TradeDirection::Sell,
					mode,
					amount,
				},
			)
			.unwrap();
			assert_eq!(quote, MarketQuote::sold(&sell, 150));
			assert!(!quote.supply_capped);
		}
	}

	/// A pool's rate can be above the partner max. A partner can lower its max
	/// after the pool's creation.
	#[test]
	fn the_pools_creator_rate_prices_the_trade_not_the_partner_max() {
		let decaying = DexFees {
			fee_decay_seconds: 12,
			fee_decay_start_bps: 5_000,
			..FEES
		};
		let buy = QuoteRequest {
			direction: TradeDirection::Buy,
			mode: TradeMode::ExactIn,
			amount: 1_000_000_000,
		};
		// `(fee_bps, fee, out_amount)` past the decay window, then halfway
		// through it.
		for (creator_fee_bps, standard, halfway) in [
			(
				0,
				(110, 11_000_000, 15_221_824_618),
				(1_333, 133_300_000, 13_402_560_048),
			),
			(
				40,
				(150, 15_000_000, 15_162_593_804),
				(1_363, 136_300_000, 13_357_717_573),
			),
			(
				160,
				(270, 27_000_000, 14_984_791_899),
				(1_453, 145_300_000, 13_223_127_709),
			),
		] {
			let market = PoolMarket {
				creator_fee_bps,
				fees: &decaying,
				..MARKET
			};
			let past = quote(&market, buy).unwrap();
			assert_eq!((past.fee_bps, past.fee, past.out_amount), standard);

			let inside = quote(
				&PoolMarket {
					created_at: NOW - 6,
					..market
				},
				buy,
			)
			.unwrap();
			assert_eq!(
				(inside.fee_bps, inside.fee, inside.out_amount),
				halfway
			);
		}
	}

	#[test]
	fn the_fee_decays_from_the_pools_created_at() {
		let decaying = DexFees {
			fee_decay_seconds: 12,
			fee_decay_start_bps: 5_000,
			..FEES
		};
		let fresh = quote(
			&PoolMarket {
				created_at: NOW,
				fees: &decaying,
				..MARKET
			},
			QuoteRequest {
				direction: TradeDirection::Buy,
				mode: TradeMode::ExactIn,
				amount: 1_000_000_000,
			},
		)
		.unwrap();
		assert_eq!(fresh.fee_bps, 5_000);
		assert_eq!(fresh.fee, 500_000_000);

		assert_eq!(
			quote(
				&PoolMarket {
					created_at: -1,
					..MARKET
				},
				QuoteRequest {
					direction: TradeDirection::Sell,
					mode: TradeMode::ExactIn,
					amount: 1_000,
				}
			),
			Err(MarketQuoteError::FeeOutOfRange)
		);
	}
}
