use crate::math::amm::{self, AmmInput, BuyArgs, SellArgs};
use crate::nexus::types::DexFees;
use crate::utils::{
	Landing, MarketQuote, MarketQuoteError, QuoteRequest, TradeDirection,
	TradeMode,
};

/// Read `quote_reserves` and `created_at` from the `Pool` account, not from a
/// vault.
#[derive(Clone, Copy, Debug)]
pub struct PoolMarket<'a> {
	/// The base vault's token amount. The program prices from it, not from
	/// `pool.base_reserves`.
	pub base_vault_amount: u64,
	pub quote_reserves: u64,
	pub created_at: i64,
	/// `PartnerConfig.dex` of the trade's partner on the pool's platform.
	pub fees: &'a DexFees,
	pub landing: Landing,
}

/// Calculates a trade on a DEX pool. The status is not checked: pass an
/// `Active` pool.
pub fn quote(
	market: &PoolMarket<'_>,
	request: QuoteRequest,
) -> Result<MarketQuote, MarketQuoteError> {
	let fee_bps = market
		.fees
		.effective_fee_bps(market.created_at, market.landing.unix_timestamp)
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

	const FEES: DexFees = DexFees {
		creation_fee_cents: 0,
		protocol_fee_bps: 80,
		lp_fee_bps: 30,
		creator_fee_bps: 40,
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
