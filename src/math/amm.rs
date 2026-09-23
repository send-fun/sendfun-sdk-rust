use std::fmt;

const BPS_DIVISOR: u128 = 10_000;

/// Bps-scale denominators only. The `+ denominator` step can overflow.
fn ceil_div(numerator: u128, denominator: u128) -> Option<u128> {
	numerator
		.checked_add(denominator)?
		.checked_sub(1)?
		.checked_div(denominator)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmInput {
	pub quote_reserves: u64,
	pub base_reserves: u64,
	pub amount: u64,
	pub fee_bps: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeQuote {
	pub base_amount: u64,
	pub quote_amount: u64,
	/// Platform fee, in quote units.
	pub fee: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmError {
	InsufficientLiquidity,
	InvalidAmount,
	Overflow,
}

impl fmt::Display for AmmError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InsufficientLiquidity => write!(f, "Insufficient liquidity"),
			Self::InvalidAmount => write!(f, "Invalid amount"),
			Self::Overflow => write!(f, "Arithmetic overflow"),
		}
	}
}

impl std::error::Error for AmmError {}

/// A Token-2022 transfer fee for one epoch. Use the fee of the landing epoch.
/// A stale fee gives a wrong price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintFee {
	/// 0 to `10_000`.
	pub bps: u16,
	pub maximum_fee: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteError {
	Amm(AmmError),
	InvalidTransferFee,
	/// No transfer amount lands exactly the requested amount.
	TransferFeeNotSettleable,
}

impl From<AmmError> for QuoteError {
	fn from(error: AmmError) -> Self {
		Self::Amm(error)
	}
}

impl fmt::Display for QuoteError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Amm(error) => error.fmt(f),
			Self::InvalidTransferFee => {
				write!(f, "Transfer fee rate above 10000 bps")
			}
			Self::TransferFeeNotSettleable => {
				write!(f, "Transfer fee not settleable")
			}
		}
	}
}

impl std::error::Error for QuoteError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Amm(error) => Some(error),
			_ => None,
		}
	}
}

fn validate_mint_fee(fee: MintFee) -> Result<(), QuoteError> {
	if u128::from(fee.bps) > BPS_DIVISOR {
		return Err(QuoteError::InvalidTransferFee);
	}
	Ok(())
}

/// The transfer fee on `amount`, as SPL calculates it: rounds up, then caps at
/// `maximum_fee`. `0` for `None`.
pub fn fee_on(amount: u64, fee: Option<MintFee>) -> Result<u64, QuoteError> {
	let Some(fee) = fee else {
		return Ok(0);
	};
	validate_mint_fee(fee)?;
	if amount == 0 || fee.bps == 0 {
		return Ok(0);
	}

	let numerator = u128::from(amount)
		.checked_mul(u128::from(fee.bps))
		.ok_or(AmmError::Overflow)?;
	let raw = ceil_div(numerator, BPS_DIVISOR).ok_or(AmmError::Overflow)?;
	let raw = u64::try_from(raw).map_err(|_| AmmError::Overflow)?;

	Ok(raw.min(fee.maximum_fee))
}

pub fn amount_after_fee(
	amount: u64,
	fee: Option<MintFee>,
) -> Result<u64, QuoteError> {
	amount
		.checked_sub(fee_on(amount, fee)?)
		.ok_or_else(|| AmmError::Overflow.into())
}

/// Mirrors SPL's `TransferFee::calculate_pre_fee_amount` line for line.
fn pre_fee_amount(amount: u64, fee: MintFee) -> Option<u64> {
	let bps = u128::from(fee.bps);
	match (bps, amount) {
		(0, _) => Some(amount),
		// `gross_up` never gets here. Kept to match SPL.
		(_, 0) => Some(0),
		(BPS_DIVISOR, _) => amount.checked_add(fee.maximum_fee),
		_ => {
			let numerator = u128::from(amount).checked_mul(BPS_DIVISOR)?;
			let denominator = BPS_DIVISOR.checked_sub(bps)?;
			let raw_pre_fee_amount = ceil_div(numerator, denominator)?;

			if raw_pre_fee_amount.checked_sub(u128::from(amount))?
				>= u128::from(fee.maximum_fee)
			{
				amount.checked_add(fee.maximum_fee)
			} else {
				u64::try_from(raw_pre_fee_amount).ok()
			}
		}
	}
}

/// The amount to send so that exactly `amount` lands after the transfer fee.
/// Fails with `TransferFeeNotSettleable` when no sent amount lands exactly
/// `amount`.
pub fn gross_up(amount: u64, fee: Option<MintFee>) -> Result<u64, QuoteError> {
	let Some(schedule) = fee else {
		return Ok(amount);
	};
	validate_mint_fee(schedule)?;
	if amount == 0 {
		return Ok(0);
	}

	let pre_fee = pre_fee_amount(amount, schedule)
		.ok_or(QuoteError::TransferFeeNotSettleable)?;
	let implied_fee = fee_on(pre_fee, fee)?;
	let gross = amount
		.checked_add(implied_fee)
		.ok_or(QuoteError::TransferFeeNotSettleable)?;

	if fee_on(gross, fee)? != implied_fee {
		return Err(QuoteError::TransferFeeNotSettleable);
	}

	Ok(gross)
}

/// `amm.amount` is the user's amount: the quote that the user sends for
/// [`buy_exact_in_with_fees`], the base that the user receives for
/// [`buy_exact_out_with_fees`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyArgs {
	pub amm: AmmInput,
	pub quote_fee: Option<MintFee>,
	pub base_fee: Option<MintFee>,
	/// Launchpad: `bonding_curve.real_base_reserves`. `None` for a DEX pool.
	pub base_reserve_cap: Option<u64>,
}

/// `amm.amount` is the user's amount: the base that the user sends for
/// [`sell_exact_in_with_fees`], the quote that the user receives for
/// [`sell_exact_out_with_fees`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellArgs {
	pub amm: AmmInput,
	pub quote_fee: Option<MintFee>,
	pub base_fee: Option<MintFee>,
}

/// `base_amount` and `quote_amount` are the transfer amounts. For the user's
/// amounts, read `base_to_user` and `quote_from_user`. Do not calculate them
/// from the transfer amounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyQuote {
	pub base_amount: u64,
	pub quote_amount: u64,
	/// Platform fee, in quote units, on the quote that reaches the vault.
	pub fee: u64,
	pub base_transfer_fee: u64,
	pub quote_transfer_fee: u64,
	/// The base that the buyer receives, after the base mint's transfer fee.
	pub base_to_user: u64,
	/// The quote that the buyer sends, with the transfer fee. Equals
	/// `quote_amount`.
	pub quote_from_user: u64,
}

