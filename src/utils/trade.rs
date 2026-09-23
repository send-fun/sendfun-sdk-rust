use std::fmt;

use solana_address::Address;

use super::pda::find_associated_token_pda;
use crate::constants::TOKEN_2022_PROGRAM_ID;
use crate::math::amm::{BuyQuote, MintFee, QuoteError, SellQuote};

/// `launchpad::types::TradeDirection`. `.into()` converts it to
/// `dex::types::TradeDirection`.
pub use crate::launchpad::types::TradeDirection;

impl From<TradeDirection> for crate::dex::types::TradeDirection {
	fn from(direction: TradeDirection) -> Self {
		match direction {
			TradeDirection::Buy => Self::Buy,
			TradeDirection::Sell => Self::Sell,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TradeMode {
	ExactIn,
	ExactOut,
}

/// The accounts of a trade instruction. `user_base_account` and
/// `user_quote_account` can be any token accounts that `user` owns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TradeAccounts {
	pub user: Address,
	pub payer: Address,
	/// Bonding curve or pool.
	pub market: Address,
	pub base_mint: Address,
	pub quote_mint: Address,
	pub base_vault: Address,
	pub quote_vault: Address,
	pub user_base_account: Address,
	pub user_quote_account: Address,
	/// Signs unless it is `DEFAULT_PARTNER`.
	pub partner: Address,
	/// The `PartnerConfig` PDA of `partner` on the market's `platform_config`.
	pub partner_config: Address,
	pub quote_token_program: Address,
}

impl TradeAccounts {
	/// Sets `user`, `payer`, `user_base_account` and `user_quote_account` to
	/// fixed keys that no wallet holds.
	#[must_use]
	pub const fn with_stand_in_trader(self) -> StandInTrade {
		StandInTrade(Self {
			user: Address::new_from_array([1; 32]),
			payer: Address::new_from_array([2; 32]),
			user_base_account: Address::new_from_array([3; 32]),
			user_quote_account: Address::new_from_array([4; 32]),
			..self
		})
	}
}

/// A trade with the keys of [`TradeAccounts::with_stand_in_trader`].
/// [`MarketHooks::screen`](crate::transfer_hook::MarketHooks::screen) takes
/// only this type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StandInTrade(TradeAccounts);

impl std::ops::Deref for StandInTrade {
	type Target = TradeAccounts;

	fn deref(&self) -> &TradeAccounts {
		&self.0
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuoteRequest {
	pub direction: TradeDirection,
	pub mode: TradeMode,
	/// `ExactIn`: the amount that the user sends. `ExactOut`: the amount that
	/// the user receives.
	pub amount: u64,
}

/// The transfer fees and the Unix time, in seconds, when the trade lands. Get
/// each fee from
/// [`MintState::fee_at`](crate::transfer_hook::MintState::fee_at) with the
/// landing epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landing {
	pub base_fee: Option<MintFee>,
	pub quote_fee: Option<MintFee>,
	pub unix_timestamp: i64,
}

/// The trade that the program would make. The caller decides whether to send
/// it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MarketQuote {
	/// The amount that the user sends, with the input mint's transfer fee.
	pub in_amount: u64,
	/// The amount that the user receives, after the output mint's transfer fee.
	pub out_amount: u64,
	/// Platform fee, in quote units.
	pub fee: u64,
	/// Platform fee rate in bps, with the fee decay premium.
	pub fee_bps: u16,
	/// `true` when the supply left on the curve makes the fill smaller than the
	/// request. The program fills a capped `ExactIn` buy. It refuses a capped
	/// `ExactOut` buy.
	pub supply_capped: bool,
}

impl MarketQuote {
	/// Only the supply cap makes a buy smaller than its request.
	pub(crate) const fn bought(
		quote: &BuyQuote,
		request: QuoteRequest,
		fee_bps: u16,
	) -> Self {
		let requested = match request.mode {
			TradeMode::ExactIn => quote.quote_from_user,
			TradeMode::ExactOut => quote.base_to_user,
		};
		Self {
			in_amount: quote.quote_from_user,
			out_amount: quote.base_to_user,
			fee: quote.fee,
			fee_bps,
			supply_capped: requested < request.amount,
		}
	}

	pub(crate) const fn sold(quote: &SellQuote, fee_bps: u16) -> Self {
		Self {
			in_amount: quote.base_from_user,
			out_amount: quote.quote_to_user,
			fee: quote.fee,
			fee_bps,
			supply_capped: false,
		}
	}
}

/// A trade that the program refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarketQuoteError {
	Quote(QuoteError),
	FeeOutOfRange,
	/// `real_base_reserves` is 0. The program error is `ThresholdReached`.
	SupplyExhausted,
	/// The sell takes more quote from the curve than `real_quote_reserves`.
	ExceedsQuoteHeld,
}

impl From<QuoteError> for MarketQuoteError {
	fn from(error: QuoteError) -> Self {
		Self::Quote(error)
	}
}

impl fmt::Display for MarketQuoteError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Quote(error) => error.fmt(f),
			Self::FeeOutOfRange => write!(f, "fee out of range"),
			Self::SupplyExhausted => write!(f, "the curve has no supply left"),
			Self::ExceedsQuoteHeld => {
				write!(f, "sell exceeds the quote the curve holds")
			}
		}
	}
}

impl std::error::Error for MarketQuoteError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Quote(error) => Some(error),
			_ => None,
		}
	}
}

pub(crate) struct TradeStateFields<'a> {
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	/// The `platform_config` of the curve or pool. Seeds `partner_config`.
	pub platform_config: &'a Address,
}

pub(crate) struct DerivedTradeAccounts {
	pub user_base_account: Address,
	pub user_quote_account: Address,
	pub partner_config: Address,
}

pub(crate) struct TradeUserContext<'a> {
	pub user: &'a Address,
	pub partner: &'a Address,
	pub quote_token_program: &'a Address,
}

pub(crate) struct DeriveTradeArgs<'a> {
	pub state: &'a TradeStateFields<'a>,
	pub ctx: TradeUserContext<'a>,
}

pub(crate) fn derive_trade_accounts(
	args: DeriveTradeArgs<'_>,
) -> DerivedTradeAccounts {
	let state = args.state;
	let user = args.ctx.user;
	let partner = args.ctx.partner;
	let quote_token_program = args.ctx.quote_token_program;

	let (user_base_account, _) = find_associated_token_pda(
		user,
		state.base_mint,
		&TOKEN_2022_PROGRAM_ID,
	);
	let (user_quote_account, _) =
		find_associated_token_pda(user, state.quote_mint, quote_token_program);
	let (partner_config, _) = crate::nexus::pda::find_partner_config_pda(
		state.platform_config,
		partner,
	);

	DerivedTradeAccounts {
		user_base_account,
		user_quote_account,
		partner_config,
	}
}