/// Same field rules as [`BuyQuote`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellQuote {
	pub base_amount: u64,
	pub quote_amount: u64,
	/// Platform fee, in quote units, on the quote output of the AMM before the
	/// fee.
	pub fee: u64,
	pub base_transfer_fee: u64,
	pub quote_transfer_fee: u64,
	/// The base that the seller sends, with the transfer fee. Equals
	/// `base_amount`.
	pub base_from_user: u64,
	/// The quote that the seller receives, after the quote mint's transfer fee.
	pub quote_to_user: u64,
}

struct BuyLegs {
	base_amount: u64,
	quote_amount: u64,
	fee: u64,
	quote_transfer_fee: u64,
	base_fee: Option<MintFee>,
}

fn buy_quote(legs: BuyLegs) -> Result<BuyQuote, QuoteError> {
	let base_transfer_fee = fee_on(legs.base_amount, legs.base_fee)?;
	Ok(BuyQuote {
		base_amount: legs.base_amount,
		quote_amount: legs.quote_amount,
		fee: legs.fee,
		base_transfer_fee,
		quote_transfer_fee: legs.quote_transfer_fee,
		base_to_user: legs
			.base_amount
			.checked_sub(base_transfer_fee)
			.ok_or(AmmError::Overflow)?,
		quote_from_user: legs.quote_amount,
	})
}

/// Set `min_amount_out` to `calculate_slippage_down(q.base_to_user, bps)`.
pub fn buy_exact_in_with_fees(args: BuyArgs) -> Result<BuyQuote, QuoteError> {
	let quote_from_user = args.amm.amount;
	if quote_from_user == 0 {
		return Err(AmmError::InvalidAmount.into());
	}

	let quote_into_vault = amount_after_fee(quote_from_user, args.quote_fee)?;
	if quote_into_vault == 0 {
		return Err(AmmError::InvalidAmount.into());
	}

	let uncapped = buy_exact_in(AmmInput {
		amount: quote_into_vault,
		..args.amm
	})?;

	// Above the cap, price `cap` as exact-out. The user pays only for `cap`, at
	// or below the offer.
	let Some(cap) = args
		.base_reserve_cap
		.filter(|cap| uncapped.base_amount > *cap)
	else {
		return buy_quote(BuyLegs {
			base_amount: uncapped.base_amount,
			quote_amount: quote_from_user,
			fee: uncapped.fee,
			quote_transfer_fee: quote_from_user
				.checked_sub(quote_into_vault)
				.ok_or(AmmError::Overflow)?,
			base_fee: args.base_fee,
		});
	};

	let capped = buy_exact_out(AmmInput {
		amount: cap,
		..args.amm
	})?;
	let quote_from_user = gross_up(capped.quote_amount, args.quote_fee)?;

	buy_quote(BuyLegs {
		base_amount: cap,
		quote_amount: quote_from_user,
		fee: capped.fee,
		quote_transfer_fee: quote_from_user
			.checked_sub(capped.quote_amount)
			.ok_or(AmmError::Overflow)?,
		base_fee: args.base_fee,
	})
}

/// Set `max_amount_in` to `calculate_slippage_up(q.quote_from_user, bps)`.
pub fn buy_exact_out_with_fees(args: BuyArgs) -> Result<BuyQuote, QuoteError> {
	let base_to_user = args.amm.amount;
	if base_to_user == 0 {
		return Err(AmmError::InvalidAmount.into());
	}

	let base_out_of_vault = gross_up(base_to_user, args.base_fee)?;
	let base_amount = args
		.base_reserve_cap
		.map_or(base_out_of_vault, |cap| base_out_of_vault.min(cap));

	let bought = buy_exact_out(AmmInput {
		amount: base_amount,
		..args.amm
	})?;
	let quote_from_user = gross_up(bought.quote_amount, args.quote_fee)?;

	buy_quote(BuyLegs {
		base_amount,
		quote_amount: quote_from_user,
		fee: bought.fee,
		quote_transfer_fee: quote_from_user
			.checked_sub(bought.quote_amount)
			.ok_or(AmmError::Overflow)?,
		base_fee: args.base_fee,
	})
}

/// Set `min_amount_out` to `calculate_slippage_down(q.quote_to_user, bps)`.
pub fn sell_exact_in_with_fees(
	args: SellArgs,
) -> Result<SellQuote, QuoteError> {
	let base_from_user = args.amm.amount;
	if base_from_user == 0 {
		return Err(AmmError::InvalidAmount.into());
	}

	let base_into_vault = amount_after_fee(base_from_user, args.base_fee)?;
	if base_into_vault == 0 {
		return Err(AmmError::InvalidAmount.into());
	}

	let sold = sell_exact_in(AmmInput {
		amount: base_into_vault,
		..args.amm
	})?;
	let quote_transfer_fee = fee_on(sold.quote_amount, args.quote_fee)?;

	Ok(SellQuote {
		base_amount: base_from_user,
		quote_amount: sold.quote_amount,
		fee: sold.fee,
		base_transfer_fee: base_from_user
			.checked_sub(base_into_vault)
			.ok_or(AmmError::Overflow)?,
		quote_transfer_fee,
		base_from_user,
		quote_to_user: sold
			.quote_amount
			.checked_sub(quote_transfer_fee)
			.ok_or(AmmError::Overflow)?,
	})
}

/// Set `max_amount_in` to `calculate_slippage_up(q.base_from_user, bps)`.
pub fn sell_exact_out_with_fees(
	args: SellArgs,
) -> Result<SellQuote, QuoteError> {
	let quote_to_user = args.amm.amount;
	if quote_to_user == 0 {
		return Err(AmmError::InvalidAmount.into());
	}

	let quote_out_of_vault = gross_up(quote_to_user, args.quote_fee)?;
	let sold = sell_exact_out(AmmInput {
		amount: quote_out_of_vault,
		..args.amm
	})?;
	let base_from_user = gross_up(sold.base_amount, args.base_fee)?;

	Ok(SellQuote {
		base_amount: base_from_user,
		quote_amount: quote_out_of_vault,
		fee: sold.fee,
		base_transfer_fee: base_from_user
			.checked_sub(sold.base_amount)
			.ok_or(AmmError::Overflow)?,
		quote_transfer_fee: quote_out_of_vault
			.checked_sub(quote_to_user)
			.ok_or(AmmError::Overflow)?,
		base_from_user,
		quote_to_user,
	})
}

/// Prices a buy of exactly `input.amount` base units. Set `max_amount_in` to
/// `calculate_slippage_up(q.quote_amount, bps)`.
pub fn buy_exact_out(input: AmmInput) -> Result<TradeQuote, AmmError> {
	let base_amount = input.amount;
	let quote_before_fee = calculate_input_for_output(
		input.quote_reserves,
		input.base_reserves,
		base_amount,
	)?;

	// The gross quote rounds up. The buyer pays the remainder.
	let fee_bps = u128::from(input.fee_bps);
	let quote = u128::from(quote_before_fee);

	let divisor = BPS_DIVISOR
		.checked_sub(fee_bps)
		.ok_or(AmmError::InvalidAmount)?;
	if divisor == 0 {
		return Err(AmmError::InvalidAmount);
	}

	let numerator = quote.checked_mul(BPS_DIVISOR).ok_or(AmmError::Overflow)?;
	let total_quote = ceil_div(numerator, divisor).ok_or(AmmError::Overflow)?;

	let fee_amount =
		total_quote.checked_sub(quote).ok_or(AmmError::Overflow)?;

	let total_quote =
		u64::try_from(total_quote).map_err(|_| AmmError::Overflow)?;
	let fee_amount =
		u64::try_from(fee_amount).map_err(|_| AmmError::Overflow)?;

	Ok(TradeQuote {
		base_amount,
		quote_amount: total_quote,
		fee: fee_amount,
	})
}

/// Prices a buy that spends exactly `input.amount` quote units. Set
/// `min_amount_out` to `calculate_slippage_down(q.base_amount, bps)`.
pub fn buy_exact_in(input: AmmInput) -> Result<TradeQuote, AmmError> {
	let quote_amount = input.amount;
	if quote_amount == 0 {
		return Err(AmmError::InvalidAmount);
	}

	// The net quote rounds down. The remainder is the fee.
	let fee_bps_128 = u128::from(input.fee_bps);
	let quote_128 = u128::from(quote_amount);

	let net_factor = BPS_DIVISOR
		.checked_sub(fee_bps_128)
		.ok_or(AmmError::InvalidAmount)?;
	if net_factor == 0 {
		return Err(AmmError::InvalidAmount);
	}

	let net_quote_128 = quote_128
		.checked_mul(net_factor)
		.ok_or(AmmError::Overflow)?
		.checked_div(BPS_DIVISOR)
		.ok_or(AmmError::Overflow)?;
	let net_quote =
		u64::try_from(net_quote_128).map_err(|_| AmmError::Overflow)?;
	if net_quote == 0 {
		return Err(AmmError::InvalidAmount);
	}

	let fee = quote_amount
		.checked_sub(net_quote)
		.ok_or(AmmError::Overflow)?;

	let base_amount =
		calculate_output(input.quote_reserves, input.base_reserves, net_quote)?;

	Ok(TradeQuote {
		base_amount,
		quote_amount,
		fee,
	})
}

/// Prices a sale of exactly `input.amount` base units. Set `min_amount_out` to
/// `calculate_slippage_down(q.quote_amount, bps)`.
pub fn sell_exact_in(input: AmmInput) -> Result<TradeQuote, AmmError> {
	if u128::from(input.fee_bps) > BPS_DIVISOR {
		return Err(AmmError::InvalidAmount);
	}

	let base_amount = input.amount;
	let quote_before_fee = calculate_output(
		input.base_reserves,
		input.quote_reserves,
		base_amount,
	)?;

	// The fee rounds up.
	let numerator = u128::from(quote_before_fee)
		.checked_mul(u128::from(input.fee_bps))
		.ok_or(AmmError::Overflow)?;
	let fee_amount =
		ceil_div(numerator, BPS_DIVISOR).ok_or(AmmError::Overflow)?;
	let fee_amount =
		u64::try_from(fee_amount).map_err(|_| AmmError::Overflow)?;
	let quote_after_fee = quote_before_fee
		.checked_sub(fee_amount)
		.ok_or(AmmError::Overflow)?;

	Ok(TradeQuote {
		base_amount,
		quote_amount: quote_after_fee,
		fee: fee_amount,
	})
}

/// Prices a sale that gives exactly `input.amount` quote units after the fee.
/// Set `max_amount_in` to `calculate_slippage_up(q.base_amount, bps)`.
pub fn sell_exact_out(input: AmmInput) -> Result<TradeQuote, AmmError> {
	let quote_amount = input.amount;
	if quote_amount == 0 {
		return Err(AmmError::InvalidAmount);
	}

	// The gross quote rounds up. The seller pays the remainder.
	let fee_bps_128 = u128::from(input.fee_bps);
	let quote_128 = u128::from(quote_amount);

	let divisor = BPS_DIVISOR
		.checked_sub(fee_bps_128)
		.ok_or(AmmError::InvalidAmount)?;
	if divisor == 0 {
		return Err(AmmError::InvalidAmount);
	}

	let numerator = quote_128
		.checked_mul(BPS_DIVISOR)
		.ok_or(AmmError::Overflow)?;
	let quote_before_fee =
		ceil_div(numerator, divisor).ok_or(AmmError::Overflow)?;

	let fee = quote_before_fee
		.checked_sub(quote_128)
		.ok_or(AmmError::Overflow)?;

	let quote_before_fee =
		u64::try_from(quote_before_fee).map_err(|_| AmmError::Overflow)?;
	let fee = u64::try_from(fee).map_err(|_| AmmError::Overflow)?;

	let base_amount = calculate_input_for_output(
		input.base_reserves,
		input.quote_reserves,
		quote_before_fee,
	)?;

	Ok(TradeQuote {
		base_amount,
		quote_amount,
		fee,
	})
}

/// `amount` plus `slippage_bps`, rounded up. Use it for `max_amount_in`.
pub fn calculate_slippage_up(
	amount: u64,
	slippage_bps: u16,
) -> Result<u64, AmmError> {
	let numerator = u128::from(amount)
		.checked_mul(
			BPS_DIVISOR
				.checked_add(u128::from(slippage_bps))
				.ok_or(AmmError::Overflow)?,
		)
		.ok_or(AmmError::Overflow)?;
	let result = ceil_div(numerator, BPS_DIVISOR).ok_or(AmmError::Overflow)?;
	u64::try_from(result).map_err(|_| AmmError::Overflow)
}

/// `amount` minus `slippage_bps`, rounded down. Use it for `min_amount_out`.
pub fn calculate_slippage_down(
	amount: u64,
	slippage_bps: u16,
) -> Result<u64, AmmError> {
	let factor = BPS_DIVISOR
		.checked_sub(u128::from(slippage_bps))
		.ok_or(AmmError::InvalidAmount)?;
	let numerator = u128::from(amount)
		.checked_mul(factor)
		.ok_or(AmmError::Overflow)?;
	let result = numerator
		.checked_div(BPS_DIVISOR)
		.ok_or(AmmError::Overflow)?;
	u64::try_from(result).map_err(|_| AmmError::Overflow)
}

/// Constant-product output for `amount_in`. Rounds down.
pub fn calculate_output(
	reserve_in: u64,
	reserve_out: u64,
	amount_in: u64,
) -> Result<u64, AmmError> {
	if reserve_in == 0 || reserve_out == 0 {
		return Err(AmmError::InsufficientLiquidity);
	}
	if amount_in == 0 {
		return Err(AmmError::InvalidAmount);
	}

	let k = u128::from(reserve_in)
		.checked_mul(u128::from(reserve_out))
		.ok_or(AmmError::Overflow)?;
	let new_reserve_in = u128::from(reserve_in)
		.checked_add(u128::from(amount_in))
		.ok_or(AmmError::Overflow)?;

	// Not `ceil_div`: `k + reserve_in + amount_in` reaches exactly `u128::MAX`
	// with u64 inputs. A wider input makes the pre-add overflow.
	let mut new_reserve_out =
		k.checked_div(new_reserve_in).ok_or(AmmError::Overflow)?;
	let remainder = k.checked_rem(new_reserve_in).ok_or(AmmError::Overflow)?;
	if remainder > 0 {
		new_reserve_out =
			new_reserve_out.checked_add(1).ok_or(AmmError::Overflow)?;
	}

	let amount_out = u128::from(reserve_out)
		.checked_sub(new_reserve_out)
		.ok_or(AmmError::InsufficientLiquidity)?;
	if amount_out == 0 {
		return Err(AmmError::InsufficientLiquidity);
	}

	u64::try_from(amount_out).map_err(|_| AmmError::Overflow)
}

/// Constant-product input for `amount_out`. Rounds up.
pub fn calculate_input_for_output(
	reserve_in: u64,
	reserve_out: u64,
	amount_out: u64,
) -> Result<u64, AmmError> {
	if reserve_in == 0 || reserve_out == 0 {
		return Err(AmmError::InsufficientLiquidity);
	}
	if amount_out == 0 || amount_out >= reserve_out {
		return Err(AmmError::InvalidAmount);
	}

	let k = u128::from(reserve_in)
		.checked_mul(u128::from(reserve_out))
		.ok_or(AmmError::Overflow)?;
	let new_reserve_out = u128::from(reserve_out)
		.checked_sub(u128::from(amount_out))
		.ok_or(AmmError::Overflow)?;

	// Not `ceil_div`: same u64-width margin as `calculate_output`.
	let mut new_reserve_in =
		k.checked_div(new_reserve_out).ok_or(AmmError::Overflow)?;
	let remainder = k.checked_rem(new_reserve_out).ok_or(AmmError::Overflow)?;
	if remainder > 0 {
		new_reserve_in =
			new_reserve_in.checked_add(1).ok_or(AmmError::Overflow)?;
	}

	let amount_in = new_reserve_in
		.checked_sub(u128::from(reserve_in))
		.ok_or(AmmError::InsufficientLiquidity)?;
	if amount_in == 0 {
		return Err(AmmError::InsufficientLiquidity);
	}

	u64::try_from(amount_in).map_err(|_| AmmError::Overflow)
}

/// The TypeScript SDK tests use the same vectors. Change an expected value in
/// both.
#[cfg(test)]
mod transfer_fee_tests {
	use super::*;

	const LAUNCH_QUOTE_RESERVES: u64 = 30_000_000_000;
	const LAUNCH_BASE_RESERVES: u64 = 1_000_000_000_000_000;
	const FEE_BPS: u16 = 100;

	const QUOTE_20_BPS: MintFee = MintFee {
		bps: 20,
		maximum_fee: u64::MAX,
	};

	const BASE_100_BPS: MintFee = MintFee {
		bps: 100,
		maximum_fee: u64::MAX,
	};

	fn amm(amount: u64) -> AmmInput {
		AmmInput {
			quote_reserves: LAUNCH_QUOTE_RESERVES,
			base_reserves: LAUNCH_BASE_RESERVES,
			amount,
			fee_bps: FEE_BPS,
		}
	}

	#[track_caller]
	fn assert_buy(quote: &BuyQuote, expected: [u64; 7]) {
		assert_eq!(
			[
				quote.base_amount,
				quote.quote_amount,
				quote.fee,
				quote.base_transfer_fee,
				quote.quote_transfer_fee,
				quote.base_to_user,
				quote.quote_from_user,
			],
			expected,
			"[base_amount, quote_amount, fee, base_transfer_fee, \
			 quote_transfer_fee, base_to_user, quote_from_user]"
		);
	}

	#[track_caller]
	fn assert_sell(quote: &SellQuote, expected: [u64; 7]) {
		assert_eq!(
			[
				quote.base_amount,
				quote.quote_amount,
				quote.fee,
				quote.base_transfer_fee,
				quote.quote_transfer_fee,
				quote.base_from_user,
				quote.quote_to_user,
			],
			expected,
			"[base_amount, quote_amount, fee, base_transfer_fee, \
			 quote_transfer_fee, base_from_user, quote_to_user]"
		);
	}

	#[test]
	fn a_zero_rate_never_moves_an_amount() {
		let fee = Some(MintFee {
			bps: 0,
			maximum_fee: 0,
		});
		assert_eq!(fee_on(1_000_000, fee).unwrap(), 0);
		assert_eq!(gross_up(1_000_000, fee).unwrap(), 1_000_000);
	}

	#[test]
	fn the_fee_rounds_up() {
		let fee = Some(QUOTE_20_BPS);
		assert_eq!(fee_on(1, fee).unwrap(), 1);
		assert_eq!(fee_on(10_000, fee).unwrap(), 20);
		assert_eq!(fee_on(10_001, fee).unwrap(), 21);
	}

	#[test]
	fn the_fee_is_zero_on_a_zero_amount() {
		let fee = Some(MintFee {
			bps: 10_000,
			maximum_fee: u64::MAX,
		});
		assert_eq!(fee_on(0, fee).unwrap(), 0);
	}

	#[test]
	fn the_maximum_fee_caps_after_rounding() {
		let fee = Some(MintFee {
			bps: 100,
			maximum_fee: 500,
		});
		assert_eq!(fee_on(1_000_000, fee).unwrap(), 500);
	}

	#[test]
	fn the_fee_stays_superadditive_across_a_split_at_the_cap() {
		// 300 + 400 >= 500: a split booking can only collect more.
		let fee = Some(MintFee {
			bps: 100,
			maximum_fee: 500,
		});
		assert_eq!(fee_on(30_000, fee).unwrap(), 300);
		assert_eq!(fee_on(40_000, fee).unwrap(), 400);
		assert_eq!(fee_on(70_000, fee).unwrap(), 500);
	}

	#[test]
	fn a_mint_with_no_config_is_the_identity() {
		assert_eq!(fee_on(1_000_000, None).unwrap(), 0);
		assert_eq!(amount_after_fee(1_000_000, None).unwrap(), 1_000_000);
		assert_eq!(gross_up(1_000_000, None).unwrap(), 1_000_000);
	}

	#[test]
	fn an_out_of_range_rate_is_rejected() {
		let fee = Some(MintFee {
			bps: 10_001,
			maximum_fee: 0,
		});
		assert_eq!(
			fee_on(1_000, fee).unwrap_err(),
			QuoteError::InvalidTransferFee
		);
	}

	#[test]
	fn amount_after_fee_deducts_what_fee_on_charges() {
		assert_eq!(
			amount_after_fee(1_000_000_000, Some(QUOTE_20_BPS)).unwrap(),
			998_000_000
		);
		assert_eq!(
			amount_after_fee(10_000_000_000_000, Some(BASE_100_BPS)).unwrap(),
			9_900_000_000_000
		);
	}

	#[test]
	fn gross_up_is_a_no_op_at_zero() {
		let fee = Some(MintFee {
			bps: 100,
			maximum_fee: u64::MAX,
		});
		assert_eq!(gross_up(0, fee).unwrap(), 0);
	}

	#[test]
	fn gross_up_lands_the_exact_amount() {
		for bps in [1_u16, 20, 100, 500, 3_333, 9_999] {
			let fee = Some(MintFee {
				bps,
				maximum_fee: u64::MAX,
			});
			for amount in [1_u64, 7, 1_000, 999_983, 1_000_000_000] {
				let gross = gross_up(amount, fee).unwrap();
				let landed = gross - fee_on(gross, fee).unwrap();
				assert_eq!(
					landed, amount,
					"bps {bps} amount {amount} grossed to {gross}"
				);
			}
		}
	}

	#[test]
	fn the_maximum_fee_clamps_the_gross_up() {
		let fee = Some(MintFee {
			bps: 100,
			maximum_fee: 500,
		});
		assert_eq!(fee_on(1_000_000, fee).unwrap(), 500);
		let gross = gross_up(1_000_000, fee).unwrap();
		assert_eq!(gross, 1_000_500);
		assert_eq!(gross - fee_on(gross, fee).unwrap(), 1_000_000);
	}

	#[test]
	fn a_full_rate_with_a_finite_cap_still_settles() {
		let fee = Some(MintFee {
			bps: 10_000,
			maximum_fee: 1_000,
		});
		let gross = gross_up(4_200, fee).unwrap();
		assert_eq!(gross, 5_200);
		assert_eq!(gross - fee_on(gross, fee).unwrap(), 4_200);
	}

	#[test]
	fn a_full_rate_without_a_cap_is_rejected() {
		let fee = Some(MintFee {
			bps: 10_000,
			maximum_fee: u64::MAX,
		});
		assert_eq!(
			gross_up(1_000, fee).unwrap_err(),
			QuoteError::TransferFeeNotSettleable
		);
	}

	#[test]
	fn a_gross_up_past_u64_is_rejected() {
		let fee = Some(MintFee {
			bps: 5_000,
			maximum_fee: u64::MAX,
		});
		assert_eq!(
			gross_up(u64::MAX, fee).unwrap_err(),
			QuoteError::TransferFeeNotSettleable
		);
	}

	#[test]
	fn buy_exact_in_pins_the_users_leg_at_what_they_sent() {
		let quote = buy_exact_in_with_fees(BuyArgs {
			amm: amm(1_000_000_000),
			quote_fee: None,
			base_fee: None,
			base_reserve_cap: None,
		})
		.unwrap();
		assert_buy(
			&quote,
			[
				31_945_788_964_181,
				1_000_000_000,
				10_000_000,
				0,
				0,
				31_945_788_964_181,
				1_000_000_000,
			],
		);
	}

	#[test]
	fn buy_exact_in_prices_only_what_reached_the_vault() {
		let quote = buy_exact_in_with_fees(BuyArgs {
			amm: amm(1_000_000_000),
			quote_fee: Some(QUOTE_20_BPS),
			base_fee: None,
			base_reserve_cap: None,
		})
		.unwrap();
		assert_buy(
			&quote,
			[
				31_883_934_501_139,
				1_000_000_000,
				9_980_000,
				0,
				2_000_000,
				31_883_934_501_139,
				1_000_000_000,
			],
		);
	}

	#[test]
	fn buy_exact_in_nets_the_base_leg_down_for_the_buyer() {
		let quote = buy_exact_in_with_fees(BuyArgs {
			amm: amm(1_000_000_000),
			quote_fee: None,
			base_fee: Some(BASE_100_BPS),
			base_reserve_cap: None,
		})
		.unwrap();
		assert_buy(
			&quote,
			[
				31_945_788_964_181,
				1_000_000_000,
				10_000_000,
				319_457_889_642,
				0,
				31_626_331_074_539,
				1_000_000_000,
			],
		);
	}

	#[test]
	fn an_uncapped_fill_ignores_a_cap_it_stays_under() {
		let quote = buy_exact_in_with_fees(BuyArgs {
			amm: amm(1_000_000_000),
			quote_fee: None,
			base_fee: None,
			base_reserve_cap: Some(u64::MAX),
		})
		.unwrap();
		assert_buy(
			&quote,
			[
				31_945_788_964_181,
				1_000_000_000,
				10_000_000,
				0,
				0,
				31_945_788_964_181,
				1_000_000_000,
			],
		);
	}

	#[test]
	fn a_capped_fill_grosses_the_users_leg_up_from_the_curves() {
		let plain = buy_exact_in_with_fees(BuyArgs {
			amm: amm(1_000_000_000),
			quote_fee: None,
			base_fee: None,
			base_reserve_cap: Some(1_000_000_000_000),
		})
		.unwrap();
		assert_buy(
			&plain,
			[
				1_000_000_000_000,
				30_333_365,
				303_334,
				0,
				0,
				1_000_000_000_000,
				30_333_365,
			],
		);

		let charged = buy_exact_in_with_fees(BuyArgs {
			amm: amm(1_000_000_000),
			quote_fee: Some(QUOTE_20_BPS),
			base_fee: None,
			base_reserve_cap: Some(1_000_000_000_000),
		})
		.unwrap();
		// The curve leg stays 30_333_365. Only the user's leg grows.
		assert_buy(
			&charged,
			[
				1_000_000_000_000,
				30_394_154,
				303_334,
				0,
				60_789,
				1_000_000_000_000,
				30_394_154,
			],
		);
	}

	#[test]
	fn buy_exact_in_rejects_a_leg_the_mint_eats_whole() {
		let err = buy_exact_in_with_fees(BuyArgs {
			amm: amm(100),
			quote_fee: Some(MintFee {
				bps: 10_000,
				maximum_fee: u64::MAX,
			}),
			base_fee: None,
			base_reserve_cap: None,
		})
		.unwrap_err();
		assert_eq!(err, QuoteError::Amm(AmmError::InvalidAmount));
	}

	#[test]
	fn buy_exact_out_moves_exactly_what_the_user_asked_for() {
		let quote = buy_exact_out_with_fees(BuyArgs {
			amm: amm(10_000_000_000_000),
			quote_fee: None,
			base_fee: None,
			base_reserve_cap: None,
		})
		.unwrap();
		assert_buy(
			&quote,
			[
				10_000_000_000_000,
				306_091_217,
				3_060_913,
				0,
				0,
				10_000_000_000_000,
				306_091_217,
			],
		);
	}

	#[test]
	fn buy_exact_out_grosses_both_legs_up() {
		let quote = buy_exact_out_with_fees(BuyArgs {
			amm: amm(10_000_000_000_000),
			quote_fee: Some(QUOTE_20_BPS),
			base_fee: Some(BASE_100_BPS),
			base_reserve_cap: None,
		})
		.unwrap();
		assert_buy(
			&quote,
			[
				10_101_010_101_011,
				309_834_264,
				3_092_146,
				101_010_101_011,
				619_669,
				10_000_000_000_000,
				309_834_264,
			],
		);
	}

	#[test]
	fn buy_exact_out_fills_only_up_to_the_cap() {
		let quote = buy_exact_out_with_fees(BuyArgs {
			amm: amm(10_000_000_000_000),
			quote_fee: None,
			base_fee: None,
			base_reserve_cap: Some(1_000_000_000_000),
		})
		.unwrap();
		assert_buy(
			&quote,
			[
				1_000_000_000_000,
				30_333_365,
				303_334,
				0,
				0,
				1_000_000_000_000,
				30_333_365,
			],
		);
	}

	#[test]
	fn buy_exact_out_nets_the_capped_fill_down_through_the_base_mint() {
		let quote = buy_exact_out_with_fees(BuyArgs {
			amm: amm(10_000_000_000_000),
			quote_fee: None,
			base_fee: Some(BASE_100_BPS),
			base_reserve_cap: Some(1_000_000_000_000),
		})
		.unwrap();
		assert_buy(
			&quote,
			[
				1_000_000_000_000,
				30_333_365,
				303_334,
				10_000_000_000,
				0,
				990_000_000_000,
				30_333_365,
			],
		);
	}

	#[test]
	fn sell_exact_in_pays_the_user_the_net() {
		let quote = sell_exact_in_with_fees(SellArgs {
			amm: amm(10_000_000_000_000),
			quote_fee: None,
			base_fee: None,
		})
		.unwrap();
		assert_sell(
			&quote,
			[
				10_000_000_000_000,
				294_059_404,
				2_970_298,
				0,
				0,
				10_000_000_000_000,
				294_059_404,
			],
		);
	}

	#[test]
	fn sell_exact_in_books_only_the_base_that_landed() {
		let quote = sell_exact_in_with_fees(SellArgs {
			amm: amm(10_000_000_000_000),
			quote_fee: None,
			base_fee: Some(BASE_100_BPS),
		})
		.unwrap();
		assert_sell(
			&quote,
			[
				10_000_000_000_000,
				291_147_637,
				2_940_886,
				100_000_000_000,
				0,
				10_000_000_000_000,
				291_147_637,
			],
		);
	}

	#[test]
	fn sell_exact_in_takes_the_quote_mints_cut_off_the_outbound_leg() {
		let quote = sell_exact_in_with_fees(SellArgs {
			amm: amm(10_000_000_000_000),
			quote_fee: Some(QUOTE_20_BPS),
			base_fee: None,
		})
		.unwrap();
		// No gross-up: the mint takes its fee from the transfer.
		assert_sell(
			&quote,
			[
				10_000_000_000_000,
				294_059_404,
				2_970_298,
				0,
				588_119,
				10_000_000_000_000,
				293_471_285,
			],
		);
	}

	#[test]
	fn sell_exact_in_rejects_a_leg_the_mint_eats_whole() {
		let err = sell_exact_in_with_fees(SellArgs {
			amm: amm(100),
			quote_fee: None,
			base_fee: Some(MintFee {
				bps: 10_000,
				maximum_fee: u64::MAX,
			}),
		})
		.unwrap_err();
		assert_eq!(err, QuoteError::Amm(AmmError::InvalidAmount));
	}

	#[test]
	fn sell_exact_out_lands_exactly_what_the_user_asked_for() {
		let quote = sell_exact_out_with_fees(SellArgs {
			amm: amm(100_000_000),
			quote_fee: None,
			base_fee: None,
		})
		.unwrap();
		assert_sell(
			&quote,
			[
				3_378_378_411_599,
				100_000_000,
				1_010_102,
				0,
				0,
				3_378_378_411_599,
				100_000_000,
			],
		);
	}

	#[test]
	fn sell_exact_out_grosses_both_legs_up() {
		let quote = sell_exact_out_with_fees(SellArgs {
			amm: amm(100_000_000),
			quote_fee: Some(QUOTE_20_BPS),
			base_fee: Some(BASE_100_BPS),
		})
		.unwrap();
		assert_sell(
			&quote,
			[
				3_419_365_278_607,
				100_200_401,
				1_012_126,
				34_193_652_787,
				200_401,
				3_419_365_278_607,
				100_000_000,
			],
		);
	}

	#[test]
	fn sell_exact_out_surfaces_an_unsettleable_quote_leg() {
		let err = sell_exact_out_with_fees(SellArgs {
			amm: amm(100_000_000),
			quote_fee: Some(MintFee {
				bps: 10_000,
				maximum_fee: u64::MAX,
			}),
			base_fee: None,
		})
		.unwrap_err();
		assert_eq!(err, QuoteError::TransferFeeNotSettleable);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const INITIAL_VIRTUAL_QUOTE: u64 = 30_000_000_000; // 30 SOL
	const INITIAL_VIRTUAL_BASE: u64 = 1_000_000_000_000_000; // 1B tokens (6 decimals)
	const FEE_BPS: u16 = 100;

	const BASE_INPUT: AmmInput = AmmInput {
		quote_reserves: INITIAL_VIRTUAL_QUOTE,
		base_reserves: INITIAL_VIRTUAL_BASE,
		amount: 0,
		fee_bps: FEE_BPS,
	};

	#[test]
	fn test_ceil_div_edges() {
		assert_eq!(ceil_div(0, BPS_DIVISOR), Some(0));
		assert_eq!(ceil_div(1, BPS_DIVISOR), Some(1));
		assert_eq!(ceil_div(BPS_DIVISOR, BPS_DIVISOR), Some(1));
		assert_eq!(ceil_div(BPS_DIVISOR + 1, BPS_DIVISOR), Some(2));
		assert_eq!(ceil_div(u128::MAX - BPS_DIVISOR + 1, BPS_DIVISOR), None);
		assert_eq!(ceil_div(0, 0), None);
		assert_eq!(ceil_div(1, 0), None);
	}

	#[test]
	fn test_calculate_output_basic() {
		let output = calculate_output(
			INITIAL_VIRTUAL_QUOTE,
			INITIAL_VIRTUAL_BASE,
			1_000_000_000,
		)
		.unwrap();
		assert_eq!(output, 32_258_064_516_129);
	}

	#[test]
	fn test_calculate_output_ceiling_protects_reserves() {
		let output = calculate_output(1000, 1000, 10).unwrap();
		assert_eq!(output, 9);
	}

	#[test]
	fn test_roundtrip_favors_protocol() {
		let initial_quote = 100_000_000_000u64;
		let initial_base = 1_000_000_000u64;

		let tokens_out =
			calculate_output(initial_quote, initial_base, 10_000_000_000)
				.unwrap();

		let new_quote = initial_quote + 10_000_000_000;
		let new_base = initial_base - tokens_out;

		let sol_out =
			calculate_output(new_base, new_quote, tokens_out).unwrap();
		assert_eq!(sol_out, 9_999_999_900);
	}

	#[test]
	fn test_calculate_input_for_output() {
		let quote_needed = calculate_input_for_output(
			100_000_000_000,
			1_000_000_000,
			100_000_000,
		)
		.unwrap();

		let actual_out =
			calculate_output(100_000_000_000, 1_000_000_000, quote_needed)
				.unwrap();
		assert_eq!(actual_out, 100_000_000);
	}

	#[test]
	fn test_zero_amount_fails() {
		assert_eq!(
			calculate_output(100, 100, 0).unwrap_err(),
			AmmError::InvalidAmount
		);
	}

	#[test]
	fn test_output_exceeds_reserves() {
		assert_eq!(
			calculate_input_for_output(
				100_000_000_000,
				1_000_000_000,
				2_000_000_000
			)
			.unwrap_err(),
			AmmError::InvalidAmount
		);
	}

	#[test]
	fn test_buy_exact_out_with_fee() {
		let quote = buy_exact_out(AmmInput {
			amount: 10_000_000_000_000,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(quote.base_amount, 10_000_000_000_000);
		assert_eq!(quote.quote_amount, 306_091_217);
		assert_eq!(quote.fee, 3_060_913);
	}

	#[test]
	fn test_buy_exact_out_small_amounts_ceiling() {
		let quote = buy_exact_out(AmmInput {
			quote_reserves: 10_000,
			base_reserves: 10_000,
			amount: 99,
			fee_bps: FEE_BPS,
		})
		.unwrap();

		assert_eq!(quote.quote_amount, 102);
		assert_eq!(quote.fee, 2);
	}

	#[test]
	fn test_buy_exact_out_zero_fee() {
		let quote = buy_exact_out(AmmInput {
			amount: 10_000_000_000_000,
			fee_bps: 0,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(quote.fee, 0);
		let raw_cost = calculate_input_for_output(
			INITIAL_VIRTUAL_QUOTE,
			INITIAL_VIRTUAL_BASE,
			10_000_000_000_000,
		)
		.unwrap();
		assert_eq!(quote.quote_amount, raw_cost);
	}

	#[test]
	fn test_buy_exact_in_basic() {
		let quote = buy_exact_in(AmmInput {
			amount: 1_000_000_000,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(quote.quote_amount, 1_000_000_000);
		assert_eq!(quote.fee, 10_000_000);
		assert_eq!(quote.base_amount, 31_945_788_964_181);
	}

	#[test]
	fn test_buy_exact_in_zero_amount() {
		assert_eq!(
			buy_exact_in(AmmInput {
				amount: 0,
				..BASE_INPUT
			})
			.unwrap_err(),
			AmmError::InvalidAmount
		);
	}

	#[test]
	fn test_buy_exact_in_zero_fee() {
		let quote = buy_exact_in(AmmInput {
			amount: 1_000_000_000,
			fee_bps: 0,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(quote.fee, 0);
		assert_eq!(quote.quote_amount, 1_000_000_000);
	}

	#[test]
	fn test_sell_exact_in_with_fee() {
		let quote = sell_exact_in(AmmInput {
			amount: 10_000_000_000_000,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(quote.base_amount, 10_000_000_000_000);
		assert_eq!(quote.quote_amount, 294_059_404);
		assert_eq!(quote.fee, 2_970_298);
	}

	#[test]
	fn test_sell_exact_in_fee_ceiling() {
		let quote = sell_exact_in(AmmInput {
			quote_reserves: 1_000_000_000_000,
			base_reserves: 1_000_000_000_000,
			amount: 9_999_999,
			fee_bps: FEE_BPS,
		})
		.unwrap();

		assert_eq!(quote.fee, 99_999);
		assert_eq!(quote.quote_amount, 9_899_900);
	}

	#[test]
	fn test_sell_exact_in_rejects_fee_above_full_bps() {
		assert_eq!(
			sell_exact_in(AmmInput {
				amount: 10_000_000_000_000,
				fee_bps: 10_001,
				..BASE_INPUT
			})
			.unwrap_err(),
			AmmError::InvalidAmount
		);
	}

	/// Keep `fee_bps == 10_000` valid for TypeScript parity.
	#[test]
	fn test_sell_exact_in_full_fee_is_allowed() {
		let quote = sell_exact_in(AmmInput {
			quote_reserves: 1_000_000,
			base_reserves: 1_000_000,
			amount: 1_000,
			fee_bps: 10_000,
		})
		.unwrap();

		assert_eq!(quote.base_amount, 1_000);
		assert_eq!(quote.quote_amount, 0);
		assert_eq!(quote.fee, 999);
	}

	#[test]
	fn test_sell_exact_in_zero_fee() {
		let quote = sell_exact_in(AmmInput {
			amount: 10_000_000_000_000,
			fee_bps: 0,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(quote.fee, 0);
		let raw_output = calculate_output(
			INITIAL_VIRTUAL_BASE,
			INITIAL_VIRTUAL_QUOTE,
			10_000_000_000_000,
		)
		.unwrap();
		assert_eq!(quote.quote_amount, raw_output);
	}

	#[test]
	fn test_sell_exact_out_roundtrip_with_sell_exact_in() {
		let sell = sell_exact_in(AmmInput {
			amount: 10_000_000_000_000,
			..BASE_INPUT
		})
		.unwrap();

		let target = sell_exact_out(AmmInput {
			amount: sell.quote_amount,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(target.quote_amount, sell.quote_amount);
		assert_eq!(target.base_amount, 9_999_999_967_007);

		let verify = sell_exact_in(AmmInput {
			amount: target.base_amount,
			..BASE_INPUT
		})
		.unwrap();
		assert_eq!(verify.quote_amount, 294_059_404);
	}

	#[test]
	fn test_sell_exact_out_zero_fee() {
		let quote = sell_exact_out(AmmInput {
			amount: 1_000_000_000,
			fee_bps: 0,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(quote.fee, 0);
		assert_eq!(quote.quote_amount, 1_000_000_000);
		let raw = calculate_input_for_output(
			INITIAL_VIRTUAL_BASE,
			INITIAL_VIRTUAL_QUOTE,
			1_000_000_000,
		)
		.unwrap();
		assert_eq!(quote.base_amount, raw);
	}

	#[test]
	fn test_sell_exact_out_fee_reversal() {
		let quote = sell_exact_out(AmmInput {
			amount: 1_000_000_000,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(quote.quote_amount, 1_000_000_000);
		assert_eq!(quote.fee, 10_101_011);
		let actual_gross = calculate_output(
			INITIAL_VIRTUAL_BASE,
			INITIAL_VIRTUAL_QUOTE,
			quote.base_amount,
		)
		.unwrap();
		assert_eq!(actual_gross, 1_010_101_011);
	}

	#[test]
	fn test_sell_exact_out_hardcoded() {
		let quote = sell_exact_out(AmmInput {
			amount: 1_000_000_000,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(quote.quote_amount, 1_000_000_000);
		assert_eq!(quote.fee, 10_101_011);
		assert_eq!(quote.base_amount, 34_843_205_607_004);
	}

	#[test]
	fn test_sell_exact_out_zero_amount() {
		assert_eq!(
			sell_exact_out(AmmInput {
				amount: 0,
				..BASE_INPUT
			})
			.unwrap_err(),
			AmmError::InvalidAmount
		);
	}

	#[test]
	fn test_sell_exact_out_cross_consistency_with_slippage() {
		let slippage_bps = 50;

		let quote = sell_exact_out(AmmInput {
			amount: 1_000_000_000,
			..BASE_INPUT
		})
		.unwrap();

		let max_base =
			calculate_slippage_up(quote.base_amount, slippage_bps).unwrap();
		assert_eq!(max_base, 35_017_421_635_040);

		let verify = sell_exact_in(AmmInput {
			amount: max_base,
			..BASE_INPUT
		})
		.unwrap();
		assert_eq!(verify.quote_amount, 1_004_830_836);
	}

	#[test]
	fn test_calculate_slippage_up_basic() {
		let result = calculate_slippage_up(1_000_000_000, 100).unwrap();
		assert_eq!(result, 1_010_000_000);
	}

	#[test]
	fn test_calculate_slippage_up_ceiling() {
		let result = calculate_slippage_up(101, 50).unwrap();
		assert_eq!(result, 102);
	}

	#[test]
	fn test_calculate_slippage_up_zero_slippage() {
		let result = calculate_slippage_up(1_000_000_000, 0).unwrap();
		assert_eq!(result, 1_000_000_000);
	}

	#[test]
	fn test_calculate_slippage_down_basic() {
		let result = calculate_slippage_down(1_000_000_000, 100).unwrap();
		assert_eq!(result, 990_000_000);
	}

	#[test]
	fn test_calculate_slippage_down_floor() {
		let result = calculate_slippage_down(101, 50).unwrap();
		assert_eq!(result, 100);
	}

	#[test]
	fn test_calculate_slippage_down_zero_slippage() {
		let result = calculate_slippage_down(1_000_000_000, 0).unwrap();
		assert_eq!(result, 1_000_000_000);
	}

	#[test]
	fn test_calculate_slippage_down_100_percent_slippage() {
		let result = calculate_slippage_down(1_000_000_000, 10_000).unwrap();
		assert_eq!(result, 0);
	}

	#[test]
	fn test_buy_sell_roundtrip() {
		let buy = buy_exact_out(AmmInput {
			amount: 10_000_000_000_000,
			..BASE_INPUT
		})
		.unwrap();

		let new_quote = INITIAL_VIRTUAL_QUOTE + buy.quote_amount - buy.fee;
		let new_base = INITIAL_VIRTUAL_BASE - buy.base_amount;

		let sell = sell_exact_in(AmmInput {
			quote_reserves: new_quote,
			base_reserves: new_base,
			amount: buy.base_amount,
			fee_bps: FEE_BPS,
		})
		.unwrap();

		// Fees and rounding make the round trip return less.
		assert!(sell.quote_amount < buy.quote_amount);
	}

	#[test]
	fn test_buy_exact_in_roundtrip_with_buy_exact_out() {
		let buy = buy_exact_in(AmmInput {
			amount: 1_000_000_000,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(buy.base_amount, 31_945_788_964_181);
		assert_eq!(buy.fee, 10_000_000);

		let target = buy_exact_out(AmmInput {
			amount: buy.base_amount,
			..BASE_INPUT
		})
		.unwrap();

		assert_eq!(target.base_amount, buy.base_amount);
		assert_eq!(target.quote_amount, 1_000_000_000);
		assert_eq!(target.fee, 10_000_000);
	}
}
